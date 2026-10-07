//! Account-scoped payment tracking. Submitted txids are untrusted wallet reports.
//! Testnet observation is intentionally absent until exact named-transaction
//! recipient and amount verification is available from a trustworthy source.

use super::*;
use zerant_zcash::{
    address::inspect_address,
    lightclient::{
        LightClientNetwork, LightClientTransactionState, fetch_light_client_readiness,
        fetch_light_client_transaction_state,
    },
    payment::validate_txid,
    zip321::{create_payment_request, inspect_payment_request},
};

const MAX_PAYMENT_RECORDS: i64 = 50;
const MAX_EXPORT_PAYMENT_RECORDS: i64 = 5_000;
const MIN_CONFIRMATIONS: i32 = 10;

#[derive(Deserialize)]
pub(super) struct ListQuery {
    cursor: Option<String>,
}

#[derive(Serialize)]
pub(super) struct PaymentPage {
    items: Vec<PaymentView>,
    next_cursor: Option<String>,
}

#[derive(Serialize)]
pub(super) struct PaymentExportView {
    id: Uuid,
    request_digest: String,
    recipient: String,
    amount_zat: i64,
    network: String,
    min_confirmations: i32,
    state: String,
    txid: Option<String>,
    created_at: OffsetDateTime,
    expires_at: OffsetDateTime,
    submitted_at: Option<OffsetDateTime>,
    network_state: Option<String>,
    observed_height: Option<i64>,
    confirmations: Option<i64>,
    observed_at: Option<OffsetDateTime>,
}

