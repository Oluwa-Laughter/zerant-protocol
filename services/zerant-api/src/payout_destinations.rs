//! Ephemeral, encrypted Zcash payout destinations shared by a holder with one issuer
//! organization. A payout destination is never a Zerant identity attribute and is never
//! exposed through public issuer or credential APIs.

use super::*;
use zerant_zcash::address::inspect_address;

const PAYOUT_SCOPE: &str = "payout-destination";
const PAYOUT_TTL_DAYS: i64 = 7;
const MAX_PAYOUT_RECORDS: i64 = 50;
const MAX_EXPORT_PAYOUT_RECORDS: i64 = 5_000;
const MAX_ACTIVE_PAYOUTS_PER_ACCOUNT: i64 = 25;
const MAX_PAYOUT_ORGANIZATIONS: i64 = 256;

const ACTIVE_RELATIONSHIP_EXISTS_SQL: &str = "
    SELECT EXISTS(
        SELECT 1
        FROM issued_credentials c
        WHERE c.issuer_profile_id = $1
          AND c.subject_account_id = $2
          AND c.revoked_at IS NULL
          AND c.expires_at > NOW()
    )";

const ELIGIBLE_PAYOUT_ORGANIZATIONS_SQL: &str = "
    SELECT DISTINCT p.issuer_id, p.display_name
    FROM issuer_profiles p
    JOIN issued_credentials c ON c.issuer_profile_id = p.id
    WHERE c.subject_account_id = $1
      AND c.revoked_at IS NULL
      AND c.expires_at > NOW()
      AND p.retired_at IS NULL
    ORDER BY p.display_name ASC, p.issuer_id ASC
    LIMIT $2";

const EXPIRE_HOLDER_PAYOUTS_SQL: &str = "
    UPDATE zcash_payout_destinations AS payout
    SET state = 'expired', ciphertext = NULL, data_nonce = NULL, wrapped_dek = NULL,
        wrap_nonce = NULL, key_version = NULL
    WHERE payout.subject_account_id = $1
      AND payout.state = 'active'
      AND (
          payout.expires_at <= NOW()
          OR NOT EXISTS (
              SELECT 1
              FROM issued_credentials c
              WHERE c.issuer_profile_id = payout.issuer_profile_id
                AND c.subject_account_id = payout.subject_account_id
                AND c.revoked_at IS NULL
                AND c.expires_at > NOW()
          )
      )";

const EXPIRE_ISSUER_PAYOUTS_SQL: &str = "
    UPDATE zcash_payout_destinations AS payout
    SET state = 'expired', ciphertext = NULL, data_nonce = NULL, wrapped_dek = NULL,
        wrap_nonce = NULL, key_version = NULL
    WHERE payout.issuer_profile_id = $1
      AND payout.state = 'active'
      AND (
          payout.expires_at <= NOW()
          OR NOT EXISTS (
              SELECT 1
              FROM issued_credentials c
              WHERE c.issuer_profile_id = payout.issuer_profile_id
                AND c.subject_account_id = payout.subject_account_id
                AND c.revoked_at IS NULL
                AND c.expires_at > NOW()
          )
      )";

