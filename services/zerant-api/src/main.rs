use aes_gcm::{
    Aes256Gcm, KeyInit,
    aead::{Aead, Payload, rand_core::RngCore},
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header::SET_COOKIE},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use native_tls::TlsConnector;
use postgres_native_tls::MakeTlsConnector;
use rand::rngs::OsRng;
use reddsa::{Signature, VerificationKey, orchard::SpendAuth};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    convert::TryFrom,
    env,
    net::{IpAddr, SocketAddr},
    str::FromStr,
    sync::Arc,
};
use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};
use tokio_postgres::{NoTls, Row, Transaction, config::SslMode};
use tower_http::trace::TraceLayer;
use tracing::info;
use uuid::Uuid;
use zerant_zcash::{Adapter, HttpRegtestTransport};

const SESSION_COOKIE: &str = "zerant_session";
const AUTH_ATTEMPT_COOKIE: &str = "zerant_auth_attempt";
const MAX_CREDENTIAL_BYTES: usize = 256 * 1024;
const SESSION_TTL_DAYS: i64 = 7;
const ZECAUTH_TTL_MINUTES: i64 = 5;

#[derive(Clone)]
struct AppState {
    db: Pool,
    cipher: Arc<VaultCipher>,
    public_origin: String,
    zcash_chain: String,
    allowed_scopes: BTreeSet<String>,
}

#[derive(Clone)]
struct VaultCipher {
    kek: Aes256Gcm,
    key_version: i32,
}

#[derive(Debug, thiserror::Error)]
enum ApiError {
    #[error("unauthorized")]
    Unauthorized,
    #[error("not found")]
    NotFound,
    #[error("invalid request")]
    Invalid,
    #[error("service unavailable")]
    Unavailable,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self {
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Invalid => StatusCode::BAD_REQUEST,
            Self::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        };
        (
            status,
            Json(serde_json::json!({ "error": self.to_string() })),
        )
            .into_response()
    }
}

#[derive(Serialize)]
struct Health {
    status: &'static str,
    storage: &'static str,
    authentication: &'static str,
    zcash_boundary: &'static str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StoreCredential {
    credential: Value,
}