pub(super) async fn export(
    db: &Pool,
    account: Uuid,
) -> Result<(Vec<PaymentExportView>, bool), ApiError> {
    let client = db_client(db).await?;
    let rows = client
        .query(
            "SELECT id, request_digest, recipient, amount_zat, network, min_confirmations,
                state, txid, created_at, expires_at, submitted_at, network_state,
                observed_height, confirmations, observed_at
         FROM zcash_payments WHERE account_id = $1
         ORDER BY created_at DESC, id DESC LIMIT $2",
            &[&account, &(MAX_EXPORT_PAYMENT_RECORDS + 1)],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let complete = rows.len() <= MAX_EXPORT_PAYMENT_RECORDS as usize;
    let payments = rows
        .into_iter()
        .take(MAX_EXPORT_PAYMENT_RECORDS as usize)
        .map(|row| {
            let digest: Vec<u8> = row.get("request_digest");
            PaymentExportView {
                id: row.get("id"),
                request_digest: URL_SAFE_NO_PAD.encode(digest),
                recipient: row.get("recipient"),
                amount_zat: row.get("amount_zat"),
                network: row.get("network"),
                min_confirmations: row.get("min_confirmations"),
                state: row.get("state"),
                txid: row.get("txid"),
                created_at: row.get("created_at"),
                expires_at: row.get("expires_at"),
                submitted_at: row.get("submitted_at"),
                network_state: row.get("network_state"),
                observed_height: row.get("observed_height"),
                confirmations: row.get("confirmations"),
                observed_at: row.get("observed_at"),
            }
        })
        .collect();
    Ok((payments, complete))
}

fn page_cursor(payment: &PaymentView) -> String {
    URL_SAFE_NO_PAD.encode(format!(
        "{}:{}",
        payment.created_at.unix_timestamp_nanos(),
        payment.id
    ))
}

fn parse_page_cursor(cursor: &str) -> Result<(OffsetDateTime, Uuid), ApiError> {
    if cursor.len() > 128 {
        return Err(ApiError::Invalid);
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(cursor)
        .map_err(|_| ApiError::Invalid)?;
    let raw = std::str::from_utf8(&bytes).map_err(|_| ApiError::Invalid)?;
    let (time, id) = raw.split_once(':').ok_or(ApiError::Invalid)?;
    let time = time.parse::<i128>().map_err(|_| ApiError::Invalid)?;
    let time = OffsetDateTime::from_unix_timestamp_nanos(time).map_err(|_| ApiError::Invalid)?;
    let id = Uuid::parse_str(id).map_err(|_| ApiError::Invalid)?;
    Ok((time, id))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PrepareInput {
    canonical_uri: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SubmitInput {
    txid: String,
}

#[derive(Serialize)]
pub(super) struct PaymentView {
    id: Uuid,
    recipient: String,
    amount_zat: i64,
    network: String,
    min_confirmations: i32,
    state: String,
    txid: Option<String>,
    created_at: OffsetDateTime,
    expires_at: OffsetDateTime,
    submitted_at: Option<OffsetDateTime>,
    network_state: Option<String>,
    observed_height: Option<i64>,
    confirmations: Option<i64>,
    observed_at: Option<OffsetDateTime>,
    payment_uri: Option<String>,
    transparent_only: bool,
}

fn reopen_payment_uri(
    recipient: &str,
    amount_zat: i64,
    network: &str,
    digest: &[u8],
) -> Option<String> {
    if amount_zat <= 0 || network != "zcash:testnet" || digest.len() != 32 {
        return None;
    }
    let whole = amount_zat / 100_000_000;
    let fraction = amount_zat % 100_000_000;
    let amount = if fraction == 0 {
        whole.to_string()
    } else {
        format!("{whole}.{fraction:08}")
            .trim_end_matches('0')
            .to_owned()
    };
    let uri = create_payment_request(recipient, &amount, "testnet")
        .ok()?
        .canonical_uri;
    (Sha256::digest(uri.as_bytes()).as_slice() == digest).then_some(uri)
}

fn view(row: &Row) -> PaymentView {
    let state: String = row.get("state");
    let payment_uri = if state == "prepared"
        && row.get::<_, OffsetDateTime>("expires_at") > OffsetDateTime::now_utc()
    {
        reopen_payment_uri(
            row.get("recipient"),
            row.get("amount_zat"),
            row.get("network"),
            row.get::<_, Vec<u8>>("request_digest").as_slice(),
        )
    } else {
        None
    };
    PaymentView {
        id: row.get("id"),
        recipient: row.get("recipient"),
        amount_zat: row.get("amount_zat"),
        network: row.get("network"),
        min_confirmations: row.get("min_confirmations"),
        state,
        txid: row.get("txid"),
        created_at: row.get("created_at"),
        expires_at: row.get("expires_at"),
        submitted_at: row.get("submitted_at"),
        network_state: row.get("network_state"),
        observed_height: row.get("observed_height"),
        confirmations: row.get("confirmations"),
        observed_at: row.get("observed_at"),
        payment_uri,
        transparent_only: inspect_address(row.get("recipient"))
            .map(|address| address.transparent_only)
            .unwrap_or(true),
    }
}

const VIEW_COLUMNS: &str = "id, request_digest, recipient, amount_zat, network, min_confirmations, state, txid, created_at, expires_at, submitted_at, network_state, observed_height, confirmations, observed_at";

fn payment_light_client_network(network: &str) -> Result<LightClientNetwork, ApiError> {
    match network {
        "zcash:mainnet" => Ok(LightClientNetwork::Mainnet),
        "zcash:testnet" => Ok(LightClientNetwork::Testnet),
        _ => Err(ApiError::Unavailable),
    }
}

fn observation_projection(
    observation: LightClientTransactionState,
) -> Result<(&'static str, Option<i64>, i64), ApiError> {
    match observation {
        LightClientTransactionState::Mempool => Ok(("mempool", None, 0)),
        LightClientTransactionState::Forked => Ok(("forked", None, 0)),
        LightClientTransactionState::Mined {
            height,
            confirmations,
        } => Ok((
            "mined",
            Some(i64::try_from(height).map_err(|_| ApiError::Unavailable)?),
            i64::try_from(confirmations).map_err(|_| ApiError::Unavailable)?,
        )),
    }
}

fn trackable_request(
    uri: &str,
    expected_network: &str,
) -> Result<(String, i64, Vec<u8>), ApiError> {
    let summary = inspect_payment_request(uri).map_err(|_| ApiError::Invalid)?;
    if summary.canonical_uri != uri || summary.payment_count != 1 {
        return Err(ApiError::Invalid);
    }
    let payment = &summary.payments[0];
    if payment.memo_present
        || payment.label.is_some()
        || payment.message.is_some()
        || !payment.other_param_names.is_empty()
    {
        return Err(ApiError::Invalid);
    }
    let amount = payment
        .amount_zat
        .filter(|amount| *amount > 0)
        .ok_or(ApiError::Invalid)?;
    let amount = i64::try_from(amount).map_err(|_| ApiError::Invalid)?;
    let address = inspect_address(&payment.recipient).map_err(|_| ApiError::Invalid)?;
    if format!("zcash:{}", address.network) != expected_network {
        return Err(ApiError::Invalid);
    }
    Ok((
        payment.recipient.clone(),
        amount,
        Sha256::digest(uri.as_bytes()).to_vec(),
    ))
}

pub(super) async fn prepare_for_account(
    db: &Pool,
    account: Uuid,
    network: &str,
    canonical_uri: &str,
) -> Result<PaymentView, ApiError> {
    let (recipient, amount, digest) = trackable_request(canonical_uri, network)?;
    let client = db_client(db).await?;
    let row = client.query_one(
        &format!("INSERT INTO zcash_payments(id, account_id, request_digest, recipient, amount_zat, network, min_confirmations, state, expires_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, 'prepared', NOW() + INTERVAL '24 hours')
            RETURNING {VIEW_COLUMNS}"),
        &[&Uuid::new_v4(), &account, &digest, &recipient, &amount, &network, &MIN_CONFIRMATIONS],
    ).await.map_err(|_| ApiError::Unavailable)?;
    Ok(view(&row))
}

pub(super) async fn prepare(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<PrepareInput>,
) -> Result<Json<PaymentView>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    enforce_account_rate_limit(&state.db, account, "zcash_payment_prepare", 60).await?;
    Ok(Json(
        prepare_for_account(&state.db, account, &state.zcash_chain, &input.canonical_uri).await?,
    ))
}

pub(super) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListQuery>,
) -> Result<Json<PaymentPage>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    let client = db_client(&state.db).await?;
    client
        .execute(
            "UPDATE zcash_payments SET state = 'expired'
         WHERE account_id = $1 AND state = 'prepared' AND expires_at <= NOW()",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let fetch_limit = MAX_PAYMENT_RECORDS + 1;
    let rows = if let Some(cursor) = query.cursor.as_deref() {
        let (created_at, id) = parse_page_cursor(cursor)?;
        client
            .query(
                &format!(
                    "SELECT {VIEW_COLUMNS} FROM zcash_payments
                WHERE account_id = $1 AND (created_at, id) < ($2, $3)
                ORDER BY created_at DESC, id DESC LIMIT $4"
                ),
                &[&account, &created_at, &id, &fetch_limit],
            )
            .await
            .map_err(|_| ApiError::Unavailable)?
    } else {
        client
            .query(
                &format!(
                    "SELECT {VIEW_COLUMNS} FROM zcash_payments
                WHERE account_id = $1 ORDER BY created_at DESC, id DESC LIMIT $2"
                ),
                &[&account, &fetch_limit],
            )
            .await
            .map_err(|_| ApiError::Unavailable)?
    };
    let has_more = rows.len() > MAX_PAYMENT_RECORDS as usize;
    let items: Vec<_> = rows
        .iter()
        .take(MAX_PAYMENT_RECORDS as usize)
        .map(view)
        .collect();
    let next_cursor = if has_more {
        items.last().map(page_cursor)
    } else {
        None
    };
    Ok(Json(PaymentPage { items, next_cursor }))
}

pub(super) async fn submit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(input): Json<SubmitInput>,
) -> Result<Json<PaymentView>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    enforce_account_rate_limit(&state.db, account, "zcash_payment_submit", 120).await?;
    validate_txid(&input.txid).map_err(|_| ApiError::Invalid)?;
    let txid = input.txid.to_ascii_lowercase();
    let mut client = db_client(&state.db).await?;
    let transaction = client
        .transaction()
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let row = transaction
        .query_opt(
            &format!(
                "SELECT {VIEW_COLUMNS} FROM zcash_payments
            WHERE id = $1 AND account_id = $2 FOR UPDATE"
            ),
            &[&id, &account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::NotFound)?;
    let current = view(&row);
    if current.state == "submitted" {
        if current.txid.as_deref() != Some(&txid) {
            return Err(ApiError::Conflict);
        }
        transaction
            .commit()
            .await
            .map_err(|_| ApiError::Unavailable)?;
        return Ok(Json(current));
    }
    if current.state != "prepared" || current.expires_at <= OffsetDateTime::now_utc() {
        return Err(ApiError::Conflict);
    }
    let row = transaction
        .query_opt(
            &format!(
                "UPDATE zcash_payments
            SET state = 'submitted', txid = $1, submitted_at = NOW()
            WHERE id = $2 AND account_id = $3 AND state = 'prepared' AND expires_at > NOW()
            RETURNING {VIEW_COLUMNS}"
            ),
            &[&txid, &id, &account],
        )
        .await
        .map_err(|error| {
            if error.code() == Some(&tokio_postgres::error::SqlState::UNIQUE_VIOLATION) {
                ApiError::Conflict
            } else {
                ApiError::Unavailable
            }
        })?
        .ok_or(ApiError::Conflict)?;
    transaction
        .commit()
        .await
        .map_err(|_| ApiError::Unavailable)?;
    Ok(Json(view(&row)))
}

pub(super) async fn observe(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<PaymentView>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    enforce_account_rate_limit(&state.db, account, "zcash_payment_observe", 60).await?;
    let client = db_client(&state.db).await?;
    let row = client
        .query_opt(
            &format!("SELECT {VIEW_COLUMNS} FROM zcash_payments WHERE id = $1 AND account_id = $2"),
            &[&id, &account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::NotFound)?;
    let current = view(&row);
    if current.state != "submitted" || current.network != state.zcash_chain {
        return Err(ApiError::Conflict);
    }
    let txid = current.txid.as_deref().ok_or(ApiError::Conflict)?;
    if state.light_client_endpoints.is_empty() {
        return Err(ApiError::Unavailable);
    }
    let expected_network = payment_light_client_network(&current.network)?;
    let network_label = light_client_network_label(expected_network);
    let high_water = load_light_client_high_water(&client, network_label).await?;
    let mut observed = None;
    for endpoint in &state.light_client_endpoints {
        let Ok(readiness) = fetch_light_client_readiness(
            endpoint,
            expected_network,
            state.light_client_allow_loopback,
        )
        .await
        else {
            continue;
        };
        if !readiness.synced
            || !light_client_height_is_acceptable(readiness.block_height, high_water)
        {
            continue;
        }
        if let Ok(value) = fetch_light_client_transaction_state(
            endpoint,
            expected_network,
            state.light_client_allow_loopback,
            txid,
        )
        .await
        {
            observed = Some(value);
            break;
        }
    }
    let (network_state, observed_height, confirmations) =
        observation_projection(observed.ok_or(ApiError::Unavailable)?)?;
    let row = client
        .query_one(
            &format!(
                "UPDATE zcash_payments
                 SET network_state = $1, observed_height = $2, confirmations = $3, observed_at = NOW()
                 WHERE id = $4 AND account_id = $5 AND state = 'submitted' AND txid = $6
                 RETURNING {VIEW_COLUMNS}"
            ),
            &[
                &network_state,
                &observed_height,
                &confirmations,
                &id,
                &account,
                &txid,
            ],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    Ok(Json(view(&row)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracked_payment_requires_exact_canonical_single_payment() {
        const ADDRESS: &str = "tmEZhbWHTpdKMw5it8YDspUXSMGQyFwovpU";
        let uri = zerant_zcash::zip321::create_payment_request(ADDRESS, "1.25", "testnet")
            .unwrap()
            .canonical_uri;
        let (recipient, amount, digest) = trackable_request(&uri, "zcash:testnet").unwrap();
        assert_eq!(recipient, ADDRESS);
        assert_eq!(amount, 125_000_000);
        assert_eq!(digest.len(), 32);
        assert!(trackable_request(&uri, "zcash:mainnet").is_err());
        assert!(trackable_request(&(uri + "&memo=secret"), "zcash:testnet").is_err());
        let labeled = format!("zcash:{ADDRESS}?amount=1.25&label=private");
        assert!(trackable_request(&labeled, "zcash:testnet").is_err());
        assert!(parse_page_cursor("malformed").is_err());
        assert_eq!(
            reopen_payment_uri(ADDRESS, amount, "zcash:testnet", &digest),
            Some("zcash:tmEZhbWHTpdKMw5it8YDspUXSMGQyFwovpU?amount=1.25".into())
        );
        assert!(reopen_payment_uri(ADDRESS, amount + 1, "zcash:testnet", &digest).is_none());
        assert!(reopen_payment_uri(ADDRESS, amount, "zcash:mainnet", &digest).is_none());
        assert!(reopen_payment_uri(ADDRESS, amount, "zcash:testnet", &[0; 32]).is_none());
    }

    #[test]
    fn network_observation_projection_keeps_intent_verification_separate() {
        assert_eq!(
            observation_projection(LightClientTransactionState::Mempool).unwrap(),
            ("mempool", None, 0)
        );
        assert_eq!(
            observation_projection(LightClientTransactionState::Forked).unwrap(),
            ("forked", None, 0)
        );
        assert_eq!(
            observation_projection(LightClientTransactionState::Mined {
                height: 100,
                confirmations: 7,
            })
            .unwrap(),
            ("mined", Some(100), 7)
        );
    }

    // Run with ZERANT_TEST_DATABASE_URL pointing at a disposable PostgreSQL database.
    #[tokio::test]
    async fn payment_records_bind_account_intent_and_one_txid() {
        let Ok(database_url) = env::var("ZERANT_TEST_DATABASE_URL") else {
            return;
        };
        let config = tokio_postgres::Config::from_str(&database_url).unwrap();
        let manager = Manager::from_config(
            config,
            NoTls,
            ManagerConfig {
                recycling_method: RecyclingMethod::Fast,
            },
        );
        let db = Pool::builder(manager).max_size(8).build().unwrap();
        run_migrations(&db).await.unwrap();
        let origin = url::Url::parse("https://zerant.example").unwrap();
        let webauthn = WebauthnBuilder::new("zerant.example", &origin)
            .unwrap()
            .rp_name("Zerant")
            .build()
            .unwrap();
        let state = AppState {
            db: db.clone(),
            cipher: Arc::new(
                VaultCipher::from_local_keys(
                    BTreeMap::from([(1, Aes256Gcm::new_from_slice(&[7; 32]).unwrap())]),
                    1,
                )
                .unwrap(),
            ),
            webauthn: Arc::new(webauthn),
            public_origin: "https://zerant.example".into(),
            zcash_chain: "zcash:testnet".into(),
            light_client_endpoints: Vec::new(),
            light_client_allow_loopback: false,
            light_client_cache: Arc::new(tokio::sync::Mutex::new(None)),
            allowed_scopes: BTreeSet::from(["auth".into()]),
        };
        let mut client = db_client(&db).await.unwrap();
        let mut accounts = Vec::new();
        let mut headers = Vec::new();
        for _ in 0..2 {
            let account = Uuid::new_v4();
            let handle = zerant_public_handle(account.as_bytes());
            client
                .execute(
                    "INSERT INTO accounts(id, public_handle) VALUES ($1, $2)",
                    &[&account, &handle],
                )
                .await
                .unwrap();
            let token = format!("test_session_{}_random", Uuid::new_v4());
            let hash = Sha256::digest(token.as_bytes()).to_vec();
            client.execute("INSERT INTO sessions(id, account_id, token_hash, expires_at) VALUES ($1, $2, $3, NOW() + INTERVAL '1 day')", &[&Uuid::new_v4(), &account, &hash]).await.unwrap();
            let mut header = HeaderMap::new();
            header.insert(
                axum::http::header::COOKIE,
                format!("zerant_session={token}").parse().unwrap(),
            );
            accounts.push(account);
            headers.push(header);
        }
        let uri = zerant_zcash::zip321::create_payment_request(
            "tmEZhbWHTpdKMw5it8YDspUXSMGQyFwovpU",
            "1.25",
            "testnet",
        )
        .unwrap()
        .canonical_uri;
        let first = prepare(
            State(state.clone()),
            headers[0].clone(),
            Json(PrepareInput {
                canonical_uri: uri.clone(),
            }),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(first.state, "prepared");
        assert_eq!(first.amount_zat, 125_000_000);
        let prepared_events = client
            .query(
                "SELECT event_type, object_id, label, context, counterparty
                 FROM trust_events WHERE account_id = $1 AND object_id = $2
                 ORDER BY id",
                &[&accounts[0], &first.id.to_string()],
            )
            .await
            .unwrap();
        assert_eq!(prepared_events.len(), 1);
        assert_eq!(
            prepared_events[0].get::<_, String>(0),
            "zcash_payment_prepared"
        );
        assert_eq!(prepared_events[0].get::<_, String>(1), first.id.to_string());
        assert_eq!(prepared_events[0].get::<_, String>(2), "Zcash payment");
        assert_eq!(
            prepared_events[0].get::<_, Option<String>>(3).as_deref(),
            Some("zcash:testnet")
        );
        assert!(prepared_events[0].get::<_, Option<String>>(4).is_none());
        assert!(
            client
                .execute(
                    "UPDATE zcash_payments
                     SET network_state = 'mempool', confirmations = 0, observed_at = NOW()
                     WHERE id = $1",
                    &[&first.id],
                )
                .await
                .is_err()
        );
        assert_eq!(
            parse_page_cursor(&page_cursor(&first)).unwrap(),
            (first.created_at, first.id)
        );
        assert!(
            list(
                State(state.clone()),
                headers[1].clone(),
                Query(ListQuery { cursor: None })
            )
            .await
            .unwrap()
            .0
            .items
            .is_empty()
        );
        let txid = "a".repeat(64);
        assert!(matches!(
            submit(
                State(state.clone()),
                headers[1].clone(),
                Path(first.id),
                Json(SubmitInput { txid: txid.clone() })
            )
            .await,
            Err(ApiError::NotFound)
        ));
        assert!(matches!(
            submit(
                State(state.clone()),
                headers[0].clone(),
                Path(first.id),
                Json(SubmitInput { txid: "bad".into() })
            )
            .await,
            Err(ApiError::Invalid)
        ));
        let submitted = submit(
            State(state.clone()),
            headers[0].clone(),
            Path(first.id),
            Json(SubmitInput { txid: txid.clone() }),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(submitted.state, "submitted");
        assert_eq!(submitted.txid.as_deref(), Some(txid.as_str()));
        assert!(submitted.network_state.is_none());
        let submitted_events = client
            .query(
                "SELECT event_type, object_id, label, context, counterparty
                 FROM trust_events WHERE account_id = $1 AND object_id = $2
                 ORDER BY id",
                &[&accounts[0], &first.id.to_string()],
            )
            .await
            .unwrap();
        assert_eq!(submitted_events.len(), 2);
        assert_eq!(
            submitted_events[1].get::<_, String>(0),
            "zcash_payment_submitted"
        );
        assert_eq!(
            submitted_events[1].get::<_, String>(1),
            first.id.to_string()
        );
        assert_eq!(submitted_events[1].get::<_, String>(2), "Zcash payment");
        assert_eq!(
            submitted_events[1].get::<_, Option<String>>(3).as_deref(),
            Some("zcash:testnet")
        );
        assert!(submitted_events[1].get::<_, Option<String>>(4).is_none());
        client
            .execute(
                "UPDATE zcash_payments
                 SET network_state = 'mempool', confirmations = 0, observed_at = NOW()
                 WHERE id = $1",
                &[&first.id],
            )
            .await
            .unwrap();
        let observed_mempool = list(
            State(state.clone()),
            headers[0].clone(),
            Query(ListQuery { cursor: None }),
        )
        .await
        .unwrap()
        .0
        .items
        .into_iter()
        .find(|payment| payment.id == first.id)
        .unwrap();
        assert_eq!(observed_mempool.network_state.as_deref(), Some("mempool"));
        assert_eq!(observed_mempool.confirmations, Some(0));
        assert!(observed_mempool.observed_height.is_none());
        assert!(observed_mempool.observed_at.is_some());
        assert!(
            client
                .execute(
                    "UPDATE zcash_payments
                     SET network_state = 'mined', observed_height = NULL,
                         confirmations = 10, observed_at = NOW()
                     WHERE id = $1",
                    &[&first.id],
                )
                .await
                .is_err()
        );
        client
            .execute(
                "UPDATE zcash_payments
                 SET network_state = 'mined', observed_height = 100,
                     confirmations = 10, observed_at = NOW()
                 WHERE id = $1",
                &[&first.id],
            )
            .await
            .unwrap();
        assert!(
            client
                .execute(
                    "UPDATE zcash_payments SET state = 'confirmed' WHERE id = $1",
                    &[&first.id]
                )
                .await
                .is_err()
        );
        assert_eq!(
            submit(
                State(state.clone()),
                headers[0].clone(),
                Path(first.id),
                Json(SubmitInput { txid: txid.clone() })
            )
            .await
            .unwrap()
            .0
            .txid,
            Some(txid.clone())
        );
        assert!(matches!(
            submit(
                State(state.clone()),
                headers[0].clone(),
                Path(first.id),
                Json(SubmitInput {
                    txid: "b".repeat(64)
                })
            )
            .await,
            Err(ApiError::Conflict)
        ));
        let second = prepare(
            State(state.clone()),
            headers[1].clone(),
            Json(PrepareInput {
                canonical_uri: uri.clone(),
            }),
        )
        .await
        .unwrap()
        .0;
        assert!(matches!(
            submit(
                State(state.clone()),
                headers[1].clone(),
                Path(second.id),
                Json(SubmitInput { txid })
            )
            .await,
            Err(ApiError::Conflict)
        ));
        client
            .execute(
                "UPDATE zcash_payments SET expires_at = NOW() - INTERVAL '1 minute' WHERE id = $1",
                &[&second.id],
            )
            .await
            .unwrap_err();
        let third = prepare(
            State(state.clone()),
            headers[1].clone(),
            Json(PrepareInput { canonical_uri: uri }),
        )
        .await
        .unwrap()
        .0;
        client.execute("UPDATE zcash_payments SET created_at = NOW() - INTERVAL '2 days', expires_at = NOW() - INTERVAL '1 day' WHERE id = $1", &[&third.id]).await.unwrap();
        assert!(matches!(
            submit(
                State(state.clone()),
                headers[1].clone(),
                Path(third.id),
                Json(SubmitInput {
                    txid: "c".repeat(64)
                })
            )
            .await,
            Err(ApiError::Conflict)
        ));
        assert_eq!(
            list(
                State(state.clone()),
                headers[1].clone(),
                Query(ListQuery { cursor: None })
            )
            .await
            .unwrap()
            .0
            .items
            .iter()
            .find(|payment| payment.id == third.id)
            .unwrap()
            .state,
            "expired"
        );
        for _ in 0..51 {
            client.execute(
                "INSERT INTO zcash_payments(id, account_id, request_digest, recipient, amount_zat, network, min_confirmations, state, expires_at)
                 VALUES ($1, $2, $3, $4, $5, 'zcash:testnet', 10, 'prepared', NOW() + INTERVAL '1 day')",
                &[&Uuid::new_v4(), &accounts[0], &vec![4_u8; 32], &"tmEZhbWHTpdKMw5it8YDspUXSMGQyFwovpU", &125_000_000_i64],
            ).await.unwrap();
        }
        let page = list(
            State(state.clone()),
            headers[0].clone(),
            Query(ListQuery { cursor: None }),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(page.items.len(), 50);
        let next = list(
            State(state),
            headers[0].clone(),
            Query(ListQuery {
                cursor: page.next_cursor,
            }),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(next.items.len(), 2);
        assert!(next.next_cursor.is_none());
        let (exported, complete) = export(&db, accounts[0]).await.unwrap();
        assert!(complete);
        assert_eq!(exported.len(), 52);
        let submitted_export = exported
            .iter()
            .find(|payment| payment.id == first.id)
            .unwrap();
        let expected_txid = "a".repeat(64);
        assert_eq!(
            submitted_export.txid.as_deref(),
            Some(expected_txid.as_str())
        );
        assert_eq!(submitted_export.network_state.as_deref(), Some("mined"));
        assert_eq!(submitted_export.observed_height, Some(100));
        assert_eq!(submitted_export.confirmations, Some(10));
        assert!(submitted_export.observed_at.is_some());
        assert_eq!(
            URL_SAFE_NO_PAD
                .decode(&submitted_export.request_digest)
                .unwrap()
                .len(),
            32
        );
        assert_eq!(export(&db, accounts[1]).await.unwrap().0.len(), 2);
        let cleanup = client.transaction().await.unwrap();
        cleanup
            .query_one(
                "SELECT set_config('zerant.allow_account_delete', 'on', true)",
                &[],
            )
            .await
            .unwrap();
        for account in accounts {
            cleanup
                .execute("DELETE FROM accounts WHERE id = $1", &[&account])
                .await
                .unwrap();
        }
        cleanup.commit().await.unwrap();
    }
}