#[derive(Deserialize, Serialize)]
struct PayoutSecret {
    recipient: String,
    purpose: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CreatePayoutDestinationInput {
    issuer_id: String,
    recipient: String,
    purpose: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PreparePayoutPaymentInput {
    amount_zec: String,
}

#[derive(Serialize)]
pub(super) struct PayoutExportView {
    id: Uuid,
    issuer_id: String,
    issuer_name: String,
    network: String,
    state: String,
    recipient: Option<String>,
    purpose: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    expires_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    withdrawn_at: Option<OffsetDateTime>,
}

#[derive(Serialize)]
pub(super) struct HolderPayoutPage {
    items: Vec<HolderPayoutView>,
    organizations: Vec<PayoutOrganizationView>,
}

#[derive(Serialize)]
pub(super) struct PayoutOrganizationView {
    issuer_id: String,
    display_name: String,
}

#[derive(Serialize)]
pub(super) struct IssuerPayoutPage {
    items: Vec<IssuerPayoutView>,
}

#[derive(Serialize)]
pub(super) struct HolderPayoutView {
    id: Uuid,
    issuer_id: String,
    issuer_name: String,
    network: String,
    state: String,
    recipient: Option<String>,
    purpose: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    expires_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    withdrawn_at: Option<OffsetDateTime>,
}

#[derive(Serialize)]
pub(super) struct IssuerPayoutView {
    id: Uuid,
    network: String,
    state: String,
    purpose: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    expires_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    withdrawn_at: Option<OffsetDateTime>,
}

fn expected_network(chain: &str) -> Result<&'static str, ApiError> {
    match chain {
        "zcash:testnet" => Ok("testnet"),
        "zcash:mainnet" => Ok("mainnet"),
        _ => Err(ApiError::Unavailable),
    }
}

fn valid_purpose(value: &str) -> bool {
    let len = value.chars().count();
    (3..=160).contains(&len) && !value.chars().any(char::is_control)
}

async fn expire_holder_due(
    client: &deadpool_postgres::Object,
    account: Uuid,
) -> Result<(), ApiError> {
    client
        .execute(EXPIRE_HOLDER_PAYOUTS_SQL, &[&account])
        .await
        .map_err(|_| ApiError::Unavailable)?;
    Ok(())
}

async fn expire_issuer_due(
    client: &deadpool_postgres::Object,
    issuer: Uuid,
) -> Result<(), ApiError> {
    client
        .execute(EXPIRE_ISSUER_PAYOUTS_SQL, &[&issuer])
        .await
        .map_err(|_| ApiError::Unavailable)?;
    Ok(())
}

fn decrypted_secret(state: &AppState, row: &Row) -> Result<Option<PayoutSecret>, ApiError> {
    let record_state: String = row.get("state");
    if record_state != "active" {
        return Ok(None);
    }
    let subject: Uuid = row.get("subject_account_id");
    let id: Uuid = row.get("id");
    let ciphertext: Option<Vec<u8>> = row.get("ciphertext");
    let data_nonce: Option<Vec<u8>> = row.get("data_nonce");
    let wrapped_dek: Option<Vec<u8>> = row.get("wrapped_dek");
    let wrap_nonce: Option<Vec<u8>> = row.get("wrap_nonce");
    let key_version: Option<i32> = row.get("key_version");
    let created_at: OffsetDateTime = row.get("created_at");
    let crypto_row = CredentialRow {
        id,
        ciphertext: ciphertext.ok_or(ApiError::Unavailable)?,
        data_nonce: data_nonce.ok_or(ApiError::Unavailable)?,
        wrapped_dek: wrapped_dek.ok_or(ApiError::Unavailable)?,
        wrap_nonce: wrap_nonce.ok_or(ApiError::Unavailable)?,
        key_version: key_version.ok_or(ApiError::Unavailable)?,
        created_at,
        updated_at: created_at,
    };
    let plaintext = state
        .cipher
        .decrypt_scoped(PAYOUT_SCOPE, subject, &crypto_row)?;
    serde_json::from_slice(&plaintext)
        .map(Some)
        .map_err(|_| ApiError::Unavailable)
}

fn holder_view(state: &AppState, row: &Row) -> Result<HolderPayoutView, ApiError> {
    let secret = decrypted_secret(state, row)?;
    Ok(HolderPayoutView {
        id: row.get("id"),
        issuer_id: row.get("issuer_id"),
        issuer_name: row.get("issuer_name"),
        network: row.get("network"),
        state: row.get("state"),
        recipient: secret.as_ref().map(|value| value.recipient.clone()),
        purpose: secret.as_ref().map(|value| value.purpose.clone()),
        created_at: row.get("created_at"),
        expires_at: row.get("expires_at"),
        withdrawn_at: row.get("withdrawn_at"),
    })
}

fn issuer_view(state: &AppState, row: &Row) -> Result<IssuerPayoutView, ApiError> {
    let secret = decrypted_secret(state, row)?;
    Ok(IssuerPayoutView {
        id: row.get("id"),
        network: row.get("network"),
        state: row.get("state"),
        purpose: secret.as_ref().map(|value| value.purpose.clone()),
        created_at: row.get("created_at"),
        expires_at: row.get("expires_at"),
        withdrawn_at: row.get("withdrawn_at"),
    })
}

pub(super) async fn export(
    state: &AppState,
    account: Uuid,
) -> Result<(Vec<PayoutExportView>, bool), ApiError> {
    let client = db_client(&state.db).await?;
    expire_holder_due(&client, account).await?;
    let rows = client
        .query(
            "SELECT d.id, d.subject_account_id, p.issuer_id, p.display_name AS issuer_name,
                d.network, d.state, d.ciphertext, d.data_nonce, d.wrapped_dek,
                d.wrap_nonce, d.key_version, d.created_at, d.expires_at, d.withdrawn_at
         FROM zcash_payout_destinations d
         JOIN issuer_profiles p ON p.id = d.issuer_profile_id
         WHERE d.subject_account_id = $1
         ORDER BY d.created_at DESC, d.id DESC LIMIT $2",
            &[&account, &(MAX_EXPORT_PAYOUT_RECORDS + 1)],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let complete = rows.len() <= MAX_EXPORT_PAYOUT_RECORDS as usize;
    let mut items = Vec::with_capacity(rows.len().min(MAX_EXPORT_PAYOUT_RECORDS as usize));
    for row in rows.into_iter().take(MAX_EXPORT_PAYOUT_RECORDS as usize) {
        let secret = decrypted_secret(state, &row)?;
        items.push(PayoutExportView {
            id: row.get("id"),
            issuer_id: row.get("issuer_id"),
            issuer_name: row.get("issuer_name"),
            network: row.get("network"),
            state: row.get("state"),
            recipient: secret.as_ref().map(|value| value.recipient.clone()),
            purpose: secret.as_ref().map(|value| value.purpose.clone()),
            created_at: row.get("created_at"),
            expires_at: row.get("expires_at"),
            withdrawn_at: row.get("withdrawn_at"),
        });
    }
    Ok((items, complete))
}

async fn record_private_holder_issuer_event(
    tx: &Transaction<'_>,
    issuer_profile_id: Uuid,
    event_type: &str,
    object_id: &str,
    network: &str,
) -> Result<(), ApiError> {
    tx.execute(
        "INSERT INTO issuer_events
         (issuer_profile_id, actor_account_id, actor_zerant_id, event_type,
          object_id, label, context, counterparty)
         VALUES ($1,NULL,'private-holder',$2,$3,'Private payout destination',$4,NULL)",
        &[&issuer_profile_id, &event_type, &object_id, &network],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;
    Ok(())
}

pub(super) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreatePayoutDestinationInput>,
) -> Result<(StatusCode, Json<HolderPayoutView>), ApiError> {
    let account = account_id(&headers, &state.db).await?;
    enforce_account_rate_limit(&state.db, account, "zcash_payout_share", 20).await?;
    validate_public_issuer_id(input.issuer_id.trim())?;

    let purpose = input.purpose.trim();
    if !valid_purpose(purpose) {
        return Err(ApiError::Invalid);
    }

    let inspected = inspect_address(input.recipient.trim()).map_err(|_| ApiError::Invalid)?;
    if inspected.network != expected_network(&state.zcash_chain)? || inspected.transparent_only {
        return Err(ApiError::Invalid);
    }

    let id = Uuid::new_v4();
    let secret = serde_json::to_vec(&PayoutSecret {
        recipient: inspected.canonical,
        purpose: purpose.to_owned(),
    })
    .map_err(|_| ApiError::Unavailable)?;
    let encrypted = state
        .cipher
        .encrypt_scoped(PAYOUT_SCOPE, account, id, &secret)?;

    let mut client = db_client(&state.db).await?;
    let tx = client
        .transaction()
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let issuer = tx
        .query_opt(
            "SELECT id, display_name FROM issuer_profiles
         WHERE issuer_id = $1 AND retired_at IS NULL",
            &[&input.issuer_id.trim()],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::NotFound)?;
    let issuer_profile_id: Uuid = issuer.get(0);
    let issuer_name: String = issuer.get(1);

    let has_active_relationship: bool = tx
        .query_one(
            ACTIVE_RELATIONSHIP_EXISTS_SQL,
            &[&issuer_profile_id, &account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .get(0);
    if !has_active_relationship {
        return Err(ApiError::Forbidden);
    }

    let active_count: i64 = tx
        .query_one(
            "SELECT COUNT(*) FROM zcash_payout_destinations
         WHERE subject_account_id = $1 AND state = 'active' AND expires_at > NOW()",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .get(0);
    if active_count >= MAX_ACTIVE_PAYOUTS_PER_ACCOUNT {
        return Err(ApiError::Conflict);
    }

    let row = tx
        .query_opt(
            "INSERT INTO zcash_payout_destinations
         (id, issuer_profile_id, subject_account_id, network, transparent_only,
          ciphertext, data_nonce, wrapped_dek, wrap_nonce, key_version, state, expires_at)
         VALUES ($1,$2,$3,$4,FALSE,$5,$6,$7,$8,$9,'active',NOW() + ($10 * INTERVAL '1 day'))
         ON CONFLICT (issuer_profile_id, subject_account_id) WHERE state = 'active'
         DO NOTHING
         RETURNING id, subject_account_id, network, state, ciphertext, data_nonce,
                   wrapped_dek, wrap_nonce, key_version, created_at, expires_at, withdrawn_at",
            &[
                &id,
                &issuer_profile_id,
                &account,
                &state.zcash_chain,
                &encrypted.ciphertext,
                &encrypted.data_nonce,
                &encrypted.wrapped_dek,
                &encrypted.wrap_nonce,
                &encrypted.key_version,
                &PAYOUT_TTL_DAYS,
            ],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::Conflict)?;

    tx.execute(
        "INSERT INTO trust_events(account_id, event_type, object_id, label, context, counterparty)
         VALUES ($1, 'zcash_payout_shared', $2, 'Private payout destination', $3, $4)",
        &[&account, &id.to_string(), &state.zcash_chain, &issuer_name],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;

    let audit_id = id.to_string();
    record_private_holder_issuer_event(
        &tx,
        issuer_profile_id,
        "payout_destination_received",
        &audit_id,
        &state.zcash_chain,
    )
    .await?;
    tx.commit().await.map_err(|_| ApiError::Unavailable)?;

    let secret = decrypted_secret(&state, &row)?.ok_or(ApiError::Unavailable)?;
    Ok((
        StatusCode::CREATED,
        Json(HolderPayoutView {
            id,
            issuer_id: input.issuer_id.trim().to_owned(),
            issuer_name,
            network: state.zcash_chain,
            state: "active".into(),
            recipient: Some(secret.recipient),
            purpose: Some(secret.purpose),
            created_at: row.get("created_at"),
            expires_at: row.get("expires_at"),
            withdrawn_at: None,
        }),
    ))
}

pub(super) async fn list_holder(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<HolderPayoutPage>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    let client = db_client(&state.db).await?;
    expire_holder_due(&client, account).await?;
    let rows = client
        .query(
            "SELECT d.id, d.subject_account_id, p.issuer_id, p.display_name AS issuer_name,
                d.network, d.state, d.ciphertext, d.data_nonce, d.wrapped_dek,
                d.wrap_nonce, d.key_version, d.created_at, d.expires_at, d.withdrawn_at
         FROM zcash_payout_destinations d
         JOIN issuer_profiles p ON p.id = d.issuer_profile_id
         WHERE d.subject_account_id = $1
         ORDER BY d.created_at DESC, d.id DESC LIMIT $2",
            &[&account, &MAX_PAYOUT_RECORDS],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let items = rows
        .iter()
        .map(|row| holder_view(&state, row))
        .collect::<Result<Vec<_>, _>>()?;

    let organization_rows = client
        .query(
            ELIGIBLE_PAYOUT_ORGANIZATIONS_SQL,
            &[&account, &MAX_PAYOUT_ORGANIZATIONS],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let organizations = organization_rows
        .into_iter()
        .map(|row| PayoutOrganizationView {
            issuer_id: row.get(0),
            display_name: row.get(1),
        })
        .collect();

    Ok(Json(HolderPayoutPage {
        items,
        organizations,
    }))
}

pub(super) async fn list_issuer(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<IssuerPayoutPage>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    let access = issuer_access(&state.db, account).await?;
    require_issuer_role(&access, &["owner", "admin", "issuer"])?;
    let client = db_client(&state.db).await?;
    expire_issuer_due(&client, access.profile_id).await?;
    let rows = client
        .query(
            "SELECT d.id, d.subject_account_id,
                d.network, d.state, d.ciphertext, d.data_nonce, d.wrapped_dek,
                d.wrap_nonce, d.key_version, d.created_at, d.expires_at, d.withdrawn_at
         FROM zcash_payout_destinations d
         WHERE d.issuer_profile_id = $1
         ORDER BY d.created_at DESC, d.id DESC LIMIT $2",
            &[&access.profile_id, &MAX_PAYOUT_RECORDS],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let items = rows
        .iter()
        .map(|row| issuer_view(&state, row))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(IssuerPayoutPage { items }))
}

pub(super) async fn prepare_payment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(input): Json<PreparePayoutPaymentInput>,
) -> Result<Json<payments::PaymentView>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    enforce_account_rate_limit(&state.db, account, "zcash_payout_prepare_payment", 60).await?;
    let access = issuer_access(&state.db, account).await?;
    require_issuer_role(&access, &["owner", "admin", "issuer"])?;

    let client = db_client(&state.db).await?;
    expire_issuer_due(&client, access.profile_id).await?;
    let row = client
        .query_opt(
            "SELECT id, subject_account_id, network, state, ciphertext, data_nonce,
                    wrapped_dek, wrap_nonce, key_version, created_at, expires_at, withdrawn_at
             FROM zcash_payout_destinations
             WHERE id = $1
               AND issuer_profile_id = $2
               AND state = 'active'
               AND expires_at > NOW()",
            &[&id, &access.profile_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::NotFound)?;

    let secret = decrypted_secret(&state, &row)?.ok_or(ApiError::NotFound)?;
    let network: String = row.get("network");
    if network != state.zcash_chain {
        return Err(ApiError::Invalid);
    }
    let expected_network = expected_network(&state.zcash_chain)?;
    let summary = zerant_zcash::zip321::create_payment_request(
        &secret.recipient,
        input.amount_zec.trim(),
        expected_network,
    )
    .map_err(|_| ApiError::Invalid)?;

    Ok(Json(
        payments::prepare_for_account(
            &state.db,
            account,
            &state.zcash_chain,
            &summary.canonical_uri,
        )
        .await?,
    ))
}

pub(super) async fn withdraw(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<HolderPayoutView>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    enforce_account_rate_limit(&state.db, account, "zcash_payout_withdraw", 40).await?;
    let mut client = db_client(&state.db).await?;
    let tx = client
        .transaction()
        .await
        .map_err(|_| ApiError::Unavailable)?;

    tx.execute(
        "UPDATE zcash_payout_destinations
         SET state = 'expired', ciphertext = NULL, data_nonce = NULL, wrapped_dek = NULL,
             wrap_nonce = NULL, key_version = NULL
         WHERE subject_account_id = $1 AND state = 'active' AND expires_at <= NOW()",
        &[&account],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;

    let row = tx
        .query_opt(
            "UPDATE zcash_payout_destinations d
         SET state = 'withdrawn', ciphertext = NULL, data_nonce = NULL, wrapped_dek = NULL,
             wrap_nonce = NULL, key_version = NULL, withdrawn_at = NOW()
         FROM issuer_profiles p
         WHERE d.id = $1 AND d.subject_account_id = $2 AND d.state = 'active'
           AND d.expires_at > NOW() AND p.id = d.issuer_profile_id
         RETURNING d.id, d.subject_account_id, p.id AS issuer_profile_id,
                   p.issuer_id, p.display_name AS issuer_name, d.network, d.state,
                   d.created_at, d.expires_at, d.withdrawn_at",
            &[&id, &account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::Conflict)?;

    let issuer_profile_id: Uuid = row.get("issuer_profile_id");
    let issuer_name: String = row.get("issuer_name");
    let network: String = row.get("network");

    tx.execute(
        "INSERT INTO trust_events(account_id, event_type, object_id, label, context, counterparty)
         VALUES ($1, 'zcash_payout_withdrawn', $2, 'Private payout destination', $3, $4)",
        &[&account, &id.to_string(), &network, &issuer_name],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;

    let audit_id = id.to_string();
    record_private_holder_issuer_event(
        &tx,
        issuer_profile_id,
        "payout_destination_withdrawn",
        &audit_id,
        &network,
    )
    .await?;
    tx.commit().await.map_err(|_| ApiError::Unavailable)?;

    Ok(Json(HolderPayoutView {
        id,
        issuer_id: row.get("issuer_id"),
        issuer_name,
        network,
        state: "withdrawn".into(),
        recipient: None,
        purpose: None,
        created_at: row.get("created_at"),
        expires_at: row.get("expires_at"),
        withdrawn_at: row.get("withdrawn_at"),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payout_purpose_is_bounded_and_text_only() {
        assert!(valid_purpose("Contributor reward"));
        assert!(!valid_purpose("x"));
        assert!(!valid_purpose(&"x".repeat(161)));
        assert!(!valid_purpose("bad\ncontext"));
    }

    #[test]
    fn issuer_payout_listing_does_not_serialize_the_recipient() {
        let source = include_str!("payout_destinations.rs");
        let issuer_view = source
            .split("pub(super) struct IssuerPayoutView")
            .nth(1)
            .and_then(|value| value.split("fn expected_network").next())
            .unwrap();
        assert!(!issuer_view.contains("recipient:"));
        assert!(issuer_view.contains("purpose:"));
    }

    #[test]
    fn payout_payment_preparation_uses_active_private_record_only() {
        let source = include_str!("payout_destinations.rs");
        for required in [
            "zcash_payout_prepare_payment",
            "issuer_profile_id = $2",
            "state = 'active'",
            "expires_at > NOW()",
            "payments::prepare_for_account",
            "create_payment_request",
        ] {
            assert!(source.contains(required), "{required}");
        }
    }

    #[test]
    fn payout_access_expires_when_the_trust_relationship_ends() {
        for sql in [EXPIRE_HOLDER_PAYOUTS_SQL, EXPIRE_ISSUER_PAYOUTS_SQL] {
            assert!(sql.contains("NOT EXISTS"));
            assert!(sql.contains("issued_credentials"));
            assert!(sql.contains("revoked_at IS NULL"));
            assert!(sql.contains("expires_at > NOW()"));
            assert!(sql.contains("ciphertext = NULL"));
            assert!(sql.contains("wrapped_dek = NULL"));
        }
    }

    #[test]
    fn one_active_payout_migration_is_relationship_scoped_and_scrubs_duplicates() {
        let migration = include_str!("../migrations/0043_one_active_payout_per_relationship.sql");
        for required in [
            "PARTITION BY issuer_profile_id, subject_account_id",
            "state = 'expired'",
            "ciphertext = NULL",
            "wrapped_dek = NULL",
            "CREATE UNIQUE INDEX",
            "WHERE state = 'active'",
        ] {
            assert!(migration.contains(required), "{required}");
        }
    }

    #[test]
    fn payout_relationship_requires_live_non_revoked_issuer_trust() {
        for sql in [
            ACTIVE_RELATIONSHIP_EXISTS_SQL,
            ELIGIBLE_PAYOUT_ORGANIZATIONS_SQL,
        ] {
            assert!(sql.contains("issued_credentials"));
            assert!(sql.contains("revoked_at IS NULL"));
            assert!(sql.contains("expires_at > NOW()"));
        }
        assert!(ELIGIBLE_PAYOUT_ORGANIZATIONS_SQL.contains("p.retired_at IS NULL"));
    }

    #[test]
    fn payout_network_mapping_is_explicit() {
        assert_eq!(expected_network("zcash:testnet").unwrap(), "testnet");
        assert_eq!(expected_network("zcash:mainnet").unwrap(), "mainnet");
        assert!(expected_network("zcash:regtest").is_err());
    }
}