#[derive(Serialize)]
struct StoredCredential {
    id: Uuid,
    credential: Value,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

struct CredentialRow {
    id: Uuid,
    ciphertext: Vec<u8>,
    data_nonce: Vec<u8>,
    wrapped_dek: Vec<u8>,
    wrap_nonce: Vec<u8>,
    key_version: i32,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

struct EncryptedCredential {
    ciphertext: Vec<u8>,
    data_nonce: Vec<u8>,
    wrapped_dek: Vec<u8>,
    wrap_nonce: Vec<u8>,
    key_version: i32,
}

impl CredentialRow {
    fn from_row(row: Row) -> Self {
        Self {
            id: row.get("id"),
            ciphertext: row.get("ciphertext"),
            data_nonce: row.get("data_nonce"),
            wrapped_dek: row.get("wrapped_dek"),
            wrap_nonce: row.get("wrap_nonce"),
            key_version: row.get("key_version"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ChallengeQuery {
    scopes: Option<String>,
}

#[derive(Serialize)]
struct ZecAuthChallenge {
    domain: String,
    uri: String,
    version: u8,
    chain: String,
    nonce: String,
    issued_at: String,
    expiration_time: String,
    statement: String,
    scopes: ScopeSet,
    message: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScopeSet {
    required: Vec<Scope>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Scope {
    #[serde(rename = "type")]
    scope_type: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VerifyZecAuth {
    pubkey: String,
    signature: String,
    message: String,
    granted: Vec<String>,
}

struct ChallengeRow {
    id: Uuid,
    requested_scopes: Value,
}

#[derive(Serialize)]
struct Authenticated {
    authenticated: bool,
    identity: String,
    scopes: Vec<String>,
}

#[derive(Serialize)]
struct SessionInfo {
    authenticated: bool,
    identity: String,
    scopes: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InspectPaymentRequest {
    uri: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InspectAddress {
    address: String,
}

#[derive(Serialize)]
struct ZcashStatus {
    capabilities: Vec<String>,
    receipt_verification: bool,
    sendmany_advertised: bool,
    sendfromaccount_advertised: bool,
    pczt_complete: bool,
    wallet: String,
    chain_height: u64,
}

impl VaultCipher {
    fn from_env() -> Result<Self, ApiError> {
        let encoded = env::var("ZERANT_VAULT_KEK_B64").map_err(|_| ApiError::Unavailable)?;
        let mut bytes = URL_SAFE_NO_PAD
            .decode(encoded.as_bytes())
            .map_err(|_| ApiError::Unavailable)?;
        if bytes.len() != 32 {
            return Err(ApiError::Unavailable);
        }
        let kek = Aes256Gcm::new_from_slice(&bytes).map_err(|_| ApiError::Unavailable)?;
        bytes.fill(0);
        let key_version = env::var("ZERANT_VAULT_KEY_VERSION")
            .ok()
            .and_then(|value| value.parse::<i32>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(1);
        Ok(Self { kek, key_version })
    }

    fn data_aad(account_id: Uuid, credential_id: Uuid, key_version: i32) -> Vec<u8> {
        format!("zerant:credential:data:v1:{account_id}:{credential_id}:{key_version}").into_bytes()
    }

    fn key_aad(account_id: Uuid, credential_id: Uuid, key_version: i32) -> Vec<u8> {
        format!("zerant:credential:key:v1:{account_id}:{credential_id}:{key_version}").into_bytes()
    }

    fn encrypt(
        &self,
        account_id: Uuid,
        credential_id: Uuid,
        plaintext: &[u8],
    ) -> Result<EncryptedCredential, ApiError> {
        let mut dek_bytes = [0_u8; 32];
        let mut data_nonce = [0_u8; 12];
        let mut wrap_nonce = [0_u8; 12];
        OsRng.fill_bytes(&mut dek_bytes);
        OsRng.fill_bytes(&mut data_nonce);
        OsRng.fill_bytes(&mut wrap_nonce);

        let dek = Aes256Gcm::new_from_slice(&dek_bytes).map_err(|_| ApiError::Unavailable)?;
        let data_aad = Self::data_aad(account_id, credential_id, self.key_version);
        let key_aad = Self::key_aad(account_id, credential_id, self.key_version);

        let ciphertext = dek
            .encrypt(
                (&data_nonce).into(),
                Payload {
                    msg: plaintext,
                    aad: &data_aad,
                },
            )
            .map_err(|_| ApiError::Unavailable)?;

        let wrapped_dek = self
            .kek
            .encrypt(
                (&wrap_nonce).into(),
                Payload {
                    msg: &dek_bytes,
                    aad: &key_aad,
                },
            )
            .map_err(|_| ApiError::Unavailable)?;

        dek_bytes.fill(0);
        Ok(EncryptedCredential {
            ciphertext,
            data_nonce: data_nonce.to_vec(),
            wrapped_dek,
            wrap_nonce: wrap_nonce.to_vec(),
            key_version: self.key_version,
        })
    }

    fn decrypt(&self, account_id: Uuid, row: &CredentialRow) -> Result<Vec<u8>, ApiError> {
        if row.key_version != self.key_version
            || row.data_nonce.len() != 12
            || row.wrap_nonce.len() != 12
        {
            return Err(ApiError::Unavailable);
        }
        let key_aad = Self::key_aad(account_id, row.id, row.key_version);
        let mut dek_bytes = self
            .kek
            .decrypt(
                row.wrap_nonce.as_slice().into(),
                Payload {
                    msg: &row.wrapped_dek,
                    aad: &key_aad,
                },
            )
            .map_err(|_| ApiError::Unavailable)?;

        if dek_bytes.len() != 32 {
            dek_bytes.fill(0);
            return Err(ApiError::Unavailable);
        }

        let dek = Aes256Gcm::new_from_slice(&dek_bytes).map_err(|_| ApiError::Unavailable)?;
        let data_aad = Self::data_aad(account_id, row.id, row.key_version);
        let plaintext = dek
            .decrypt(
                row.data_nonce.as_slice().into(),
                Payload {
                    msg: &row.ciphertext,
                    aad: &data_aad,
                },
            )
            .map_err(|_| ApiError::Unavailable);
        dek_bytes.fill(0);
        plaintext
    }
}

fn cookie_value(headers: &HeaderMap, name: &str) -> Result<String, ApiError> {
    let cookie = headers
        .get(axum::http::header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .ok_or(ApiError::Unauthorized)?;
    cookie
        .split(';')
        .map(str::trim)
        .find_map(|part| part.strip_prefix(&format!("{name}=")))
        .filter(|value| value.len() >= 32 && value.len() <= 512)
        .map(ToOwned::to_owned)
        .ok_or(ApiError::Unauthorized)
}

fn session_token(headers: &HeaderMap) -> Result<String, ApiError> {
    cookie_value(headers, SESSION_COOKIE)
}

async fn db_client(db: &Pool) -> Result<deadpool_postgres::Client, ApiError> {
    db.get().await.map_err(|_| ApiError::Unavailable)
}

async fn account_id(headers: &HeaderMap, db: &Pool) -> Result<Uuid, ApiError> {
    let token = session_token(headers)?;
    let hash = Sha256::digest(token.as_bytes()).to_vec();
    let client = db_client(db).await?;
    client
        .query_opt(
            "SELECT account_id FROM sessions WHERE token_hash = $1 AND expires_at > NOW()",
            &[&hash],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .map(|row| row.get(0))
        .ok_or(ApiError::Unauthorized)
}

async fn health() -> Json<Health> {
    Json(Health {
        status: "ok",
        storage: "postgres",
        authentication: "zecauth",
        zcash_boundary: "z3-zallet",
    })
}

fn scope_alias(value: &str) -> Option<&'static str> {
    match value {
        "signin" | "auth" => Some("auth"),
        "sign-transaction" | "request_payment" => Some("request_payment"),
        "view-address" | "view_address" => Some("view_address"),
        "view-balance" | "view_balance" => Some("view_balance"),
        "view-incoming" | "view_incoming" => Some("view_incoming"),
        "view-history" | "view_history" => Some("view_history"),
        "view-full" | "view_full" => Some("view_full"),
        _ => None,
    }
}

fn requested_scopes(
    query: &ChallengeQuery,
    allowed: &BTreeSet<String>,
) -> Result<Vec<String>, ApiError> {
    let mut scopes = vec!["auth".to_owned()];
    if let Some(raw) = &query.scopes {
        for item in raw
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let scope = scope_alias(item).ok_or(ApiError::Invalid)?;
            if !allowed.contains(scope) {
                return Err(ApiError::Unauthorized);
            }
            if !scopes.iter().any(|value| value == scope) {
                scopes.push(scope.to_owned());
            }
        }
    }
    Ok(scopes)
}

fn domain_from_origin(origin: &str) -> Result<String, ApiError> {
    let url = url::Url::parse(origin).map_err(|_| ApiError::Unavailable)?;
    if url.scheme() != "https" {
        return Err(ApiError::Unavailable);
    }
    url.host_str()
        .map(ToOwned::to_owned)
        .ok_or(ApiError::Unavailable)
}

fn canonical_challenge_message(
    domain: &str,
    uri: &str,
    chain: &str,
    nonce: &str,
    issued_at: &str,
    expiration_time: &str,
    statement: &str,
) -> String {
    format!(
        "{domain} wants you to sign in with your Zcash wallet.\n\nURI: {uri}\nVersion: 1\nChain: {chain}\nNonce: {nonce}\nIssued At: {issued_at}\nExpiration Time: {expiration_time}\nStatement: {statement}"
    )
}

async fn zecauth_challenge(
    State(state): State<AppState>,
    Query(query): Query<ChallengeQuery>,
) -> Result<Response, ApiError> {
    let scopes = requested_scopes(&query, &state.allowed_scopes)?;
    let domain = domain_from_origin(&state.public_origin)?;
    let now = OffsetDateTime::now_utc();
    let expires = now + Duration::minutes(ZECAUTH_TTL_MINUTES);

    let mut nonce_bytes = [0_u8; 24];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = URL_SAFE_NO_PAD.encode(nonce_bytes);
    nonce_bytes.fill(0);

    let issued_at = now.format(&Rfc3339).map_err(|_| ApiError::Unavailable)?;
    let expiration_time = expires
        .format(&Rfc3339)
        .map_err(|_| ApiError::Unavailable)?;
    let uri = format!("{}/app", state.public_origin.trim_end_matches('/'));
    let statement = "Authenticate to Zerant without exposing Zcash spending authority.".to_owned();
    let message = canonical_challenge_message(
        &domain,
        &uri,
        &state.zcash_chain,
        &nonce,
        &issued_at,
        &expiration_time,
        &statement,
    );
    let nonce_hash = Sha256::digest(nonce.as_bytes()).to_vec();
    let mut attempt_bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut attempt_bytes);
    let attempt = URL_SAFE_NO_PAD.encode(attempt_bytes);
    attempt_bytes.fill(0);
    let attempt_hash = Sha256::digest(attempt.as_bytes()).to_vec();
    let scope_set = ScopeSet {
        required: scopes
            .iter()
            .map(|scope| Scope {
                scope_type: scope.clone(),
            })
            .collect(),
    };

    let client = db_client(&state.db).await?;
    client
        .execute(
            "INSERT INTO zecauth_challenges
             (id, nonce_hash, attempt_hash, message, chain, requested_scopes, expires_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
            &[
                &Uuid::new_v4(),
                &nonce_hash,
                &attempt_hash,
                &message,
                &state.zcash_chain,
                &serde_json::to_value(&scope_set).map_err(|_| ApiError::Unavailable)?,
                &expires,
            ],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let cookie = format!(
        "{AUTH_ATTEMPT_COOKIE}={attempt}; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age={}",
        ZECAUTH_TTL_MINUTES * 60
    );
    let mut response = Json(ZecAuthChallenge {
        domain,
        uri,
        version: 1,
        chain: state.zcash_chain.clone(),
        nonce,
        issued_at,
        expiration_time,
        statement,
        scopes: scope_set,
        message,
    })
    .into_response();
    response.headers_mut().insert(
        SET_COOKIE,
        HeaderValue::from_str(&cookie).map_err(|_| ApiError::Unavailable)?,
    );
    Ok(response)
}

fn decode_hex_array<const N: usize>(value: &str) -> Result<[u8; N], ApiError> {
    if value.len() != N * 2 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ApiError::Invalid);
    }
    let bytes = hex::decode(value).map_err(|_| ApiError::Invalid)?;
    bytes.try_into().map_err(|_| ApiError::Invalid)
}

fn verify_redpallas(pubkey: &str, signature: &str, message: &[u8]) -> Result<[u8; 32], ApiError> {
    let key_bytes = decode_hex_array::<32>(pubkey)?;
    let signature_bytes = decode_hex_array::<64>(signature)?;
    let key =
        VerificationKey::<SpendAuth>::try_from(key_bytes).map_err(|_| ApiError::Unauthorized)?;
    let signature: Signature<SpendAuth> = signature_bytes.into();
    key.verify(message, &signature)
        .map_err(|_| ApiError::Unauthorized)?;
    Ok(key_bytes)
}

async fn consume_challenge(tx: &Transaction<'_>, message: &str) -> Result<ChallengeRow, ApiError> {
    tx.query_opt(
        "UPDATE zecauth_challenges
         SET consumed_at = NOW()
         WHERE message = $1 AND consumed_at IS NULL AND expires_at > NOW()
         RETURNING id, requested_scopes",
        &[&message],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?
    .map(|row| ChallengeRow {
        id: row.get(0),
        requested_scopes: row.get(1),
    })
    .ok_or(ApiError::Unauthorized)
}

async fn zecauth_verify(
    State(state): State<AppState>,
    Json(input): Json<VerifyZecAuth>,
) -> Result<Response, ApiError> {
    if input.message.len() > 16 * 1024 || input.granted.len() > 16 {
        return Err(ApiError::Invalid);
    }
    let verification_key =
        verify_redpallas(&input.pubkey, &input.signature, input.message.as_bytes())?;

    let mut client = db_client(&state.db).await?;
    let tx = client
        .transaction()
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let challenge = consume_challenge(&tx, &input.message).await?;
    let requested: ScopeSet =
        serde_json::from_value(challenge.requested_scopes).map_err(|_| ApiError::Unavailable)?;
    let requested: BTreeSet<_> = requested
        .required
        .into_iter()
        .map(|scope| scope.scope_type)
        .collect();

    let granted: BTreeSet<String> = input
        .granted
        .iter()
        .filter_map(|scope| scope_alias(scope).map(ToOwned::to_owned))
        .filter(|scope| requested.contains(scope))
        .collect();
    if !granted.contains("auth") {
        return Err(ApiError::Unauthorized);
    }

    let existing = tx
        .query_opt(
            "SELECT account_id FROM zecauth_identities WHERE verification_key = $1",
            &[&verification_key.as_slice()],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let account = match existing {
        Some(row) => row.get::<_, Uuid>(0),
        None => {
            let account = Uuid::new_v4();
            tx.execute("INSERT INTO accounts(id) VALUES ($1)", &[&account])
                .await
                .map_err(|_| ApiError::Unavailable)?;
            tx.execute(
                "INSERT INTO zecauth_identities(account_id, verification_key) VALUES ($1, $2)",
                &[&account, &verification_key.as_slice()],
            )
            .await
            .map_err(|_| ApiError::Unavailable)?;
            account
        }
    };

    let scopes: Vec<_> = granted.into_iter().collect();
    let scopes_json = serde_json::to_value(&scopes).map_err(|_| ApiError::Unavailable)?;
    tx.execute(
        "UPDATE zecauth_challenges
         SET authenticated_account_id = $1, authenticated_scopes = $2
         WHERE id = $3 AND authenticated_account_id IS NULL",
        &[&account, &scopes_json, &challenge.id],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;

    tx.commit().await.map_err(|_| ApiError::Unavailable)?;
    Ok(Json(Authenticated {
        authenticated: true,
        identity: hex::encode(verification_key),
        scopes,
    })
    .into_response())
}

async fn redeem_zecauth_session(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let attempt = cookie_value(&headers, AUTH_ATTEMPT_COOKIE)?;
    let attempt_hash = Sha256::digest(attempt.as_bytes()).to_vec();
    let mut client = db_client(&state.db).await?;
    let tx = client
        .transaction()
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let row = tx
        .query_opt(
            "UPDATE zecauth_challenges
             SET attempt_hash = NULL
             WHERE attempt_hash = $1
               AND authenticated_account_id IS NOT NULL
               AND authenticated_scopes IS NOT NULL
               AND consumed_at IS NOT NULL
               AND expires_at > NOW()
             RETURNING authenticated_account_id, authenticated_scopes",
            &[&attempt_hash],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::Unauthorized)?;
    let account: Uuid = row.get(0);
    let scopes_json: Value = row.get(1);
    let scopes: Vec<String> =
        serde_json::from_value(scopes_json).map_err(|_| ApiError::Unavailable)?;

    let mut token_bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut token_bytes);
    let token = URL_SAFE_NO_PAD.encode(token_bytes);
    token_bytes.fill(0);
    let token_hash = Sha256::digest(token.as_bytes()).to_vec();
    let expires = OffsetDateTime::now_utc() + Duration::days(SESSION_TTL_DAYS);
    let scopes_json = serde_json::to_value(&scopes).map_err(|_| ApiError::Unavailable)?;

    tx.execute(
        "INSERT INTO sessions(id, account_id, token_hash, scopes, expires_at)
         VALUES ($1, $2, $3, $4, $5)",
        &[
            &Uuid::new_v4(),
            &account,
            &token_hash,
            &scopes_json,
            &expires,
        ],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;
    tx.commit().await.map_err(|_| ApiError::Unavailable)?;

    let session_cookie = format!(
        "{SESSION_COOKIE}={token}; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age={}",
        SESSION_TTL_DAYS * 24 * 60 * 60
    );
    let clear_attempt =
        format!("{AUTH_ATTEMPT_COOKIE}=; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age=0");
    let mut response =
        Json(serde_json::json!({ "authenticated": true, "scopes": scopes })).into_response();
    response.headers_mut().append(
        SET_COOKIE,
        HeaderValue::from_str(&session_cookie).map_err(|_| ApiError::Unavailable)?,
    );
    response.headers_mut().append(
        SET_COOKIE,
        HeaderValue::from_str(&clear_attempt).map_err(|_| ApiError::Unavailable)?,
    );
    Ok(response)
}

async fn list_credentials(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<StoredCredential>>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    let client = db_client(&state.db).await?;
    let rows = client
        .query(
            "SELECT id, ciphertext, data_nonce, wrapped_dek, wrap_nonce, key_version, created_at, updated_at
             FROM credential_envelopes
             WHERE account_id = $1
             ORDER BY created_at DESC
             LIMIT 256",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let mut credentials = Vec::with_capacity(rows.len());
    for row in rows {
        let row = CredentialRow::from_row(row);
        let mut plaintext = state.cipher.decrypt(account, &row)?;
        let credential = serde_json::from_slice(&plaintext).map_err(|_| ApiError::Unavailable)?;
        plaintext.fill(0);
        credentials.push(StoredCredential {
            id: row.id,
            credential,
            created_at: row.created_at,
            updated_at: row.updated_at,
        });
    }
    Ok(Json(credentials))
}

async fn store_credential(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<StoreCredential>,
) -> Result<(StatusCode, Json<StoredCredential>), ApiError> {
    let account = account_id(&headers, &state.db).await?;
    let mut plaintext = serde_json::to_vec(&input.credential).map_err(|_| ApiError::Invalid)?;
    if plaintext.is_empty() || plaintext.len() > MAX_CREDENTIAL_BYTES {
        plaintext.fill(0);
        return Err(ApiError::Invalid);
    }

    let id = Uuid::new_v4();
    let encrypted = state.cipher.encrypt(account, id, &plaintext)?;
    plaintext.fill(0);

    let client = db_client(&state.db).await?;
    let row = client
        .query_one(
            "INSERT INTO credential_envelopes
             (id, account_id, ciphertext, data_nonce, wrapped_dek, wrap_nonce, key_version)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             RETURNING id, ciphertext, data_nonce, wrapped_dek, wrap_nonce, key_version, created_at, updated_at",
            &[
                &id,
                &account,
                &encrypted.ciphertext,
                &encrypted.data_nonce,
                &encrypted.wrapped_dek,
                &encrypted.wrap_nonce,
                &encrypted.key_version,
            ],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let row = CredentialRow::from_row(row);

    let mut restored = state.cipher.decrypt(account, &row)?;
    let credential = serde_json::from_slice(&restored).map_err(|_| ApiError::Unavailable)?;
    restored.fill(0);

    Ok((
        StatusCode::CREATED,
        Json(StoredCredential {
            id: row.id,
            credential,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }),
    ))
}

async fn delete_credential(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    let client = db_client(&state.db).await?;
    let affected = client
        .execute(
            "DELETE FROM credential_envelopes WHERE id = $1 AND account_id = $2",
            &[&id, &account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    if affected != 1 {
        return Err(ApiError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn session_info(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<SessionInfo>, ApiError> {
    let token = session_token(&headers)?;
    let hash = Sha256::digest(token.as_bytes()).to_vec();
    let client = db_client(&state.db).await?;
    let row = client
        .query_opt(
            "SELECT encode(i.verification_key, 'hex'), s.scopes
             FROM sessions s
             JOIN zecauth_identities i ON i.account_id = s.account_id
             WHERE s.token_hash = $1 AND s.expires_at > NOW()",
            &[&hash],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::Unauthorized)?;
    let identity: String = row.get(0);
    let scopes_value: Value = row.get(1);
    let scopes: Vec<String> =
        serde_json::from_value(scopes_value).map_err(|_| ApiError::Unavailable)?;
    Ok(Json(SessionInfo {
        authenticated: true,
        identity,
        scopes,
    }))
}

async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Result<Response, ApiError> {
    let token = session_token(&headers)?;
    let hash = Sha256::digest(token.as_bytes()).to_vec();
    let client = db_client(&state.db).await?;
    client
        .execute("DELETE FROM sessions WHERE token_hash = $1", &[&hash])
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let clear = format!("{SESSION_COOKIE}=; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age=0");
    let mut response = StatusCode::NO_CONTENT.into_response();
    response.headers_mut().insert(
        SET_COOKIE,
        HeaderValue::from_str(&clear).map_err(|_| ApiError::Unavailable)?,
    );
    Ok(response)
}

fn z3_adapter() -> Result<Adapter<HttpRegtestTransport>, ApiError> {
    let user = env::var("Z3_REGTEST_RPC_ROUTER_USER").unwrap_or_else(|_| "zebra".into());
    let password = env::var("Z3_REGTEST_RPC_ROUTER_PASSWORD").map_err(|_| ApiError::Unavailable)?;
    HttpRegtestTransport::new(user, password)
        .map(Adapter)
        .map_err(|_| ApiError::Unavailable)
}

async fn inspect_zcash_address(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<InspectAddress>,
) -> Result<Json<zerant_zcash::address::AddressSummary>, ApiError> {
    let _ = account_id(&headers, &state.db).await?;
    let summary =
        zerant_zcash::address::inspect_address(&input.address).map_err(|_| ApiError::Invalid)?;
    Ok(Json(summary))
}

async fn inspect_zcash_payment_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<InspectPaymentRequest>,
) -> Result<Json<zerant_zcash::zip321::PaymentRequestSummary>, ApiError> {
    let _ = account_id(&headers, &state.db).await?;
    let summary =
        zerant_zcash::zip321::inspect_payment_request(&input.uri).map_err(|_| ApiError::Invalid)?;
    Ok(Json(summary))
}

async fn zcash_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ZcashStatus>, ApiError> {
    let _ = account_id(&headers, &state.db).await?;
    let result = tokio::task::spawn_blocking(|| {
        let adapter = z3_adapter()?;
        let capabilities = adapter.capabilities().map_err(|_| ApiError::Unavailable)?;
        let chain = adapter.chain_status().map_err(|_| ApiError::Unavailable)?;
        let wallet = match adapter
            .wallet_readiness()
            .map_err(|_| ApiError::Unavailable)?
        {
            zerant_zcash::WalletReadiness::RpcReachable => "rpc_reachable",
            zerant_zcash::WalletReadiness::KeyStorePresent => "keystore_present",
        };
        Ok::<_, ApiError>(ZcashStatus {
            capabilities: capabilities.methods,
            receipt_verification: capabilities.receipt_verification,
            sendmany_advertised: capabilities.sendmany_advertised,
            sendfromaccount_advertised: capabilities.sendfromaccount_advertised,
            pczt_complete: capabilities.pczt_complete,
            wallet: wallet.into(),
            chain_height: chain.blocks,
        })
    })
    .await
    .map_err(|_| ApiError::Unavailable)??;
    Ok(Json(result))
}

fn app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/auth/zecauth/challenge", get(zecauth_challenge))
        .route("/v1/auth/zecauth/verify", post(zecauth_verify))
        .route("/v1/auth/zecauth/session", get(redeem_zecauth_session))
        .route("/v1/session", get(session_info).delete(logout))
        .route(
            "/v1/credentials",
            get(list_credentials).post(store_credential),
        )
        .route("/v1/credentials/{id}", delete(delete_credential))
        .route("/v1/zcash/status", get(zcash_status))
        .route("/v1/zcash/address/inspect", post(inspect_zcash_address))
        .route(
            "/v1/zcash/payment-request/inspect",
            post(inspect_zcash_payment_request),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

fn env_required(name: &str) -> Result<String, ApiError> {
    env::var(name).map_err(|_| ApiError::Unavailable)
}

async fn run_migrations(pool: &Pool) -> Result<(), ApiError> {
    let client = db_client(pool).await?;
    client
        .batch_execute(include_str!("../migrations/0001_server_vault.sql"))
        .await
        .map_err(|_| ApiError::Unavailable)?;
    client
        .batch_execute(include_str!("../migrations/0002_zecauth.sql"))
        .await
        .map_err(|_| ApiError::Unavailable)?;
    client
        .batch_execute(include_str!(
            "../migrations/0003_zecauth_browser_redeem.sql"
        ))
        .await
        .map_err(|_| ApiError::Unavailable)?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "zerant_api=info,tower_http=info".into()),
        )
        .init();

    let database_url = env_required("DATABASE_URL").map_err(|error| error.to_string())?;
    let public_origin = env_required("ZERANT_PUBLIC_ORIGIN").map_err(|error| error.to_string())?;
    let zcash_chain = env::var("ZERANT_ZCASH_CHAIN").unwrap_or_else(|_| "zcash:testnet".into());
    if !matches!(zcash_chain.as_str(), "zcash:testnet" | "zcash:mainnet") {
        return Err(ApiError::Unavailable.to_string().into());
    }

    let allowed_scopes: BTreeSet<String> = env::var("ZERANT_ZECAUTH_SCOPES")
        .unwrap_or_else(|_| "auth,request_payment".into())
        .split(',')
        .filter_map(|value| scope_alias(value.trim()).map(ToOwned::to_owned))
        .collect();
    if !allowed_scopes.contains("auth") {
        return Err(ApiError::Unavailable.to_string().into());
    }

    let mut pg_config = tokio_postgres::Config::from_str(&database_url)?;
    let database_tls = env::var("ZERANT_DATABASE_TLS").unwrap_or_else(|_| "require".into());
    let manager = match database_tls.as_str() {
        "disable" => {
            pg_config.ssl_mode(SslMode::Disable);
            Manager::from_config(
                pg_config,
                NoTls,
                ManagerConfig {
                    recycling_method: RecyclingMethod::Fast,
                },
            )
        }
        "require" => {
            pg_config.ssl_mode(SslMode::Require);
            let connector = TlsConnector::builder().build()?;
            Manager::from_config(
                pg_config,
                MakeTlsConnector::new(connector),
                ManagerConfig {
                    recycling_method: RecyclingMethod::Fast,
                },
            )
        }
        _ => return Err(ApiError::Unavailable.to_string().into()),
    };
    let db = Pool::builder(manager).max_size(20).build()?;
    run_migrations(&db)
        .await
        .map_err(|error| error.to_string())?;

    let state = AppState {
        db,
        cipher: Arc::new(VaultCipher::from_env().map_err(|error| error.to_string())?),
        public_origin,
        zcash_chain,
        allowed_scopes,
    };

    let port = env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(8080);
    let bind_ip: IpAddr = env::var("ZERANT_BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1".into())
        .parse()?;
    let address = SocketAddr::new(bind_ip, port);
    let listener = tokio::net::TcpListener::bind(address).await?;
    info!(%address, "zerant api listening");
    axum::serve(listener, app(state)).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zecauth_scope_aliases_are_explicit() {
        assert_eq!(scope_alias("signin"), Some("auth"));
        assert_eq!(scope_alias("sign-transaction"), Some("request_payment"));
        assert_eq!(scope_alias("view-full"), Some("view_full"));
        assert_eq!(scope_alias("unknown"), None);
    }

    #[test]
    fn challenge_message_is_domain_chain_nonce_and_expiry_bound() {
        let message = canonical_challenge_message(
            "zerant.example",
            "https://zerant.example/app",
            "zcash:testnet",
            "nonce1234567890123456",
            "2026-10-04T08:00:00Z",
            "2026-10-04T08:05:00Z",
            "Authenticate to Zerant.",
        );
        for expected in [
            "zerant.example wants you to sign in with your Zcash wallet.",
            "URI: https://zerant.example/app",
            "Version: 1",
            "Chain: zcash:testnet",
            "Nonce: nonce1234567890123456",
            "Expiration Time: 2026-10-04T08:05:00Z",
        ] {
            assert!(message.contains(expected));
        }
    }

    #[test]
    fn malformed_redpallas_inputs_fail_before_verification() {
        assert!(verify_redpallas("00", &"00".repeat(64), b"message").is_err());
        assert!(verify_redpallas(&"00".repeat(32), "00", b"message").is_err());
    }

    #[test]
    fn envelope_encryption_roundtrips_and_tampering_fails() {
        let cipher = VaultCipher {
            kek: Aes256Gcm::new_from_slice(&[7_u8; 32]).unwrap(),
            key_version: 1,
        };
        let account = Uuid::from_u128(1);
        let id = Uuid::from_u128(2);
        let plaintext = br#"{\"claim\":\"membership.active\",\"value\":true}"#;

        let encrypted = cipher.encrypt(account, id, plaintext).unwrap();
        assert_ne!(encrypted.ciphertext, plaintext);
        assert_eq!(encrypted.data_nonce.len(), 12);
        assert_eq!(encrypted.wrap_nonce.len(), 12);
        assert_eq!(encrypted.wrapped_dek.len(), 48);

        let row = CredentialRow {
            id,
            ciphertext: encrypted.ciphertext.clone(),
            data_nonce: encrypted.data_nonce.clone(),
            wrapped_dek: encrypted.wrapped_dek.clone(),
            wrap_nonce: encrypted.wrap_nonce.clone(),
            key_version: encrypted.key_version,
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        };
        assert_eq!(cipher.decrypt(account, &row).unwrap(), plaintext);

        let mut tampered = row;
        tampered.ciphertext[0] ^= 1;
        assert!(cipher.decrypt(account, &tampered).is_err());
    }

    #[test]
    fn envelope_encryption_uses_fresh_keys_and_nonces() {
        let cipher = VaultCipher {
            kek: Aes256Gcm::new_from_slice(&[9_u8; 32]).unwrap(),
            key_version: 1,
        };
        let account = Uuid::from_u128(3);
        let first = cipher
            .encrypt(account, Uuid::from_u128(4), b"same")
            .unwrap();
        let second = cipher
            .encrypt(account, Uuid::from_u128(5), b"same")
            .unwrap();

        assert_ne!(first.data_nonce, second.data_nonce);
        assert_ne!(first.wrap_nonce, second.wrap_nonce);
        assert_ne!(first.wrapped_dek, second.wrapped_dek);
        assert_ne!(first.ciphertext, second.ciphertext);
    }
}
