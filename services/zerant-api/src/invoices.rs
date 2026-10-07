//! Account-owned Zcash invoices. Public invoice links expose only the exact payment
//! request and lifecycle state; they never imply settlement or expose owner identity.

use super::*;
use zerant_zcash::{address::inspect_address, zip321::create_payment_request};

const MAX_INVOICE_RECORDS: i64 = 50;
const MAX_EXPORT_INVOICE_RECORDS: i64 = 5_000;
const INVOICE_TTL_HOURS: i64 = 24;

#[derive(Serialize)]
pub(super) struct InvoiceExportView {
    id: Uuid,
    request_digest: String,
    recipient: String,
    amount_zat: i64,
    network: String,
    state: String,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    expires_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    cancelled_at: Option<OffsetDateTime>,
}

pub(super) async fn export(
    db: &Pool,
    account: Uuid,
) -> Result<(Vec<InvoiceExportView>, bool), ApiError> {
    let client = db_client(db).await?;
    client
        .execute(
            "UPDATE zcash_invoices SET state = 'expired'
             WHERE account_id = $1 AND state = 'open' AND expires_at <= NOW()",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let rows = client
        .query(
            "SELECT id, request_digest, recipient, amount_zat, network, state, created_at, expires_at, cancelled_at
             FROM zcash_invoices WHERE account_id = $1
             ORDER BY created_at DESC, id DESC LIMIT $2",
            &[&account, &(MAX_EXPORT_INVOICE_RECORDS + 1)],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let complete = rows.len() <= MAX_EXPORT_INVOICE_RECORDS as usize;
    let invoices = rows
        .into_iter()
        .take(MAX_EXPORT_INVOICE_RECORDS as usize)
        .map(|row| {
            let digest: Vec<u8> = row.get("request_digest");
            InvoiceExportView {
                id: row.get("id"),
                request_digest: URL_SAFE_NO_PAD.encode(digest),
                recipient: row.get("recipient"),
                amount_zat: row.get("amount_zat"),
                network: row.get("network"),
                state: row.get("state"),
                created_at: row.get("created_at"),
                expires_at: row.get("expires_at"),
                cancelled_at: row.get("cancelled_at"),
            }
        })
        .collect();
    Ok((invoices, complete))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CreateInvoiceInput {
    recipient: String,
    amount_zec: String,
}

#[derive(Serialize)]
pub(super) struct InvoicePage {
    items: Vec<InvoiceView>,
}

#[derive(Serialize)]
pub(super) struct InvoiceView {
    id: Uuid,
    recipient: String,
    amount_zat: i64,
    network: String,
    state: String,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    expires_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    cancelled_at: Option<OffsetDateTime>,
    payment_uri: Option<String>,
}

#[derive(Serialize)]
pub(super) struct PublicInvoiceView {
    id: Uuid,
    amount_zat: i64,
    network: String,
    state: String,
    #[serde(with = "time::serde::rfc3339")]
    expires_at: OffsetDateTime,
    payment_uri: Option<String>,
    transparent_only: bool,
}

const VIEW_COLUMNS: &str = "id, request_digest, recipient, amount_zat, network, state, created_at, expires_at, cancelled_at";

fn expected_network(chain: &str) -> Result<&'static str, ApiError> {
    match chain {
        "zcash:testnet" => Ok("testnet"),
        "zcash:mainnet" => Ok("mainnet"),
        _ => Err(ApiError::Unavailable),
    }
}

fn format_amount_zec(amount_zat: i64) -> Option<String> {
    if amount_zat <= 0 {
        return None;
    }
    let whole = amount_zat / 100_000_000;
    let fraction = amount_zat % 100_000_000;
    Some(if fraction == 0 {
        whole.to_string()
    } else {
        format!("{whole}.{fraction:08}")
            .trim_end_matches('0')
            .to_owned()
    })
}

fn payment_uri(row: &Row, effective_state: &str) -> Option<String> {
    if effective_state != "open" {
        return None;
    }
    let network: String = row.get("network");
    let expected = expected_network(&network).ok()?;
    let recipient: String = row.get("recipient");
    let amount_zat: i64 = row.get("amount_zat");
    let amount = format_amount_zec(amount_zat)?;
    let uri = create_payment_request(&recipient, &amount, expected)
        .ok()?
        .canonical_uri;
    let digest: Vec<u8> = row.get("request_digest");
    (digest.len() == 32 && Sha256::digest(uri.as_bytes()).as_slice() == digest.as_slice())
        .then_some(uri)
}

fn effective_state(row: &Row, now: OffsetDateTime) -> String {
    let state: String = row.get("state");
    if state == "open" && row.get::<_, OffsetDateTime>("expires_at") <= now {
        "expired".into()
    } else {
        state
    }
}

fn owner_view(row: &Row, now: OffsetDateTime) -> InvoiceView {
    let state = effective_state(row, now);
    InvoiceView {
        id: row.get("id"),
        recipient: row.get("recipient"),
        amount_zat: row.get("amount_zat"),
        network: row.get("network"),
        state: state.clone(),
        created_at: row.get("created_at"),
        expires_at: row.get("expires_at"),
        cancelled_at: row.get("cancelled_at"),
        payment_uri: payment_uri(row, &state),
    }
}

fn public_view(row: &Row, now: OffsetDateTime) -> PublicInvoiceView {
    let state = effective_state(row, now);
    PublicInvoiceView {
        id: row.get("id"),
        amount_zat: row.get("amount_zat"),
        network: row.get("network"),
        state: state.clone(),
        expires_at: row.get("expires_at"),
        payment_uri: payment_uri(row, &state),
        transparent_only: inspect_address(row.get("recipient"))
            .map(|address| address.transparent_only)
            .unwrap_or(true),
    }
}

pub(super) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateInvoiceInput>,
) -> Result<Json<InvoiceView>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    enforce_account_rate_limit(&state.db, account, "zcash_invoice_create", 30).await?;
    let expected = expected_network(&state.zcash_chain)?;
    let summary = create_payment_request(input.recipient.trim(), input.amount_zec.trim(), expected)
        .map_err(|_| ApiError::Invalid)?;
    if summary.payment_count != 1 {
        return Err(ApiError::Invalid);
    }
    let payment = summary.payments.first().ok_or(ApiError::Invalid)?;
    let amount_zat = payment
        .amount_zat
        .filter(|value| *value > 0)
        .ok_or(ApiError::Invalid)?;
    let amount_zat = i64::try_from(amount_zat).map_err(|_| ApiError::Invalid)?;
    let digest = Sha256::digest(summary.canonical_uri.as_bytes()).to_vec();
    let id = Uuid::new_v4();
    let client = db_client(&state.db).await?;
    let row = client
        .query_one(
            &format!(
                "INSERT INTO zcash_invoices(id, account_id, request_digest, recipient, amount_zat, network, state, expires_at)\n                 VALUES ($1, $2, $3, $4, $5, $6, 'open', NOW() + ($7 * INTERVAL '1 hour'))\n                 RETURNING {VIEW_COLUMNS}"
            ),
            &[
                &id,
                &account,
                &digest,
                &payment.recipient,
                &amount_zat,
                &state.zcash_chain,
                &INVOICE_TTL_HOURS,
            ],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    Ok(Json(owner_view(&row, OffsetDateTime::now_utc())))
}

pub(super) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<InvoicePage>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    let client = db_client(&state.db).await?;
    client
        .execute(
            "UPDATE zcash_invoices SET state = 'expired'\n             WHERE account_id = $1 AND state = 'open' AND expires_at <= NOW()",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let rows = client
        .query(
            &format!(
                "SELECT {VIEW_COLUMNS} FROM zcash_invoices\n                 WHERE account_id = $1 ORDER BY created_at DESC, id DESC LIMIT $2"
            ),
            &[&account, &MAX_INVOICE_RECORDS],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let now = OffsetDateTime::now_utc();
    Ok(Json(InvoicePage {
        items: rows.iter().map(|row| owner_view(row, now)).collect(),
    }))
}

pub(super) async fn cancel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<InvoiceView>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    enforce_account_rate_limit(&state.db, account, "zcash_invoice_cancel", 60).await?;
    let mut client = db_client(&state.db).await?;
    let tx = client
        .transaction()
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let row = tx
        .query_opt(
            &format!(
                "SELECT {VIEW_COLUMNS} FROM zcash_invoices WHERE id = $1 AND account_id = $2 FOR UPDATE"
            ),
            &[&id, &account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::NotFound)?;
    let current = owner_view(&row, OffsetDateTime::now_utc());
    if current.state == "cancelled" {
        tx.commit().await.map_err(|_| ApiError::Unavailable)?;
        return Ok(Json(current));
    }
    if current.state != "open" {
        return Err(ApiError::Conflict);
    }
    let row = tx
        .query_one(
            &format!(
                "UPDATE zcash_invoices SET state = 'cancelled', cancelled_at = NOW()\n                 WHERE id = $1 AND account_id = $2 AND state = 'open'\n                 RETURNING {VIEW_COLUMNS}"
            ),
            &[&id, &account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    tx.commit().await.map_err(|_| ApiError::Unavailable)?;
    Ok(Json(owner_view(&row, OffsetDateTime::now_utc())))
}

pub(super) async fn public_get(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<PublicInvoiceView>, ApiError> {
    let client = db_client(&state.db).await?;
    let row = client
        .query_opt(
            &format!("SELECT {VIEW_COLUMNS} FROM zcash_invoices WHERE id = $1"),
            &[&id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(public_view(&row, OffsetDateTime::now_utc())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invoice_amount_format_is_exact_and_bounded() {
        assert_eq!(format_amount_zec(100_000_000).as_deref(), Some("1"));
        assert_eq!(
            format_amount_zec(123_456_789).as_deref(),
            Some("1.23456789")
        );
        assert_eq!(format_amount_zec(1).as_deref(), Some("0.00000001"));
        assert!(format_amount_zec(0).is_none());
    }

    #[test]
    fn invoice_network_mapping_is_strict() {
        assert_eq!(expected_network("zcash:testnet").unwrap(), "testnet");
        assert_eq!(expected_network("zcash:mainnet").unwrap(), "mainnet");
        assert!(expected_network("zcash:regtest").is_err());
    }
}
