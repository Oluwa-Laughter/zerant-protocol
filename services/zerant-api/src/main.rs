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
use josekit::jwk::{Jwk, alg::ed::EdCurve};
use native_tls::TlsConnector;
use postgres_native_tls::MakeTlsConnector;
use rand::rngs::OsRng;
use reddsa::{Signature, VerificationKey, orchard::SpendAuth};
use secp256k1::{
    Message as SecpMessage, PublicKey as SecpPublicKey, Secp256k1,
    ecdsa::{RecoverableSignature, RecoveryId},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
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
use zerant_core::validate_origin;
use zerant_credential::{
    CREDENTIAL_SCHEMA, Claim, ClaimValue, CredentialKind, CredentialPayload, HOLDER_LOCAL_AUDIENCE,
    IssuerTrustManifest, OrdinaryClaim, PublicJwk, REVOCATION_SCHEMA, RevocationSnapshot,
    SourceEventClaim, SourceSchemaAuthorization, TrustedIssuer, TrustedIssuerKey,
    revocation_digest, sign_credential, sign_revocation_snapshot, verify_credential,
};
use zerant_disclosure::{
    Context as DisclosureContext, Decision, Evidence, REQUEST_SCHEMA, Request, VerifierPin,
    respond, sign_request,
};
use zerant_zcash::{Adapter, HttpRegtestTransport};

const SESSION_COOKIE: &str = "zerant_session";
const AUTH_ATTEMPT_COOKIE: &str = "zerant_auth_attempt";
const MAX_CREDENTIAL_BYTES: usize = 256 * 1024;
const MAX_ACCOUNT_CREDENTIALS: i64 = 256;
const MAX_ACCOUNT_EXPORT_EVENTS: i64 = 5_000;
const SESSION_TTL_DAYS: i64 = 7;
const ZECAUTH_TTL_MINUTES: i64 = 5;
const WRITE_RATE_WINDOW_SECONDS: i64 = 60;
const MAX_ACTIVE_ZECAUTH_CHALLENGES: i64 = 10_000;

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
    keks: BTreeMap<i32, Aes256Gcm>,
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
    #[error("conflict")]
    Conflict,
    #[error("too many requests")]
    TooManyRequests,
    #[error("service unavailable")]
    Unavailable,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self {
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Invalid => StatusCode::BAD_REQUEST,
            Self::Conflict => StatusCode::CONFLICT,
            Self::TooManyRequests => StatusCode::TOO_MANY_REQUESTS,
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
    revoked: bool,
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

#[derive(Serialize)]
struct AccountSummary {
    zerant_id: String,
    credential_count: i64,
    issuer_profile: Option<String>,
    verifier_profile: Option<String>,
    can_delete: bool,
}

#[derive(Serialize)]
struct AccountExport {
    export_version: u8,
    generated_at: OffsetDateTime,
    zerant_id: String,
    credentials: Vec<StoredCredential>,
    activity: Vec<ActivityEventView>,
    activity_complete: bool,
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VerifyWalletMessage {
    pubkey: String,
    signature: String,
    message: String,
    granted: Vec<String>,
    signing_mode: String,
}

struct ChallengeRow {
    id: Uuid,
    requested_scopes: Value,
    chain: String,
}

#[derive(Serialize)]
struct Authenticated {
    authenticated: bool,
    identity: String,
    zerant_id: String,
    scopes: Vec<String>,
}

#[derive(Serialize)]
struct SessionInfo {
    authenticated: bool,
    identity: String,
    zerant_id: String,
    scopes: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegisterIssuer {
    display_name: String,
}

#[derive(Serialize)]
struct IssuerProfileView {
    display_name: String,
    issuer_id: String,
    created_at: OffsetDateTime,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RotateIssuerKey {
    compromise_current: bool,
}

#[derive(Serialize)]
struct IssuerKeyView {
    active: bool,
    compromised: bool,
    valid_from: OffsetDateTime,
    retired_at: Option<OffsetDateTime>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IssueCredential {
    holder_zerant_id: String,
    credential_schema_id: Option<Uuid>,
    claim_type: Option<String>,
    value: String,
    context: Option<String>,
    occurred_at: Option<u64>,
    expires_in_days: Option<u16>,
}

#[derive(Serialize)]
struct IssuedCredentialView {
    credential_id: String,
    holder_zerant_id: String,
    credential_schema_id: Option<Uuid>,
    claim_type: String,
    context: String,
    issued_at: OffsetDateTime,
    expires_at: OffsetDateTime,
    revoked: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateCredentialSchema {
    display_name: String,
    description: String,
    claim_type: Option<String>,
    context: String,
    default_expiry_days: u16,
}

#[derive(Serialize)]
struct CredentialSchemaView {
    id: Uuid,
    issuer_id: String,
    issuer_name: String,
    display_name: String,
    description: String,
    claim_type: String,
    context: String,
    default_expiry_days: i32,
    active: bool,
    created_at: OffsetDateTime,
}

struct ResolvedCredentialSchema {
    id: Uuid,
    issuer_id: String,
    display_name: String,
    claim_type: String,
    context: String,
    default_expiry_days: u16,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegisterVerifier {
    display_name: String,
    origin: String,
}

#[derive(Serialize)]
struct VerifierProfileView {
    display_name: String,
    origin: String,
    created_at: OffsetDateTime,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RotateVerifierKey {
    compromise_current: bool,
}

#[derive(Serialize)]
struct VerifierKeyView {
    active: bool,
    compromised: bool,
    valid_from: OffsetDateTime,
    retired_at: Option<OffsetDateTime>,
}

#[derive(Serialize)]
struct IssuerDirectoryEntry {
    display_name: String,
    issuer_id: String,
    schemas: Vec<CredentialSchemaView>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateVerificationRequest {
    holder_zerant_id: String,
    purpose: String,
    credential_schema_id: Option<Uuid>,
    claim_type: Option<String>,
    context: Option<String>,
    accepted_issuer_ids: Vec<String>,
}

#[derive(Serialize)]
struct HolderRequestView {
    id: Uuid,
    verifier_name: String,
    verifier_origin: String,
    purpose: String,
    credential_name: Option<String>,
    claim_type: String,
    context: String,
    created_at: OffsetDateTime,
    expires_at: OffsetDateTime,
}

#[derive(Serialize)]
struct VerifierRequestView {
    id: Uuid,
    holder_zerant_id: String,
    purpose: String,
    credential_schema_id: Option<Uuid>,
    credential_name: Option<String>,
    claim_type: String,
    context: String,
    status: String,
    verified: bool,
    created_at: OffsetDateTime,
    expires_at: OffsetDateTime,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DecideRequest {
    decision: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ActivityQuery {
    limit: Option<u16>,
    cursor: Option<String>,
}

#[derive(Clone, Serialize)]
struct ActivityEventView {
    id: i64,
    event_type: String,
    object_id: String,
    label: String,
    context: Option<String>,
    counterparty: Option<String>,
    created_at: OffsetDateTime,
}

#[derive(Serialize)]
struct ActivityPage {
    items: Vec<ActivityEventView>,
    next_cursor: Option<String>,
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
    fn decode_kek(encoded: &str) -> Result<Aes256Gcm, ApiError> {
        let mut bytes = URL_SAFE_NO_PAD
            .decode(encoded.as_bytes())
            .map_err(|_| ApiError::Unavailable)?;
        if bytes.len() != 32 {
            bytes.fill(0);
            return Err(ApiError::Unavailable);
        }
        let kek = Aes256Gcm::new_from_slice(&bytes).map_err(|_| ApiError::Unavailable)?;
        bytes.fill(0);
        Ok(kek)
    }

    fn from_env() -> Result<Self, ApiError> {
        let key_version = env::var("ZERANT_VAULT_KEY_VERSION")
            .ok()
            .and_then(|value| value.parse::<i32>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(1);

        let mut keks = BTreeMap::new();

        if let Ok(raw) = env::var("ZERANT_VAULT_KEYS_B64") {
            let encoded_keys: BTreeMap<String, String> =
                serde_json::from_str(&raw).map_err(|_| ApiError::Unavailable)?;
            if encoded_keys.is_empty() || encoded_keys.len() > 16 {
                return Err(ApiError::Unavailable);
            }
            for (version, encoded) in encoded_keys {
                let version = version
                    .parse::<i32>()
                    .ok()
                    .filter(|value| *value > 0)
                    .ok_or(ApiError::Unavailable)?;
                if keks.insert(version, Self::decode_kek(&encoded)?).is_some() {
                    return Err(ApiError::Unavailable);
                }
            }
        }

        if let Ok(encoded) = env::var("ZERANT_VAULT_KEK_B64")
            && let std::collections::btree_map::Entry::Vacant(entry) = keks.entry(key_version)
        {
            entry.insert(Self::decode_kek(&encoded)?);
        }

        if !keks.contains_key(&key_version) {
            return Err(ApiError::Unavailable);
        }

        Ok(Self { keks, key_version })
    }

    fn key_for_version(&self, version: i32) -> Result<&Aes256Gcm, ApiError> {
        self.keks.get(&version).ok_or(ApiError::Unavailable)
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
        let kek = self.key_for_version(self.key_version)?;

        let ciphertext = dek
            .encrypt(
                (&data_nonce).into(),
                Payload {
                    msg: plaintext,
                    aad: &data_aad,
                },
            )
            .map_err(|_| ApiError::Unavailable)?;

        let wrapped_dek = kek
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
        if row.data_nonce.len() != 12 || row.wrap_nonce.len() != 12 {
            return Err(ApiError::Unavailable);
        }

        let kek = self.key_for_version(row.key_version)?;
        let key_aad = Self::key_aad(account_id, row.id, row.key_version);
        let mut dek_bytes = kek
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

async fn enforce_account_rate_limit(
    db: &Pool,
    account: Uuid,
    action: &str,
    max_requests: i32,
) -> Result<(), ApiError> {
    if max_requests <= 0 {
        return Err(ApiError::Unavailable);
    }
    let cap = max_requests.checked_add(1).ok_or(ApiError::Unavailable)?;
    let client = db_client(db).await?;
    let row = client
        .query_one(
            "INSERT INTO account_rate_limits(account_id, action, window_started_at, request_count)
             VALUES ($1, $2, NOW(), 1)
             ON CONFLICT (account_id, action)
             DO UPDATE SET
                 window_started_at = CASE
                     WHEN account_rate_limits.window_started_at <=
                          NOW() - ($3::bigint * INTERVAL '1 second')
                     THEN NOW()
                     ELSE account_rate_limits.window_started_at
                 END,
                 request_count = CASE
                     WHEN account_rate_limits.window_started_at <=
                          NOW() - ($3::bigint * INTERVAL '1 second')
                     THEN 1
                     ELSE LEAST(account_rate_limits.request_count + 1, $4)
                 END
             RETURNING request_count",
            &[&account, &action, &WRITE_RATE_WINDOW_SECONDS, &cap],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let count: i32 = row.get(0);
    if count > max_requests {
        return Err(ApiError::TooManyRequests);
    }
    Ok(())
}

fn zerant_public_handle(verification_key: &[u8]) -> String {
    let digest = Sha256::digest(verification_key);
    format!("zr_{}", &hex::encode(digest)[..24])
}

fn random_id() -> String {
    let mut bytes = [0_u8; 16];
    OsRng.fill_bytes(&mut bytes);
    let encoded = URL_SAFE_NO_PAD.encode(bytes);
    bytes.fill(0);
    encoded
}

fn random_challenge() -> String {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    let encoded = URL_SAFE_NO_PAD.encode(bytes);
    bytes.fill(0);
    encoded
}

fn bounded_proof_expiry(
    now: u64,
    request_expiry: u64,
    source_expiry: u64,
) -> Result<u64, ApiError> {
    if request_expiry <= now || source_expiry <= now {
        return Err(ApiError::Conflict);
    }
    let expiry = now
        .checked_add(300)
        .ok_or(ApiError::Unavailable)?
        .min(request_expiry)
        .min(source_expiry);
    if expiry <= now {
        return Err(ApiError::Conflict);
    }
    Ok(expiry)
}

fn public_jwk(private: &Jwk) -> Result<PublicJwk, ApiError> {
    let public = private.to_public_key().map_err(|_| ApiError::Unavailable)?;
    let value = serde_json::to_value(public).map_err(|_| ApiError::Unavailable)?;
    let result = PublicJwk {
        kty: value
            .get("kty")
            .and_then(Value::as_str)
            .ok_or(ApiError::Unavailable)?
            .to_owned(),
        crv: value
            .get("crv")
            .and_then(Value::as_str)
            .ok_or(ApiError::Unavailable)?
            .to_owned(),
        x: value
            .get("x")
            .and_then(Value::as_str)
            .ok_or(ApiError::Unavailable)?
            .to_owned(),
    };
    result.validate().map_err(|_| ApiError::Unavailable)?;
    Ok(result)
}

async fn ensure_account_credential_key(
    state: &AppState,
    account: Uuid,
) -> Result<PublicJwk, ApiError> {
    let client = db_client(&state.db).await?;
    if let Some(row) = client
        .query_opt(
            "SELECT public_jwk FROM account_credential_keys WHERE account_id = $1",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
    {
        let value: Value = row.get(0);
        return serde_json::from_value(value).map_err(|_| ApiError::Unavailable);
    }

    let private = Jwk::generate_ed_key(EdCurve::Ed25519).map_err(|_| ApiError::Unavailable)?;
    let public = public_jwk(&private)?;
    let mut plaintext = serde_json::to_vec(&private).map_err(|_| ApiError::Unavailable)?;
    let encrypted = state.cipher.encrypt(account, account, &plaintext)?;
    plaintext.fill(0);
    let public_value = serde_json::to_value(&public).map_err(|_| ApiError::Unavailable)?;

    let inserted = client
        .execute(
            "INSERT INTO account_credential_keys
             (account_id, public_jwk, ciphertext, data_nonce, wrapped_dek, wrap_nonce, key_version)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (account_id) DO NOTHING",
            &[
                &account,
                &public_value,
                &encrypted.ciphertext,
                &encrypted.data_nonce,
                &encrypted.wrapped_dek,
                &encrypted.wrap_nonce,
                &encrypted.key_version,
            ],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    if inserted == 1 {
        Ok(public)
    } else {
        let row = client
            .query_one(
                "SELECT public_jwk FROM account_credential_keys WHERE account_id = $1",
                &[&account],
            )
            .await
            .map_err(|_| ApiError::Unavailable)?;
        let value: Value = row.get(0);
        serde_json::from_value(value).map_err(|_| ApiError::Unavailable)
    }
}

fn generate_pairwise_holder_key() -> Result<(Jwk, PublicJwk), ApiError> {
    let private = Jwk::generate_ed_key(EdCurve::Ed25519).map_err(|_| ApiError::Unavailable)?;
    let public = public_jwk(&private)?;
    Ok((private, public))
}

async fn load_or_create_pairwise_holder_key(
    state: &AppState,
    holder_account: Uuid,
    verifier_profile_id: Uuid,
) -> Result<(Jwk, PublicJwk), ApiError> {
    let client = db_client(&state.db).await?;

    if let Some(row) = client
        .query_opt(
            "SELECT id, public_jwk, ciphertext, data_nonce, wrapped_dek, wrap_nonce, key_version
             FROM holder_pairwise_keys
             WHERE holder_account_id = $1 AND verifier_profile_id = $2",
            &[&holder_account, &verifier_profile_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
    {
        let id: Uuid = row.get(0);
        let public_value: Value = row.get(1);
        let public: PublicJwk =
            serde_json::from_value(public_value).map_err(|_| ApiError::Unavailable)?;
        public.validate().map_err(|_| ApiError::Unavailable)?;

        let now = OffsetDateTime::now_utc();
        let secret = CredentialRow {
            id,
            ciphertext: row.get(2),
            data_nonce: row.get(3),
            wrapped_dek: row.get(4),
            wrap_nonce: row.get(5),
            key_version: row.get(6),
            created_at: now,
            updated_at: now,
        };
        let mut plaintext = state.cipher.decrypt(holder_account, &secret)?;
        let private: Jwk = serde_json::from_slice(&plaintext).map_err(|_| ApiError::Unavailable)?;
        plaintext.fill(0);
        return Ok((private, public));
    }

    let id = Uuid::new_v4();
    let (private, public) = generate_pairwise_holder_key()?;
    let public_value = serde_json::to_value(&public).map_err(|_| ApiError::Unavailable)?;
    let mut plaintext = serde_json::to_vec(&private).map_err(|_| ApiError::Unavailable)?;
    let encrypted = state.cipher.encrypt(holder_account, id, &plaintext)?;
    plaintext.fill(0);

    let inserted = client
        .execute(
            "INSERT INTO holder_pairwise_keys
             (id, holder_account_id, verifier_profile_id, public_jwk, ciphertext,
              data_nonce, wrapped_dek, wrap_nonce, key_version)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
             ON CONFLICT (holder_account_id, verifier_profile_id) DO NOTHING",
            &[
                &id,
                &holder_account,
                &verifier_profile_id,
                &public_value,
                &encrypted.ciphertext,
                &encrypted.data_nonce,
                &encrypted.wrapped_dek,
                &encrypted.wrap_nonce,
                &encrypted.key_version,
            ],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    if inserted == 1 {
        return Ok((private, public));
    }

    let row = client
        .query_one(
            "SELECT id, public_jwk, ciphertext, data_nonce, wrapped_dek, wrap_nonce, key_version
             FROM holder_pairwise_keys
             WHERE holder_account_id = $1 AND verifier_profile_id = $2",
            &[&holder_account, &verifier_profile_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let existing_id: Uuid = row.get(0);
    let public_value: Value = row.get(1);
    let existing_public: PublicJwk =
        serde_json::from_value(public_value).map_err(|_| ApiError::Unavailable)?;
    existing_public
        .validate()
        .map_err(|_| ApiError::Unavailable)?;

    let now = OffsetDateTime::now_utc();
    let secret = CredentialRow {
        id: existing_id,
        ciphertext: row.get(2),
        data_nonce: row.get(3),
        wrapped_dek: row.get(4),
        wrap_nonce: row.get(5),
        key_version: row.get(6),
        created_at: now,
        updated_at: now,
    };
    let mut bytes = state.cipher.decrypt(holder_account, &secret)?;
    let existing_private: Jwk =
        serde_json::from_slice(&bytes).map_err(|_| ApiError::Unavailable)?;
    bytes.fill(0);
    Ok((existing_private, existing_public))
}

async fn health() -> Json<Health> {
    Json(Health {
        status: "ok",
        storage: "postgres",
        authentication: "zecauth",
        zcash_boundary: "z3-zallet",
    })
}

fn activity_cursor(created_at: OffsetDateTime, id: i64) -> String {
    URL_SAFE_NO_PAD.encode(format!("{}:{id}", created_at.unix_timestamp_nanos()))
}

fn parse_activity_cursor(value: &str) -> Result<(OffsetDateTime, i64), ApiError> {
    if value.len() > 256 {
        return Err(ApiError::Invalid);
    }
    let decoded = URL_SAFE_NO_PAD
        .decode(value.as_bytes())
        .map_err(|_| ApiError::Invalid)?;
    let text = std::str::from_utf8(&decoded).map_err(|_| ApiError::Invalid)?;
    let (nanos, id) = text.split_once(':').ok_or(ApiError::Invalid)?;
    let nanos = nanos.parse::<i128>().map_err(|_| ApiError::Invalid)?;
    let id = id.parse::<i64>().map_err(|_| ApiError::Invalid)?;
    if id <= 0 {
        return Err(ApiError::Invalid);
    }
    let created_at =
        OffsetDateTime::from_unix_timestamp_nanos(nanos).map_err(|_| ApiError::Invalid)?;
    Ok((created_at, id))
}

async fn list_activity(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ActivityQuery>,
) -> Result<Json<ActivityPage>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    let limit = usize::from(query.limit.unwrap_or(20).clamp(1, 100));
    let fetch_limit = i64::try_from(limit + 1).map_err(|_| ApiError::Invalid)?;
    let client = db_client(&state.db).await?;

    let rows = if let Some(cursor) = query.cursor.as_deref() {
        let (created_at, id) = parse_activity_cursor(cursor)?;
        client
            .query(
                "SELECT id, event_type, object_id, label, context, counterparty, created_at
                 FROM trust_events
                 WHERE account_id = $1
                   AND (created_at, id) < ($2, $3)
                 ORDER BY created_at DESC, id DESC
                 LIMIT $4",
                &[&account, &created_at, &id, &fetch_limit],
            )
            .await
            .map_err(|_| ApiError::Unavailable)?
    } else {
        client
            .query(
                "SELECT id, event_type, object_id, label, context, counterparty, created_at
                 FROM trust_events
                 WHERE account_id = $1
                 ORDER BY created_at DESC, id DESC
                 LIMIT $2",
                &[&account, &fetch_limit],
            )
            .await
            .map_err(|_| ApiError::Unavailable)?
    };

    let has_more = rows.len() > limit;
    let mut items = Vec::with_capacity(limit.min(rows.len()));
    for row in rows.into_iter().take(limit) {
        items.push(ActivityEventView {
            id: row.get(0),
            event_type: row.get(1),
            object_id: row.get(2),
            label: row.get(3),
            context: row.get(4),
            counterparty: row.get(5),
            created_at: row.get(6),
        });
    }

    let next_cursor = if has_more {
        items
            .last()
            .map(|item| activity_cursor(item.created_at, item.id))
    } else {
        None
    };

    Ok(Json(ActivityPage { items, next_cursor }))
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
            "DELETE FROM zecauth_challenges WHERE expires_at <= NOW() - INTERVAL '1 day'",
            &[],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let active: i64 = client
        .query_one(
            "SELECT COUNT(*) FROM zecauth_challenges
             WHERE consumed_at IS NULL AND expires_at > NOW()",
            &[],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .get(0);
    if active >= MAX_ACTIVE_ZECAUTH_CHALLENGES {
        return Err(ApiError::TooManyRequests);
    }

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

fn encode_compact_size(size: usize, out: &mut Vec<u8>) -> Result<(), ApiError> {
    if size <= 0xfc {
        out.push(size as u8);
    } else if size <= 0xffff {
        out.push(0xfd);
        out.extend_from_slice(&(size as u16).to_le_bytes());
    } else if size <= u32::MAX as usize {
        out.push(0xfe);
        out.extend_from_slice(&(size as u32).to_le_bytes());
    } else {
        return Err(ApiError::Invalid);
    }
    Ok(())
}

fn zcash_signed_message_hash(message: &str) -> Result<[u8; 32], ApiError> {
    if !message.is_ascii() || message.len() > 16 * 1024 {
        return Err(ApiError::Invalid);
    }
    const PREFIX: &[u8] = b"Zcash Signed Message:\n";
    let mut payload = Vec::with_capacity(PREFIX.len() + message.len() + 10);
    encode_compact_size(PREFIX.len(), &mut payload)?;
    payload.extend_from_slice(PREFIX);
    encode_compact_size(message.len(), &mut payload)?;
    payload.extend_from_slice(message.as_bytes());
    let first = Sha256::digest(&payload);
    let second = Sha256::digest(first);
    let mut digest = [0_u8; 32];
    digest.copy_from_slice(&second);
    Ok(digest)
}

fn verify_zcash_wallet_message(
    pubkey: &str,
    signature: &str,
    message: &str,
) -> Result<[u8; 33], ApiError> {
    let clean_pubkey = pubkey.strip_prefix("0x").unwrap_or(pubkey);
    let pubkey_bytes = hex::decode(clean_pubkey).map_err(|_| ApiError::Invalid)?;
    if pubkey_bytes.len() != 33 && pubkey_bytes.len() != 65 {
        return Err(ApiError::Invalid);
    }
    let expected = SecpPublicKey::from_slice(&pubkey_bytes).map_err(|_| ApiError::Unauthorized)?;

    let clean_signature = signature.strip_prefix("0x").unwrap_or(signature);
    let signature_bytes = hex::decode(clean_signature).map_err(|_| ApiError::Invalid)?;
    if signature_bytes.len() != 65 {
        return Err(ApiError::Invalid);
    }
    let header = signature_bytes[0];
    if !(27..=34).contains(&header) {
        return Err(ApiError::Invalid);
    }
    let recovery = if header >= 31 {
        header - 31
    } else {
        header - 27
    };
    let recovery_id = RecoveryId::try_from(i32::from(recovery)).map_err(|_| ApiError::Invalid)?;
    let recoverable = RecoverableSignature::from_compact(&signature_bytes[1..], recovery_id)
        .map_err(|_| ApiError::Unauthorized)?;
    let digest = zcash_signed_message_hash(message)?;
    let secp_message = SecpMessage::from_digest(digest);
    let recovered = Secp256k1::verification_only()
        .recover_ecdsa(secp_message, &recoverable)
        .map_err(|_| ApiError::Unauthorized)?;
    if recovered != expected {
        return Err(ApiError::Unauthorized);
    }
    Ok(recovered.serialize())
}

async fn consume_challenge(tx: &Transaction<'_>, message: &str) -> Result<ChallengeRow, ApiError> {
    tx.query_opt(
        "UPDATE zecauth_challenges
         SET consumed_at = NOW()
         WHERE message = $1 AND consumed_at IS NULL AND expires_at > NOW()
         RETURNING id, requested_scopes, chain",
        &[&message],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?
    .map(|row| ChallengeRow {
        id: row.get(0),
        requested_scopes: row.get(1),
        chain: row.get(2),
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

    let zerant_id = zerant_public_handle(&verification_key);
    tx.execute(
        "UPDATE accounts SET public_handle = COALESCE(public_handle, $2) WHERE id = $1",
        &[&account, &zerant_id],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;

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
        zerant_id,
        scopes,
    })
    .into_response())
}

async fn wallet_message_verify(
    State(state): State<AppState>,
    Json(input): Json<VerifyWalletMessage>,
) -> Result<Response, ApiError> {
    if input.message.len() > 16 * 1024
        || input.granted.len() > 16
        || input.signing_mode != "derived"
    {
        return Err(ApiError::Invalid);
    }

    let public_key = verify_zcash_wallet_message(&input.pubkey, &input.signature, &input.message)?;

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
            "SELECT account_id
             FROM wallet_message_identities
             WHERE chain = $1 AND public_key = $2",
            &[&challenge.chain, &public_key.as_slice()],
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
                "INSERT INTO wallet_message_identities(account_id, chain, public_key)
                 VALUES ($1, $2, $3)",
                &[&account, &challenge.chain, &public_key.as_slice()],
            )
            .await
            .map_err(|_| ApiError::Unavailable)?;
            account
        }
    };

    let mut identity_material = Vec::with_capacity(challenge.chain.len() + public_key.len());
    identity_material.extend_from_slice(challenge.chain.as_bytes());
    identity_material.extend_from_slice(&public_key);
    let zerant_id = zerant_public_handle(&identity_material);
    tx.execute(
        "UPDATE accounts SET public_handle = COALESCE(public_handle, $2) WHERE id = $1",
        &[&account, &zerant_id],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;

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
        identity: zerant_id.clone(),
        zerant_id,
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

async fn ensure_credential_capacity(db: &Pool, account: Uuid) -> Result<(), ApiError> {
    let client = db_client(db).await?;
    let row = client
        .query_one(
            "SELECT COUNT(*) FROM credential_envelopes WHERE account_id = $1",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let count: i64 = row.get(0);
    if count >= MAX_ACCOUNT_CREDENTIALS {
        return Err(ApiError::Conflict);
    }
    Ok(())
}

async fn account_summary(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AccountSummary>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    let client = db_client(&state.db).await?;
    let row = client
        .query_one(
            "SELECT a.public_handle,
                    (SELECT COUNT(*) FROM credential_envelopes e WHERE e.account_id = a.id),
                    (SELECT display_name FROM issuer_profiles p WHERE p.account_id = a.id),
                    (SELECT display_name FROM verifier_profiles v WHERE v.account_id = a.id)
             FROM accounts a
             WHERE a.id = $1",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let zerant_id: Option<String> = row.get(0);
    let issuer_profile: Option<String> = row.get(2);
    let verifier_profile: Option<String> = row.get(3);
    Ok(Json(AccountSummary {
        zerant_id: zerant_id.ok_or(ApiError::Unavailable)?,
        credential_count: row.get(1),
        can_delete: issuer_profile.is_none() && verifier_profile.is_none(),
        issuer_profile,
        verifier_profile,
    }))
}

async fn export_account(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AccountExport>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    enforce_account_rate_limit(&state.db, account, "account_export", 10).await?;
    let client = db_client(&state.db).await?;

    let account_row = client
        .query_one(
            "SELECT public_handle FROM accounts WHERE id = $1",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let zerant_id: Option<String> = account_row.get(0);

    let rows = client
        .query(
            "SELECT e.id, e.ciphertext, e.data_nonce, e.wrapped_dek, e.wrap_nonce,
                    e.key_version, e.created_at, e.updated_at,
                    COALESCE(c.revoked_at IS NOT NULL, false) AS revoked
             FROM credential_envelopes e
             LEFT JOIN issued_credentials c ON c.vault_record_id = e.id
             WHERE e.account_id = $1
             ORDER BY e.created_at DESC",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    if i64::try_from(rows.len()).map_err(|_| ApiError::Unavailable)? > MAX_ACCOUNT_CREDENTIALS {
        return Err(ApiError::Conflict);
    }

    let mut credentials = Vec::with_capacity(rows.len());
    for row in rows {
        let revoked: bool = row.get("revoked");
        let row = CredentialRow::from_row(row);
        let mut plaintext = state.cipher.decrypt(account, &row)?;
        let credential = serde_json::from_slice(&plaintext).map_err(|_| ApiError::Unavailable)?;
        plaintext.fill(0);
        credentials.push(StoredCredential {
            id: row.id,
            credential,
            revoked,
            created_at: row.created_at,
            updated_at: row.updated_at,
        });
    }

    let event_limit = MAX_ACCOUNT_EXPORT_EVENTS
        .checked_add(1)
        .ok_or(ApiError::Unavailable)?;
    let event_rows = client
        .query(
            "SELECT id, event_type, object_id, label, context, counterparty, created_at
             FROM trust_events
             WHERE account_id = $1
             ORDER BY created_at DESC, id DESC
             LIMIT $2",
            &[&account, &event_limit],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let activity_complete = i64::try_from(event_rows.len()).map_err(|_| ApiError::Unavailable)?
        <= MAX_ACCOUNT_EXPORT_EVENTS;
    let mut activity = Vec::new();
    for row in event_rows
        .into_iter()
        .take(usize::try_from(MAX_ACCOUNT_EXPORT_EVENTS).map_err(|_| ApiError::Unavailable)?)
    {
        activity.push(ActivityEventView {
            id: row.get(0),
            event_type: row.get(1),
            object_id: row.get(2),
            label: row.get(3),
            context: row.get(4),
            counterparty: row.get(5),
            created_at: row.get(6),
        });
    }

    Ok(Json(AccountExport {
        export_version: 1,
        generated_at: OffsetDateTime::now_utc(),
        zerant_id: zerant_id.ok_or(ApiError::Unavailable)?,
        credentials,
        activity,
        activity_complete,
    }))
}

async fn delete_account(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    let mut client = db_client(&state.db).await?;
    let tx = client
        .transaction()
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let roles = tx
        .query_one(
            "SELECT EXISTS(SELECT 1 FROM issuer_profiles WHERE account_id = $1),
                    EXISTS(SELECT 1 FROM verifier_profiles WHERE account_id = $1)",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let has_issuer: bool = roles.get(0);
    let has_verifier: bool = roles.get(1);
    if has_issuer || has_verifier {
        return Err(ApiError::Conflict);
    }

    tx.query_one(
        "SELECT set_config('zerant.allow_account_delete', 'on', true)",
        &[],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;

    let affected = tx
        .execute("DELETE FROM accounts WHERE id = $1", &[&account])
        .await
        .map_err(|_| ApiError::Unavailable)?;
    if affected != 1 {
        return Err(ApiError::NotFound);
    }
    tx.commit().await.map_err(|_| ApiError::Unavailable)?;

    let clear = format!("{SESSION_COOKIE}=; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age=0");
    let mut response = StatusCode::NO_CONTENT.into_response();
    response.headers_mut().insert(
        SET_COOKIE,
        HeaderValue::from_str(&clear).map_err(|_| ApiError::Unavailable)?,
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
            "SELECT e.id, e.ciphertext, e.data_nonce, e.wrapped_dek, e.wrap_nonce,
                    e.key_version, e.created_at, e.updated_at,
                    COALESCE(c.revoked_at IS NOT NULL, false) AS revoked
             FROM credential_envelopes e
             LEFT JOIN issued_credentials c ON c.vault_record_id = e.id
             WHERE e.account_id = $1
             ORDER BY e.created_at DESC
             LIMIT 256",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let mut credentials = Vec::with_capacity(rows.len());
    for row in rows {
        let revoked: bool = row.get("revoked");
        let row = CredentialRow::from_row(row);
        let mut plaintext = state.cipher.decrypt(account, &row)?;
        let credential = serde_json::from_slice(&plaintext).map_err(|_| ApiError::Unavailable)?;
        plaintext.fill(0);
        credentials.push(StoredCredential {
            id: row.id,
            credential,
            revoked,
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
    enforce_account_rate_limit(&state.db, account, "credential_store", 30).await?;
    ensure_credential_capacity(&state.db, account).await?;
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
            revoked: false,
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

async fn signed_issuer_revocation_snapshot(
    state: &AppState,
    issuer_profile_id: Uuid,
    issuer_id: &str,
    issuer_key_id: &str,
    issuer_private: &Jwk,
    issued_at: u64,
    next_update: u64,
) -> Result<String, ApiError> {
    let client = db_client(&state.db).await?;
    let version_row = client
        .query_opt(
            "SELECT version FROM issuer_revocation_state WHERE issuer_profile_id = $1",
            &[&issuer_profile_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let version_i64 = version_row.map(|row| row.get::<_, i64>(0)).unwrap_or(1);
    if version_i64 <= 0 {
        return Err(ApiError::Unavailable);
    }
    let version = u64::try_from(version_i64).map_err(|_| ApiError::Unavailable)?;

    let rows = client
        .query(
            "SELECT revocation_digest
             FROM issued_credentials
             WHERE issuer_profile_id = $1
               AND revoked_at IS NOT NULL
               AND revocation_digest IS NOT NULL
             ORDER BY revocation_digest ASC",
            &[&issuer_profile_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let mut revoked_digests = Vec::with_capacity(rows.len());
    for row in rows {
        let digest: String = row.get(0);
        if revoked_digests.last() != Some(&digest) {
            revoked_digests.push(digest);
        }
    }

    let snapshot = RevocationSnapshot {
        schema: REVOCATION_SCHEMA.into(),
        issuer_id: issuer_id.to_owned(),
        issuer_key_id: issuer_key_id.to_owned(),
        version,
        issued_at,
        next_update,
        revoked_digests,
    };

    sign_revocation_snapshot(&snapshot, issuer_private).map_err(|_| ApiError::Invalid)
}

fn schema_slug(display_name: &str) -> String {
    let mut slug = String::with_capacity(display_name.len());
    let mut last_dash = false;
    for ch in display_name.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash && !slug.is_empty() {
            slug.push('-');
            last_dash = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    slug
}

async fn resolve_issuer_schema(
    db: &Pool,
    account: Uuid,
    schema_id: Uuid,
) -> Result<ResolvedCredentialSchema, ApiError> {
    let client = db_client(db).await?;
    let row = client
        .query_opt(
            "SELECT s.id, s.issuer_profile_id, p.issuer_id, p.display_name, s.display_name,
                    s.claim_type, s.context, s.default_expiry_days
             FROM credential_schemas s
             JOIN issuer_profiles p ON p.id = s.issuer_profile_id
             WHERE s.id = $1 AND p.account_id = $2 AND s.active = TRUE",
            &[&schema_id, &account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::NotFound)?;
    let days: i32 = row.get(7);
    let default_expiry_days = u16::try_from(days).map_err(|_| ApiError::Unavailable)?;
    Ok(ResolvedCredentialSchema {
        id: row.get(0),
        issuer_id: row.get(2),
        display_name: row.get(4),
        claim_type: row.get(5),
        context: row.get(6),
        default_expiry_days,
    })
}

async fn resolve_public_schema(
    db: &Pool,
    schema_id: Uuid,
) -> Result<ResolvedCredentialSchema, ApiError> {
    let client = db_client(db).await?;
    let row = client
        .query_opt(
            "SELECT s.id, s.issuer_profile_id, p.issuer_id, p.display_name, s.display_name,
                    s.claim_type, s.context, s.default_expiry_days
             FROM credential_schemas s
             JOIN issuer_profiles p ON p.id = s.issuer_profile_id
             WHERE s.id = $1 AND s.active = TRUE",
            &[&schema_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::NotFound)?;
    let days: i32 = row.get(7);
    let default_expiry_days = u16::try_from(days).map_err(|_| ApiError::Unavailable)?;
    Ok(ResolvedCredentialSchema {
        id: row.get(0),
        issuer_id: row.get(2),
        display_name: row.get(4),
        claim_type: row.get(5),
        context: row.get(6),
        default_expiry_days,
    })
}

async fn deactivate_issuer_schema(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(schema_id): Path<Uuid>,
) -> Result<Json<CredentialSchemaView>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    enforce_account_rate_limit(&state.db, account, "credential_schema_deactivate", 20).await?;
    let client = db_client(&state.db).await?;
    let row = client
        .query_opt(
            "UPDATE credential_schemas s
             SET active = FALSE, updated_at = NOW()
             FROM issuer_profiles p
             WHERE s.issuer_profile_id = p.id
               AND p.account_id = $1
               AND s.id = $2
               AND s.active = TRUE
             RETURNING s.id, p.issuer_id, p.display_name, s.display_name, s.description,
                       s.claim_type, s.context, s.default_expiry_days, s.active, s.created_at",
            &[&account, &schema_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::Conflict)?;

    Ok(Json(CredentialSchemaView {
        id: row.get(0),
        issuer_id: row.get(1),
        issuer_name: row.get(2),
        display_name: row.get(3),
        description: row.get(4),
        claim_type: row.get(5),
        context: row.get(6),
        default_expiry_days: row.get(7),
        active: row.get(8),
        created_at: row.get(9),
    }))
}

async fn list_issuer_schemas(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<CredentialSchemaView>>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    let client = db_client(&state.db).await?;
    let rows = client
        .query(
            "SELECT s.id, p.issuer_id, p.display_name, s.display_name, s.description,
                    s.claim_type, s.context, s.default_expiry_days, s.active, s.created_at
             FROM credential_schemas s
             JOIN issuer_profiles p ON p.id = s.issuer_profile_id
             WHERE p.account_id = $1
             ORDER BY s.created_at DESC
             LIMIT 128",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    Ok(Json(
        rows.into_iter()
            .map(|row| CredentialSchemaView {
                id: row.get(0),
                issuer_id: row.get(1),
                issuer_name: row.get(2),
                display_name: row.get(3),
                description: row.get(4),
                claim_type: row.get(5),
                context: row.get(6),
                default_expiry_days: row.get(7),
                active: row.get(8),
                created_at: row.get(9),
            })
            .collect(),
    ))
}

async fn create_issuer_schema(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateCredentialSchema>,
) -> Result<(StatusCode, Json<CredentialSchemaView>), ApiError> {
    let account = account_id(&headers, &state.db).await?;
    enforce_account_rate_limit(&state.db, account, "credential_schema_create", 20).await?;

    let display_name = input.display_name.trim().to_owned();
    let description = input.description.trim().to_owned();
    let context = input.context.trim().to_owned();
    let slug = schema_slug(&display_name);
    let claim_type = input
        .claim_type
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("credential.{slug}"));
    if !valid_short_text(&display_name, 2, 120)
        || !valid_short_text(&description, 2, 512)
        || !valid_short_text(&claim_type, 2, 120)
        || claim_type == "reputation.threshold"
        || !valid_short_text(&context, 2, 120)
        || !(1..=365).contains(&input.default_expiry_days)
    {
        return Err(ApiError::Invalid);
    }
    if slug.len() < 2 || slug.len() > 120 {
        return Err(ApiError::Invalid);
    }

    let client = db_client(&state.db).await?;
    let issuer = client
        .query_opt(
            "SELECT id, issuer_id, display_name FROM issuer_profiles WHERE account_id = $1",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::Unauthorized)?;
    let profile_id: Uuid = issuer.get(0);
    let issuer_id: String = issuer.get(1);
    let issuer_name: String = issuer.get(2);
    let days = i32::from(input.default_expiry_days);
    let id = Uuid::new_v4();

    let row = client
        .query_one(
            "INSERT INTO credential_schemas
             (id, issuer_profile_id, slug, display_name, description, claim_type, context,
              default_expiry_days)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
             RETURNING created_at",
            &[
                &id,
                &profile_id,
                &slug,
                &display_name,
                &description,
                &claim_type,
                &context,
                &days,
            ],
        )
        .await
        .map_err(|error| {
            if error
                .as_db_error()
                .is_some_and(|db| db.code().code() == "23505")
            {
                ApiError::Conflict
            } else {
                ApiError::Unavailable
            }
        })?;

    Ok((
        StatusCode::CREATED,
        Json(CredentialSchemaView {
            id,
            issuer_id,
            issuer_name,
            display_name,
            description,
            claim_type,
            context,
            default_expiry_days: days,
            active: true,
            created_at: row.get(0),
        }),
    ))
}

async fn get_verifier_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<VerifierProfileView>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    let client = db_client(&state.db).await?;
    let row = client
        .query_opt(
            "SELECT display_name, origin, created_at
             FROM verifier_profiles WHERE account_id = $1",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::NotFound)?;

    Ok(Json(VerifierProfileView {
        display_name: row.get(0),
        origin: row.get(1),
        created_at: row.get(2),
    }))
}

async fn list_verifier_keys(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<VerifierKeyView>>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    let client = db_client(&state.db).await?;
    let rows = client
        .query(
            "SELECT k.valid_from, k.retired_at, k.compromised_at
             FROM verifier_signing_keys k
             JOIN verifier_profiles v ON v.id = k.verifier_profile_id
             WHERE v.account_id = $1
             ORDER BY k.valid_from DESC
             LIMIT 32",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    Ok(Json(
        rows.into_iter()
            .map(|row| {
                let retired_at: Option<OffsetDateTime> = row.get(1);
                let compromised_at: Option<OffsetDateTime> = row.get(2);
                VerifierKeyView {
                    active: retired_at.is_none() && compromised_at.is_none(),
                    compromised: compromised_at.is_some(),
                    valid_from: row.get(0),
                    retired_at,
                }
            })
            .collect(),
    ))
}

async fn rotate_verifier_key(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<RotateVerifierKey>,
) -> Result<Json<VerifierKeyView>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    enforce_account_rate_limit(&state.db, account, "verifier_key_rotate", 3).await?;

    let new_key_row_id = Uuid::new_v4();
    let new_key_id = format!("key-{}", Uuid::new_v4().simple());
    let mut new_private =
        Jwk::generate_ed_key(EdCurve::Ed25519).map_err(|_| ApiError::Unavailable)?;
    new_private.set_key_id(new_key_id.clone());
    let new_public = public_jwk(&new_private)?;
    let public_value = serde_json::to_value(&new_public).map_err(|_| ApiError::Unavailable)?;
    let mut plaintext = serde_json::to_vec(&new_private).map_err(|_| ApiError::Unavailable)?;
    let encrypted = state.cipher.encrypt(account, new_key_row_id, &plaintext)?;
    plaintext.fill(0);

    let now = OffsetDateTime::now_utc();
    let mut client = db_client(&state.db).await?;
    let tx = client
        .transaction()
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let current = tx
        .query_opt(
            "SELECT v.id, k.id, k.verifier_key_id
             FROM verifier_profiles v
             JOIN verifier_signing_keys k ON k.verifier_profile_id = v.id
             WHERE v.account_id = $1
               AND k.retired_at IS NULL
               AND k.compromised_at IS NULL
             FOR UPDATE OF k",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::Conflict)?;

    let profile_id: Uuid = current.get(0);
    let current_key_row_id: Uuid = current.get(1);
    let current_key_id: String = current.get(2);

    let affected = tx
        .execute(
            "UPDATE verifier_signing_keys
             SET retired_at = $3,
                 compromised_at = CASE WHEN $4 THEN $3 ELSE compromised_at END
             WHERE id = $1
               AND verifier_profile_id = $2
               AND retired_at IS NULL
               AND compromised_at IS NULL",
            &[
                &current_key_row_id,
                &profile_id,
                &now,
                &input.compromise_current,
            ],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    if affected != 1 {
        return Err(ApiError::Conflict);
    }

    if input.compromise_current {
        tx.execute(
            "UPDATE verification_requests
             SET status = 'expired',
                 decided_at = COALESCE(decided_at, $3)
             WHERE verifier_profile_id = $1
               AND verifier_key_id = $2
               AND status = 'pending'",
            &[&profile_id, &current_key_id, &now],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    }

    tx.execute(
        "INSERT INTO verifier_signing_keys
         (id, verifier_profile_id, verifier_key_id, public_jwk, ciphertext, data_nonce,
          wrapped_dek, wrap_nonce, key_version, valid_from)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        &[
            &new_key_row_id,
            &profile_id,
            &new_key_id,
            &public_value,
            &encrypted.ciphertext,
            &encrypted.data_nonce,
            &encrypted.wrapped_dek,
            &encrypted.wrap_nonce,
            &encrypted.key_version,
            &now,
        ],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;

    tx.commit().await.map_err(|_| ApiError::Unavailable)?;

    Ok(Json(VerifierKeyView {
        active: true,
        compromised: false,
        valid_from: now,
        retired_at: None,
    }))
}

async fn register_verifier(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<RegisterVerifier>,
) -> Result<(StatusCode, Json<VerifierProfileView>), ApiError> {
    let account = account_id(&headers, &state.db).await?;
    enforce_account_rate_limit(&state.db, account, "verifier_register", 5).await?;
    let display_name = input.display_name.trim().to_owned();
    let origin = input.origin.trim().to_owned();
    if !valid_short_text(&display_name, 2, 120) || validate_origin(&origin, &[]).is_err() {
        return Err(ApiError::Invalid);
    }

    let client = db_client(&state.db).await?;
    if client
        .query_opt(
            "SELECT 1 FROM verifier_profiles WHERE account_id = $1 OR origin = $2",
            &[&account, &origin],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .is_some()
    {
        return Err(ApiError::Conflict);
    }

    let profile_id = Uuid::new_v4();
    let verifier_id = format!("zerant:verifier:{}", profile_id.simple());
    let verifier_key_id = format!("key-{}", Uuid::new_v4().simple());
    let mut private = Jwk::generate_ed_key(EdCurve::Ed25519).map_err(|_| ApiError::Unavailable)?;
    private.set_key_id(verifier_key_id.clone());
    let public = public_jwk(&private)?;
    let public_value = serde_json::to_value(&public).map_err(|_| ApiError::Unavailable)?;
    let mut plaintext = serde_json::to_vec(&private).map_err(|_| ApiError::Unavailable)?;
    let encrypted = state.cipher.encrypt(account, profile_id, &plaintext)?;
    plaintext.fill(0);

    let mut tx_client = db_client(&state.db).await?;
    let tx = tx_client
        .transaction()
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let row = tx
        .query_one(
            "INSERT INTO verifier_profiles
             (id, account_id, display_name, origin, verifier_id, verifier_key_id, public_jwk,
              ciphertext, data_nonce, wrapped_dek, wrap_nonce, key_version)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
             RETURNING display_name, origin, created_at",
            &[
                &profile_id,
                &account,
                &display_name,
                &origin,
                &verifier_id,
                &verifier_key_id,
                &public_value,
                &encrypted.ciphertext,
                &encrypted.data_nonce,
                &encrypted.wrapped_dek,
                &encrypted.wrap_nonce,
                &encrypted.key_version,
            ],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let created_at: OffsetDateTime = row.get(2);
    tx.execute(
        "INSERT INTO verifier_signing_keys
         (id, verifier_profile_id, verifier_key_id, public_jwk, ciphertext, data_nonce,
          wrapped_dek, wrap_nonce, key_version, valid_from)
         VALUES ($1, $1, $2, $3, $4, $5, $6, $7, $8, $9)",
        &[
            &profile_id,
            &verifier_key_id,
            &public_value,
            &encrypted.ciphertext,
            &encrypted.data_nonce,
            &encrypted.wrapped_dek,
            &encrypted.wrap_nonce,
            &encrypted.key_version,
            &created_at,
        ],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;

    tx.commit().await.map_err(|_| ApiError::Unavailable)?;

    Ok((
        StatusCode::CREATED,
        Json(VerifierProfileView {
            display_name: row.get(0),
            origin: row.get(1),
            created_at,
        }),
    ))
}

async fn issuer_directory(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<IssuerDirectoryEntry>>, ApiError> {
    let _ = account_id(&headers, &state.db).await?;
    let client = db_client(&state.db).await?;
    let rows = client
        .query(
            "SELECT p.display_name, p.issuer_id,
                    s.id, s.display_name, s.description, s.claim_type, s.context,
                    s.default_expiry_days, s.active, s.created_at
             FROM issuer_profiles p
             LEFT JOIN credential_schemas s
               ON s.issuer_profile_id = p.id AND s.active = TRUE
             ORDER BY p.display_name ASC, s.display_name ASC
             LIMIT 1024",
            &[],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let mut result: Vec<IssuerDirectoryEntry> = Vec::new();
    for row in rows {
        let issuer_id: String = row.get(1);
        let index = match result.iter().position(|item| item.issuer_id == issuer_id) {
            Some(index) => index,
            None => {
                result.push(IssuerDirectoryEntry {
                    display_name: row.get(0),
                    issuer_id: issuer_id.clone(),
                    schemas: Vec::new(),
                });
                result.len() - 1
            }
        };

        if let Some(id) = row.get::<_, Option<Uuid>>(2) {
            let issuer_name = result[index].display_name.clone();
            result[index].schemas.push(CredentialSchemaView {
                id,
                issuer_id: issuer_id.clone(),
                issuer_name,
                display_name: row.get(3),
                description: row.get(4),
                claim_type: row.get(5),
                context: row.get(6),
                default_expiry_days: row.get(7),
                active: row.get(8),
                created_at: row.get(9),
            });
        }
    }
    Ok(Json(result))
}

async fn create_verification_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateVerificationRequest>,
) -> Result<(StatusCode, Json<VerifierRequestView>), ApiError> {
    let account = account_id(&headers, &state.db).await?;
    enforce_account_rate_limit(&state.db, account, "verification_request", 60).await?;
    let holder_zerant_id = input.holder_zerant_id.trim().to_owned();
    let purpose = input.purpose.trim().to_owned();
    if !holder_zerant_id.starts_with("zr_")
        || !valid_short_text(&holder_zerant_id, 27, 27)
        || !valid_short_text(&purpose, 2, 1024)
    {
        return Err(ApiError::Invalid);
    }

    let client = db_client(&state.db).await?;
    let (credential_schema_id, credential_name, claim_type, context, mut accepted) =
        if let Some(schema_id) = input.credential_schema_id {
            let schema = resolve_public_schema(&state.db, schema_id).await?;
            if !input.accepted_issuer_ids.is_empty()
                && input.accepted_issuer_ids != vec![schema.issuer_id.clone()]
            {
                return Err(ApiError::Invalid);
            }
            (
                Some(schema.id),
                Some(schema.display_name),
                schema.claim_type,
                schema.context,
                vec![schema.issuer_id],
            )
        } else {
            let claim_type = input
                .claim_type
                .as_deref()
                .map(str::trim)
                .filter(|value| valid_short_text(value, 2, 120))
                .ok_or(ApiError::Invalid)?
                .to_owned();
            let context = input
                .context
                .as_deref()
                .map(str::trim)
                .filter(|value| valid_short_text(value, 2, 120))
                .ok_or(ApiError::Invalid)?
                .to_owned();
            if claim_type == "reputation.threshold"
                || input.accepted_issuer_ids.is_empty()
                || input.accepted_issuer_ids.len() > 32
            {
                return Err(ApiError::Invalid);
            }
            (None, None, claim_type, context, input.accepted_issuer_ids)
        };

    accepted.sort();
    accepted.dedup();
    if accepted.is_empty() {
        return Err(ApiError::Invalid);
    }

    for issuer_id in &accepted {
        if client
            .query_opt(
                "SELECT 1 FROM issuer_profiles WHERE issuer_id = $1",
                &[issuer_id],
            )
            .await
            .map_err(|_| ApiError::Unavailable)?
            .is_none()
        {
            return Err(ApiError::Invalid);
        }
    }

    let holder = client
        .query_opt(
            "SELECT id FROM accounts WHERE public_handle = $1",
            &[&holder_zerant_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::NotFound)?;
    let subject_account: Uuid = holder.get(0);

    let verifier = client
        .query_opt(
            "SELECT v.id, v.display_name, v.origin, v.verifier_id,
                    k.id, k.verifier_key_id, k.ciphertext, k.data_nonce,
                    k.wrapped_dek, k.wrap_nonce, k.key_version
             FROM verifier_profiles v
             JOIN verifier_signing_keys k ON k.verifier_profile_id = v.id
             WHERE v.account_id = $1
               AND k.retired_at IS NULL
               AND k.compromised_at IS NULL",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::Unauthorized)?;

    let profile_id: Uuid = verifier.get(0);
    let origin: String = verifier.get(2);
    let verifier_id: String = verifier.get(3);
    let signing_key_row_id: Uuid = verifier.get(4);
    let verifier_key_id: String = verifier.get(5);
    let verifier_secret = secret_row(&verifier, signing_key_row_id, 6);
    let private = decrypt_stored_jwk(&state, account, &verifier_secret)?;

    let now = OffsetDateTime::now_utc();
    let now_i64 = now.unix_timestamp();
    if now_i64 < 0 {
        return Err(ApiError::Unavailable);
    }
    let issued_at = now_i64 as u64;
    let expires_at = issued_at + 300;
    let request = Request {
        schema: REQUEST_SCHEMA.into(),
        request_id: random_id(),
        verifier_id,
        verifier_key_id: verifier_key_id.clone(),
        verifier_origin: origin,
        purpose: purpose.clone(),
        accepted_issuer_ids: accepted.clone(),
        claim_type: claim_type.clone(),
        context: Some(context.clone()),
        predicate: None,
        challenge: random_challenge(),
        nonce: random_challenge(),
        issued_at,
        expires_at,
    };
    let request_jws = sign_request(&request, &private, &[]).map_err(|_| ApiError::Invalid)?;
    let db_id = Uuid::new_v4();
    let expires_time =
        OffsetDateTime::from_unix_timestamp(expires_at as i64).map_err(|_| ApiError::Invalid)?;
    let accepted_value = serde_json::to_value(&accepted).map_err(|_| ApiError::Unavailable)?;

    client
        .execute(
            "INSERT INTO verification_requests
             (id, verifier_profile_id, subject_account_id, request_id, request_jws, purpose,
              claim_type, context, accepted_issuer_ids, expires_at, credential_schema_id,
              verifier_key_id)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
            &[
                &db_id,
                &profile_id,
                &subject_account,
                &request.request_id,
                &request_jws,
                &purpose,
                &claim_type,
                &context,
                &accepted_value,
                &expires_time,
                &credential_schema_id,
                &verifier_key_id,
            ],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    Ok((
        StatusCode::CREATED,
        Json(VerifierRequestView {
            id: db_id,
            holder_zerant_id,
            purpose,
            credential_schema_id,
            credential_name,
            claim_type,
            context,
            status: "pending".into(),
            verified: false,
            created_at: now,
            expires_at: expires_time,
        }),
    ))
}

async fn list_verifier_requests(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<VerifierRequestView>>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    let client = db_client(&state.db).await?;
    client
        .execute(
            "UPDATE verification_requests r
             SET status = 'expired'
             FROM verifier_profiles p
             WHERE r.verifier_profile_id = p.id
               AND p.account_id = $1
               AND r.status = 'pending'
               AND r.expires_at <= NOW()",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let rows = client
        .query(
            "SELECT r.id, a.public_handle, r.purpose, r.credential_schema_id, s.display_name,
                    r.claim_type, r.context, r.status, r.created_at, r.expires_at
             FROM verification_requests r
             JOIN verifier_profiles p ON p.id = r.verifier_profile_id
             JOIN accounts a ON a.id = r.subject_account_id
             LEFT JOIN credential_schemas s ON s.id = r.credential_schema_id
             WHERE p.account_id = $1
             ORDER BY r.created_at DESC
             LIMIT 256",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let mut result = Vec::with_capacity(rows.len());
    for row in rows {
        let status: String = row.get(7);
        let holder: Option<String> = row.get(1);
        result.push(VerifierRequestView {
            id: row.get(0),
            holder_zerant_id: holder.ok_or(ApiError::Unavailable)?,
            purpose: row.get(2),
            credential_schema_id: row.get(3),
            credential_name: row.get(4),
            claim_type: row.get(5),
            context: row.get(6),
            verified: status == "approved",
            status,
            created_at: row.get(8),
            expires_at: row.get(9),
        });
    }
    Ok(Json(result))
}

async fn list_holder_requests(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<HolderRequestView>>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    let client = db_client(&state.db).await?;
    client
        .execute(
            "UPDATE verification_requests
             SET status = 'expired'
             WHERE subject_account_id = $1
               AND status = 'pending'
               AND expires_at <= NOW()",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let rows = client
        .query(
            "SELECT r.id, v.display_name, v.origin, r.purpose, s.display_name,
                    r.claim_type, r.context, r.created_at, r.expires_at
             FROM verification_requests r
             JOIN verifier_profiles v ON v.id = r.verifier_profile_id
             LEFT JOIN credential_schemas s ON s.id = r.credential_schema_id
             WHERE r.subject_account_id = $1
               AND r.status = 'pending'
               AND r.expires_at > NOW()
             ORDER BY r.created_at DESC
             LIMIT 64",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    Ok(Json(
        rows.into_iter()
            .map(|row| HolderRequestView {
                id: row.get(0),
                verifier_name: row.get(1),
                verifier_origin: row.get(2),
                purpose: row.get(3),
                credential_name: row.get(4),
                claim_type: row.get(5),
                context: row.get(6),
                created_at: row.get(7),
                expires_at: row.get(8),
            })
            .collect(),
    ))
}

async fn decide_holder_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(input): Json<DecideRequest>,
) -> Result<Json<Value>, ApiError> {
    let holder_account = account_id(&headers, &state.db).await?;
    enforce_account_rate_limit(&state.db, holder_account, "verification_decision", 60).await?;
    if input.decision == "deny" {
        let client = db_client(&state.db).await?;
        let affected = client
            .execute(
                "UPDATE verification_requests
                 SET status = 'denied', decided_at = NOW()
                 WHERE id = $1 AND subject_account_id = $2
                   AND status = 'pending' AND expires_at > NOW()",
                &[&id, &holder_account],
            )
            .await
            .map_err(|_| ApiError::Unavailable)?;
        if affected != 1 {
            return Err(ApiError::Conflict);
        }
        return Ok(Json(
            serde_json::json!({ "status": "denied", "verified": false }),
        ));
    }
    if input.decision != "approve" {
        return Err(ApiError::Invalid);
    }

    let client = db_client(&state.db).await?;
    let request_row = client
        .query_opt(
            "SELECT r.request_jws, r.claim_type, r.context, r.accepted_issuer_ids,
                    r.expires_at, r.status,
                    v.id, v.account_id, v.origin, v.verifier_id,
                    k.verifier_key_id, k.public_jwk, k.valid_from, k.compromised_at
             FROM verification_requests r
             JOIN verifier_profiles v ON v.id = r.verifier_profile_id
             JOIN verifier_signing_keys k
               ON k.verifier_profile_id = v.id
              AND k.verifier_key_id = r.verifier_key_id
             WHERE r.id = $1 AND r.subject_account_id = $2",
            &[&id, &holder_account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::NotFound)?;

    let status: String = request_row.get(5);
    let request_expires: OffsetDateTime = request_row.get(4);
    let now = OffsetDateTime::now_utc();
    if status != "pending" || request_expires <= now {
        return Err(ApiError::Conflict);
    }

    let request_jws: String = request_row.get(0);
    let claim_type: String = request_row.get(1);
    let context: String = request_row.get(2);
    let accepted_value: Value = request_row.get(3);
    let accepted: Vec<String> =
        serde_json::from_value(accepted_value).map_err(|_| ApiError::Unavailable)?;

    let candidates = client
        .query(
            "SELECT c.vault_record_id,
                    p.id, p.account_id, p.issuer_id,
                    k.id, k.issuer_key_id, k.public_jwk,
                    k.ciphertext, k.data_nonce, k.wrapped_dek, k.wrap_nonce, k.key_version,
                    k.valid_from, k.compromised_at, c.expires_at
             FROM issued_credentials c
             JOIN issuer_profiles p ON p.id = c.issuer_profile_id
             JOIN issuer_signing_keys k
               ON k.issuer_profile_id = p.id
              AND k.issuer_key_id = c.issuer_key_id
             WHERE c.subject_account_id = $1
               AND c.claim_type = $2
               AND c.context = $3
               AND c.revoked_at IS NULL
               AND c.expires_at > NOW()
             ORDER BY c.created_at DESC",
            &[&holder_account, &claim_type, &context],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let candidate = candidates
        .into_iter()
        .find(|row| {
            let issuer_id: String = row.get(3);
            let vault_record_id: Option<Uuid> = row.get(0);
            vault_record_id.is_some() && accepted.contains(&issuer_id)
        })
        .ok_or(ApiError::NotFound)?;

    let vault_record_id: Uuid = candidate
        .get::<_, Option<Uuid>>(0)
        .ok_or(ApiError::NotFound)?;
    let vault_row = client
        .query_opt(
            "SELECT id, ciphertext, data_nonce, wrapped_dek, wrap_nonce, key_version,
                    created_at, updated_at
             FROM credential_envelopes
             WHERE id = $1 AND account_id = $2",
            &[&vault_record_id, &holder_account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::NotFound)?;
    let vault_row = CredentialRow::from_row(vault_row);
    let mut private_record = state.cipher.decrypt(holder_account, &vault_row)?;
    let record: Value =
        serde_json::from_slice(&private_record).map_err(|_| ApiError::Unavailable)?;
    private_record.fill(0);
    if record.get("type").and_then(Value::as_str) != Some("zerant.private-credential")
        || record.get("claim_type").and_then(Value::as_str) != Some(claim_type.as_str())
        || record.get("context").and_then(Value::as_str) != Some(context.as_str())
    {
        return Err(ApiError::Invalid);
    }
    let value = record
        .get("value")
        .and_then(Value::as_str)
        .ok_or(ApiError::Invalid)?
        .to_owned();
    let source_jws = record
        .get("signed_credential")
        .and_then(Value::as_str)
        .ok_or(ApiError::Invalid)?
        .to_owned();

    let issuer_profile_id: Uuid = candidate.get(1);
    let issuer_account: Uuid = candidate.get(2);
    let issuer_id: String = candidate.get(3);
    let issuer_signing_key_id: Uuid = candidate.get(4);
    let issuer_key_id: String = candidate.get(5);
    let issuer_public_value: Value = candidate.get(6);
    let issuer_public: PublicJwk =
        serde_json::from_value(issuer_public_value).map_err(|_| ApiError::Unavailable)?;
    let issuer_secret = secret_row(&candidate, issuer_signing_key_id, 7);
    let issuer_private = decrypt_stored_jwk(&state, issuer_account, &issuer_secret)?;
    let issuer_valid_from_time: OffsetDateTime = candidate.get(12);
    let issuer_compromised_at: Option<OffsetDateTime> = candidate.get(13);
    let source_expires_at: OffsetDateTime = candidate.get(14);

    let now_i64 = now.unix_timestamp();
    let request_expiry_i64 = request_expires.unix_timestamp();
    if now_i64 < 0 || request_expiry_i64 <= now_i64 {
        return Err(ApiError::Conflict);
    }
    let now_u64 = now_i64 as u64;
    let source_expiry_i64 = source_expires_at.unix_timestamp();
    if source_expiry_i64 <= now_i64 {
        return Err(ApiError::Conflict);
    }
    let proof_expiry =
        bounded_proof_expiry(now_u64, request_expiry_i64 as u64, source_expiry_i64 as u64)?;

    let issuer_valid_from = issuer_valid_from_time.unix_timestamp();
    if issuer_valid_from < 0 {
        return Err(ApiError::Unavailable);
    }
    let key_valid_until = source_expiry_i64 as u64;
    let trust = IssuerTrustManifest {
        issuers: vec![TrustedIssuer {
            issuer_id: issuer_id.clone(),
            keys: vec![TrustedIssuerKey {
                issuer_key_id: issuer_key_id.clone(),
                public_key: issuer_public.clone(),
                valid_from: issuer_valid_from as u64,
                valid_until: key_valid_until,
                compromised: issuer_compromised_at.is_some(),
            }],
            allowed_claim_types: vec![claim_type.clone()],
            allowed_contexts: vec![context.clone()],
            source_schemas: vec![SourceSchemaAuthorization {
                source_schema_id: claim_type.clone(),
                version: "0.1".into(),
                context: context.clone(),
                categories: vec![value.clone()],
            }],
            policies: vec![],
        }],
    };

    let revocation_jws = signed_issuer_revocation_snapshot(
        &state,
        issuer_profile_id,
        &issuer_id,
        &issuer_key_id,
        &issuer_private,
        now_u64,
        proof_expiry,
    )
    .await?;

    let verified_source = verify_credential(
        &source_jws,
        &revocation_jws,
        &trust,
        &issuer_id,
        HOLDER_LOCAL_AUDIENCE,
        &[],
        now_u64,
        0,
    )
    .map_err(|_| ApiError::Invalid)?;

    let holder_public = ensure_account_credential_key(&state, holder_account).await?;
    if verified_source.payload.subject_key != holder_public {
        return Err(ApiError::Unauthorized);
    }

    let verifier_profile_id: Uuid = request_row.get(6);
    let (pairwise_private, pairwise_public) =
        load_or_create_pairwise_holder_key(&state, holder_account, verifier_profile_id).await?;

    let verifier_origin: String = request_row.get(8);
    let attestation = CredentialPayload {
        schema: CREDENTIAL_SCHEMA.into(),
        kind: CredentialKind::Attestation,
        credential_id: random_id(),
        issuer_id: issuer_id.clone(),
        issuer_key_id: issuer_key_id.clone(),
        subject_key: pairwise_public,
        audience: verifier_origin.clone(),
        issued_at: now_u64,
        expires_at: proof_expiry,
        revocation_handle: random_id(),
        claim: Claim::Ordinary(OrdinaryClaim {
            claim_type: claim_type.clone(),
            value: ClaimValue::String(value),
            context: Some(context.clone()),
        }),
    };
    let attestation_jws =
        sign_credential(&attestation, &issuer_private).map_err(|_| ApiError::Invalid)?;

    let verifier_public_value: Value = request_row.get(11);
    let verifier_public: PublicJwk =
        serde_json::from_value(verifier_public_value).map_err(|_| ApiError::Unavailable)?;
    let verifier_valid_from_time: OffsetDateTime = request_row.get(12);
    let verifier_compromised_at: Option<OffsetDateTime> = request_row.get(13);
    let verifier_valid_from = verifier_valid_from_time.unix_timestamp();
    if verifier_valid_from < 0 {
        return Err(ApiError::Unavailable);
    }
    let pin = VerifierPin {
        verifier_id: request_row.get(9),
        key_id: request_row.get(10),
        key: verifier_public,
        allowed_origins: vec![verifier_origin.clone()],
        valid_from: verifier_valid_from as u64,
        valid_until: request_expiry_i64 as u64,
        compromised: verifier_compromised_at.is_some(),
    };
    let disclosure_context = DisclosureContext {
        pin: &pin,
        origin: &verifier_origin,
        trust: &trust,
        now: now_u64,
        allowed_loopback: &[],
    };
    let evidence = Evidence {
        attestation_jws: &attestation_jws,
        revocation_jws: &revocation_jws,
        issuer_id: &issuer_id,
        minimum_revocation_version: 0,
    };
    let response = respond(
        &request_jws,
        &disclosure_context,
        Decision::Approve,
        Some(&evidence),
        &pairwise_private,
    )
    .map_err(|_| ApiError::Invalid)?
    .ok_or(ApiError::Invalid)?;

    let verifier_account: Uuid = request_row.get(7);
    let encrypted_response = state
        .cipher
        .encrypt(verifier_account, id, response.as_bytes())?;
    let affected = client
        .execute(
            "UPDATE verification_requests
             SET status = 'approved',
                 response_ciphertext = $3,
                 response_data_nonce = $4,
                 response_wrapped_dek = $5,
                 response_wrap_nonce = $6,
                 response_key_version = $7,
                 decided_at = NOW()
             WHERE id = $1 AND subject_account_id = $2
               AND status = 'pending' AND expires_at > NOW()",
            &[
                &id,
                &holder_account,
                &encrypted_response.ciphertext,
                &encrypted_response.data_nonce,
                &encrypted_response.wrapped_dek,
                &encrypted_response.wrap_nonce,
                &encrypted_response.key_version,
            ],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    if affected != 1 {
        return Err(ApiError::Conflict);
    }

    Ok(Json(
        serde_json::json!({ "status": "approved", "verified": true }),
    ))
}

fn secret_row(row: &Row, object_id: Uuid, offset: usize) -> CredentialRow {
    let now = OffsetDateTime::now_utc();
    CredentialRow {
        id: object_id,
        ciphertext: row.get(offset),
        data_nonce: row.get(offset + 1),
        wrapped_dek: row.get(offset + 2),
        wrap_nonce: row.get(offset + 3),
        key_version: row.get(offset + 4),
        created_at: now,
        updated_at: now,
    }
}

fn decrypt_stored_jwk(
    state: &AppState,
    owner_account: Uuid,
    secret: &CredentialRow,
) -> Result<Jwk, ApiError> {
    let mut bytes = state.cipher.decrypt(owner_account, secret)?;
    let key = serde_json::from_slice(&bytes).map_err(|_| ApiError::Unavailable)?;
    bytes.fill(0);
    Ok(key)
}

async fn get_issuer_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<IssuerProfileView>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    let client = db_client(&state.db).await?;
    let row = client
        .query_opt(
            "SELECT display_name, issuer_id, created_at
             FROM issuer_profiles
             WHERE account_id = $1",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::NotFound)?;

    Ok(Json(IssuerProfileView {
        display_name: row.get(0),
        issuer_id: row.get(1),
        created_at: row.get(2),
    }))
}

async fn list_issuer_keys(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<IssuerKeyView>>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    let client = db_client(&state.db).await?;
    let rows = client
        .query(
            "SELECT k.valid_from, k.retired_at, k.compromised_at
             FROM issuer_signing_keys k
             JOIN issuer_profiles p ON p.id = k.issuer_profile_id
             WHERE p.account_id = $1
             ORDER BY k.valid_from DESC
             LIMIT 32",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    Ok(Json(
        rows.into_iter()
            .map(|row| {
                let retired_at: Option<OffsetDateTime> = row.get(1);
                let compromised_at: Option<OffsetDateTime> = row.get(2);
                IssuerKeyView {
                    active: retired_at.is_none() && compromised_at.is_none(),
                    compromised: compromised_at.is_some(),
                    valid_from: row.get(0),
                    retired_at,
                }
            })
            .collect(),
    ))
}

async fn rotate_issuer_key(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<RotateIssuerKey>,
) -> Result<Json<IssuerKeyView>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    enforce_account_rate_limit(&state.db, account, "issuer_key_rotate", 3).await?;

    let new_key_row_id = Uuid::new_v4();
    let new_key_id = format!("key-{}", Uuid::new_v4().simple());
    let mut new_private =
        Jwk::generate_ed_key(EdCurve::Ed25519).map_err(|_| ApiError::Unavailable)?;
    new_private.set_key_id(new_key_id.clone());
    let new_public = public_jwk(&new_private)?;
    let public_value = serde_json::to_value(&new_public).map_err(|_| ApiError::Unavailable)?;
    let mut plaintext = serde_json::to_vec(&new_private).map_err(|_| ApiError::Unavailable)?;
    let encrypted = state.cipher.encrypt(account, new_key_row_id, &plaintext)?;
    plaintext.fill(0);

    let now = OffsetDateTime::now_utc();
    let mut client = db_client(&state.db).await?;
    let tx = client
        .transaction()
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let current = tx
        .query_opt(
            "SELECT p.id, k.id, k.issuer_key_id
             FROM issuer_profiles p
             JOIN issuer_signing_keys k ON k.issuer_profile_id = p.id
             WHERE p.account_id = $1
               AND k.retired_at IS NULL
               AND k.compromised_at IS NULL
             FOR UPDATE OF k",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::Conflict)?;

    let profile_id: Uuid = current.get(0);
    let current_key_row_id: Uuid = current.get(1);
    let current_key_id: String = current.get(2);

    let affected = tx
        .execute(
            "UPDATE issuer_signing_keys
             SET retired_at = $3,
                 compromised_at = CASE WHEN $4 THEN $3 ELSE compromised_at END
             WHERE id = $1
               AND issuer_profile_id = $2
               AND retired_at IS NULL
               AND compromised_at IS NULL",
            &[
                &current_key_row_id,
                &profile_id,
                &now,
                &input.compromise_current,
            ],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    if affected != 1 {
        return Err(ApiError::Conflict);
    }

    if input.compromise_current {
        tx.execute(
            "UPDATE issued_credentials
             SET revoked_at = COALESCE(revoked_at, $3)
             WHERE issuer_profile_id = $1
               AND issuer_key_id = $2
               AND revoked_at IS NULL",
            &[&profile_id, &current_key_id, &now],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

        tx.execute(
            "INSERT INTO issuer_revocation_state(issuer_profile_id, version, updated_at)
             VALUES ($1, 2, $2)
             ON CONFLICT (issuer_profile_id)
             DO UPDATE SET version = issuer_revocation_state.version + 1, updated_at = $2",
            &[&profile_id, &now],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    }

    tx.execute(
        "INSERT INTO issuer_signing_keys
         (id, issuer_profile_id, issuer_key_id, public_jwk, ciphertext, data_nonce,
          wrapped_dek, wrap_nonce, key_version, valid_from)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        &[
            &new_key_row_id,
            &profile_id,
            &new_key_id,
            &public_value,
            &encrypted.ciphertext,
            &encrypted.data_nonce,
            &encrypted.wrapped_dek,
            &encrypted.wrap_nonce,
            &encrypted.key_version,
            &now,
        ],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;

    tx.commit().await.map_err(|_| ApiError::Unavailable)?;

    Ok(Json(IssuerKeyView {
        active: true,
        compromised: false,
        valid_from: now,
        retired_at: None,
    }))
}

async fn register_issuer(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<RegisterIssuer>,
) -> Result<(StatusCode, Json<IssuerProfileView>), ApiError> {
    let account = account_id(&headers, &state.db).await?;
    enforce_account_rate_limit(&state.db, account, "issuer_register", 5).await?;
    let display_name = input.display_name.trim().to_owned();
    if !(2..=120).contains(&display_name.chars().count())
        || display_name.chars().any(char::is_control)
    {
        return Err(ApiError::Invalid);
    }

    let client = db_client(&state.db).await?;
    if client
        .query_opt(
            "SELECT 1 FROM issuer_profiles WHERE account_id = $1",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .is_some()
    {
        return Err(ApiError::Conflict);
    }

    let profile_id = Uuid::new_v4();
    let issuer_id = format!("zerant:issuer:{}", profile_id.simple());
    let issuer_key_id = format!("key-{}", Uuid::new_v4().simple());
    let mut private = Jwk::generate_ed_key(EdCurve::Ed25519).map_err(|_| ApiError::Unavailable)?;
    private.set_key_id(issuer_key_id.clone());
    let public = public_jwk(&private)?;

    let mut plaintext = serde_json::to_vec(&private).map_err(|_| ApiError::Unavailable)?;
    let encrypted = state.cipher.encrypt(account, profile_id, &plaintext)?;
    plaintext.fill(0);
    let public_value = serde_json::to_value(&public).map_err(|_| ApiError::Unavailable)?;

    let mut tx_client = db_client(&state.db).await?;
    let tx = tx_client
        .transaction()
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let row = tx
        .query_one(
            "INSERT INTO issuer_profiles
             (id, account_id, display_name, issuer_id, issuer_key_id, public_jwk,
              ciphertext, data_nonce, wrapped_dek, wrap_nonce, key_version)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
             RETURNING display_name, issuer_id, created_at",
            &[
                &profile_id,
                &account,
                &display_name,
                &issuer_id,
                &issuer_key_id,
                &public_value,
                &encrypted.ciphertext,
                &encrypted.data_nonce,
                &encrypted.wrapped_dek,
                &encrypted.wrap_nonce,
                &encrypted.key_version,
            ],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let created_at: OffsetDateTime = row.get(2);
    tx.execute(
        "INSERT INTO issuer_signing_keys
         (id, issuer_profile_id, issuer_key_id, public_jwk, ciphertext, data_nonce,
          wrapped_dek, wrap_nonce, key_version, valid_from)
         VALUES ($1, $1, $2, $3, $4, $5, $6, $7, $8, $9)",
        &[
            &profile_id,
            &issuer_key_id,
            &public_value,
            &encrypted.ciphertext,
            &encrypted.data_nonce,
            &encrypted.wrapped_dek,
            &encrypted.wrap_nonce,
            &encrypted.key_version,
            &created_at,
        ],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;

    tx.commit().await.map_err(|_| ApiError::Unavailable)?;

    Ok((
        StatusCode::CREATED,
        Json(IssuerProfileView {
            display_name: row.get(0),
            issuer_id: row.get(1),
            created_at,
        }),
    ))
}

fn valid_short_text(value: &str, min: usize, max: usize) -> bool {
    let length = value.chars().count();
    (min..=max).contains(&length) && !value.chars().any(char::is_control)
}

async fn issue_private_credential(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<IssueCredential>,
) -> Result<(StatusCode, Json<IssuedCredentialView>), ApiError> {
    let account = account_id(&headers, &state.db).await?;
    enforce_account_rate_limit(&state.db, account, "credential_issue", 30).await?;
    let holder_zerant_id = input.holder_zerant_id.trim().to_owned();
    let value = input.value.trim().to_owned();
    if !holder_zerant_id.starts_with("zr_")
        || !valid_short_text(&holder_zerant_id, 27, 27)
        || !valid_short_text(&value, 1, 512)
    {
        return Err(ApiError::Invalid);
    }

    let client = db_client(&state.db).await?;
    let issuer = client
        .query_opt(
            "SELECT p.id, p.display_name, p.issuer_id,
                    k.id, k.issuer_key_id, k.ciphertext, k.data_nonce,
                    k.wrapped_dek, k.wrap_nonce, k.key_version
             FROM issuer_profiles p
             JOIN issuer_signing_keys k ON k.issuer_profile_id = p.id
             WHERE p.account_id = $1
               AND k.retired_at IS NULL
               AND k.compromised_at IS NULL",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::Unauthorized)?;

    let (credential_schema_id, schema_display_name, claim_type, context, expires_days) =
        if let Some(schema_id) = input.credential_schema_id {
            let schema = resolve_issuer_schema(&state.db, account, schema_id).await?;
            (
                Some(schema.id),
                Some(schema.display_name),
                schema.claim_type,
                schema.context,
                schema.default_expiry_days,
            )
        } else {
            let claim_type = input
                .claim_type
                .as_deref()
                .map(str::trim)
                .filter(|value| valid_short_text(value, 2, 120))
                .ok_or(ApiError::Invalid)?
                .to_owned();
            let context = input
                .context
                .as_deref()
                .map(str::trim)
                .filter(|value| valid_short_text(value, 2, 120))
                .ok_or(ApiError::Invalid)?
                .to_owned();
            let expires_days = input.expires_in_days.unwrap_or(90);
            if !(1..=365).contains(&expires_days) {
                return Err(ApiError::Invalid);
            }
            (None, None, claim_type, context, expires_days)
        };

    let subject = client
        .query_opt(
            "SELECT id FROM accounts WHERE public_handle = $1",
            &[&holder_zerant_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::NotFound)?;
    let subject_account: Uuid = subject.get(0);
    ensure_credential_capacity(&state.db, subject_account).await?;
    let subject_key = ensure_account_credential_key(&state, subject_account).await?;

    let profile_id: Uuid = issuer.get(0);
    let issuer_name: String = issuer.get(1);
    let issuer_id: String = issuer.get(2);
    let signing_key_id: Uuid = issuer.get(3);
    let issuer_key_id: String = issuer.get(4);
    let now = OffsetDateTime::now_utc();
    let secret_row = CredentialRow {
        id: signing_key_id,
        ciphertext: issuer.get(5),
        data_nonce: issuer.get(6),
        wrapped_dek: issuer.get(7),
        wrap_nonce: issuer.get(8),
        key_version: issuer.get(9),
        created_at: now,
        updated_at: now,
    };
    let mut private_bytes = state.cipher.decrypt(account, &secret_row)?;
    let private: Jwk = serde_json::from_slice(&private_bytes).map_err(|_| ApiError::Unavailable)?;
    private_bytes.fill(0);

    let issued_at_i64 = now.unix_timestamp();
    if issued_at_i64 < 0 {
        return Err(ApiError::Unavailable);
    }
    let issued_at = issued_at_i64 as u64;
    let expires_at = issued_at
        .checked_add(u64::from(expires_days) * 86_400)
        .ok_or(ApiError::Invalid)?;
    let occurred_at = input.occurred_at.unwrap_or(issued_at);
    if occurred_at > issued_at {
        return Err(ApiError::Invalid);
    }

    let credential_id = random_id();
    let payload = CredentialPayload {
        schema: CREDENTIAL_SCHEMA.into(),
        kind: CredentialKind::Source,
        credential_id: credential_id.clone(),
        issuer_id,
        issuer_key_id: issuer_key_id.clone(),
        subject_key,
        audience: HOLDER_LOCAL_AUDIENCE.into(),
        issued_at,
        expires_at,
        revocation_handle: random_id(),
        claim: Claim::Source(SourceEventClaim {
            claim_type: claim_type.clone(),
            value: value.clone(),
            source_schema_version: "0.1".into(),
            context: context.clone(),
            occurred_at,
        }),
    };
    let token = sign_credential(&payload, &private).map_err(|_| ApiError::Invalid)?;
    let source_revocation_digest =
        revocation_digest(&payload.revocation_handle).map_err(|_| ApiError::Invalid)?;

    let holder_record = serde_json::json!({
        "type": "zerant.private-credential",
        "issuer": issuer_name,
        "credential_id": credential_id,
        "credential_schema_id": credential_schema_id,
        "credential_name": schema_display_name,
        "claim_type": claim_type,
        "value": value,
        "context": context,
        "issued_at": issued_at,
        "expires_at": expires_at,
        "signed_credential": token,
    });
    let mut holder_plaintext =
        serde_json::to_vec(&holder_record).map_err(|_| ApiError::Unavailable)?;
    let vault_record_id = Uuid::new_v4();
    let encrypted = state
        .cipher
        .encrypt(subject_account, vault_record_id, &holder_plaintext)?;
    holder_plaintext.fill(0);

    let issued_time =
        OffsetDateTime::from_unix_timestamp(issued_at_i64).map_err(|_| ApiError::Invalid)?;
    let expiry_time = now + Duration::days(i64::from(expires_days));

    let mut tx_client = db_client(&state.db).await?;
    let tx = tx_client
        .transaction()
        .await
        .map_err(|_| ApiError::Unavailable)?;

    tx.execute(
        "INSERT INTO credential_envelopes
         (id, account_id, ciphertext, data_nonce, wrapped_dek, wrap_nonce, key_version)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
        &[
            &vault_record_id,
            &subject_account,
            &encrypted.ciphertext,
            &encrypted.data_nonce,
            &encrypted.wrapped_dek,
            &encrypted.wrap_nonce,
            &encrypted.key_version,
        ],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;

    tx.execute(
        "INSERT INTO issued_credentials
         (id, issuer_profile_id, subject_account_id, credential_id, claim_type, context,
          issued_at, expires_at, vault_record_id, revocation_digest, credential_schema_id,
          issuer_key_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
        &[
            &Uuid::new_v4(),
            &profile_id,
            &subject_account,
            &payload.credential_id,
            &claim_type,
            &context,
            &issued_time,
            &expiry_time,
            &vault_record_id,
            &source_revocation_digest,
            &credential_schema_id,
            &issuer_key_id,
        ],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;

    tx.execute(
        "INSERT INTO issuer_revocation_state(issuer_profile_id)
         VALUES ($1)
         ON CONFLICT (issuer_profile_id) DO NOTHING",
        &[&profile_id],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;

    tx.commit().await.map_err(|_| ApiError::Unavailable)?;

    Ok((
        StatusCode::CREATED,
        Json(IssuedCredentialView {
            credential_id: payload.credential_id,
            holder_zerant_id,
            credential_schema_id,
            claim_type,
            context,
            issued_at: issued_time,
            expires_at: expiry_time,
            revoked: false,
        }),
    ))
}

async fn revoke_issued_credential(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(credential_id): Path<String>,
) -> Result<Json<IssuedCredentialView>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    enforce_account_rate_limit(&state.db, account, "credential_revoke", 30).await?;
    if credential_id.is_empty() || credential_id.len() > 128 {
        return Err(ApiError::Invalid);
    }

    let mut client = db_client(&state.db).await?;
    let tx = client
        .transaction()
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let row = tx
        .query_opt(
            "UPDATE issued_credentials c
             SET revoked_at = NOW()
             FROM issuer_profiles p, accounts a
             WHERE c.issuer_profile_id = p.id
               AND p.account_id = $1
               AND c.credential_id = $2
               AND c.subject_account_id = a.id
               AND c.revoked_at IS NULL
             RETURNING c.issuer_profile_id, c.credential_id, a.public_handle,
                       c.credential_schema_id, c.claim_type, c.context, c.issued_at, c.expires_at",
            &[&account, &credential_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::Conflict)?;

    let issuer_profile_id: Uuid = row.get(0);
    tx.execute(
        "INSERT INTO issuer_revocation_state(issuer_profile_id, version, updated_at)
         VALUES ($1, 2, NOW())
         ON CONFLICT (issuer_profile_id)
         DO UPDATE SET version = issuer_revocation_state.version + 1, updated_at = NOW()",
        &[&issuer_profile_id],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;

    tx.commit().await.map_err(|_| ApiError::Unavailable)?;

    let holder: Option<String> = row.get(2);
    Ok(Json(IssuedCredentialView {
        credential_id: row.get(1),
        holder_zerant_id: holder.ok_or(ApiError::Unavailable)?,
        credential_schema_id: row.get(3),
        claim_type: row.get(4),
        context: row.get(5),
        issued_at: row.get(6),
        expires_at: row.get(7),
        revoked: true,
    }))
}

async fn list_issued_credentials(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<IssuedCredentialView>>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    let client = db_client(&state.db).await?;
    let rows = client
        .query(
            "SELECT c.credential_id, a.public_handle, c.credential_schema_id, c.claim_type, c.context,
                    c.issued_at, c.expires_at, c.revoked_at IS NOT NULL
             FROM issued_credentials c
             JOIN issuer_profiles p ON p.id = c.issuer_profile_id
             JOIN accounts a ON a.id = c.subject_account_id
             WHERE p.account_id = $1
             ORDER BY c.created_at DESC
             LIMIT 256",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let mut result = Vec::with_capacity(rows.len());
    for row in rows {
        let holder: Option<String> = row.get(1);
        result.push(IssuedCredentialView {
            credential_id: row.get(0),
            holder_zerant_id: holder.ok_or(ApiError::Unavailable)?,
            credential_schema_id: row.get(2),
            claim_type: row.get(3),
            context: row.get(4),
            issued_at: row.get(5),
            expires_at: row.get(6),
            revoked: row.get(7),
        });
    }
    Ok(Json(result))
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
            "SELECT a.public_handle, s.scopes
             FROM sessions s
             JOIN accounts a ON a.id = s.account_id
             WHERE s.token_hash = $1 AND s.expires_at > NOW()",
            &[&hash],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::Unauthorized)?;
    let zerant_id: Option<String> = row.get(0);
    let scopes_value: Value = row.get(1);
    let scopes: Vec<String> =
        serde_json::from_value(scopes_value).map_err(|_| ApiError::Unavailable)?;
    Ok(Json(SessionInfo {
        authenticated: true,
        identity: zerant_id.clone().ok_or(ApiError::Unavailable)?,
        zerant_id: zerant_id.ok_or(ApiError::Unavailable)?,
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
    let account = account_id(&headers, &state.db).await?;
    enforce_account_rate_limit(&state.db, account, "zcash_address_inspect", 120).await?;
    let summary =
        zerant_zcash::address::inspect_address(&input.address).map_err(|_| ApiError::Invalid)?;
    Ok(Json(summary))
}

async fn inspect_zcash_payment_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<InspectPaymentRequest>,
) -> Result<Json<zerant_zcash::zip321::PaymentRequestSummary>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    enforce_account_rate_limit(&state.db, account, "zcash_payment_inspect", 120).await?;
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
        .route("/v1/auth/wallet/verify", post(wallet_message_verify))
        .route("/v1/auth/session", get(redeem_zecauth_session))
        .route("/v1/auth/zecauth/session", get(redeem_zecauth_session))
        .route("/v1/session", get(session_info).delete(logout))
        .route("/v1/account", get(account_summary).delete(delete_account))
        .route("/v1/account/export", get(export_account))
        .route("/v1/activity", get(list_activity))
        .route("/v1/issuer", get(get_issuer_profile).post(register_issuer))
        .route("/v1/issuer/keys", get(list_issuer_keys))
        .route("/v1/issuer/keys/rotate", post(rotate_issuer_key))
        .route(
            "/v1/issuer/schemas",
            get(list_issuer_schemas).post(create_issuer_schema),
        )
        .route(
            "/v1/issuer/schemas/{schema_id}/deactivate",
            post(deactivate_issuer_schema),
        )
        .route("/v1/issuers", get(issuer_directory))
        .route(
            "/v1/verifier",
            get(get_verifier_profile).post(register_verifier),
        )
        .route("/v1/verifier/keys", get(list_verifier_keys))
        .route("/v1/verifier/keys/rotate", post(rotate_verifier_key))
        .route(
            "/v1/verifier/requests",
            get(list_verifier_requests).post(create_verification_request),
        )
        .route("/v1/holder/requests", get(list_holder_requests))
        .route(
            "/v1/holder/requests/{id}/decision",
            post(decide_holder_request),
        )
        .route(
            "/v1/issuer/credentials",
            get(list_issued_credentials).post(issue_private_credential),
        )
        .route(
            "/v1/issuer/credentials/{credential_id}/revoke",
            post(revoke_issued_credential),
        )
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
    const MIGRATION_LOCK_ID: i64 = 9_248_177_301;
    let client = db_client(pool).await?;
    client
        .query_one("SELECT pg_advisory_lock($1)", &[&MIGRATION_LOCK_ID])
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let result = async {
        for migration in [
            include_str!("../migrations/0001_server_vault.sql"),
            include_str!("../migrations/0002_zecauth.sql"),
            include_str!("../migrations/0003_zecauth_browser_redeem.sql"),
            include_str!("../migrations/0004_trust_network.sql"),
            include_str!("../migrations/0005_verification_network.sql"),
            include_str!("../migrations/0006_revocation_lifecycle.sql"),
            include_str!("../migrations/0007_trust_activity.sql"),
            include_str!("../migrations/0008_rate_limits.sql"),
            include_str!("../migrations/0009_credential_schemas.sql"),
            include_str!("../migrations/0010_account_controls.sql"),
            include_str!("../migrations/0011_pairwise_holder_keys.sql"),
            include_str!("../migrations/0012_issuer_key_lifecycle.sql"),
            include_str!("../migrations/0013_verifier_key_lifecycle.sql"),
            include_str!("../migrations/0014_wallet_message_auth.sql"),
        ] {
            client
                .batch_execute(migration)
                .await
                .map_err(|_| ApiError::Unavailable)?;
        }
        Ok(())
    }
    .await;

    let _ = client
        .query_one("SELECT pg_advisory_unlock($1)", &[&MIGRATION_LOCK_ID])
        .await;
    result
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        if let Ok(mut terminate) = signal(SignalKind::terminate()) {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {},
                _ = terminate.recv() => {},
            }
            return;
        }
    }

    let _ = tokio::signal::ctrl_c().await;
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
    let public_origin = env::var("ZERANT_PUBLIC_ORIGIN")
        .or_else(|_| {
            env::var("VERCEL_URL").map(|host| {
                if host.starts_with("http://") || host.starts_with("https://") {
                    host
                } else {
                    format!("https://{host}")
                }
            })
        })
        .map_err(|_| ApiError::Unavailable.to_string())?;
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
    axum::serve(listener, app(state))
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proof_expiry_is_bounded_by_request_and_source() {
        assert_eq!(bounded_proof_expiry(1_000, 1_500, 2_000).unwrap(), 1_300);
        assert_eq!(bounded_proof_expiry(1_000, 1_100, 2_000).unwrap(), 1_100);
        assert_eq!(bounded_proof_expiry(1_000, 1_500, 1_050).unwrap(), 1_050);
        assert!(bounded_proof_expiry(1_000, 1_000, 2_000).is_err());
        assert!(bounded_proof_expiry(1_000, 1_500, 1_000).is_err());
    }

    #[test]
    fn pairwise_holder_keys_are_distinct() {
        let (_, first) = generate_pairwise_holder_key().unwrap();
        let (_, second) = generate_pairwise_holder_key().unwrap();
        assert_ne!(first, second);
        assert_ne!(first.x, second.x);
    }

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
    fn zcash_wallet_message_signature_recovers_expected_key() {
        use secp256k1::SecretKey;

        let secp = Secp256k1::new();
        let secret = SecretKey::from_byte_array([7_u8; 32]).unwrap();
        let public = SecpPublicKey::from_secret_key(&secp, &secret);
        let message = canonical_challenge_message(
            "zerant.example",
            "https://zerant.example/app",
            "zcash:mainnet",
            "nonce1234567890123456",
            "2026-10-04T08:00:00Z",
            "2026-10-04T08:05:00Z",
            "Authenticate to Zerant.",
        );
        let digest = zcash_signed_message_hash(&message).unwrap();
        let signature = secp.sign_ecdsa_recoverable(SecpMessage::from_digest(digest), &secret);
        let (recovery_id, compact) = signature.serialize_compact();
        let mut encoded = Vec::with_capacity(65);
        encoded.push(31 + i32::from(recovery_id) as u8);
        encoded.extend_from_slice(&compact);

        let verified = verify_zcash_wallet_message(
            &hex::encode(public.serialize_uncompressed()),
            &hex::encode(&encoded),
            &message,
        )
        .unwrap();
        assert_eq!(verified, public.serialize());
        assert!(
            verify_zcash_wallet_message(
                &hex::encode(public.serialize_uncompressed()),
                &hex::encode(&encoded),
                &(message + "tampered"),
            )
            .is_err()
        );
    }

    #[test]
    fn envelope_encryption_roundtrips_and_tampering_fails() {
        let cipher = VaultCipher {
            keks: BTreeMap::from([(1, Aes256Gcm::new_from_slice(&[7_u8; 32]).unwrap())]),
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
            keks: BTreeMap::from([(1, Aes256Gcm::new_from_slice(&[9_u8; 32]).unwrap())]),
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

    #[test]
    fn envelope_key_rotation_keeps_old_records_readable() {
        let account = Uuid::from_u128(10);
        let old_id = Uuid::from_u128(11);
        let old_cipher = VaultCipher {
            keks: BTreeMap::from([(1, Aes256Gcm::new_from_slice(&[1_u8; 32]).unwrap())]),
            key_version: 1,
        };
        let encrypted = old_cipher.encrypt(account, old_id, b"old-record").unwrap();

        let row = CredentialRow {
            id: old_id,
            ciphertext: encrypted.ciphertext,
            data_nonce: encrypted.data_nonce,
            wrapped_dek: encrypted.wrapped_dek,
            wrap_nonce: encrypted.wrap_nonce,
            key_version: encrypted.key_version,
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        };

        let rotated = VaultCipher {
            keks: BTreeMap::from([
                (1, Aes256Gcm::new_from_slice(&[1_u8; 32]).unwrap()),
                (2, Aes256Gcm::new_from_slice(&[2_u8; 32]).unwrap()),
            ]),
            key_version: 2,
        };

        assert_eq!(rotated.decrypt(account, &row).unwrap(), b"old-record");

        let new_record = rotated
            .encrypt(account, Uuid::from_u128(12), b"new-record")
            .unwrap();
        assert_eq!(new_record.key_version, 2);

        let missing_old_key = VaultCipher {
            keks: BTreeMap::from([(2, Aes256Gcm::new_from_slice(&[2_u8; 32]).unwrap())]),
            key_version: 2,
        };
        assert!(missing_old_key.decrypt(account, &row).is_err());
    }
}
