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
use hmac::{Hmac, Mac};
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
    time::{Duration as StdDuration, Instant},
};
use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};
use tokio_postgres::{NoTls, Row, Transaction, config::SslMode};
use tower_http::trace::TraceLayer;
use tracing::info;
use uuid::Uuid;
use webauthn_rs::prelude::{
    DiscoverableAuthentication, DiscoverableKey, Passkey, PasskeyAuthentication,
    PasskeyRegistration, PublicKeyCredential, RegisterPublicKeyCredential, Webauthn,
    WebauthnBuilder,
};
use zerant_core::{MAX_SAFE_INTEGER, validate_origin};
use zerant_credential::{
    CREDENTIAL_SCHEMA, Claim, ClaimValue, CredentialKind, CredentialPayload, HOLDER_LOCAL_AUDIENCE,
    IssuerTrustManifest, OrdinaryClaim, PublicJwk, REVOCATION_SCHEMA, RevocationSnapshot,
    SourceEventClaim, SourceSchemaAuthorization, TrustedIssuer, TrustedIssuerKey,
    revocation_digest, sign_credential, sign_revocation_snapshot, verify_credential,
};
use zerant_disclosure::{
    Context as DisclosureContext, Decision, Evidence, REQUEST_SCHEMA, Request, VerifierPin,
    respond, sign_request, verify_request, verify_response,
};
use zerant_zcash::{
    Adapter, HttpRegtestTransport,
    lightclient::{
        LightClientNetwork, fetch_light_client_readiness, validate_light_client_endpoint,
    },
};

mod passkey_discoverable;
mod payments;
mod vault_rotation;

const SESSION_COOKIE: &str = "zerant_session";
const AUTH_ATTEMPT_COOKIE: &str = "zerant_auth_attempt";
const LINK_ATTEMPT_COOKIE: &str = "zerant_link_attempt";
const PASSKEY_ATTEMPT_COOKIE: &str = "zerant_passkey_attempt";
const MAX_CREDENTIAL_BYTES: usize = 256 * 1024;
const MAX_ACCOUNT_CREDENTIALS: i64 = 256;
const MAX_ACCOUNT_EXPORT_EVENTS: i64 = 5_000;
const SESSION_TTL_DAYS: i64 = 7;
const MAX_ACTIVE_SESSIONS_PER_ACCOUNT: i64 = 20;
const SESSION_TOUCH_INTERVAL_MINUTES: i64 = 5;
const ZECAUTH_TTL_MINUTES: i64 = 5;
const PASSKEY_TTL_MINUTES: i64 = 5;
const PASSKEY_SECURITY_REAUTH_MINUTES: i64 = 15;
const MAX_ACCOUNT_PASSKEYS: i64 = 10;
const MAX_ACTIVE_PASSKEY_CHALLENGES: i64 = 10_000;
const WRITE_RATE_WINDOW_SECONDS: i64 = 60;
const MAX_ACTIVE_ZECAUTH_CHALLENGES: i64 = 10_000;
const MAX_ACTIVE_VERIFIER_API_KEYS: i64 = 20;
const MAX_ACTIVE_VERIFIER_WEBHOOKS: i64 = 5;
const MAX_ACTIVE_VERIFICATION_POLICIES: i64 = 100;
const RETENTION_BATCH_LIMIT: i64 = 5_000;
const RETENTION_LOCK_ID: i64 = 9_248_177_302;
const MAX_WEBHOOK_ATTEMPTS: i32 = 8;
const VERIFIER_API_KEY_PREFIX: &str = "zrt_vk_";
const VERIFIER_PROOF_PACKAGE_SCHEMA: &str = "zerant.verifier-proof-package.v0.1";
const VERIFIER_WEBHOOK_SECRET_PREFIX: &str = "zrt_whsec_";

#[derive(Clone)]
struct AppState {
    db: Pool,
    cipher: Arc<VaultCipher>,
    webauthn: Arc<Webauthn>,
    public_origin: String,
    zcash_chain: String,
    light_client_endpoints: Vec<String>,
    light_client_allow_loopback: bool,
    light_client_cache: Arc<tokio::sync::Mutex<Option<LightClientCacheEntry>>>,
    allowed_scopes: BTreeSet<String>,
}

#[derive(Clone)]
struct LightClientCacheEntry {
    checked_at: Instant,
    readiness: Option<ZcashNetworkReadiness>,
}

const LIGHT_CLIENT_SUCCESS_TTL: StdDuration = StdDuration::from_secs(20);
const LIGHT_CLIENT_FAILURE_TTL: StdDuration = StdDuration::from_secs(60);
const LIGHT_CLIENT_STALE_GRACE: Duration = Duration::seconds(90);
const LIGHT_CLIENT_REFRESH_LOCK_MAINNET: i64 = 9_248_177_401;
const LIGHT_CLIENT_REFRESH_LOCK_TESTNET: i64 = 9_248_177_402;
const LIGHT_CLIENT_MAX_ROLLBACK_BLOCKS: u64 = 20;
const LIGHT_CLIENT_DEGRADED_DISPLAY_TTL: Duration = Duration::minutes(15);

impl LightClientCacheEntry {
    fn ttl(&self) -> StdDuration {
        match self.readiness.as_ref().map(|readiness| readiness.state) {
            Some(ZcashNetworkState::Ready | ZcashNetworkState::Syncing) => LIGHT_CLIENT_SUCCESS_TTL,
            Some(ZcashNetworkState::Degraded | ZcashNetworkState::NotConfigured) | None => {
                LIGHT_CLIENT_FAILURE_TTL
            }
        }
    }

    fn is_fresh(&self) -> bool {
        self.checked_at.elapsed() < self.ttl()
    }
}

#[derive(Debug, Clone)]
struct PersistedLightClientReadiness {
    checked_at: OffsetDateTime,
    readiness: Option<ZcashNetworkReadiness>,
}

impl PersistedLightClientReadiness {
    fn ttl(&self) -> Duration {
        if self.readiness.is_some() {
            Duration::seconds(i64::try_from(LIGHT_CLIENT_SUCCESS_TTL.as_secs()).unwrap_or(20))
        } else {
            Duration::seconds(i64::try_from(LIGHT_CLIENT_FAILURE_TTL.as_secs()).unwrap_or(60))
        }
    }

    fn is_fresh(&self, now: OffsetDateTime) -> bool {
        now >= self.checked_at && now - self.checked_at < self.ttl()
    }

    fn within_stale_grace(&self, now: OffsetDateTime) -> bool {
        self.readiness.is_some()
            && now >= self.checked_at
            && now - self.checked_at < LIGHT_CLIENT_STALE_GRACE
    }
}

fn light_client_configuration_fingerprint(endpoints: &[String]) -> String {
    let mut digest = Sha256::new();
    for endpoint in endpoints {
        digest.update(
            u64::try_from(endpoint.len())
                .unwrap_or(u64::MAX)
                .to_be_bytes(),
        );
        digest.update(endpoint.as_bytes());
    }
    hex::encode(digest.finalize())
}

fn parse_light_client_endpoints(
    endpoints: Option<&str>,
    legacy_endpoint: Option<&str>,
    allow_loopback_http: bool,
) -> Result<Vec<String>, ApiError> {
    if endpoints.is_some() && legacy_endpoint.is_some() {
        return Err(ApiError::Unavailable);
    }

    let Some(raw) = endpoints.or(legacy_endpoint) else {
        return Ok(Vec::new());
    };

    let values: Vec<&str> = if endpoints.is_some() {
        raw.split(',').map(str::trim).collect()
    } else {
        vec![raw.trim()]
    };

    if values.is_empty() || values.len() > 4 || values.iter().any(|value| value.is_empty()) {
        return Err(ApiError::Unavailable);
    }

    let mut validated = Vec::with_capacity(values.len());
    for value in values {
        let endpoint = validate_light_client_endpoint(value, allow_loopback_http)
            .map_err(|_| ApiError::Unavailable)?;
        if !validated.contains(&endpoint) {
            validated.push(endpoint);
        }
    }

    if validated.is_empty() || validated.len() > 4 {
        return Err(ApiError::Unavailable);
    }
    Ok(validated)
}

fn light_client_network_label(network: LightClientNetwork) -> &'static str {
    match network {
        LightClientNetwork::Mainnet => "mainnet",
        LightClientNetwork::Testnet => "testnet",
    }
}

fn light_client_refresh_lock(network: LightClientNetwork) -> i64 {
    match network {
        LightClientNetwork::Mainnet => LIGHT_CLIENT_REFRESH_LOCK_MAINNET,
        LightClientNetwork::Testnet => LIGHT_CLIENT_REFRESH_LOCK_TESTNET,
    }
}

fn light_client_height_is_acceptable(candidate: u64, high_water: Option<u64>) -> bool {
    match high_water {
        None => true,
        Some(high_water) => candidate
            .checked_add(LIGHT_CLIENT_MAX_ROLLBACK_BLOCKS)
            .is_some_and(|bounded| bounded >= high_water),
    }
}

trait VaultKeyProvider: Send + Sync {
    fn active_version(&self) -> i32;
    fn has_version(&self, version: i32) -> bool;
    fn wrap(
        &self,
        version: i32,
        nonce: &[u8],
        plaintext: &[u8],
        aad: &[u8],
    ) -> Result<Vec<u8>, ApiError>;
    fn unwrap(
        &self,
        version: i32,
        nonce: &[u8],
        ciphertext: &[u8],
        aad: &[u8],
    ) -> Result<Vec<u8>, ApiError>;
}

struct LocalVaultKeyProvider {
    keks: BTreeMap<i32, Aes256Gcm>,
    active_version: i32,
}

impl VaultKeyProvider for LocalVaultKeyProvider {
    fn active_version(&self) -> i32 {
        self.active_version
    }

    fn has_version(&self, version: i32) -> bool {
        self.keks.contains_key(&version)
    }

    fn wrap(
        &self,
        version: i32,
        nonce: &[u8],
        plaintext: &[u8],
        aad: &[u8],
    ) -> Result<Vec<u8>, ApiError> {
        if nonce.len() != 12 {
            return Err(ApiError::Unavailable);
        }
        let key = self.keks.get(&version).ok_or(ApiError::Unavailable)?;
        key.encrypt(
            nonce.into(),
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| ApiError::Unavailable)
    }

    fn unwrap(
        &self,
        version: i32,
        nonce: &[u8],
        ciphertext: &[u8],
        aad: &[u8],
    ) -> Result<Vec<u8>, ApiError> {
        if nonce.len() != 12 {
            return Err(ApiError::Unavailable);
        }
        let key = self.keks.get(&version).ok_or(ApiError::Unavailable)?;
        key.decrypt(
            nonce.into(),
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map_err(|_| ApiError::Unavailable)
    }
}

struct VaultCipher {
    keys: Arc<dyn VaultKeyProvider>,
}

#[derive(Debug, thiserror::Error)]
enum ApiError {
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
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
            Self::Forbidden => StatusCode::FORBIDDEN,
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
struct RetentionMaintenanceSummary {
    skipped: bool,
    expired_requests: u64,
    sessions: u64,
    zecauth_challenges: u64,
    passkey_challenges: u64,
    rate_limits: u64,
    webhook_deliveries: u64,
    proof_material: u64,
    expired_payments: u64,
    deleted_expired_payments: u64,
}

#[derive(Serialize)]
struct Health {
    status: &'static str,
    storage: &'static str,
    authentication: &'static str,
    zcash_boundary: &'static str,
}

#[derive(Serialize)]
struct OperationalZcashHealth {
    configured: bool,
    network: String,
    available: Option<bool>,
    synced: Option<bool>,
    checked_at: Option<OffsetDateTime>,
    last_success_at: Option<OffsetDateTime>,
    stale: bool,
}

#[derive(Serialize)]
struct OperationalHealthSnapshot {
    healthy: bool,
    attention_required: bool,
    pending_verifications: i64,
    overdue_verifications: i64,
    webhook_pending: i64,
    webhook_dead: i64,
    maintenance_last_success_at: Option<OffsetDateTime>,
    maintenance_stale: bool,
    zcash: OperationalZcashHealth,
}

fn operational_health_flags(
    maintenance_stale: bool,
    zcash_unhealthy: bool,
    overdue_verifications: i64,
    webhook_pending: i64,
    webhook_dead: i64,
) -> (bool, bool) {
    let attention_required = maintenance_stale
        || zcash_unhealthy
        || overdue_verifications > 0
        || webhook_dead > 0
        || webhook_pending > 100;
    (!attention_required, attention_required)
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
    expired: bool,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

fn credential_expired_at(credential: &Value, now: OffsetDateTime) -> bool {
    credential
        .get("expires_at")
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_u64().and_then(|raw| i64::try_from(raw).ok()))
        })
        .is_some_and(|expires_at| expires_at <= now.unix_timestamp())
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
    passkey_count: i64,
    issuer_profile: Option<String>,
    issuer_role: Option<String>,
    issuer_retired: bool,
    verifier_profile: Option<String>,
    verifier_retired: bool,
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
    payments: Vec<payments::PaymentExportView>,
    payments_complete: bool,
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

#[derive(Clone, Deserialize)]
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
struct LinkedZcashMethod {
    method: &'static str,
    chain: Option<String>,
    created_at: OffsetDateTime,
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

#[derive(Serialize)]
struct SessionView {
    id: Uuid,
    auth_method: String,
    current: bool,
    created_at: OffsetDateTime,
    last_seen_at: OffsetDateTime,
    expires_at: OffsetDateTime,
}

#[derive(Serialize)]
struct SessionRevokeResult {
    revoked: u64,
}

#[derive(Serialize)]
struct PasskeyRegistrationStart {
    zerant_id: String,
    public_key: webauthn_rs::prelude::CreationChallengeResponse,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PasskeyAuthenticationStart {
    zerant_id: String,
}

#[derive(Serialize)]
struct PasskeyAuthenticationStartResponse {
    public_key: webauthn_rs::prelude::RequestChallengeResponse,
}

#[derive(Serialize)]
struct PasskeyView {
    id: Uuid,
    last_used_at: Option<OffsetDateTime>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegisterIssuer {
    display_name: String,
}

#[derive(Serialize)]
struct PublicIssuerKeyView {
    key_id: String,
    public_jwk: PublicJwk,
    valid_from: OffsetDateTime,
    retired_at: Option<OffsetDateTime>,
    compromised_at: Option<OffsetDateTime>,
}

#[derive(Serialize)]
struct PublicCredentialSchemaView {
    id: Uuid,
    display_name: String,
    description: String,
    claim_type: String,
    context: String,
    default_expiry_days: i32,
    version: i32,
    active: bool,
    supersedes_schema_id: Option<Uuid>,
    retired_at: Option<OffsetDateTime>,
    created_at: OffsetDateTime,
}

#[derive(Serialize)]
struct PublicIssuerMetadata {
    issuer_id: String,
    display_name: String,
    keys: Vec<PublicIssuerKeyView>,
    credential_schemas: Vec<PublicCredentialSchemaView>,
    revocation_version: u64,
}

#[derive(Serialize)]
struct PublicRevocationPublication {
    issuer_id: String,
    version: u64,
    issuer_key_id: String,
    snapshot_jws: String,
    issued_at: OffsetDateTime,
    next_update: OffsetDateTime,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PublicIssuerDirectoryQuery {
    limit: Option<u16>,
    cursor: Option<String>,
}

#[derive(Clone, Serialize)]
struct PublicIssuerDirectoryItem {
    issuer_id: String,
    display_name: String,
    active_key_id: Option<String>,
    active_schema_count: u32,
    revocation_version: u64,
    created_at: OffsetDateTime,
    #[serde(skip)]
    profile_id: Uuid,
}

#[derive(Serialize)]
struct PublicIssuerDirectoryPage {
    items: Vec<PublicIssuerDirectoryItem>,
    next_cursor: Option<String>,
}

#[derive(Serialize)]
struct IssuerProfileView {
    display_name: String,
    issuer_id: String,
    retired_at: Option<OffsetDateTime>,
    created_at: OffsetDateTime,
}

#[derive(Clone)]
struct IssuerAccess {
    profile_id: Uuid,
    owner_account_id: Uuid,
    role: String,
    retired_at: Option<OffsetDateTime>,
}

#[derive(Serialize)]
struct IssuerMemberView {
    zerant_id: String,
    role: String,
    owner: bool,
    joined_at: OffsetDateTime,
}

#[derive(Serialize)]
struct IssuerInvitationView {
    id: Uuid,
    issuer_name: String,
    issuer_id: String,
    invited_zerant_id: String,
    role: String,
    status: String,
    created_at: OffsetDateTime,
    expires_at: OffsetDateTime,
}

#[derive(Clone, Serialize)]
struct IssuerActivityEventView {
    id: i64,
    event_type: String,
    actor_zerant_id: String,
    object_id: String,
    label: String,
    context: Option<String>,
    counterparty: Option<String>,
    created_at: OffsetDateTime,
}

#[derive(Serialize)]
struct IssuerActivityPage {
    items: Vec<IssuerActivityEventView>,
    next_cursor: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InviteIssuerMember {
    zerant_id: String,
    role: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DecideIssuerInvitation {
    decision: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TransferIssuerOwnership {
    zerant_id: String,
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateCredentialSchemaVersion {
    description: String,
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
    version: i32,
    active: bool,
    supersedes_schema_id: Option<Uuid>,
    retired_at: Option<OffsetDateTime>,
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
    retired_at: Option<OffsetDateTime>,
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateVerifierApiKey {
    name: String,
    scopes: Vec<String>,
    expires_in_days: Option<u16>,
}

#[derive(Serialize)]
struct VerifierApiKeyView {
    id: Uuid,
    name: String,
    key_prefix: String,
    scopes: Vec<String>,
    created_at: OffsetDateTime,
    last_used_at: Option<OffsetDateTime>,
    expires_at: Option<OffsetDateTime>,
    revoked: bool,
}

#[derive(Serialize)]
struct CreatedVerifierApiKey {
    key: VerifierApiKeyView,
    secret: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateVerifierWebhook {
    name: String,
    url: String,
}

#[derive(Serialize)]
struct VerifierWebhookView {
    id: Uuid,
    name: String,
    url: String,
    created_at: OffsetDateTime,
    last_delivery_at: Option<OffsetDateTime>,
    disabled: bool,
    pending_deliveries: i64,
    dead_deliveries: i64,
}

#[derive(Serialize)]
struct CreatedVerifierWebhook {
    webhook: VerifierWebhookView,
    secret: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateVerificationPolicy {
    display_name: String,
    description: String,
    purpose: String,
    credential_schema_id: Uuid,
    request_ttl_seconds: Option<u16>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateVerificationPolicyVersion {
    description: String,
    purpose: String,
    credential_schema_id: Uuid,
    request_ttl_seconds: u16,
}

#[derive(Serialize)]
struct VerificationPolicyView {
    id: Uuid,
    display_name: String,
    description: String,
    purpose: String,
    credential_schema_id: Uuid,
    credential_name: String,
    issuer_name: String,
    request_ttl_seconds: i32,
    version: i32,
    active: bool,
    supersedes_policy_id: Option<Uuid>,
    retired_at: Option<OffsetDateTime>,
    created_at: OffsetDateTime,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreatePolicyVerificationRequest {
    holder_zerant_id: String,
}

struct ResolvedVerificationPolicy {
    id: Uuid,
    purpose: String,
    credential_schema_id: Uuid,
    request_ttl_seconds: u16,
}

#[derive(Serialize)]
struct WebhookDispatchSummary {
    claimed: usize,
    delivered: usize,
    retried: usize,
    dead: usize,
    pending: i64,
    finalized: bool,
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
    #[serde(default)]
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
struct HolderRequestPreview {
    request: HolderRequestView,
    issuer_id: String,
    issuer_name: String,
    value: String,
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

#[derive(Serialize)]
struct ProofCredentialSchemaView {
    id: Uuid,
    display_name: String,
    version: i32,
    claim_type: String,
    context: String,
}

#[derive(Serialize)]
struct ProofSigningKeyView {
    key_id: String,
    public_jwk: PublicJwk,
    valid_from: OffsetDateTime,
    retired_at: Option<OffsetDateTime>,
    compromised_at: Option<OffsetDateTime>,
}

#[derive(Serialize)]
struct VerifierProofPackage {
    schema: &'static str,
    request_id: Uuid,
    protocol_request_id: String,
    verifier_origin: String,
    credential_schema: Option<ProofCredentialSchemaView>,
    issuer_id: String,
    issuer_key: ProofSigningKeyView,
    verifier_id: String,
    verifier_key: ProofSigningKeyView,
    request_jws: String,
    response_jws: String,
    revocation_jws: String,
    decided_at: OffsetDateTime,
    proof_expires_at: OffsetDateTime,
    verified_result: VerifiedProofResultView,
}

#[derive(Serialize)]
struct VerifiedProofResultView {
    claim_type: String,
    value: Value,
    context: Option<String>,
}

#[derive(Serialize)]
struct VerifierHumanProofView {
    request_id: Uuid,
    verifier_origin: String,
    credential_name: Option<String>,
    credential_version: Option<i32>,
    issuer_id: String,
    claim_type: String,
    value: Value,
    context: Option<String>,
    decided_at: OffsetDateTime,
    proof_expires_at: OffsetDateTime,
}

fn verified_proof_result(claim: &Claim) -> Result<VerifiedProofResultView, ApiError> {
    match claim {
        Claim::Ordinary(claim) => {
            let value = match &claim.value {
                ClaimValue::String(value) => Value::String(value.clone()),
                ClaimValue::Boolean(value) => Value::Bool(*value),
                ClaimValue::Integer(value) => {
                    if value.unsigned_abs() > MAX_SAFE_INTEGER {
                        return Err(ApiError::Unavailable);
                    }
                    Value::Number((*value).into())
                }
            };
            Ok(VerifiedProofResultView {
                claim_type: claim.claim_type.clone(),
                value,
                context: claim.context.clone(),
            })
        }
        Claim::Threshold(claim) => Ok(VerifiedProofResultView {
            claim_type: claim.claim_type.clone(),
            value: Value::Bool(claim.value),
            context: Some(claim.context.clone()),
        }),
        Claim::Source(_) => Err(ApiError::Unavailable),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DecideRequest {
    decision: String,
    expected_issuer_id: Option<String>,
    expected_value: Option<String>,
}

fn approval_matches_preview(input: &DecideRequest, issuer_id: &str, value: &str) -> bool {
    input.decision == "approve"
        && input.expected_issuer_id.as_deref() == Some(issuer_id)
        && input.expected_value.as_deref() == Some(value)
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
struct CreatePaymentRequest {
    recipient: String,
    amount_zec: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InspectAddress {
    address: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ZcashNetworkState {
    Ready,
    Syncing,
    Degraded,
    NotConfigured,
}

#[derive(Debug, Clone, Serialize)]
struct ZcashNetworkReadiness {
    configured: bool,
    network: String,
    state: ZcashNetworkState,
    network_actions_enabled: bool,
    synced: bool,
    block_height: Option<u64>,
    estimated_height: Option<u64>,
    lag: Option<u64>,
    last_confirmed_at: Option<String>,
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

impl LocalVaultKeyProvider {
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
        let key_version = match env::var("ZERANT_VAULT_KEY_VERSION") {
            Ok(value) => value
                .parse::<i32>()
                .ok()
                .filter(|value| *value > 0)
                .ok_or(ApiError::Unavailable)?,
            Err(env::VarError::NotPresent) => 1,
            Err(_) => return Err(ApiError::Unavailable),
        };

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

        Ok(Self {
            keks,
            active_version: key_version,
        })
    }
}

impl VaultCipher {
    fn from_env() -> Result<Self, ApiError> {
        Ok(Self {
            keys: Arc::new(LocalVaultKeyProvider::from_env()?),
        })
    }

    #[cfg(test)]
    fn from_local_keys(
        keks: BTreeMap<i32, Aes256Gcm>,
        active_version: i32,
    ) -> Result<Self, ApiError> {
        if active_version <= 0 || !keks.contains_key(&active_version) {
            return Err(ApiError::Unavailable);
        }
        Ok(Self {
            keys: Arc::new(LocalVaultKeyProvider {
                keks,
                active_version,
            }),
        })
    }

    fn active_key_version(&self) -> i32 {
        self.keys.active_version()
    }

    fn has_key_version(&self, version: i32) -> bool {
        self.keys.has_version(version)
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
        let key_version = self.active_key_version();
        let data_aad = Self::data_aad(account_id, credential_id, key_version);
        let key_aad = Self::key_aad(account_id, credential_id, key_version);

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
            .keys
            .wrap(key_version, &wrap_nonce, &dek_bytes, &key_aad)?;

        dek_bytes.fill(0);
        Ok(EncryptedCredential {
            ciphertext,
            data_nonce: data_nonce.to_vec(),
            wrapped_dek,
            wrap_nonce: wrap_nonce.to_vec(),
            key_version,
        })
    }

    fn decrypt(&self, account_id: Uuid, row: &CredentialRow) -> Result<Vec<u8>, ApiError> {
        if row.data_nonce.len() != 12 || row.wrap_nonce.len() != 12 {
            return Err(ApiError::Unavailable);
        }

        let key_aad = Self::key_aad(account_id, row.id, row.key_version);
        let mut dek_bytes =
            self.keys
                .unwrap(row.key_version, &row.wrap_nonce, &row.wrapped_dek, &key_aad)?;

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

async fn create_account_session(
    tx: &Transaction<'_>,
    account: Uuid,
    scopes: &[String],
    auth_method: &str,
) -> Result<String, ApiError> {
    if !matches!(auth_method, "legacy" | "passkey" | "zcash") {
        return Err(ApiError::Unavailable);
    }

    tx.query_one(
        "SELECT id FROM accounts WHERE id = $1 FOR UPDATE",
        &[&account],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;

    tx.execute(
        "DELETE FROM sessions
         WHERE account_id = $1 AND expires_at <= NOW()",
        &[&account],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;

    let active: i64 = tx
        .query_one(
            "SELECT COUNT(*) FROM sessions
             WHERE account_id = $1 AND expires_at > NOW()",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .get(0);

    if active >= MAX_ACTIVE_SESSIONS_PER_ACCOUNT {
        let remove_count = active
            .checked_sub(MAX_ACTIVE_SESSIONS_PER_ACCOUNT)
            .and_then(|value| value.checked_add(1))
            .ok_or(ApiError::Unavailable)?;
        tx.execute(
            "DELETE FROM sessions
             WHERE id IN (
                 SELECT id FROM sessions
                 WHERE account_id = $1 AND expires_at > NOW()
                 ORDER BY last_seen_at ASC, created_at ASC
                 LIMIT $2
             )",
            &[&account, &remove_count],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    }

    let mut token_bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut token_bytes);
    let token = URL_SAFE_NO_PAD.encode(token_bytes);
    token_bytes.fill(0);
    let token_hash = Sha256::digest(token.as_bytes()).to_vec();
    let expires = OffsetDateTime::now_utc() + Duration::days(SESSION_TTL_DAYS);
    let scopes_json = serde_json::to_value(scopes).map_err(|_| ApiError::Unavailable)?;

    tx.execute(
        "INSERT INTO sessions
         (id, account_id, token_hash, scopes, auth_method, expires_at, last_seen_at)
         VALUES ($1, $2, $3, $4, $5, $6, NOW())",
        &[
            &Uuid::new_v4(),
            &account,
            &token_hash,
            &scopes_json,
            &auth_method,
            &expires,
        ],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;
    Ok(token)
}

fn session_cookie_header(token: &str) -> String {
    format!(
        "{SESSION_COOKIE}={token}; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age={}",
        SESSION_TTL_DAYS * 24 * 60 * 60
    )
}

fn clear_cookie_header(name: &str) -> String {
    format!("{name}=; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age=0")
}

fn normalize_verifier_api_scopes(mut scopes: Vec<String>) -> Result<Vec<String>, ApiError> {
    if scopes.is_empty() || scopes.len() > 3 {
        return Err(ApiError::Invalid);
    }
    scopes.sort();
    scopes.dedup();
    if scopes.is_empty()
        || scopes.iter().any(|scope| {
            !matches!(
                scope.as_str(),
                "requests:create" | "requests:read" | "proofs:read"
            )
        })
    {
        return Err(ApiError::Invalid);
    }
    Ok(scopes)
}

fn generate_verifier_api_secret() -> String {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    let encoded = URL_SAFE_NO_PAD.encode(bytes);
    bytes.fill(0);
    format!("{VERIFIER_API_KEY_PREFIX}{encoded}")
}

fn generate_webhook_secret() -> String {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    let encoded = URL_SAFE_NO_PAD.encode(bytes);
    bytes.fill(0);
    format!("{VERIFIER_WEBHOOK_SECRET_PREFIX}{encoded}")
}

fn parse_webhook_url(input: &str) -> Result<url::Url, ApiError> {
    if input.len() > 2048 {
        return Err(ApiError::Invalid);
    }
    let url = url::Url::parse(input).map_err(|_| ApiError::Invalid)?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || url.query().is_some()
        || url.port_or_known_default() != Some(443)
    {
        return Err(ApiError::Invalid);
    }
    let host = url.host_str().ok_or(ApiError::Invalid)?;
    if host.eq_ignore_ascii_case("localhost") || host.ends_with(".localhost") {
        return Err(ApiError::Invalid);
    }
    Ok(url)
}

fn webhook_ip_is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            !(a == 0
                || a == 10
                || a == 127
                || (a == 100 && (64..=127).contains(&b))
                || (a == 169 && b == 254)
                || (a == 172 && (16..=31).contains(&b))
                || (a == 192 && b == 168)
                || (a == 192 && b == 0 && c == 0)
                || (a == 192 && b == 0 && c == 2)
                || (a == 198 && (b == 18 || b == 19))
                || (a == 198 && b == 51 && c == 100)
                || (a == 203 && b == 0 && c == 113)
                || a >= 224)
        }
        IpAddr::V6(ip) => {
            if let Some(v4) = ip.to_ipv4_mapped() {
                return webhook_ip_is_public(IpAddr::V4(v4));
            }
            let segments = ip.segments();
            !(ip.is_unspecified()
                || ip.is_loopback()
                || ip.is_multicast()
                || (segments[0] & 0xfe00) == 0xfc00
                || (segments[0] & 0xffc0) == 0xfe80
                || (segments[0] == 0x2001 && segments[1] == 0x0db8))
        }
    }
}

async fn webhook_public_addrs(url: &url::Url) -> Result<(String, Vec<SocketAddr>), ApiError> {
    let host = url.host_str().ok_or(ApiError::Invalid)?.to_owned();
    let mut addrs: Vec<SocketAddr> = tokio::net::lookup_host((host.as_str(), 443))
        .await
        .map_err(|_| ApiError::Unavailable)?
        .collect();
    addrs.sort();
    addrs.dedup();
    if addrs.is_empty() || addrs.iter().any(|addr| !webhook_ip_is_public(addr.ip())) {
        return Err(ApiError::Invalid);
    }
    Ok((host, addrs))
}

fn webhook_signature(secret: &[u8], timestamp: i64, body: &[u8]) -> Result<String, ApiError> {
    let mut mac =
        <Hmac<Sha256> as Mac>::new_from_slice(secret).map_err(|_| ApiError::Unavailable)?;
    mac.update(timestamp.to_string().as_bytes());
    mac.update(b".");
    mac.update(body);
    Ok(format!("v1={}", hex::encode(mac.finalize().into_bytes())))
}

fn webhook_retry_seconds(attempt_count: i32) -> i64 {
    match attempt_count {
        0 | 1 => 60,
        2 => 300,
        3 => 900,
        4 => 3_600,
        5 => 21_600,
        6 => 43_200,
        _ => 86_400,
    }
}

fn bearer_token(headers: &HeaderMap) -> Result<&str, ApiError> {
    let header = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or(ApiError::Unauthorized)?;
    let token = header
        .strip_prefix("Bearer ")
        .filter(|value| value.starts_with(VERIFIER_API_KEY_PREFIX))
        .filter(|value| (48..=128).contains(&value.len()))
        .ok_or(ApiError::Unauthorized)?;
    if token.chars().any(char::is_whitespace) {
        return Err(ApiError::Unauthorized);
    }
    Ok(token)
}

async fn verifier_api_account_id(
    headers: &HeaderMap,
    db: &Pool,
    required_scope: &str,
) -> Result<Uuid, ApiError> {
    if !matches!(
        required_scope,
        "requests:create" | "requests:read" | "proofs:read"
    ) {
        return Err(ApiError::Unavailable);
    }
    let token = bearer_token(headers)?;
    let hash = Sha256::digest(token.as_bytes()).to_vec();
    let client = db_client(db).await?;
    let row = client
        .query_opt(
            "WITH matched AS (
                 SELECT k.id, v.account_id, k.scopes, k.last_used_at
                 FROM verifier_api_keys k
                 JOIN verifier_profiles v ON v.id = k.verifier_profile_id
                 WHERE k.token_hash = $1
                   AND v.retired_at IS NULL
                   AND v.account_id IS NOT NULL
                   AND k.revoked_at IS NULL
                   AND (k.expires_at IS NULL OR k.expires_at > NOW())
             ), touched AS (
                 UPDATE verifier_api_keys k
                 SET last_used_at = NOW()
                 FROM matched m
                 WHERE k.id = m.id
                   AND (m.last_used_at IS NULL OR m.last_used_at < NOW() - INTERVAL '5 minutes')
                 RETURNING k.id
             )
             SELECT m.account_id, m.scopes
             FROM matched m
             LEFT JOIN touched t ON t.id = m.id",
            &[&hash],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::Unauthorized)?;
    let scopes_value: Value = row.get(1);
    let scopes: Vec<String> =
        serde_json::from_value(scopes_value).map_err(|_| ApiError::Unavailable)?;
    if !scopes.iter().any(|scope| scope == required_scope) {
        return Err(ApiError::Unauthorized);
    }
    Ok(row.get(0))
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
            "WITH found AS (
                 SELECT id, account_id
                 FROM sessions
                 WHERE token_hash = $1 AND expires_at > NOW()
             ),
             touched AS (
                 UPDATE sessions s
                 SET last_seen_at = NOW()
                 FROM found f
                 WHERE s.id = f.id
                   AND s.last_seen_at < NOW() - ($2::bigint * INTERVAL '1 minute')
                 RETURNING s.id
             )
             SELECT account_id FROM found",
            &[&hash, &SESSION_TOUCH_INTERVAL_MINUTES],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .map(|row| row.get(0))
        .ok_or(ApiError::Unauthorized)
}

async fn recent_account_id(headers: &HeaderMap, db: &Pool) -> Result<Uuid, ApiError> {
    let token = session_token(headers)?;
    let hash = Sha256::digest(token.as_bytes()).to_vec();
    let client = db_client(db).await?;
    let row = client
        .query_opt(
            "WITH found AS (
                 SELECT id, account_id, created_at
                 FROM sessions
                 WHERE token_hash = $1 AND expires_at > NOW()
             ),
             touched AS (
                 UPDATE sessions s
                 SET last_seen_at = NOW()
                 FROM found f
                 WHERE s.id = f.id
                   AND s.last_seen_at < NOW() - ($3::bigint * INTERVAL '1 minute')
                 RETURNING s.id
             )
             SELECT account_id,
                    created_at > NOW() - ($2::bigint * INTERVAL '1 minute') AS recent
             FROM found",
            &[
                &hash,
                &PASSKEY_SECURITY_REAUTH_MINUTES,
                &SESSION_TOUCH_INTERVAL_MINUTES,
            ],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::Unauthorized)?;
    let recent: bool = row.get(1);
    if !recent {
        return Err(ApiError::Forbidden);
    }
    Ok(row.get(0))
}

async fn issuer_access(db: &Pool, account: Uuid) -> Result<IssuerAccess, ApiError> {
    let client = db_client(db).await?;
    let rows = client
        .query(
            "SELECT p.id, p.account_id, p.retired_at,
                    CASE WHEN p.account_id = $1 THEN 'owner' ELSE m.role END AS role
             FROM issuer_profiles p
             LEFT JOIN issuer_members m
               ON m.issuer_profile_id = p.id
              AND m.account_id = $1
             WHERE p.account_id = $1 OR m.account_id = $1
             ORDER BY CASE WHEN p.account_id = $1 THEN 0 ELSE 1 END, p.created_at ASC
             LIMIT 2",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    if rows.is_empty() {
        return Err(ApiError::NotFound);
    }
    if rows.len() != 1 {
        return Err(ApiError::Conflict);
    }
    let row = &rows[0];
    Ok(IssuerAccess {
        profile_id: row.get(0),
        owner_account_id: row.get(1),
        retired_at: row.get(2),
        role: row.get(3),
    })
}

fn require_issuer_role(access: &IssuerAccess, allowed: &[&str]) -> Result<(), ApiError> {
    if allowed.iter().any(|role| *role == access.role) {
        Ok(())
    } else {
        Err(ApiError::Forbidden)
    }
}

fn require_active_issuer(access: &IssuerAccess) -> Result<(), ApiError> {
    if access.retired_at.is_some() {
        Err(ApiError::Conflict)
    } else {
        Ok(())
    }
}

fn valid_issuer_member_role(role: &str) -> bool {
    matches!(role, "admin" | "issuer" | "auditor")
}

struct IssuerEvent<'a> {
    event_type: &'a str,
    object_id: &'a str,
    label: &'a str,
    context: Option<&'a str>,
    counterparty: Option<&'a str>,
}

async fn record_issuer_event(
    tx: &Transaction<'_>,
    issuer_profile_id: Uuid,
    actor_account_id: Uuid,
    event: IssuerEvent<'_>,
) -> Result<(), ApiError> {
    let actor_row = tx
        .query_one(
            "SELECT public_handle FROM accounts WHERE id = $1",
            &[&actor_account_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let actor_handle: Option<String> = actor_row.get(0);
    let actor_handle = actor_handle.ok_or(ApiError::Unavailable)?;
    let context = event.context.map(str::to_owned);
    let counterparty = event.counterparty.map(str::to_owned);

    tx.execute(
        "INSERT INTO issuer_events
         (issuer_profile_id, actor_account_id, actor_zerant_id, event_type,
          object_id, label, context, counterparty)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
        &[
            &issuer_profile_id,
            &actor_account_id,
            &actor_handle,
            &event.event_type,
            &event.object_id,
            &event.label,
            &context,
            &counterparty,
        ],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;
    Ok(())
}

async fn list_issuer_activity(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ActivityQuery>,
) -> Result<Json<IssuerActivityPage>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    let access = issuer_access(&state.db, account).await?;
    let limit = usize::from(query.limit.unwrap_or(20).clamp(1, 100));
    let fetch_limit = i64::try_from(limit + 1).map_err(|_| ApiError::Invalid)?;
    let client = db_client(&state.db).await?;

    let rows = if let Some(cursor) = query.cursor.as_deref() {
        let (created_at, id) = parse_activity_cursor(cursor)?;
        client
            .query(
                "SELECT id, event_type, actor_zerant_id, object_id, label,
                        context, counterparty, created_at
                 FROM issuer_events
                 WHERE issuer_profile_id = $1
                   AND (created_at, id) < ($2, $3)
                 ORDER BY created_at DESC, id DESC
                 LIMIT $4",
                &[&access.profile_id, &created_at, &id, &fetch_limit],
            )
            .await
            .map_err(|_| ApiError::Unavailable)?
    } else {
        client
            .query(
                "SELECT id, event_type, actor_zerant_id, object_id, label,
                        context, counterparty, created_at
                 FROM issuer_events
                 WHERE issuer_profile_id = $1
                 ORDER BY created_at DESC, id DESC
                 LIMIT $2",
                &[&access.profile_id, &fetch_limit],
            )
            .await
            .map_err(|_| ApiError::Unavailable)?
    };

    let has_more = rows.len() > limit;
    let mut items = Vec::with_capacity(limit.min(rows.len()));
    for row in rows.into_iter().take(limit) {
        items.push(IssuerActivityEventView {
            id: row.get(0),
            event_type: row.get(1),
            actor_zerant_id: row.get(2),
            object_id: row.get(3),
            label: row.get(4),
            context: row.get(5),
            counterparty: row.get(6),
            created_at: row.get(7),
        });
    }

    let next_cursor = if has_more {
        items
            .last()
            .map(|item| activity_cursor(item.created_at, item.id))
    } else {
        None
    };

    Ok(Json(IssuerActivityPage { items, next_cursor }))
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
        authentication: "zecauth-passkey",
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

async fn persist_passkey_challenge(
    db: &Pool,
    account: Uuid,
    kind: &str,
    state_value: Value,
) -> Result<(String, OffsetDateTime), ApiError> {
    if !matches!(
        kind,
        "register" | "authenticate" | "attach" | "discoverable"
    ) {
        return Err(ApiError::Unavailable);
    }

    let mut attempt_bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut attempt_bytes);
    let attempt = URL_SAFE_NO_PAD.encode(attempt_bytes);
    attempt_bytes.fill(0);
    let attempt_hash = Sha256::digest(attempt.as_bytes()).to_vec();
    let expires = OffsetDateTime::now_utc() + Duration::minutes(PASSKEY_TTL_MINUTES);

    const PASSKEY_CHALLENGE_LOCK_ID: i64 = 9_248_177_302;
    let mut client = db_client(db).await?;
    let tx = client
        .transaction()
        .await
        .map_err(|_| ApiError::Unavailable)?;
    tx.query_one(
        "SELECT pg_advisory_xact_lock($1)",
        &[&PASSKEY_CHALLENGE_LOCK_ID],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;

    tx.execute(
        "DELETE FROM passkey_challenges
         WHERE expires_at <= NOW() - INTERVAL '1 day'
            OR (consumed_at IS NOT NULL AND consumed_at <= NOW() - INTERVAL '1 day')",
        &[],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;

    let active: i64 = tx
        .query_one(
            "SELECT COUNT(*) FROM passkey_challenges
             WHERE consumed_at IS NULL AND expires_at > NOW()",
            &[],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .get(0);
    if active >= MAX_ACTIVE_PASSKEY_CHALLENGES {
        return Err(ApiError::TooManyRequests);
    }

    tx.execute(
        "INSERT INTO passkey_challenges
         (id, attempt_hash, account_id, kind, state, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6)",
        &[
            &Uuid::new_v4(),
            &attempt_hash,
            &account,
            &kind,
            &state_value,
            &expires,
        ],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;
    tx.commit().await.map_err(|_| ApiError::Unavailable)?;

    Ok((attempt, expires))
}

fn passkey_attempt_cookie(attempt: &str) -> String {
    format!(
        "{PASSKEY_ATTEMPT_COOKIE}={attempt}; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age={}",
        PASSKEY_TTL_MINUTES * 60
    )
}

async fn passkey_registration_start(State(state): State<AppState>) -> Result<Response, ApiError> {
    let account = Uuid::new_v4();
    let zerant_id = zerant_public_handle(account.as_bytes());
    let (public_key, registration) = state
        .webauthn
        .start_passkey_registration(account, &zerant_id, "Zerant account", None)
        .map_err(|_| ApiError::Unavailable)?;
    let registration_state =
        serde_json::to_value(registration).map_err(|_| ApiError::Unavailable)?;

    let (attempt, _) =
        persist_passkey_challenge(&state.db, account, "register", registration_state).await?;
    let cookie = passkey_attempt_cookie(&attempt);
    let mut response = Json(PasskeyRegistrationStart {
        zerant_id,
        public_key,
    })
    .into_response();
    response.headers_mut().append(
        SET_COOKIE,
        HeaderValue::from_str(&cookie).map_err(|_| ApiError::Unavailable)?,
    );
    Ok(response)
}

async fn passkey_registration_finish(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<RegisterPublicKeyCredential>,
) -> Result<Response, ApiError> {
    let attempt = cookie_value(&headers, PASSKEY_ATTEMPT_COOKIE)?;
    let attempt_hash = Sha256::digest(attempt.as_bytes()).to_vec();
    let mut client = db_client(&state.db).await?;
    let tx = client
        .transaction()
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let row = tx
        .query_opt(
            "SELECT account_id, state
             FROM passkey_challenges
             WHERE attempt_hash = $1
               AND kind = 'register'
               AND consumed_at IS NULL
               AND expires_at > NOW()
             FOR UPDATE",
            &[&attempt_hash],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::Unauthorized)?;
    let account: Uuid = row.get(0);
    let state_value: Value = row.get(1);
    let registration: PasskeyRegistration =
        serde_json::from_value(state_value).map_err(|_| ApiError::Unavailable)?;
    let passkey = state
        .webauthn
        .finish_passkey_registration(&input, &registration)
        .map_err(|_| ApiError::Unauthorized)?;
    let credential_id = URL_SAFE_NO_PAD.encode(passkey.cred_id().as_ref());

    if tx
        .query_opt(
            "SELECT 1 FROM passkey_credentials WHERE credential_id = $1",
            &[&credential_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .is_some()
    {
        return Err(ApiError::Conflict);
    }

    let zerant_id = zerant_public_handle(account.as_bytes());
    tx.execute(
        "INSERT INTO accounts(id, public_handle) VALUES ($1, $2)",
        &[&account, &zerant_id],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;
    let passkey_value = serde_json::to_value(&passkey).map_err(|_| ApiError::Unavailable)?;
    tx.execute(
        "INSERT INTO passkey_credentials(id, account_id, credential_id, passkey)
         VALUES ($1, $2, $3, $4)",
        &[&Uuid::new_v4(), &account, &credential_id, &passkey_value],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;

    tx.execute(
        "UPDATE passkey_challenges SET consumed_at = NOW() WHERE attempt_hash = $1",
        &[&attempt_hash],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;

    let scopes = vec!["auth".to_owned()];
    let token = create_account_session(&tx, account, &scopes, "passkey").await?;
    tx.commit().await.map_err(|_| ApiError::Unavailable)?;

    let mut response = Json(Authenticated {
        authenticated: true,
        identity: zerant_id.clone(),
        zerant_id,
        scopes,
    })
    .into_response();
    response.headers_mut().append(
        SET_COOKIE,
        HeaderValue::from_str(&session_cookie_header(&token)).map_err(|_| ApiError::Unavailable)?,
    );
    response.headers_mut().append(
        SET_COOKIE,
        HeaderValue::from_str(&clear_cookie_header(PASSKEY_ATTEMPT_COOKIE))
            .map_err(|_| ApiError::Unavailable)?,
    );
    Ok(response)
}

async fn passkey_authentication_start(
    State(state): State<AppState>,
    Json(input): Json<PasskeyAuthenticationStart>,
) -> Result<Response, ApiError> {
    let zerant_id = input.zerant_id.trim().to_owned();
    if !zerant_id.starts_with("zr_") || !valid_short_text(&zerant_id, 27, 27) {
        return Err(ApiError::Invalid);
    }

    let client = db_client(&state.db).await?;
    let account_row = client
        .query_opt(
            "SELECT id FROM accounts WHERE public_handle = $1",
            &[&zerant_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::Unauthorized)?;
    let account: Uuid = account_row.get(0);
    enforce_account_rate_limit(&state.db, account, "passkey_authenticate", 20).await?;

    let rows = client
        .query(
            "SELECT passkey FROM passkey_credentials
             WHERE account_id = $1
             ORDER BY created_at ASC",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    if rows.is_empty() {
        return Err(ApiError::Unauthorized);
    }

    let mut passkeys = Vec::with_capacity(rows.len());
    for row in rows {
        let value: Value = row.get(0);
        passkeys.push(serde_json::from_value::<Passkey>(value).map_err(|_| ApiError::Unavailable)?);
    }

    let (public_key, authentication) = state
        .webauthn
        .start_passkey_authentication(&passkeys)
        .map_err(|_| ApiError::Unavailable)?;
    let authentication_state =
        serde_json::to_value(authentication).map_err(|_| ApiError::Unavailable)?;

    let (attempt, _) =
        persist_passkey_challenge(&state.db, account, "authenticate", authentication_state).await?;
    let cookie = passkey_attempt_cookie(&attempt);
    let mut response = Json(PasskeyAuthenticationStartResponse { public_key }).into_response();
    response.headers_mut().append(
        SET_COOKIE,
        HeaderValue::from_str(&cookie).map_err(|_| ApiError::Unavailable)?,
    );
    Ok(response)
}

async fn passkey_authentication_finish(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<PublicKeyCredential>,
) -> Result<Response, ApiError> {
    let attempt = cookie_value(&headers, PASSKEY_ATTEMPT_COOKIE)?;
    let attempt_hash = Sha256::digest(attempt.as_bytes()).to_vec();
    let mut client = db_client(&state.db).await?;
    let tx = client
        .transaction()
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let challenge_row = tx
        .query_opt(
            "SELECT account_id, state
             FROM passkey_challenges
             WHERE attempt_hash = $1
               AND kind = 'authenticate'
               AND consumed_at IS NULL
               AND expires_at > NOW()
             FOR UPDATE",
            &[&attempt_hash],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::Unauthorized)?;
    let account: Uuid = challenge_row.get(0);
    let state_value: Value = challenge_row.get(1);
    let authentication: PasskeyAuthentication =
        serde_json::from_value(state_value).map_err(|_| ApiError::Unavailable)?;

    let result = state
        .webauthn
        .finish_passkey_authentication(&input, &authentication)
        .map_err(|_| ApiError::Unauthorized)?;
    let credential_id = URL_SAFE_NO_PAD.encode(result.cred_id().as_ref());

    let credential_row = tx
        .query_opt(
            "SELECT id, passkey FROM passkey_credentials
             WHERE account_id = $1 AND credential_id = $2
             FOR UPDATE",
            &[&account, &credential_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::Unauthorized)?;
    let passkey_id: Uuid = credential_row.get(0);
    let passkey_value: Value = credential_row.get(1);
    let mut passkey: Passkey =
        serde_json::from_value(passkey_value).map_err(|_| ApiError::Unavailable)?;

    if passkey.update_credential(&result) == Some(true) {
        let updated = serde_json::to_value(&passkey).map_err(|_| ApiError::Unavailable)?;
        tx.execute(
            "UPDATE passkey_credentials
             SET passkey = $2, last_used_at = NOW(), updated_at = NOW()
             WHERE id = $1",
            &[&passkey_id, &updated],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    } else {
        tx.execute(
            "UPDATE passkey_credentials SET last_used_at = NOW() WHERE id = $1",
            &[&passkey_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    }

    tx.execute(
        "UPDATE passkey_challenges SET consumed_at = NOW() WHERE attempt_hash = $1",
        &[&attempt_hash],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;

    let scopes = vec!["auth".to_owned()];
    let token = create_account_session(&tx, account, &scopes, "passkey").await?;
    let row = tx
        .query_one(
            "SELECT public_handle FROM accounts WHERE id = $1",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let zerant_id: Option<String> = row.get(0);
    let zerant_id = zerant_id.ok_or(ApiError::Unavailable)?;
    tx.commit().await.map_err(|_| ApiError::Unavailable)?;

    let mut response = Json(Authenticated {
        authenticated: true,
        identity: zerant_id.clone(),
        zerant_id,
        scopes,
    })
    .into_response();
    response.headers_mut().append(
        SET_COOKIE,
        HeaderValue::from_str(&session_cookie_header(&token)).map_err(|_| ApiError::Unavailable)?,
    );
    response.headers_mut().append(
        SET_COOKIE,
        HeaderValue::from_str(&clear_cookie_header(PASSKEY_ATTEMPT_COOKIE))
            .map_err(|_| ApiError::Unavailable)?,
    );
    Ok(response)
}

async fn list_account_passkeys(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<PasskeyView>>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    let client = db_client(&state.db).await?;
    let rows = client
        .query(
            "SELECT id, last_used_at, created_at, updated_at
             FROM passkey_credentials
             WHERE account_id = $1
             ORDER BY created_at ASC",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    Ok(Json(
        rows.into_iter()
            .map(|row| PasskeyView {
                id: row.get(0),
                last_used_at: row.get(1),
                created_at: row.get(2),
                updated_at: row.get(3),
            })
            .collect(),
    ))
}

async fn account_passkey_registration_start(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let account = recent_account_id(&headers, &state.db).await?;
    enforce_account_rate_limit(&state.db, account, "passkey_attach_start", 10).await?;
    let client = db_client(&state.db).await?;
    let passkey_count: i64 = client
        .query_one(
            "SELECT COUNT(*) FROM passkey_credentials WHERE account_id = $1",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .get(0);
    if passkey_count >= MAX_ACCOUNT_PASSKEYS {
        return Err(ApiError::Conflict);
    }
    let row = client
        .query_one(
            "SELECT public_handle FROM accounts WHERE id = $1",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let zerant_id: Option<String> = row.get(0);
    let zerant_id = zerant_id.ok_or(ApiError::Unavailable)?;

    let rows = client
        .query(
            "SELECT passkey FROM passkey_credentials
             WHERE account_id = $1
             ORDER BY created_at ASC",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let mut excluded = Vec::with_capacity(rows.len());
    for row in rows {
        let value: Value = row.get(0);
        let passkey: Passkey = serde_json::from_value(value).map_err(|_| ApiError::Unavailable)?;
        excluded.push(passkey.cred_id().clone());
    }
    let exclude_credentials = if excluded.is_empty() {
        None
    } else {
        Some(excluded)
    };

    let (public_key, registration) = state
        .webauthn
        .start_passkey_registration(account, &zerant_id, "Zerant account", exclude_credentials)
        .map_err(|_| ApiError::Unavailable)?;
    let registration_state =
        serde_json::to_value(registration).map_err(|_| ApiError::Unavailable)?;
    let (attempt, _) =
        persist_passkey_challenge(&state.db, account, "attach", registration_state).await?;

    let mut response = Json(PasskeyRegistrationStart {
        zerant_id,
        public_key,
    })
    .into_response();
    response.headers_mut().append(
        SET_COOKIE,
        HeaderValue::from_str(&passkey_attempt_cookie(&attempt))
            .map_err(|_| ApiError::Unavailable)?,
    );
    Ok(response)
}

async fn account_passkey_registration_finish(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<RegisterPublicKeyCredential>,
) -> Result<(StatusCode, Json<PasskeyView>), ApiError> {
    let account = recent_account_id(&headers, &state.db).await?;
    enforce_account_rate_limit(&state.db, account, "passkey_attach_finish", 10).await?;
    let attempt = cookie_value(&headers, PASSKEY_ATTEMPT_COOKIE)?;
    let attempt_hash = Sha256::digest(attempt.as_bytes()).to_vec();
    let mut client = db_client(&state.db).await?;
    let tx = client
        .transaction()
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let row = tx
        .query_opt(
            "SELECT account_id, state
             FROM passkey_challenges
             WHERE attempt_hash = $1
               AND kind = 'attach'
               AND consumed_at IS NULL
               AND expires_at > NOW()
             FOR UPDATE",
            &[&attempt_hash],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::Unauthorized)?;
    let challenge_account: Uuid = row.get(0);
    if challenge_account != account {
        return Err(ApiError::Unauthorized);
    }

    let state_value: Value = row.get(1);
    let registration: PasskeyRegistration =
        serde_json::from_value(state_value).map_err(|_| ApiError::Unavailable)?;
    let passkey = state
        .webauthn
        .finish_passkey_registration(&input, &registration)
        .map_err(|_| ApiError::Unauthorized)?;
    let credential_id = URL_SAFE_NO_PAD.encode(passkey.cred_id().as_ref());

    let passkey_count: i64 = tx
        .query_one(
            "SELECT COUNT(*) FROM passkey_credentials WHERE account_id = $1",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .get(0);
    if passkey_count >= MAX_ACCOUNT_PASSKEYS {
        return Err(ApiError::Conflict);
    }

    if tx
        .query_opt(
            "SELECT 1 FROM passkey_credentials WHERE credential_id = $1",
            &[&credential_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .is_some()
    {
        return Err(ApiError::Conflict);
    }

    let id = Uuid::new_v4();
    let passkey_value = serde_json::to_value(&passkey).map_err(|_| ApiError::Unavailable)?;
    let created = tx
        .query_one(
            "INSERT INTO passkey_credentials(id, account_id, credential_id, passkey)
             VALUES ($1, $2, $3, $4)
             RETURNING created_at, updated_at",
            &[&id, &account, &credential_id, &passkey_value],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    tx.execute(
        "UPDATE passkey_challenges SET consumed_at = NOW() WHERE attempt_hash = $1",
        &[&attempt_hash],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;
    tx.commit().await.map_err(|_| ApiError::Unavailable)?;

    Ok((
        StatusCode::CREATED,
        Json(PasskeyView {
            id,
            last_used_at: None,
            created_at: created.get(0),
            updated_at: created.get(1),
        }),
    ))
}

fn can_remove_passkey(passkey_count: i64, alternative_access: bool) -> bool {
    passkey_count > 1 || alternative_access
}

async fn access_method_count(tx: &Transaction<'_>, account: Uuid) -> Result<i64, ApiError> {
    tx.query_one(
        "SELECT (SELECT COUNT(*) FROM passkey_credentials WHERE account_id = $1)
              + (SELECT COUNT(*) FROM zecauth_identities WHERE account_id = $1)
              + (SELECT COUNT(*) FROM wallet_message_identities WHERE account_id = $1)",
        &[&account],
    )
    .await
    .map_err(|_| ApiError::Unavailable)
    .map(|row| row.get(0))
}

async fn delete_account_passkey(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let account = recent_account_id(&headers, &state.db).await?;
    enforce_account_rate_limit(&state.db, account, "passkey_remove", 20).await?;
    let token_hash = Sha256::digest(session_token(&headers)?.as_bytes()).to_vec();
    let mut client = db_client(&state.db).await?;
    let tx = client
        .transaction()
        .await
        .map_err(|_| ApiError::Unavailable)?;

    tx.query_one(
        "SELECT id FROM accounts WHERE id = $1 FOR UPDATE",
        &[&account],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;

    let current = tx
        .query_opt(
            "SELECT id FROM sessions WHERE account_id = $1 AND token_hash = $2
         AND expires_at > NOW()
         AND created_at > NOW() - ($3::bigint * INTERVAL '1 minute')",
            &[&account, &token_hash, &PASSKEY_SECURITY_REAUTH_MINUTES],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    if current.is_none() {
        return Err(ApiError::Unauthorized);
    }

    let target = tx
        .query_opt(
            "SELECT id FROM passkey_credentials
             WHERE id = $1 AND account_id = $2
             FOR UPDATE",
            &[&id, &account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::NotFound)?;

    let _target_id: Uuid = target.get(0);
    let passkey_count: i64 = tx
        .query_one(
            "SELECT COUNT(*) FROM passkey_credentials WHERE account_id = $1",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .get(0);
    let alternative_access: bool = tx
        .query_one(
            "SELECT EXISTS(
                 SELECT 1 FROM zecauth_identities WHERE account_id = $1
             ) OR EXISTS(
                 SELECT 1 FROM wallet_message_identities WHERE account_id = $1
             )",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .get(0);

    if !can_remove_passkey(passkey_count, alternative_access) {
        return Err(ApiError::Conflict);
    }

    tx.execute(
        "DELETE FROM passkey_credentials WHERE id = $1 AND account_id = $2",
        &[&id, &account],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;
    tx.commit().await.map_err(|_| ApiError::Unavailable)?;
    Ok(StatusCode::NO_CONTENT)
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

async fn account_zcash_challenge(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let token_hash = Sha256::digest(session_token(&headers)?.as_bytes()).to_vec();
    let client = db_client(&state.db).await?;
    let row = client
        .query_opt(
            "SELECT id, account_id FROM sessions
         WHERE token_hash = $1 AND expires_at > NOW()
           AND created_at > NOW() - ($2::bigint * INTERVAL '1 minute')",
            &[&token_hash, &PASSKEY_SECURITY_REAUTH_MINUTES],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::Forbidden)?;
    let session_id: Uuid = row.get(0);
    let account: Uuid = row.get(1);
    enforce_account_rate_limit(&state.db, account, "zcash_link_start", 10).await?;

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
    let statement =
        "Link Zcash sign-in to your current Zerant account without exposing spending authority."
            .to_owned();
    let message = canonical_challenge_message(
        &domain,
        &uri,
        &state.zcash_chain,
        &nonce,
        &issued_at,
        &expiration_time,
        &statement,
    );
    let scopes = ScopeSet {
        required: vec![Scope {
            scope_type: "auth".to_owned(),
        }],
    };
    let nonce_hash = Sha256::digest(nonce.as_bytes()).to_vec();
    let mut attempt_bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut attempt_bytes);
    let attempt = URL_SAFE_NO_PAD.encode(attempt_bytes);
    attempt_bytes.fill(0);
    let attempt_hash = Sha256::digest(attempt.as_bytes()).to_vec();
    client
        .execute(
            "DELETE FROM zecauth_challenges WHERE expires_at <= NOW() - INTERVAL '1 day'",
            &[],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let active: i64 = client.query_one(
        "SELECT COUNT(*) FROM zecauth_challenges WHERE consumed_at IS NULL AND expires_at > NOW()",
        &[],
    ).await.map_err(|_| ApiError::Unavailable)?.get(0);
    if active >= MAX_ACTIVE_ZECAUTH_CHALLENGES {
        return Err(ApiError::TooManyRequests);
    }
    client.execute(
        "INSERT INTO zecauth_challenges
         (id, nonce_hash, message, chain, requested_scopes, expires_at, link_account_id, link_session_id, link_attempt_hash)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        &[&Uuid::new_v4(), &nonce_hash, &message, &state.zcash_chain,
          &serde_json::to_value(&scopes).map_err(|_| ApiError::Unavailable)?,
          &expires, &account, &session_id, &attempt_hash],
    ).await.map_err(|_| ApiError::Unavailable)?;
    let mut response = Json(ZecAuthChallenge {
        domain,
        uri,
        version: 1,
        chain: state.zcash_chain.clone(),
        nonce,
        issued_at,
        expiration_time,
        statement,
        scopes,
        message,
    })
    .into_response();
    let cookie = format!(
        "{LINK_ATTEMPT_COOKIE}={attempt}; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age={}",
        ZECAUTH_TTL_MINUTES * 60
    );
    response.headers_mut().insert(
        SET_COOKIE,
        HeaderValue::from_str(&cookie).map_err(|_| ApiError::Unavailable)?,
    );
    Ok(response)
}

async fn list_linked_zcash_methods(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<LinkedZcashMethod>>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    let client = db_client(&state.db).await?;
    let mut methods = Vec::with_capacity(3);
    if let Some(row) = client
        .query_opt(
            "SELECT created_at FROM zecauth_identities WHERE account_id = $1",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
    {
        methods.push(LinkedZcashMethod {
            method: "zecauth",
            chain: None,
            created_at: row.get(0),
        });
    }
    for row in client.query(
        "SELECT chain, created_at FROM wallet_message_identities WHERE account_id = $1 ORDER BY chain LIMIT 16",
        &[&account],
    ).await.map_err(|_| ApiError::Unavailable)? {
        methods.push(LinkedZcashMethod {
            method: "wallet_message", chain: Some(row.get(0)), created_at: row.get(1),
        });
    }
    Ok(Json(methods))
}

async fn remove_linked_zcash_method(
    state: AppState,
    headers: HeaderMap,
    chain: Option<&str>,
) -> Result<Response, ApiError> {
    let account = recent_account_id(&headers, &state.db).await?;
    enforce_account_rate_limit(&state.db, account, "zcash_remove", 20).await?;
    let token_hash = Sha256::digest(session_token(&headers)?.as_bytes()).to_vec();
    let mut client = db_client(&state.db).await?;
    let tx = client
        .transaction()
        .await
        .map_err(|_| ApiError::Unavailable)?;
    tx.query_one(
        "SELECT id FROM accounts WHERE id = $1 FOR UPDATE",
        &[&account],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;

    // Recheck under the account lock: another removal may have revoked this session.
    let current = tx
        .query_opt(
            "SELECT id FROM sessions WHERE account_id = $1 AND token_hash = $2
         AND expires_at > NOW()
         AND created_at > NOW() - ($3::bigint * INTERVAL '1 minute')",
            &[&account, &token_hash, &PASSKEY_SECURITY_REAUTH_MINUTES],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    if current.is_none() {
        return Err(ApiError::Unauthorized);
    }

    let target_exists =
        if let Some(chain) = chain {
            tx.query_opt(
            "SELECT account_id FROM wallet_message_identities WHERE account_id = $1 AND chain = $2",
            &[&account, &chain],
        ).await.map_err(|_| ApiError::Unavailable)?.is_some()
        } else {
            tx.query_opt(
                "SELECT account_id FROM zecauth_identities WHERE account_id = $1",
                &[&account],
            )
            .await
            .map_err(|_| ApiError::Unavailable)?
            .is_some()
        };
    if !target_exists {
        return Err(ApiError::NotFound);
    }
    if access_method_count(&tx, account).await? <= 1 {
        return Err(ApiError::Conflict);
    }

    if let Some(chain) = chain {
        tx.execute(
            "DELETE FROM wallet_message_identities WHERE account_id = $1 AND chain = $2",
            &[&account, &chain],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    } else {
        tx.execute(
            "DELETE FROM zecauth_identities WHERE account_id = $1",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    }
    let revoked = tx
        .query(
            "DELETE FROM sessions WHERE account_id = $1 AND auth_method = 'zcash'
         RETURNING token_hash = $2 AS current",
            &[&account, &token_hash],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let clear_current = revoked.iter().any(|row| row.get::<_, bool>(0));
    tx.commit().await.map_err(|_| ApiError::Unavailable)?;

    let mut response = StatusCode::NO_CONTENT.into_response();
    if clear_current {
        response.headers_mut().insert(
            SET_COOKIE,
            HeaderValue::from_str(&clear_cookie_header(SESSION_COOKIE))
                .map_err(|_| ApiError::Unavailable)?,
        );
    }
    Ok(response)
}

async fn remove_zecauth_method(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    remove_linked_zcash_method(state, headers, None).await
}

async fn remove_wallet_message_method(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(network): Path<String>,
) -> Result<Response, ApiError> {
    let chain = match network.as_str() {
        "testnet" => "zcash:testnet",
        "mainnet" => "zcash:mainnet",
        _ => return Err(ApiError::Invalid),
    };
    remove_linked_zcash_method(state, headers, Some(chain)).await
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
           AND link_account_id IS NULL
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

enum LinkIdentity {
    ZecAuth([u8; 32]),
    WalletMessage([u8; 33]),
}

async fn finalize_zcash_link(
    state: &AppState,
    headers: &HeaderMap,
    message: &str,
    granted: &[String],
    identity: LinkIdentity,
) -> Result<Json<serde_json::Value>, ApiError> {
    finalize_zcash_link_with_attempt(state, headers, message, granted, identity, None).await
}

async fn finalize_zcash_link_with_attempt(
    state: &AppState,
    headers: &HeaderMap,
    message: &str,
    granted: &[String],
    identity: LinkIdentity,
    attempt_hash: Option<&[u8]>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if message.len() > 16 * 1024 || granted.len() > 16 {
        return Err(ApiError::Invalid);
    }
    let token_hash = Sha256::digest(session_token(headers)?.as_bytes()).to_vec();
    let mut client = db_client(&state.db).await?;
    let tx = client
        .transaction()
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let row = tx
        .query_opt(
            "UPDATE zecauth_challenges SET consumed_at = NOW(), link_attempt_hash = NULL,
                pending_link_key = NULL, pending_link_at = NULL
         WHERE message = $1 AND consumed_at IS NULL AND expires_at > NOW()
           AND link_account_id IS NOT NULL AND link_session_id IS NOT NULL
           AND (($2::bytea IS NULL AND pending_link_key IS NULL)
             OR ($2::bytea IS NOT NULL AND link_attempt_hash = $2 AND pending_link_key = $3))
         RETURNING requested_scopes, chain, link_account_id, link_session_id",
            &[
                &message,
                &attempt_hash,
                &match &identity {
                    LinkIdentity::ZecAuth(key) => Some(key.as_slice()),
                    LinkIdentity::WalletMessage(_) => None,
                },
            ],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::Unauthorized)?;
    let requested: ScopeSet =
        serde_json::from_value(row.get(0)).map_err(|_| ApiError::Unavailable)?;
    if !requested
        .required
        .iter()
        .any(|scope| scope.scope_type == "auth")
        || !granted
            .iter()
            .filter_map(|scope| scope_alias(scope))
            .any(|scope| scope == "auth")
    {
        return Err(ApiError::Unauthorized);
    }
    let chain: String = row.get(1);
    let target: Uuid = row.get(2);
    let initiating_session: Uuid = row.get(3);
    let session = tx
        .query_opt(
            "SELECT id, account_id FROM sessions
         WHERE token_hash = $1 AND expires_at > NOW()
           AND created_at > NOW() - ($2::bigint * INTERVAL '1 minute')
         FOR UPDATE",
            &[&token_hash, &PASSKEY_SECURITY_REAUTH_MINUTES],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::Unauthorized)?;
    let current_session: Uuid = session.get(0);
    let current_account: Uuid = session.get(1);
    if current_session != initiating_session || current_account != target {
        return Err(ApiError::Unauthorized);
    }
    let account = tx
        .query_one(
            "SELECT public_handle FROM accounts WHERE id = $1 FOR UPDATE",
            &[&target],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let zerant_id: String = account
        .get::<_, Option<String>>(0)
        .ok_or(ApiError::Unavailable)?;

    match identity {
        LinkIdentity::ZecAuth(key) => {
            let owned = tx
                .query_opt(
                    "SELECT account_id FROM zecauth_identities WHERE verification_key = $1",
                    &[&key.as_slice()],
                )
                .await
                .map_err(|_| ApiError::Unavailable)?;
            if owned.is_some_and(|row| row.get::<_, Uuid>(0) != target) {
                return Err(ApiError::Conflict);
            }
            let slot = tx
                .query_opt(
                    "SELECT verification_key FROM zecauth_identities WHERE account_id = $1",
                    &[&target],
                )
                .await
                .map_err(|_| ApiError::Unavailable)?;
            if let Some(row) = slot {
                if row.get::<_, Vec<u8>>(0) != key {
                    return Err(ApiError::Conflict);
                }
            } else {
                let inserted = tx
                    .execute(
                        "INSERT INTO zecauth_identities(account_id, verification_key)
                     VALUES ($1, $2) ON CONFLICT DO NOTHING",
                        &[&target, &key.as_slice()],
                    )
                    .await
                    .map_err(|_| ApiError::Unavailable)?;
                if inserted != 1 {
                    return Err(ApiError::Conflict);
                }
            }
        }
        LinkIdentity::WalletMessage(key) => {
            let owned = tx.query_opt(
                "SELECT account_id FROM wallet_message_identities WHERE chain = $1 AND public_key = $2",
                &[&chain, &key.as_slice()],
            ).await.map_err(|_| ApiError::Unavailable)?;
            if owned.is_some_and(|row| row.get::<_, Uuid>(0) != target) {
                return Err(ApiError::Conflict);
            }
            let slot = tx.query_opt(
                "SELECT public_key FROM wallet_message_identities WHERE account_id = $1 AND chain = $2",
                &[&target, &chain],
            ).await.map_err(|_| ApiError::Unavailable)?;
            if let Some(row) = slot {
                if row.get::<_, Vec<u8>>(0) != key {
                    return Err(ApiError::Conflict);
                }
            } else {
                let inserted = tx
                    .execute(
                        "INSERT INTO wallet_message_identities(account_id, chain, public_key)
                     VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
                        &[&target, &chain, &key.as_slice()],
                    )
                    .await
                    .map_err(|_| ApiError::Unavailable)?;
                if inserted != 1 {
                    return Err(ApiError::Conflict);
                }
            }
        }
    }
    tx.commit().await.map_err(|_| ApiError::Unavailable)?;
    Ok(Json(
        serde_json::json!({ "linked": true, "zerant_id": zerant_id }),
    ))
}

async fn account_zecauth_verify(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<VerifyZecAuth>,
) -> Result<Response, ApiError> {
    if input.message.len() > 16 * 1024 || input.granted.len() > 16 {
        return Err(ApiError::Invalid);
    }
    let key = verify_redpallas(&input.pubkey, &input.signature, input.message.as_bytes())?;
    let linked = finalize_zcash_link(
        &state,
        &headers,
        &input.message,
        &input.granted,
        LinkIdentity::ZecAuth(key),
    )
    .await?;
    let mut response = linked.into_response();
    response.headers_mut().append(
        SET_COOKIE,
        HeaderValue::from_str(&clear_cookie_header(LINK_ATTEMPT_COOKIE))
            .map_err(|_| ApiError::Unavailable)?,
    );
    Ok(response)
}

async fn account_zecauth_link_callback(
    State(state): State<AppState>,
    Json(input): Json<VerifyZecAuth>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if input.message.len() > 16 * 1024 || input.granted.len() > 16 {
        return Err(ApiError::Invalid);
    }
    let key = verify_redpallas(&input.pubkey, &input.signature, input.message.as_bytes())?;
    let mut client = db_client(&state.db).await?;
    let tx = client
        .transaction()
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let row = tx
        .query_opt(
            "SELECT requested_scopes, pending_link_key FROM zecauth_challenges
         WHERE message = $1 AND link_account_id IS NOT NULL AND link_session_id IS NOT NULL
           AND link_attempt_hash IS NOT NULL AND consumed_at IS NULL AND expires_at > NOW()
         FOR UPDATE",
            &[&input.message],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::Unauthorized)?;
    let requested: ScopeSet =
        serde_json::from_value(row.get(0)).map_err(|_| ApiError::Unavailable)?;
    if !requested
        .required
        .iter()
        .any(|scope| scope.scope_type == "auth")
        || !input
            .granted
            .iter()
            .filter_map(|scope| scope_alias(scope))
            .any(|scope| scope == "auth")
    {
        return Err(ApiError::Unauthorized);
    }
    let pending: Option<Vec<u8>> = row.get(1);
    if pending.as_deref().is_some_and(|existing| existing != key) {
        return Err(ApiError::Conflict);
    }
    if pending.is_none() {
        let updated = tx.execute(
            "UPDATE zecauth_challenges SET pending_link_key = $2, pending_link_at = NOW()
             WHERE message = $1 AND pending_link_key IS NULL AND consumed_at IS NULL AND expires_at > NOW()",
            &[&input.message, &key.as_slice()],
        ).await.map_err(|_| ApiError::Unavailable)?;
        if updated != 1 {
            return Err(ApiError::Conflict);
        }
    }
    tx.commit().await.map_err(|_| ApiError::Unavailable)?;
    Ok(Json(serde_json::json!({ "verified": true })))
}

async fn account_zecauth_link_complete(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let attempt = cookie_value(&headers, LINK_ATTEMPT_COOKIE)?;
    let attempt_hash = Sha256::digest(attempt.as_bytes()).to_vec();
    let session_hash = Sha256::digest(session_token(&headers)?.as_bytes()).to_vec();
    let client = db_client(&state.db).await?;
    let row = client
        .query_opt(
            "SELECT c.message, c.pending_link_key FROM zecauth_challenges c
         JOIN sessions s ON s.id = c.link_session_id AND s.account_id = c.link_account_id
         WHERE c.link_attempt_hash = $1 AND c.link_account_id IS NOT NULL
           AND c.consumed_at IS NULL AND c.expires_at > NOW()
           AND s.token_hash = $2 AND s.expires_at > NOW()
           AND s.created_at > NOW() - ($3::bigint * INTERVAL '1 minute')",
            &[
                &attempt_hash,
                &session_hash,
                &PASSKEY_SECURITY_REAUTH_MINUTES,
            ],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::Unauthorized)?;
    let message: String = row.get(0);
    let pending: Option<Vec<u8>> = row.get(1);
    let Some(key) = pending else {
        return Ok((
            StatusCode::ACCEPTED,
            Json(serde_json::json!({ "ready": false })),
        )
            .into_response());
    };
    let key: [u8; 32] = key.try_into().map_err(|_| ApiError::Unavailable)?;
    let linked = finalize_zcash_link_with_attempt(
        &state,
        &headers,
        &message,
        &["auth".to_owned()],
        LinkIdentity::ZecAuth(key),
        Some(&attempt_hash),
    )
    .await?;
    let mut response = linked.into_response();
    response.headers_mut().append(
        SET_COOKIE,
        HeaderValue::from_static(
            "zerant_link_attempt=; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age=0",
        ),
    );
    Ok(response)
}

async fn account_wallet_verify(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<VerifyWalletMessage>,
) -> Result<Response, ApiError> {
    if input.message.len() > 16 * 1024
        || input.granted.len() > 16
        || input.signing_mode != "derived"
    {
        return Err(ApiError::Invalid);
    }
    let key = verify_zcash_wallet_message(&input.pubkey, &input.signature, &input.message)?;
    let linked = finalize_zcash_link(
        &state,
        &headers,
        &input.message,
        &input.granted,
        LinkIdentity::WalletMessage(key),
    )
    .await?;
    let mut response = linked.into_response();
    response.headers_mut().append(
        SET_COOKIE,
        HeaderValue::from_str(&clear_cookie_header(LINK_ATTEMPT_COOKIE))
            .map_err(|_| ApiError::Unavailable)?,
    );
    Ok(response)
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

    let token = create_account_session(&tx, account, &scopes, "zcash").await?;
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
    let account = recent_account_id(&headers, &state.db).await?;
    let client = db_client(&state.db).await?;
    let row = client
        .query_one(
            "SELECT a.public_handle,
                    (SELECT COUNT(*) FROM credential_envelopes e WHERE e.account_id = a.id),
                    (SELECT COUNT(*) FROM passkey_credentials pk WHERE pk.account_id = a.id),
                    (SELECT p.display_name
                       FROM issuer_profiles p
                       LEFT JOIN issuer_members m
                         ON m.issuer_profile_id = p.id AND m.account_id = a.id
                      WHERE p.account_id = a.id OR m.account_id = a.id
                      LIMIT 1),
                    (SELECT CASE WHEN p.account_id = a.id THEN 'owner' ELSE m.role END
                       FROM issuer_profiles p
                       LEFT JOIN issuer_members m
                         ON m.issuer_profile_id = p.id AND m.account_id = a.id
                      WHERE p.account_id = a.id OR m.account_id = a.id
                      LIMIT 1),
                    (SELECT display_name FROM verifier_profiles v WHERE v.account_id = a.id),
                    EXISTS(SELECT 1 FROM issuer_profiles p WHERE p.account_id = a.id),
                    EXISTS(SELECT 1 FROM verifier_profiles v WHERE v.account_id = a.id AND v.retired_at IS NULL),
                    COALESCE((SELECT p.retired_at IS NOT NULL
                       FROM issuer_profiles p
                       LEFT JOIN issuer_members m
                         ON m.issuer_profile_id = p.id AND m.account_id = a.id
                      WHERE p.account_id = a.id OR m.account_id = a.id
                      LIMIT 1), FALSE),
                    COALESCE((SELECT v.retired_at IS NOT NULL FROM verifier_profiles v WHERE v.account_id = a.id), FALSE)
             FROM accounts a
             WHERE a.id = $1",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let zerant_id: Option<String> = row.get(0);
    let issuer_profile: Option<String> = row.get(3);
    let issuer_role: Option<String> = row.get(4);
    let verifier_profile: Option<String> = row.get(5);
    let owns_issuer: bool = row.get(6);
    let active_verifier: bool = row.get(7);
    let issuer_retired: bool = row.get(8);
    let verifier_retired: bool = row.get(9);
    Ok(Json(AccountSummary {
        zerant_id: zerant_id.ok_or(ApiError::Unavailable)?,
        credential_count: row.get(1),
        passkey_count: row.get(2),
        can_delete: !owns_issuer && !active_verifier,
        issuer_profile,
        issuer_role,
        issuer_retired,
        verifier_profile,
        verifier_retired,
    }))
}

async fn export_account(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AccountExport>, ApiError> {
    let account = recent_account_id(&headers, &state.db).await?;
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
        let expired = credential_expired_at(&credential, OffsetDateTime::now_utc());
        credentials.push(StoredCredential {
            id: row.id,
            credential,
            revoked,
            expired,
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

    let (payments, payments_complete) = payments::export(&state.db, account).await?;

    Ok(Json(AccountExport {
        export_version: 2,
        generated_at: OffsetDateTime::now_utc(),
        zerant_id: zerant_id.ok_or(ApiError::Unavailable)?,
        credentials,
        activity,
        activity_complete,
        payments,
        payments_complete,
    }))
}

async fn delete_account(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let account = recent_account_id(&headers, &state.db).await?;
    let mut client = db_client(&state.db).await?;
    let tx = client
        .transaction()
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let roles = tx
        .query_one(
            "SELECT EXISTS(SELECT 1 FROM issuer_profiles WHERE account_id = $1),
                    EXISTS(SELECT 1 FROM verifier_profiles WHERE account_id = $1 AND retired_at IS NULL)",
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
        let expired = credential_expired_at(&credential, OffsetDateTime::now_utc());
        credentials.push(StoredCredential {
            id: row.id,
            credential,
            revoked,
            expired,
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
            expired: credential_expired_at(&credential, OffsetDateTime::now_utc()),
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
    profile_id: Uuid,
    schema_id: Uuid,
) -> Result<ResolvedCredentialSchema, ApiError> {
    let client = db_client(db).await?;
    let row = client
        .query_opt(
            "SELECT s.id, s.issuer_profile_id, p.issuer_id, p.display_name, s.display_name,
                    s.claim_type, s.context, s.default_expiry_days
             FROM credential_schemas s
             JOIN issuer_profiles p ON p.id = s.issuer_profile_id
             WHERE s.id = $1 AND p.id = $2 AND s.active = TRUE",
            &[&schema_id, &profile_id],
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

async fn create_issuer_schema_version(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(schema_id): Path<Uuid>,
    Json(input): Json<CreateCredentialSchemaVersion>,
) -> Result<(StatusCode, Json<CredentialSchemaView>), ApiError> {
    let account = account_id(&headers, &state.db).await?;
    enforce_account_rate_limit(&state.db, account, "credential_schema_version", 20).await?;
    let access = issuer_access(&state.db, account).await?;
    require_issuer_role(&access, &["owner", "admin"])?;
    require_active_issuer(&access)?;

    let description = input.description.trim().to_owned();
    if !valid_short_text(&description, 2, 512) || !(1..=365).contains(&input.default_expiry_days) {
        return Err(ApiError::Invalid);
    }

    let mut client = db_client(&state.db).await?;
    let tx = client
        .transaction()
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let current = tx
        .query_opt(
            "SELECT s.issuer_profile_id, p.issuer_id, p.display_name, s.slug, s.display_name,
                    s.claim_type, s.context, s.version
             FROM credential_schemas s
             JOIN issuer_profiles p ON p.id = s.issuer_profile_id
             WHERE s.id = $1 AND p.id = $2 AND s.active = TRUE
             FOR UPDATE OF s",
            &[&schema_id, &access.profile_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::Conflict)?;

    let version: i32 = current.get(7);
    let next_version = version.checked_add(1).ok_or(ApiError::Conflict)?;
    let new_id = Uuid::new_v4();
    let days = i32::from(input.default_expiry_days);
    let now = OffsetDateTime::now_utc();

    tx.execute(
        "UPDATE credential_schemas
         SET active = FALSE, retired_at = $3, updated_at = $3
         WHERE id = $1 AND issuer_profile_id = $2 AND active = TRUE",
        &[&schema_id, &current.get::<_, Uuid>(0), &now],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;

    tx.execute(
        "INSERT INTO credential_schemas
         (id, issuer_profile_id, slug, display_name, description, claim_type, context,
          default_expiry_days, version, active, supersedes_schema_id)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,TRUE,$10)",
        &[
            &new_id,
            &current.get::<_, Uuid>(0),
            &current.get::<_, String>(3),
            &current.get::<_, String>(4),
            &description,
            &current.get::<_, String>(5),
            &current.get::<_, String>(6),
            &days,
            &next_version,
            &schema_id,
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

    let schema_label: String = current.get(4);
    let schema_context: String = current.get(6);
    record_issuer_event(
        &tx,
        access.profile_id,
        account,
        IssuerEvent {
            event_type: "credential_schema_versioned",
            object_id: &new_id.to_string(),
            label: &schema_label,
            context: Some(&schema_context),
            counterparty: None,
        },
    )
    .await?;

    tx.commit().await.map_err(|_| ApiError::Unavailable)?;

    Ok((
        StatusCode::CREATED,
        Json(CredentialSchemaView {
            id: new_id,
            issuer_id: current.get(1),
            issuer_name: current.get(2),
            display_name: current.get(4),
            description,
            claim_type: current.get(5),
            context: current.get(6),
            default_expiry_days: days,
            version: next_version,
            active: true,
            supersedes_schema_id: Some(schema_id),
            retired_at: None,
            created_at: now,
        }),
    ))
}

async fn deactivate_issuer_schema(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(schema_id): Path<Uuid>,
) -> Result<Json<CredentialSchemaView>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    enforce_account_rate_limit(&state.db, account, "credential_schema_deactivate", 20).await?;
    let access = issuer_access(&state.db, account).await?;
    require_issuer_role(&access, &["owner", "admin"])?;
    let mut client = db_client(&state.db).await?;
    let tx = client
        .transaction()
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let row = tx
        .query_opt(
            "UPDATE credential_schemas s
             SET active = FALSE, retired_at = NOW(), updated_at = NOW()
             FROM issuer_profiles p
             WHERE s.issuer_profile_id = p.id
               AND p.id = $1
               AND s.id = $2
               AND s.active = TRUE
             RETURNING s.id, p.issuer_id, p.display_name, s.display_name, s.description,
                       s.claim_type, s.context, s.default_expiry_days, s.version, s.active,
                       s.supersedes_schema_id, s.retired_at, s.created_at",
            &[&access.profile_id, &schema_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::Conflict)?;

    let display_name: String = row.get(3);
    let schema_context: String = row.get(6);
    record_issuer_event(
        &tx,
        access.profile_id,
        account,
        IssuerEvent {
            event_type: "credential_schema_retired",
            object_id: &schema_id.to_string(),
            label: &display_name,
            context: Some(&schema_context),
            counterparty: None,
        },
    )
    .await?;

    tx.commit().await.map_err(|_| ApiError::Unavailable)?;

    Ok(Json(CredentialSchemaView {
        id: row.get(0),
        issuer_id: row.get(1),
        issuer_name: row.get(2),
        display_name: row.get(3),
        description: row.get(4),
        claim_type: row.get(5),
        context: row.get(6),
        default_expiry_days: row.get(7),
        version: row.get(8),
        active: row.get(9),
        supersedes_schema_id: row.get(10),
        retired_at: row.get(11),
        created_at: row.get(12),
    }))
}

async fn list_issuer_schemas(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<CredentialSchemaView>>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    let access = issuer_access(&state.db, account).await?;
    let client = db_client(&state.db).await?;
    let rows = client
        .query(
            "SELECT s.id, p.issuer_id, p.display_name, s.display_name, s.description,
                    s.claim_type, s.context, s.default_expiry_days, s.version, s.active,
                    s.supersedes_schema_id, s.retired_at, s.created_at
             FROM credential_schemas s
             JOIN issuer_profiles p ON p.id = s.issuer_profile_id
             WHERE p.id = $1
             ORDER BY s.slug ASC, s.version DESC
             LIMIT 256",
            &[&access.profile_id],
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
                version: row.get(8),
                active: row.get(9),
                supersedes_schema_id: row.get(10),
                retired_at: row.get(11),
                created_at: row.get(12),
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
    let access = issuer_access(&state.db, account).await?;
    require_issuer_role(&access, &["owner", "admin"])?;
    require_active_issuer(&access)?;

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

    let mut client = db_client(&state.db).await?;
    let issuer = client
        .query_opt(
            "SELECT id, issuer_id, display_name FROM issuer_profiles WHERE id = $1",
            &[&access.profile_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::Unauthorized)?;
    let profile_id: Uuid = issuer.get(0);
    let issuer_id: String = issuer.get(1);
    let issuer_name: String = issuer.get(2);
    let days = i32::from(input.default_expiry_days);
    let id = Uuid::new_v4();

    let tx = client
        .transaction()
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let row = tx
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

    record_issuer_event(
        &tx,
        profile_id,
        account,
        IssuerEvent {
            event_type: "credential_schema_created",
            object_id: &id.to_string(),
            label: &display_name,
            context: Some(&context),
            counterparty: None,
        },
    )
    .await?;
    tx.commit().await.map_err(|_| ApiError::Unavailable)?;

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
            version: 1,
            active: true,
            supersedes_schema_id: None,
            retired_at: None,
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
            "SELECT display_name, origin, retired_at, created_at
             FROM verifier_profiles WHERE account_id = $1",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::NotFound)?;

    Ok(Json(VerifierProfileView {
        display_name: row.get(0),
        origin: row.get(1),
        retired_at: row.get(2),
        created_at: row.get(3),
    }))
}

fn verifier_retirement_blocked(active_requests: bool, active_deliveries: bool) -> bool {
    active_requests || active_deliveries
}

async fn retire_verifier_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let account = recent_account_id(&headers, &state.db).await?;
    enforce_account_rate_limit(&state.db, account, "verifier_retire", 3).await?;
    let mut client = db_client(&state.db).await?;
    let tx = client
        .transaction()
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let verifier = tx
        .query_opt(
            "SELECT id, retired_at FROM verifier_profiles WHERE account_id = $1 FOR UPDATE",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::NotFound)?;
    let verifier_profile_id: Uuid = verifier.get(0);
    let already_retired: Option<OffsetDateTime> = verifier.get(1);
    if already_retired.is_some() {
        return Err(ApiError::Conflict);
    }

    let blockers = tx
        .query_one(
            "SELECT
                 EXISTS(
                     SELECT 1 FROM verification_requests
                     WHERE verifier_profile_id = $1
                       AND status = 'pending'
                       AND expires_at > NOW()
                 ),
                 EXISTS(
                     SELECT 1
                     FROM webhook_deliveries d
                     JOIN verifier_webhooks w ON w.id = d.webhook_id
                     WHERE w.verifier_profile_id = $1
                       AND d.status IN ('pending', 'delivering')
                 )",
            &[&verifier_profile_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let active_requests: bool = blockers.get(0);
    let active_deliveries: bool = blockers.get(1);
    if verifier_retirement_blocked(active_requests, active_deliveries) {
        return Err(ApiError::Conflict);
    }

    tx.execute(
        "UPDATE verification_requests
         SET status = 'expired'
         WHERE verifier_profile_id = $1
           AND status = 'pending'
           AND expires_at <= NOW()",
        &[&verifier_profile_id],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;

    let affected = tx
        .execute(
            "UPDATE verifier_profiles
             SET retired_at = NOW()
             WHERE id = $1 AND account_id = $2 AND retired_at IS NULL",
            &[&verifier_profile_id, &account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    if affected != 1 {
        return Err(ApiError::Conflict);
    }
    tx.execute(
        "UPDATE verifier_api_keys
         SET revoked_at = COALESCE(revoked_at, NOW())
         WHERE verifier_profile_id = $1 AND revoked_at IS NULL",
        &[&verifier_profile_id],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;
    tx.execute(
        "UPDATE verifier_webhooks
         SET disabled_at = COALESCE(disabled_at, NOW())
         WHERE verifier_profile_id = $1 AND disabled_at IS NULL",
        &[&verifier_profile_id],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;
    tx.execute(
        "UPDATE verification_policies
         SET active = FALSE, retired_at = COALESCE(retired_at, NOW())
         WHERE verifier_profile_id = $1 AND active = TRUE",
        &[&verifier_profile_id],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;
    tx.commit().await.map_err(|_| ApiError::Unavailable)?;
    Ok(StatusCode::NO_CONTENT.into_response())
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
               AND v.retired_at IS NULL
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

async fn create_verifier_api_key(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateVerifierApiKey>,
) -> Result<(StatusCode, Json<CreatedVerifierApiKey>), ApiError> {
    let account = account_id(&headers, &state.db).await?;
    enforce_account_rate_limit(&state.db, account, "verifier_api_key_create", 10).await?;

    let name = input.name.trim().to_owned();
    if !valid_short_text(&name, 2, 80) {
        return Err(ApiError::Invalid);
    }
    let scopes = normalize_verifier_api_scopes(input.scopes)?;
    let now = OffsetDateTime::now_utc();
    let expires_at = match input.expires_in_days {
        Some(days) if (1..=365).contains(&days) => Some(now + Duration::days(i64::from(days))),
        Some(_) => return Err(ApiError::Invalid),
        None => None,
    };

    let mut client = db_client(&state.db).await?;
    let tx = client
        .transaction()
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let verifier = tx
        .query_opt(
            "SELECT id FROM verifier_profiles WHERE account_id = $1 AND retired_at IS NULL FOR UPDATE",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::NotFound)?;
    let verifier_profile_id: Uuid = verifier.get(0);

    let active_count: i64 = tx
        .query_one(
            "SELECT COUNT(*)
             FROM verifier_api_keys
             WHERE verifier_profile_id = $1
               AND revoked_at IS NULL
               AND (expires_at IS NULL OR expires_at > NOW())",
            &[&verifier_profile_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .get(0);
    if active_count >= MAX_ACTIVE_VERIFIER_API_KEYS {
        return Err(ApiError::Conflict);
    }

    let secret = generate_verifier_api_secret();
    let key_prefix: String = secret.chars().take(24).collect();
    let token_hash = Sha256::digest(secret.as_bytes()).to_vec();
    let scopes_value = serde_json::to_value(&scopes).map_err(|_| ApiError::Unavailable)?;
    let id = Uuid::new_v4();

    let row = tx
        .query_one(
            "INSERT INTO verifier_api_keys
             (id, verifier_profile_id, name, key_prefix, token_hash, scopes, expires_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7)
             RETURNING created_at",
            &[
                &id,
                &verifier_profile_id,
                &name,
                &key_prefix,
                &token_hash,
                &scopes_value,
                &expires_at,
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

    tx.commit().await.map_err(|_| ApiError::Unavailable)?;

    Ok((
        StatusCode::CREATED,
        Json(CreatedVerifierApiKey {
            key: VerifierApiKeyView {
                id,
                name,
                key_prefix,
                scopes,
                created_at: row.get(0),
                last_used_at: None,
                expires_at,
                revoked: false,
            },
            secret,
        }),
    ))
}

async fn list_verifier_api_keys(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<VerifierApiKeyView>>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    let client = db_client(&state.db).await?;
    let rows = client
        .query(
            "SELECT k.id, k.name, k.key_prefix, k.scopes, k.created_at,
                    k.last_used_at, k.expires_at, k.revoked_at
             FROM verifier_api_keys k
             JOIN verifier_profiles v ON v.id = k.verifier_profile_id
             WHERE v.account_id = $1
             ORDER BY k.created_at DESC
             LIMIT 100",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let mut keys = Vec::with_capacity(rows.len());
    for row in rows {
        let scopes_value: Value = row.get(3);
        let scopes: Vec<String> =
            serde_json::from_value(scopes_value).map_err(|_| ApiError::Unavailable)?;
        let revoked_at: Option<OffsetDateTime> = row.get(7);
        keys.push(VerifierApiKeyView {
            id: row.get(0),
            name: row.get(1),
            key_prefix: row.get(2),
            scopes,
            created_at: row.get(4),
            last_used_at: row.get(5),
            expires_at: row.get(6),
            revoked: revoked_at.is_some(),
        });
    }
    Ok(Json(keys))
}

async fn revoke_verifier_api_key(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<VerifierApiKeyView>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    enforce_account_rate_limit(&state.db, account, "verifier_api_key_revoke", 20).await?;
    let client = db_client(&state.db).await?;
    let row = client
        .query_opt(
            "UPDATE verifier_api_keys k
             SET revoked_at = NOW()
             FROM verifier_profiles v
             WHERE k.id = $1
               AND k.verifier_profile_id = v.id
               AND v.account_id = $2
               AND k.revoked_at IS NULL
             RETURNING k.id, k.name, k.key_prefix, k.scopes, k.created_at,
                       k.last_used_at, k.expires_at",
            &[&id, &account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::Conflict)?;

    let scopes_value: Value = row.get(3);
    let scopes: Vec<String> =
        serde_json::from_value(scopes_value).map_err(|_| ApiError::Unavailable)?;

    Ok(Json(VerifierApiKeyView {
        id: row.get(0),
        name: row.get(1),
        key_prefix: row.get(2),
        scopes,
        created_at: row.get(4),
        last_used_at: row.get(5),
        expires_at: row.get(6),
        revoked: true,
    }))
}

async fn create_verifier_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateVerifierWebhook>,
) -> Result<(StatusCode, Json<CreatedVerifierWebhook>), ApiError> {
    let account = account_id(&headers, &state.db).await?;
    enforce_account_rate_limit(&state.db, account, "verifier_webhook_create", 10).await?;

    let name = input.name.trim().to_owned();
    if !valid_short_text(&name, 2, 80) {
        return Err(ApiError::Invalid);
    }
    let url = parse_webhook_url(input.url.trim())?;
    webhook_public_addrs(&url).await?;
    let url = url.to_string();

    let mut client = db_client(&state.db).await?;
    let tx = client
        .transaction()
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let verifier = tx
        .query_opt(
            "SELECT id FROM verifier_profiles WHERE account_id = $1 AND retired_at IS NULL FOR UPDATE",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::NotFound)?;
    let verifier_profile_id: Uuid = verifier.get(0);

    let active_count: i64 = tx
        .query_one(
            "SELECT COUNT(*)
             FROM verifier_webhooks
             WHERE verifier_profile_id = $1 AND disabled_at IS NULL",
            &[&verifier_profile_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .get(0);
    if active_count >= MAX_ACTIVE_VERIFIER_WEBHOOKS {
        return Err(ApiError::Conflict);
    }

    let id = Uuid::new_v4();
    let secret = generate_webhook_secret();
    let encrypted = state.cipher.encrypt(account, id, secret.as_bytes())?;
    let row = tx
        .query_one(
            "INSERT INTO verifier_webhooks
             (id, verifier_profile_id, name, url, ciphertext, data_nonce,
              wrapped_dek, wrap_nonce, key_version)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
             RETURNING created_at",
            &[
                &id,
                &verifier_profile_id,
                &name,
                &url,
                &encrypted.ciphertext,
                &encrypted.data_nonce,
                &encrypted.wrapped_dek,
                &encrypted.wrap_nonce,
                &encrypted.key_version,
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
    tx.commit().await.map_err(|_| ApiError::Unavailable)?;

    Ok((
        StatusCode::CREATED,
        Json(CreatedVerifierWebhook {
            webhook: VerifierWebhookView {
                id,
                name,
                url,
                created_at: row.get(0),
                last_delivery_at: None,
                disabled: false,
                pending_deliveries: 0,
                dead_deliveries: 0,
            },
            secret,
        }),
    ))
}

async fn list_verifier_webhooks(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<VerifierWebhookView>>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    let client = db_client(&state.db).await?;
    let rows = client
        .query(
            "SELECT w.id, w.name, w.url, w.created_at, w.last_delivery_at, w.disabled_at,
                    COUNT(d.id) FILTER (WHERE d.status IN ('pending','delivering')),
                    COUNT(d.id) FILTER (WHERE d.status = 'dead')
             FROM verifier_webhooks w
             JOIN verifier_profiles v ON v.id = w.verifier_profile_id
             LEFT JOIN webhook_deliveries d ON d.webhook_id = w.id
             WHERE v.account_id = $1
             GROUP BY w.id
             ORDER BY w.created_at DESC
             LIMIT 100",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    Ok(Json(
        rows.into_iter()
            .map(|row| {
                let disabled_at: Option<OffsetDateTime> = row.get(5);
                VerifierWebhookView {
                    id: row.get(0),
                    name: row.get(1),
                    url: row.get(2),
                    created_at: row.get(3),
                    last_delivery_at: row.get(4),
                    disabled: disabled_at.is_some(),
                    pending_deliveries: row.get(6),
                    dead_deliveries: row.get(7),
                }
            })
            .collect(),
    ))
}

async fn disable_verifier_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<VerifierWebhookView>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    enforce_account_rate_limit(&state.db, account, "verifier_webhook_disable", 20).await?;
    let mut client = db_client(&state.db).await?;
    let tx = client
        .transaction()
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let row = tx
        .query_opt(
            "UPDATE verifier_webhooks w
             SET disabled_at = NOW()
             FROM verifier_profiles v
             WHERE w.id = $1
               AND w.verifier_profile_id = v.id
               AND v.account_id = $2
               AND w.disabled_at IS NULL
             RETURNING w.id, w.name, w.url, w.created_at, w.last_delivery_at",
            &[&id, &account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::Conflict)?;
    tx.execute(
        "UPDATE webhook_deliveries
         SET status = 'dead',
             last_error = 'webhook_disabled'
         WHERE webhook_id = $1 AND status IN ('pending','delivering')",
        &[&id],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;
    let dead_deliveries: i64 = tx
        .query_one(
            "SELECT COUNT(*) FROM webhook_deliveries WHERE webhook_id = $1 AND status = 'dead'",
            &[&id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .get(0);
    tx.commit().await.map_err(|_| ApiError::Unavailable)?;

    Ok(Json(VerifierWebhookView {
        id: row.get(0),
        name: row.get(1),
        url: row.get(2),
        created_at: row.get(3),
        last_delivery_at: row.get(4),
        disabled: true,
        pending_deliveries: 0,
        dead_deliveries,
    }))
}

async fn enqueue_verifier_webhooks(
    tx: &Transaction<'_>,
    verifier_profile_id: Uuid,
    request_id: Uuid,
    event_type: &str,
    credential_schema_id: Option<Uuid>,
    occurred_at: OffsetDateTime,
) -> Result<(), ApiError> {
    if !matches!(event_type, "verification.approved" | "verification.denied") {
        return Err(ApiError::Unavailable);
    }
    let webhooks = tx
        .query(
            "SELECT id
             FROM verifier_webhooks
             WHERE verifier_profile_id = $1 AND disabled_at IS NULL
             ORDER BY created_at ASC
             LIMIT $2",
            &[&verifier_profile_id, &MAX_ACTIVE_VERIFIER_WEBHOOKS],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let occurred_at_text = occurred_at
        .format(&Rfc3339)
        .map_err(|_| ApiError::Unavailable)?;
    for row in webhooks {
        let webhook_id: Uuid = row.get(0);
        let delivery_id = Uuid::new_v4();
        let status = if event_type == "verification.approved" {
            "approved"
        } else {
            "denied"
        };
        let payload = serde_json::json!({
            "schema": "zerant.webhook.event.v0.1",
            "event_id": delivery_id,
            "type": event_type,
            "request_id": request_id,
            "credential_schema_id": credential_schema_id,
            "status": status,
            "verified": status == "approved",
            "occurred_at": occurred_at_text,
        });
        tx.execute(
            "INSERT INTO webhook_deliveries
             (id, webhook_id, verification_request_id, event_type, payload)
             VALUES ($1,$2,$3,$4,$5)
             ON CONFLICT (webhook_id, verification_request_id, event_type) DO NOTHING",
            &[
                &delivery_id,
                &webhook_id,
                &request_id,
                &event_type,
                &payload,
            ],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    }
    Ok(())
}

async fn claim_request_webhook_deliveries(
    db: &Pool,
    request_id: Uuid,
) -> Result<Vec<Uuid>, ApiError> {
    let mut client = db_client(db).await?;
    let tx = client
        .transaction()
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let rows = tx
        .query(
            "SELECT d.id
             FROM webhook_deliveries d
             JOIN verifier_webhooks w ON w.id = d.webhook_id
             WHERE d.verification_request_id = $1
               AND d.status = 'pending'
               AND d.next_attempt_at <= NOW()
               AND w.disabled_at IS NULL
             ORDER BY d.created_at ASC
             FOR UPDATE OF d SKIP LOCKED
             LIMIT $2",
            &[&request_id, &MAX_ACTIVE_VERIFIER_WEBHOOKS],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let ids: Vec<Uuid> = rows.into_iter().map(|row| row.get(0)).collect();
    for id in &ids {
        tx.execute(
            "UPDATE webhook_deliveries
             SET status = 'delivering',
                 attempt_count = attempt_count + 1,
                 last_attempt_at = NOW()
             WHERE id = $1 AND status = 'pending'",
            &[id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    }
    tx.commit().await.map_err(|_| ApiError::Unavailable)?;
    Ok(ids)
}

async fn finish_webhook_failure(
    db: &Pool,
    delivery_id: Uuid,
    attempt_count: i32,
    status_code: Option<i32>,
    error: &str,
    force_dead: bool,
) -> Result<bool, ApiError> {
    let client = db_client(db).await?;
    let dead = force_dead || attempt_count >= MAX_WEBHOOK_ATTEMPTS;
    if dead {
        client
            .execute(
                "UPDATE webhook_deliveries
                 SET status = 'dead',
                     last_status_code = $2,
                     last_error = $3
                 WHERE id = $1 AND status = 'delivering'",
                &[&delivery_id, &status_code, &error],
            )
            .await
            .map_err(|_| ApiError::Unavailable)?;
    } else {
        let retry_at =
            OffsetDateTime::now_utc() + Duration::seconds(webhook_retry_seconds(attempt_count));
        client
            .execute(
                "UPDATE webhook_deliveries
                 SET status = 'pending',
                     next_attempt_at = $2,
                     last_status_code = $3,
                     last_error = $4
                 WHERE id = $1 AND status = 'delivering'",
                &[&delivery_id, &retry_at, &status_code, &error],
            )
            .await
            .map_err(|_| ApiError::Unavailable)?;
    }
    Ok(dead)
}

async fn deliver_webhook(state: &AppState, delivery_id: Uuid) -> Result<&'static str, ApiError> {
    let client = db_client(&state.db).await?;
    let row = client
        .query_opt(
            "SELECT d.event_type, d.payload, d.attempt_count,
                    w.id, w.url, w.ciphertext, w.data_nonce, w.wrapped_dek,
                    w.wrap_nonce, w.key_version, v.account_id
             FROM webhook_deliveries d
             JOIN verifier_webhooks w ON w.id = d.webhook_id
             JOIN verifier_profiles v ON v.id = w.verifier_profile_id
             WHERE d.id = $1
               AND d.status = 'delivering'
               AND w.disabled_at IS NULL",
            &[&delivery_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let Some(row) = row else {
        return Ok("dead");
    };

    let event_type: String = row.get(0);
    let payload: Value = row.get(1);
    let attempt_count: i32 = row.get(2);
    let webhook_id: Uuid = row.get(3);
    let url_text: String = row.get(4);
    let owner_account: Uuid = row.get(10);
    let now = OffsetDateTime::now_utc();
    let secret_row = CredentialRow {
        id: webhook_id,
        ciphertext: row.get(5),
        data_nonce: row.get(6),
        wrapped_dek: row.get(7),
        wrap_nonce: row.get(8),
        key_version: row.get(9),
        created_at: now,
        updated_at: now,
    };
    let mut secret = state.cipher.decrypt(owner_account, &secret_row)?;
    let body = serde_json::to_vec(&payload).map_err(|_| ApiError::Unavailable)?;
    let timestamp = now.unix_timestamp();
    let signature = webhook_signature(&secret, timestamp, &body)?;
    secret.fill(0);

    let url = match parse_webhook_url(&url_text) {
        Ok(url) => url,
        Err(_) => {
            let _ = finish_webhook_failure(
                &state.db,
                delivery_id,
                attempt_count,
                None,
                "unsafe_destination",
                true,
            )
            .await?;
            return Ok("dead");
        }
    };
    let (host, addrs) = match webhook_public_addrs(&url).await {
        Ok(value) => value,
        Err(_) => {
            let mut client = db_client(&state.db).await?;
            let tx = client
                .transaction()
                .await
                .map_err(|_| ApiError::Unavailable)?;
            tx.execute(
                "UPDATE verifier_webhooks
                 SET disabled_at = COALESCE(disabled_at, NOW())
                 WHERE id = $1",
                &[&webhook_id],
            )
            .await
            .map_err(|_| ApiError::Unavailable)?;
            tx.execute(
                "UPDATE webhook_deliveries
                 SET status = 'dead',
                     last_error = 'unsafe_destination'
                 WHERE webhook_id = $1
                   AND status IN ('pending','delivering')",
                &[&webhook_id],
            )
            .await
            .map_err(|_| ApiError::Unavailable)?;
            tx.commit().await.map_err(|_| ApiError::Unavailable)?;
            return Ok("dead");
        }
    };

    let http = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(StdDuration::from_secs(5))
        .resolve_to_addrs(&host, &addrs)
        .build()
        .map_err(|_| ApiError::Unavailable)?;
    let result = http
        .post(url)
        .header("user-agent", "Zerant-Webhook/1.0")
        .header("content-type", "application/json")
        .header("x-zerant-event", &event_type)
        .header("x-zerant-delivery-id", delivery_id.to_string())
        .header("x-zerant-timestamp", timestamp.to_string())
        .header("x-zerant-signature", signature)
        .body(body)
        .send()
        .await;

    match result {
        Ok(response) if response.status().is_success() => {
            let status_code = i32::from(response.status().as_u16());
            let mut client = db_client(&state.db).await?;
            let tx = client
                .transaction()
                .await
                .map_err(|_| ApiError::Unavailable)?;
            tx.execute(
                "UPDATE webhook_deliveries
                 SET status = 'delivered',
                     delivered_at = NOW(),
                     last_status_code = $2,
                     last_error = NULL
                 WHERE id = $1 AND status = 'delivering'",
                &[&delivery_id, &status_code],
            )
            .await
            .map_err(|_| ApiError::Unavailable)?;
            tx.execute(
                "UPDATE verifier_webhooks SET last_delivery_at = NOW() WHERE id = $1",
                &[&webhook_id],
            )
            .await
            .map_err(|_| ApiError::Unavailable)?;
            tx.commit().await.map_err(|_| ApiError::Unavailable)?;
            Ok("delivered")
        }
        Ok(response) => {
            let status_code = i32::from(response.status().as_u16());
            let dead = finish_webhook_failure(
                &state.db,
                delivery_id,
                attempt_count,
                Some(status_code),
                "http_error",
                false,
            )
            .await?;
            Ok(if dead { "dead" } else { "retried" })
        }
        Err(_) => {
            let dead = finish_webhook_failure(
                &state.db,
                delivery_id,
                attempt_count,
                None,
                "network_error",
                false,
            )
            .await?;
            Ok(if dead { "dead" } else { "retried" })
        }
    }
}

async fn deliver_webhook_batch(
    state: &AppState,
    ids: Vec<Uuid>,
) -> Result<(usize, usize, usize), ApiError> {
    let mut set = tokio::task::JoinSet::new();
    for id in ids {
        let state = state.clone();
        set.spawn(async move { deliver_webhook(&state, id).await });
    }

    let mut delivered = 0;
    let mut retried = 0;
    let mut dead = 0;
    while let Some(result) = set.join_next().await {
        match result.map_err(|_| ApiError::Unavailable)?? {
            "delivered" => delivered += 1,
            "retried" => retried += 1,
            _ => dead += 1,
        }
    }
    Ok((delivered, retried, dead))
}

async fn dispatch_request_webhooks(
    state: &AppState,
    request_id: Uuid,
) -> Result<WebhookDispatchSummary, ApiError> {
    let ids = claim_request_webhook_deliveries(&state.db, request_id).await?;
    let claimed = ids.len();
    let (delivered, retried, dead) = deliver_webhook_batch(state, ids).await?;

    let client = db_client(&state.db).await?;
    let row = client
        .query_opt(
            "SELECT r.status, r.expires_at,
                    COUNT(d.id) FILTER (WHERE d.status IN ('pending','delivering'))
             FROM verification_requests r
             LEFT JOIN webhook_deliveries d ON d.verification_request_id = r.id
             WHERE r.id = $1
             GROUP BY r.id, r.status, r.expires_at",
            &[&request_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::NotFound)?;
    let status: String = row.get(0);
    let expires_at: OffsetDateTime = row.get(1);
    let pending: i64 = row.get(2);

    Ok(WebhookDispatchSummary {
        claimed,
        delivered,
        retried,
        dead,
        pending,
        finalized: matches!(status.as_str(), "approved" | "denied" | "expired")
            || expires_at <= OffsetDateTime::now_utc(),
    })
}

async fn dispatch_request_webhooks_internal(
    State(state): State<AppState>,
    Path(request_id): Path<Uuid>,
) -> Result<Json<WebhookDispatchSummary>, ApiError> {
    Ok(Json(dispatch_request_webhooks(&state, request_id).await?))
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
            retired_at: None,
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
                    s.default_expiry_days, s.version, s.active, s.supersedes_schema_id,
                    s.retired_at, s.created_at
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
                version: row.get(8),
                active: row.get(9),
                supersedes_schema_id: row.get(10),
                retired_at: row.get(11),
                created_at: row.get(12),
            });
        }
    }
    Ok(Json(result))
}

fn verification_policy_view(row: &Row) -> VerificationPolicyView {
    VerificationPolicyView {
        id: row.get(0),
        display_name: row.get(1),
        description: row.get(2),
        purpose: row.get(3),
        credential_schema_id: row.get(4),
        credential_name: row.get(5),
        issuer_name: row.get(6),
        request_ttl_seconds: row.get(7),
        version: row.get(8),
        active: row.get(9),
        supersedes_policy_id: row.get(10),
        retired_at: row.get(11),
        created_at: row.get(12),
    }
}

async fn verifier_profile_id_for_account(db: &Pool, account: Uuid) -> Result<Uuid, ApiError> {
    let client = db_client(db).await?;
    client
        .query_opt(
            "SELECT id FROM verifier_profiles WHERE account_id = $1 AND retired_at IS NULL",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .map(|row| row.get(0))
        .ok_or(ApiError::Unauthorized)
}

async fn get_verification_policy_view(
    db: &Pool,
    account: Uuid,
    policy_id: Uuid,
) -> Result<VerificationPolicyView, ApiError> {
    let client = db_client(db).await?;
    let row = client
        .query_opt(
            "SELECT p.id, p.display_name, p.description, p.purpose,
                    p.credential_schema_id, s.display_name, i.display_name,
                    p.request_ttl_seconds, p.version, p.active,
                    p.supersedes_policy_id, p.retired_at, p.created_at
             FROM verification_policies p
             JOIN verifier_profiles v ON v.id = p.verifier_profile_id
             JOIN credential_schemas s ON s.id = p.credential_schema_id
             JOIN issuer_profiles i ON i.id = s.issuer_profile_id
             WHERE p.id = $1 AND v.account_id = $2",
            &[&policy_id, &account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::NotFound)?;
    Ok(verification_policy_view(&row))
}

async fn list_verification_policies(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<VerificationPolicyView>>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    let client = db_client(&state.db).await?;
    let rows = client
        .query(
            "SELECT p.id, p.display_name, p.description, p.purpose,
                    p.credential_schema_id, s.display_name, i.display_name,
                    p.request_ttl_seconds, p.version, p.active,
                    p.supersedes_policy_id, p.retired_at, p.created_at
             FROM verification_policies p
             JOIN verifier_profiles v ON v.id = p.verifier_profile_id
             JOIN credential_schemas s ON s.id = p.credential_schema_id
             JOIN issuer_profiles i ON i.id = s.issuer_profile_id
             WHERE v.account_id = $1
             ORDER BY p.created_at DESC
             LIMIT 512",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    Ok(Json(rows.iter().map(verification_policy_view).collect()))
}

async fn create_verification_policy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateVerificationPolicy>,
) -> Result<(StatusCode, Json<VerificationPolicyView>), ApiError> {
    let account = account_id(&headers, &state.db).await?;
    enforce_account_rate_limit(&state.db, account, "verification_policy_create", 20).await?;

    let display_name = input.display_name.trim().to_owned();
    let description = input.description.trim().to_owned();
    let purpose = input.purpose.trim().to_owned();
    let ttl = input.request_ttl_seconds.unwrap_or(300);
    if !valid_short_text(&display_name, 2, 120)
        || !valid_short_text(&description, 2, 1024)
        || !valid_short_text(&purpose, 2, 1024)
        || !(60..=900).contains(&ttl)
    {
        return Err(ApiError::Invalid);
    }

    let profile_id = verifier_profile_id_for_account(&state.db, account).await?;
    let schema = resolve_public_schema(&state.db, input.credential_schema_id).await?;
    let slug = schema_slug(&display_name);
    if slug.len() < 2 || slug.len() > 80 {
        return Err(ApiError::Invalid);
    }

    let client = db_client(&state.db).await?;
    let active: i64 = client
        .query_one(
            "SELECT COUNT(*) FROM verification_policies
             WHERE verifier_profile_id = $1 AND active = TRUE",
            &[&profile_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .get(0);
    if active >= MAX_ACTIVE_VERIFICATION_POLICIES {
        return Err(ApiError::TooManyRequests);
    }

    let id = Uuid::new_v4();
    client
        .execute(
            "INSERT INTO verification_policies
             (id, verifier_profile_id, slug, display_name, description, purpose,
              credential_schema_id, request_ttl_seconds)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            &[
                &id,
                &profile_id,
                &slug,
                &display_name,
                &description,
                &purpose,
                &schema.id,
                &i32::from(ttl),
            ],
        )
        .await
        .map_err(|error| {
            if error.as_db_error().and_then(|db| db.constraint())
                == Some("verification_policies_active_slug_idx")
            {
                ApiError::Conflict
            } else {
                ApiError::Unavailable
            }
        })?;

    let view = get_verification_policy_view(&state.db, account, id).await?;
    Ok((StatusCode::CREATED, Json(view)))
}

async fn create_verification_policy_version(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(policy_id): Path<Uuid>,
    Json(input): Json<CreateVerificationPolicyVersion>,
) -> Result<(StatusCode, Json<VerificationPolicyView>), ApiError> {
    let account = account_id(&headers, &state.db).await?;
    enforce_account_rate_limit(&state.db, account, "verification_policy_version", 20).await?;

    let description = input.description.trim().to_owned();
    let purpose = input.purpose.trim().to_owned();
    if !valid_short_text(&description, 2, 1024)
        || !valid_short_text(&purpose, 2, 1024)
        || !(60..=900).contains(&input.request_ttl_seconds)
    {
        return Err(ApiError::Invalid);
    }
    let schema = resolve_public_schema(&state.db, input.credential_schema_id).await?;

    let mut client = db_client(&state.db).await?;
    let tx = client
        .transaction()
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let current = tx
        .query_opt(
            "SELECT p.verifier_profile_id, p.slug, p.display_name, p.version
             FROM verification_policies p
             JOIN verifier_profiles v ON v.id = p.verifier_profile_id
             WHERE p.id = $1 AND v.account_id = $2 AND v.retired_at IS NULL AND p.active = TRUE
             FOR UPDATE OF p",
            &[&policy_id, &account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::Conflict)?;

    let profile_id: Uuid = current.get(0);
    let slug: String = current.get(1);
    let display_name: String = current.get(2);
    let version: i32 = current.get(3);
    let next_version = version.checked_add(1).ok_or(ApiError::Conflict)?;
    let now = OffsetDateTime::now_utc();
    let new_id = Uuid::new_v4();

    tx.execute(
        "UPDATE verification_policies
         SET active = FALSE, retired_at = $2
         WHERE id = $1 AND active = TRUE",
        &[&policy_id, &now],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;

    tx.execute(
        "INSERT INTO verification_policies
         (id, verifier_profile_id, slug, display_name, description, purpose,
          credential_schema_id, request_ttl_seconds, version, active,
          supersedes_policy_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, TRUE, $10)",
        &[
            &new_id,
            &profile_id,
            &slug,
            &display_name,
            &description,
            &purpose,
            &schema.id,
            &i32::from(input.request_ttl_seconds),
            &next_version,
            &policy_id,
        ],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;

    tx.commit().await.map_err(|_| ApiError::Unavailable)?;
    let view = get_verification_policy_view(&state.db, account, new_id).await?;
    Ok((StatusCode::CREATED, Json(view)))
}

async fn retire_verification_policy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(policy_id): Path<Uuid>,
) -> Result<Json<VerificationPolicyView>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    let client = db_client(&state.db).await?;
    let affected = client
        .execute(
            "UPDATE verification_policies p
             SET active = FALSE, retired_at = NOW()
             FROM verifier_profiles v
             WHERE p.id = $1
               AND p.verifier_profile_id = v.id
               AND v.account_id = $2
               AND v.retired_at IS NULL
               AND p.active = TRUE",
            &[&policy_id, &account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    if affected != 1 {
        return Err(ApiError::Conflict);
    }
    Ok(Json(
        get_verification_policy_view(&state.db, account, policy_id).await?,
    ))
}

async fn resolve_verification_policy(
    db: &Pool,
    account: Uuid,
    policy_id: Uuid,
) -> Result<ResolvedVerificationPolicy, ApiError> {
    let client = db_client(db).await?;
    let row = client
        .query_opt(
            "SELECT p.id, p.purpose, p.credential_schema_id, p.request_ttl_seconds
             FROM verification_policies p
             JOIN verifier_profiles v ON v.id = p.verifier_profile_id
             JOIN credential_schemas s ON s.id = p.credential_schema_id
             WHERE p.id = $1
               AND v.account_id = $2
               AND v.retired_at IS NULL
               AND p.active = TRUE
               AND s.active = TRUE",
            &[&policy_id, &account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::NotFound)?;
    let ttl: i32 = row.get(3);
    Ok(ResolvedVerificationPolicy {
        id: row.get(0),
        purpose: row.get(1),
        credential_schema_id: row.get(2),
        request_ttl_seconds: u16::try_from(ttl).map_err(|_| ApiError::Unavailable)?,
    })
}

async fn create_policy_verification_request_for_account(
    state: &AppState,
    account: Uuid,
    policy_id: Uuid,
    holder_zerant_id: String,
) -> Result<VerifierRequestView, ApiError> {
    let policy = resolve_verification_policy(&state.db, account, policy_id).await?;
    let input = CreateVerificationRequest {
        holder_zerant_id,
        purpose: policy.purpose,
        credential_schema_id: Some(policy.credential_schema_id),
        claim_type: None,
        context: None,
        accepted_issuer_ids: vec![],
    };
    create_verification_request_for_account(
        state,
        account,
        input,
        policy.request_ttl_seconds,
        Some(policy.id),
    )
    .await
}

async fn create_policy_verification_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(policy_id): Path<Uuid>,
    Json(input): Json<CreatePolicyVerificationRequest>,
) -> Result<(StatusCode, Json<VerifierRequestView>), ApiError> {
    let account = account_id(&headers, &state.db).await?;
    let request = create_policy_verification_request_for_account(
        &state,
        account,
        policy_id,
        input.holder_zerant_id,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(request)))
}

async fn create_integration_policy_verification_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(policy_id): Path<Uuid>,
    Json(input): Json<CreatePolicyVerificationRequest>,
) -> Result<(StatusCode, Json<VerifierRequestView>), ApiError> {
    let account = verifier_api_account_id(&headers, &state.db, "requests:create").await?;
    let request = create_policy_verification_request_for_account(
        &state,
        account,
        policy_id,
        input.holder_zerant_id,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(request)))
}

async fn create_verification_request_for_account(
    state: &AppState,
    account: Uuid,
    input: CreateVerificationRequest,
    request_ttl_seconds: u16,
    verification_policy_id: Option<Uuid>,
) -> Result<VerifierRequestView, ApiError> {
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
                "SELECT 1 FROM issuer_profiles WHERE issuer_id = $1 AND retired_at IS NULL",
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
               AND v.retired_at IS NULL
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
    let private = decrypt_stored_jwk(state, account, &verifier_secret)?;

    let now = OffsetDateTime::now_utc();
    let now_i64 = now.unix_timestamp();
    if now_i64 < 0 {
        return Err(ApiError::Unavailable);
    }
    let issued_at = now_i64 as u64;
    if !(60..=900).contains(&request_ttl_seconds) {
        return Err(ApiError::Invalid);
    }
    let expires_at = issued_at + u64::from(request_ttl_seconds);
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
              verifier_key_id, verification_policy_id)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
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
                &verification_policy_id,
            ],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    Ok(VerifierRequestView {
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
    })
}

async fn create_verification_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateVerificationRequest>,
) -> Result<(StatusCode, Json<VerifierRequestView>), ApiError> {
    let account = account_id(&headers, &state.db).await?;
    let request =
        create_verification_request_for_account(&state, account, input, 300, None).await?;
    Ok((StatusCode::CREATED, Json(request)))
}

async fn create_integration_verification_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateVerificationRequest>,
) -> Result<(StatusCode, Json<VerifierRequestView>), ApiError> {
    if input.credential_schema_id.is_none() || input.claim_type.is_some() || input.context.is_some()
    {
        return Err(ApiError::Invalid);
    }
    let account = verifier_api_account_id(&headers, &state.db, "requests:create").await?;
    let request =
        create_verification_request_for_account(&state, account, input, 300, None).await?;
    Ok((StatusCode::CREATED, Json(request)))
}

async fn get_integration_verification_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<VerifierRequestView>, ApiError> {
    let account = verifier_api_account_id(&headers, &state.db, "requests:read").await?;
    let client = db_client(&state.db).await?;
    client
        .execute(
            "UPDATE verification_requests r
             SET status = 'expired'
             FROM verifier_profiles p
             WHERE r.id = $1
               AND r.verifier_profile_id = p.id
               AND p.account_id = $2
               AND r.status = 'pending'
               AND r.expires_at <= NOW()",
            &[&id, &account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let row = client
        .query_opt(
            "SELECT r.id, a.public_handle, r.purpose, r.credential_schema_id, s.display_name,
                    r.claim_type, r.context, r.status, r.created_at, r.expires_at
             FROM verification_requests r
             JOIN verifier_profiles p ON p.id = r.verifier_profile_id
             JOIN accounts a ON a.id = r.subject_account_id
             LEFT JOIN credential_schemas s ON s.id = r.credential_schema_id
             WHERE p.account_id = $1 AND r.id = $2",
            &[&account, &id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::NotFound)?;

    let status: String = row.get(7);
    let holder: Option<String> = row.get(1);
    Ok(Json(VerifierRequestView {
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
    }))
}

async fn verifier_proof_for_account(
    state: &AppState,
    account: Uuid,
    id: Uuid,
) -> Result<VerifierProofPackage, ApiError> {
    let client = db_client(&state.db).await?;

    let row = client
        .query_opt(
            "SELECT r.status, r.request_id, r.request_jws,
                    r.response_ciphertext, r.response_data_nonce,
                    r.response_wrapped_dek, r.response_wrap_nonce, r.response_key_version,
                    r.proof_issuer_id, r.proof_issuer_key_id, r.proof_revocation_jws,
                    r.credential_schema_id, s.display_name, s.version, s.claim_type, s.context,
                    r.expires_at, r.decided_at,
                    v.verifier_id, v.origin, r.verifier_key_id, vk.public_jwk,
                    vk.valid_from, vk.retired_at, vk.compromised_at,
                    r.claim_type, r.context, sp.issuer_id
             FROM verification_requests r
             JOIN verifier_profiles v ON v.id = r.verifier_profile_id
             JOIN verifier_signing_keys vk
               ON vk.verifier_profile_id = v.id
              AND vk.verifier_key_id = r.verifier_key_id
             LEFT JOIN credential_schemas s ON s.id = r.credential_schema_id
             LEFT JOIN issuer_profiles sp ON sp.id = s.issuer_profile_id
             WHERE r.id = $1 AND v.account_id = $2",
            &[&id, &account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::NotFound)?;

    let status: String = row.get(0);
    if status != "approved" {
        return Err(ApiError::Conflict);
    }

    let response_ciphertext: Option<Vec<u8>> = row.get(3);
    let response_data_nonce: Option<Vec<u8>> = row.get(4);
    let response_wrapped_dek: Option<Vec<u8>> = row.get(5);
    let response_wrap_nonce: Option<Vec<u8>> = row.get(6);
    let response_key_version: Option<i32> = row.get(7);
    let issuer_id: Option<String> = row.get(8);
    let issuer_key_id: Option<String> = row.get(9);
    let revocation_jws: Option<String> = row.get(10);
    let decided_at: Option<OffsetDateTime> = row.get(17);

    let (
        response_ciphertext,
        response_data_nonce,
        response_wrapped_dek,
        response_wrap_nonce,
        response_key_version,
        issuer_id,
        issuer_key_id,
        revocation_jws,
        decided_at,
    ) = match (
        response_ciphertext,
        response_data_nonce,
        response_wrapped_dek,
        response_wrap_nonce,
        response_key_version,
        issuer_id,
        issuer_key_id,
        revocation_jws,
        decided_at,
    ) {
        (
            Some(ciphertext),
            Some(data_nonce),
            Some(wrapped_dek),
            Some(wrap_nonce),
            Some(key_version),
            Some(issuer_id),
            Some(issuer_key_id),
            Some(revocation_jws),
            Some(decided_at),
        ) => (
            ciphertext,
            data_nonce,
            wrapped_dek,
            wrap_nonce,
            key_version,
            issuer_id,
            issuer_key_id,
            revocation_jws,
            decided_at,
        ),
        _ => return Err(ApiError::Conflict),
    };

    let issuer_key_row = client
        .query_opt(
            "SELECT k.public_jwk, k.valid_from, k.retired_at, k.compromised_at
             FROM issuer_profiles p
             JOIN issuer_signing_keys k ON k.issuer_profile_id = p.id
             WHERE p.issuer_id = $1 AND k.issuer_key_id = $2",
            &[&issuer_id, &issuer_key_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::Unavailable)?;

    let issuer_public_value: Value = issuer_key_row.get(0);
    let issuer_public_jwk: PublicJwk =
        serde_json::from_value(issuer_public_value).map_err(|_| ApiError::Unavailable)?;
    let issuer_key = ProofSigningKeyView {
        key_id: issuer_key_id.clone(),
        public_jwk: issuer_public_jwk,
        valid_from: issuer_key_row.get(1),
        retired_at: issuer_key_row.get(2),
        compromised_at: issuer_key_row.get(3),
    };

    let verifier_public_value: Value = row.get(21);
    let verifier_public_jwk: PublicJwk =
        serde_json::from_value(verifier_public_value).map_err(|_| ApiError::Unavailable)?;
    let verifier_key = ProofSigningKeyView {
        key_id: row.get(20),
        public_jwk: verifier_public_jwk,
        valid_from: row.get(22),
        retired_at: row.get(23),
        compromised_at: row.get(24),
    };

    let secret = CredentialRow {
        id,
        ciphertext: response_ciphertext,
        data_nonce: response_data_nonce,
        wrapped_dek: response_wrapped_dek,
        wrap_nonce: response_wrap_nonce,
        key_version: response_key_version,
        created_at: decided_at,
        updated_at: decided_at,
    };
    let mut response_bytes = state.cipher.decrypt(account, &secret)?;
    if response_bytes.len() > MAX_CREDENTIAL_BYTES {
        response_bytes.fill(0);
        return Err(ApiError::Unavailable);
    }
    let response_jws = std::str::from_utf8(&response_bytes)
        .map_err(|_| ApiError::Unavailable)?
        .to_owned();
    response_bytes.fill(0);

    let claim_type: String = row.get(25);
    let context_value: String = row.get(26);
    let credential_schema_id: Option<Uuid> = row.get(11);
    let credential_schema = match credential_schema_id {
        Some(schema_id) => {
            let schema_issuer_id = row
                .get::<_, Option<String>>(27)
                .ok_or(ApiError::Unavailable)?;
            let schema_claim_type = row
                .get::<_, Option<String>>(14)
                .ok_or(ApiError::Unavailable)?;
            let schema_context = row
                .get::<_, Option<String>>(15)
                .ok_or(ApiError::Unavailable)?;
            if schema_issuer_id != issuer_id
                || schema_claim_type != claim_type
                || schema_context != context_value
            {
                return Err(ApiError::Unavailable);
            }
            Some(ProofCredentialSchemaView {
                id: schema_id,
                display_name: row
                    .get::<_, Option<String>>(12)
                    .ok_or(ApiError::Unavailable)?,
                version: row.get::<_, Option<i32>>(13).ok_or(ApiError::Unavailable)?,
                claim_type: schema_claim_type,
                context: schema_context,
            })
        }
        None => None,
    };

    issuer_key
        .public_jwk
        .validate()
        .map_err(|_| ApiError::Unavailable)?;
    verifier_key
        .public_jwk
        .validate()
        .map_err(|_| ApiError::Unavailable)?;

    let decided_seconds = decided_at.unix_timestamp();
    let proof_expires_at: OffsetDateTime = row.get(16);
    let proof_expiry_seconds = proof_expires_at.unix_timestamp();
    if decided_seconds < 0 || proof_expiry_seconds <= decided_seconds {
        return Err(ApiError::Unavailable);
    }
    let decided_u64 = u64::try_from(decided_seconds).map_err(|_| ApiError::Unavailable)?;
    let proof_expiry_u64 =
        u64::try_from(proof_expiry_seconds).map_err(|_| ApiError::Unavailable)?;

    let issuer_valid_from = issuer_key.valid_from.unix_timestamp();
    let verifier_valid_from = verifier_key.valid_from.unix_timestamp();
    if issuer_valid_from < 0 || verifier_valid_from < 0 {
        return Err(ApiError::Unavailable);
    }
    let issuer_compromised_at_decision =
        issuer_key.compromised_at.is_some_and(|at| at <= decided_at);
    let verifier_compromised_at_decision = verifier_key
        .compromised_at
        .is_some_and(|at| at <= decided_at);

    let trust = IssuerTrustManifest {
        issuers: vec![TrustedIssuer {
            issuer_id: issuer_id.clone(),
            keys: vec![TrustedIssuerKey {
                issuer_key_id: issuer_key.key_id.clone(),
                public_key: issuer_key.public_jwk.clone(),
                valid_from: u64::try_from(issuer_valid_from).map_err(|_| ApiError::Unavailable)?,
                valid_until: proof_expiry_u64,
                compromised: issuer_compromised_at_decision,
            }],
            allowed_claim_types: vec![claim_type],
            allowed_contexts: vec![context_value],
            source_schemas: vec![],
            policies: vec![],
        }],
    };

    let verifier_origin: String = row.get(19);
    let pin = VerifierPin {
        verifier_id: row.get(18),
        key_id: verifier_key.key_id.clone(),
        key: verifier_key.public_jwk.clone(),
        allowed_origins: vec![verifier_origin.clone()],
        valid_from: u64::try_from(verifier_valid_from).map_err(|_| ApiError::Unavailable)?,
        valid_until: proof_expiry_u64,
        compromised: verifier_compromised_at_decision,
    };
    let disclosure_context = DisclosureContext {
        pin: &pin,
        origin: &verifier_origin,
        trust: &trust,
        now: decided_u64,
        allowed_loopback: &[],
    };

    let request_jws: String = row.get(2);
    let protocol_request_id: String = row.get(1);
    let verified_request = verify_request(&request_jws, &pin, &verifier_origin, decided_u64, &[])
        .map_err(|_| ApiError::Unavailable)?;
    if verified_request.request_id != protocol_request_id
        || verified_request.expires_at != proof_expiry_u64
    {
        return Err(ApiError::Unavailable);
    }

    let verified_payload = verify_response(
        &request_jws,
        &response_jws,
        &disclosure_context,
        &revocation_jws,
        &issuer_id,
        0,
    )
    .map_err(|_| ApiError::Unavailable)?;
    let verified_result = verified_proof_result(&verified_payload.claim)?;

    Ok(VerifierProofPackage {
        schema: VERIFIER_PROOF_PACKAGE_SCHEMA,
        request_id: id,
        protocol_request_id,
        verifier_origin,
        credential_schema,
        issuer_id,
        issuer_key,
        verifier_id: pin.verifier_id.clone(),
        verifier_key,
        request_jws,
        response_jws,
        revocation_jws,
        decided_at,
        proof_expires_at,
        verified_result,
    })
}

async fn get_integration_verifier_proof(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<VerifierProofPackage>, ApiError> {
    let account = verifier_api_account_id(&headers, &state.db, "proofs:read").await?;
    Ok(Json(verifier_proof_for_account(&state, account, id).await?))
}

async fn get_verifier_proof(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<VerifierHumanProofView>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    let proof = verifier_proof_for_account(&state, account, id).await?;
    let credential_name = proof
        .credential_schema
        .as_ref()
        .map(|schema| schema.display_name.clone());
    let credential_version = proof
        .credential_schema
        .as_ref()
        .map(|schema| schema.version);
    Ok(Json(VerifierHumanProofView {
        request_id: proof.request_id,
        verifier_origin: proof.verifier_origin,
        credential_name,
        credential_version,
        issuer_id: proof.issuer_id,
        claim_type: proof.verified_result.claim_type,
        value: proof.verified_result.value,
        context: proof.verified_result.context,
        decided_at: proof.decided_at,
        proof_expires_at: proof.proof_expires_at,
    }))
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

async fn preview_holder_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<HolderRequestPreview>, ApiError> {
    let holder_account = account_id(&headers, &state.db).await?;
    let client = db_client(&state.db).await?;
    let row = client
        .query_opt(
            "SELECT r.id, v.display_name, v.origin, r.purpose, s.display_name,
                r.claim_type, r.context, r.created_at, r.expires_at, r.accepted_issuer_ids
         FROM verification_requests r
         JOIN verifier_profiles v ON v.id = r.verifier_profile_id
         LEFT JOIN credential_schemas s ON s.id = r.credential_schema_id
         WHERE r.id = $1 AND r.subject_account_id = $2
           AND r.status = 'pending' AND r.expires_at > NOW()",
            &[&id, &holder_account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::NotFound)?;
    let request = HolderRequestView {
        id: row.get(0),
        verifier_name: row.get(1),
        verifier_origin: row.get(2),
        purpose: row.get(3),
        credential_name: row.get(4),
        claim_type: row.get(5),
        context: row.get(6),
        created_at: row.get(7),
        expires_at: row.get(8),
    };
    let accepted: Vec<String> =
        serde_json::from_value(row.get::<_, Value>(9)).map_err(|_| ApiError::Unavailable)?;
    let (candidate, value, _) = select_holder_credential(
        &state,
        &client,
        holder_account,
        &request.claim_type,
        &request.context,
        &accepted,
    )
    .await?;
    Ok(Json(HolderRequestPreview {
        request,
        issuer_id: candidate.get(3),
        issuer_name: candidate.get(15),
        value,
    }))
}

async fn select_holder_credential(
    state: &AppState,
    client: &deadpool_postgres::Client,
    holder_account: Uuid,
    claim_type: &str,
    context: &str,
    accepted: &[String],
) -> Result<(Row, String, String), ApiError> {
    let candidates = client
        .query(
            "SELECT c.vault_record_id,
                    p.id, p.account_id, p.issuer_id,
                    k.id, k.issuer_key_id, k.public_jwk,
                    k.ciphertext, k.data_nonce, k.wrapped_dek, k.wrap_nonce, k.key_version,
                    k.valid_from, k.compromised_at, c.expires_at, p.display_name
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
        || record.get("claim_type").and_then(Value::as_str) != Some(claim_type)
        || record.get("context").and_then(Value::as_str) != Some(context)
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

    Ok((candidate, value, source_jws))
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
        let mut client = db_client(&state.db).await?;
        let tx = client
            .transaction()
            .await
            .map_err(|_| ApiError::Unavailable)?;
        let row = tx
            .query_opt(
                "UPDATE verification_requests
                 SET status = 'denied', decided_at = NOW()
                 WHERE id = $1 AND subject_account_id = $2
                   AND status = 'pending' AND expires_at > NOW()
                 RETURNING verifier_profile_id, credential_schema_id, decided_at",
                &[&id, &holder_account],
            )
            .await
            .map_err(|_| ApiError::Unavailable)?
            .ok_or(ApiError::Conflict)?;

        let verifier_profile_id: Uuid = row.get(0);
        let credential_schema_id: Option<Uuid> = row.get(1);
        let decided_at: OffsetDateTime = row.get(2);
        enqueue_verifier_webhooks(
            &tx,
            verifier_profile_id,
            id,
            "verification.denied",
            credential_schema_id,
            decided_at,
        )
        .await?;
        tx.commit().await.map_err(|_| ApiError::Unavailable)?;

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

    let (candidate, value, source_jws) = select_holder_credential(
        &state,
        &client,
        holder_account,
        &claim_type,
        &context,
        &accepted,
    )
    .await?;

    if !approval_matches_preview(&input, &candidate.get::<_, String>(3), &value) {
        return Err(ApiError::Conflict);
    }

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

    let mut tx_client = db_client(&state.db).await?;
    let tx = tx_client
        .transaction()
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let row = tx
        .query_opt(
            "UPDATE verification_requests
             SET status = 'approved',
                 response_ciphertext = $3,
                 response_data_nonce = $4,
                 response_wrapped_dek = $5,
                 response_wrap_nonce = $6,
                 response_key_version = $7,
                 proof_issuer_id = $8,
                 proof_issuer_key_id = $9,
                 proof_revocation_jws = $10,
                 decided_at = NOW()
             WHERE id = $1 AND subject_account_id = $2
               AND status = 'pending' AND expires_at > NOW()
             RETURNING credential_schema_id, decided_at",
            &[
                &id,
                &holder_account,
                &encrypted_response.ciphertext,
                &encrypted_response.data_nonce,
                &encrypted_response.wrapped_dek,
                &encrypted_response.wrap_nonce,
                &encrypted_response.key_version,
                &issuer_id,
                &issuer_key_id,
                &revocation_jws,
            ],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::Conflict)?;

    let credential_schema_id: Option<Uuid> = row.get(0);
    let decided_at: OffsetDateTime = row.get(1);
    enqueue_verifier_webhooks(
        &tx,
        verifier_profile_id,
        id,
        "verification.approved",
        credential_schema_id,
        decided_at,
    )
    .await?;
    tx.commit().await.map_err(|_| ApiError::Unavailable)?;

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

fn public_issuer_cursor(created_at: OffsetDateTime, id: Uuid) -> String {
    URL_SAFE_NO_PAD.encode(format!("{}:{id}", created_at.unix_timestamp_nanos()))
}

fn parse_public_issuer_cursor(value: &str) -> Result<(OffsetDateTime, Uuid), ApiError> {
    if value.len() > 256 {
        return Err(ApiError::Invalid);
    }
    let decoded = URL_SAFE_NO_PAD
        .decode(value.as_bytes())
        .map_err(|_| ApiError::Invalid)?;
    let text = std::str::from_utf8(&decoded).map_err(|_| ApiError::Invalid)?;
    let (nanos, id) = text.split_once(':').ok_or(ApiError::Invalid)?;
    let nanos = nanos.parse::<i128>().map_err(|_| ApiError::Invalid)?;
    let id = Uuid::parse_str(id).map_err(|_| ApiError::Invalid)?;
    let created_at =
        OffsetDateTime::from_unix_timestamp_nanos(nanos).map_err(|_| ApiError::Invalid)?;
    Ok((created_at, id))
}

fn validate_public_issuer_id(value: &str) -> Result<(), ApiError> {
    if !value.starts_with("zerant:issuer:")
        || value.len() > 128
        || value.chars().any(char::is_control)
    {
        return Err(ApiError::Invalid);
    }
    Ok(())
}

fn public_cache_headers(max_age: u32) -> Result<HeaderMap, ApiError> {
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::CACHE_CONTROL,
        HeaderValue::from_str(&format!(
            "public, max-age={max_age}, stale-while-revalidate=60"
        ))
        .map_err(|_| ApiError::Unavailable)?,
    );
    Ok(headers)
}

async fn public_issuer_directory(
    State(state): State<AppState>,
    Query(query): Query<PublicIssuerDirectoryQuery>,
) -> Result<(HeaderMap, Json<PublicIssuerDirectoryPage>), ApiError> {
    let limit = usize::from(query.limit.unwrap_or(25).clamp(1, 100));
    let fetch_limit = i64::try_from(limit + 1).map_err(|_| ApiError::Invalid)?;
    let client = db_client(&state.db).await?;

    let rows = if let Some(cursor) = query.cursor.as_deref() {
        let (created_at, profile_id) = parse_public_issuer_cursor(cursor)?;
        client
            .query(
                "SELECT p.id, p.issuer_id, p.display_name, p.created_at,
                        k.issuer_key_id,
                        COUNT(s.id) FILTER (WHERE s.active = TRUE) AS active_schema_count,
                        COALESCE(r.version, 1) AS revocation_version
                 FROM issuer_profiles p
                 LEFT JOIN issuer_signing_keys k
                   ON k.issuer_profile_id = p.id
                  AND k.retired_at IS NULL
                  AND k.compromised_at IS NULL
                 LEFT JOIN credential_schemas s ON s.issuer_profile_id = p.id
                 LEFT JOIN issuer_revocation_state r ON r.issuer_profile_id = p.id
                 WHERE p.retired_at IS NULL
                   AND (p.created_at, p.id) < ($1, $2)
                 GROUP BY p.id, p.issuer_id, p.display_name, p.created_at,
                          k.issuer_key_id, r.version
                 ORDER BY p.created_at DESC, p.id DESC
                 LIMIT $3",
                &[&created_at, &profile_id, &fetch_limit],
            )
            .await
            .map_err(|_| ApiError::Unavailable)?
    } else {
        client
            .query(
                "SELECT p.id, p.issuer_id, p.display_name, p.created_at,
                        k.issuer_key_id,
                        COUNT(s.id) FILTER (WHERE s.active = TRUE) AS active_schema_count,
                        COALESCE(r.version, 1) AS revocation_version
                 FROM issuer_profiles p
                 LEFT JOIN issuer_signing_keys k
                   ON k.issuer_profile_id = p.id
                  AND k.retired_at IS NULL
                  AND k.compromised_at IS NULL
                 LEFT JOIN credential_schemas s ON s.issuer_profile_id = p.id
                 LEFT JOIN issuer_revocation_state r ON r.issuer_profile_id = p.id
                 WHERE p.retired_at IS NULL
                 GROUP BY p.id, p.issuer_id, p.display_name, p.created_at,
                          k.issuer_key_id, r.version
                 ORDER BY p.created_at DESC, p.id DESC
                 LIMIT $1",
                &[&fetch_limit],
            )
            .await
            .map_err(|_| ApiError::Unavailable)?
    };

    let has_more = rows.len() > limit;
    let mut items = Vec::with_capacity(limit.min(rows.len()));
    for row in rows.into_iter().take(limit) {
        let schema_count: i64 = row.get(5);
        let version_i64: i64 = row.get(6);
        if schema_count < 0 || version_i64 <= 0 {
            return Err(ApiError::Unavailable);
        }
        items.push(PublicIssuerDirectoryItem {
            profile_id: row.get(0),
            issuer_id: row.get(1),
            display_name: row.get(2),
            created_at: row.get(3),
            active_key_id: row.get(4),
            active_schema_count: u32::try_from(schema_count).map_err(|_| ApiError::Unavailable)?,
            revocation_version: u64::try_from(version_i64).map_err(|_| ApiError::Unavailable)?,
        });
    }

    let next_cursor = if has_more {
        items
            .last()
            .map(|item| public_issuer_cursor(item.created_at, item.profile_id))
    } else {
        None
    };

    Ok((
        public_cache_headers(120)?,
        Json(PublicIssuerDirectoryPage { items, next_cursor }),
    ))
}

async fn public_issuer_metadata(
    State(state): State<AppState>,
    Path(issuer_id): Path<String>,
) -> Result<(HeaderMap, Json<PublicIssuerMetadata>), ApiError> {
    validate_public_issuer_id(&issuer_id)?;
    let client = db_client(&state.db).await?;
    let profile = client
        .query_opt(
            "SELECT id, display_name FROM issuer_profiles WHERE issuer_id = $1",
            &[&issuer_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::NotFound)?;
    let profile_id: Uuid = profile.get(0);

    let key_rows = client
        .query(
            "SELECT issuer_key_id, public_jwk, valid_from, retired_at, compromised_at
             FROM issuer_signing_keys
             WHERE issuer_profile_id = $1
             ORDER BY valid_from DESC
             LIMIT 64",
            &[&profile_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let mut keys = Vec::with_capacity(key_rows.len());
    for row in key_rows {
        let value: Value = row.get(1);
        let public_jwk: PublicJwk =
            serde_json::from_value(value).map_err(|_| ApiError::Unavailable)?;
        public_jwk.validate().map_err(|_| ApiError::Unavailable)?;
        keys.push(PublicIssuerKeyView {
            key_id: row.get(0),
            public_jwk,
            valid_from: row.get(2),
            retired_at: row.get(3),
            compromised_at: row.get(4),
        });
    }

    let schema_rows = client
        .query(
            "SELECT id, display_name, description, claim_type, context, default_expiry_days,
                    version, active, supersedes_schema_id, retired_at, created_at
             FROM credential_schemas
             WHERE issuer_profile_id = $1
             ORDER BY slug ASC, version DESC
             LIMIT 256",
            &[&profile_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let schemas = schema_rows
        .into_iter()
        .map(|row| PublicCredentialSchemaView {
            id: row.get(0),
            display_name: row.get(1),
            description: row.get(2),
            claim_type: row.get(3),
            context: row.get(4),
            default_expiry_days: row.get(5),
            version: row.get(6),
            active: row.get(7),
            supersedes_schema_id: row.get(8),
            retired_at: row.get(9),
            created_at: row.get(10),
        })
        .collect();

    let revocation_row = client
        .query_opt(
            "SELECT version FROM issuer_revocation_state WHERE issuer_profile_id = $1",
            &[&profile_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let version_i64 = revocation_row.map(|row| row.get::<_, i64>(0)).unwrap_or(1);
    let revocation_version = u64::try_from(version_i64).map_err(|_| ApiError::Unavailable)?;

    Ok((
        public_cache_headers(300)?,
        Json(PublicIssuerMetadata {
            issuer_id,
            display_name: profile.get(1),
            keys,
            credential_schemas: schemas,
            revocation_version,
        }),
    ))
}

async fn public_issuer_revocation(
    State(state): State<AppState>,
    Path(issuer_id): Path<String>,
) -> Result<(HeaderMap, Json<PublicRevocationPublication>), ApiError> {
    validate_public_issuer_id(&issuer_id)?;
    let client = db_client(&state.db).await?;
    let row = client
        .query_opt(
            "SELECT p.id, p.account_id, p.issuer_id, k.issuer_key_id,
                    k.ciphertext, k.data_nonce, k.wrapped_dek, k.wrap_nonce, k.key_version,
                    COALESCE(r.version, 1)
             FROM issuer_profiles p
             JOIN issuer_signing_keys k ON k.issuer_profile_id = p.id
             LEFT JOIN issuer_revocation_state r ON r.issuer_profile_id = p.id
             WHERE p.issuer_id = $1
               AND k.retired_at IS NULL
               AND k.compromised_at IS NULL
             ORDER BY k.valid_from DESC
             LIMIT 1",
            &[&issuer_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::NotFound)?;

    let profile_id: Uuid = row.get(0);
    let account_id: Uuid = row.get(1);
    let active_key_id: String = row.get(3);
    let version_i64: i64 = row.get(9);
    if version_i64 <= 0 {
        return Err(ApiError::Unavailable);
    }
    let version = u64::try_from(version_i64).map_err(|_| ApiError::Unavailable)?;
    let now = OffsetDateTime::now_utc();

    if let Some(publication) = client
        .query_opt(
            "SELECT version, issuer_key_id, snapshot_jws, issued_at, next_update
             FROM issuer_revocation_publications
             WHERE issuer_profile_id = $1",
            &[&profile_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
    {
        let published_version: i64 = publication.get(0);
        let published_key: String = publication.get(1);
        let next_update: OffsetDateTime = publication.get(4);
        if published_version == version_i64 && published_key == active_key_id && next_update > now {
            return Ok((
                public_cache_headers(60)?,
                Json(PublicRevocationPublication {
                    issuer_id,
                    version,
                    issuer_key_id: published_key,
                    snapshot_jws: publication.get(2),
                    issued_at: publication.get(3),
                    next_update,
                }),
            ));
        }
    }

    let secret = secret_row(&row, profile_id, 4);
    let private = decrypt_stored_jwk(&state, account_id, &secret)?;
    let issued_at_i64 = now.unix_timestamp();
    if issued_at_i64 < 0 {
        return Err(ApiError::Unavailable);
    }
    let issued_at = issued_at_i64 as u64;
    let next_update_time = now + Duration::hours(6);
    let next_update_i64 = next_update_time.unix_timestamp();
    if next_update_i64 <= issued_at_i64 {
        return Err(ApiError::Unavailable);
    }
    let snapshot_jws = signed_issuer_revocation_snapshot(
        &state,
        profile_id,
        &issuer_id,
        &active_key_id,
        &private,
        issued_at,
        next_update_i64 as u64,
    )
    .await?;

    client
        .execute(
            "INSERT INTO issuer_revocation_publications
             (issuer_profile_id, version, issuer_key_id, snapshot_jws, issued_at, next_update, updated_at)
             VALUES ($1,$2,$3,$4,$5,$6,NOW())
             ON CONFLICT (issuer_profile_id)
             DO UPDATE SET version = EXCLUDED.version,
                           issuer_key_id = EXCLUDED.issuer_key_id,
                           snapshot_jws = EXCLUDED.snapshot_jws,
                           issued_at = EXCLUDED.issued_at,
                           next_update = EXCLUDED.next_update,
                           updated_at = NOW()",
            &[
                &profile_id,
                &version_i64,
                &active_key_id,
                &snapshot_jws,
                &now,
                &next_update_time,
            ],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    Ok((
        public_cache_headers(60)?,
        Json(PublicRevocationPublication {
            issuer_id,
            version,
            issuer_key_id: active_key_id,
            snapshot_jws,
            issued_at: now,
            next_update: next_update_time,
        }),
    ))
}

async fn list_issuer_team(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<IssuerMemberView>>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    let access = issuer_access(&state.db, account).await?;
    let client = db_client(&state.db).await?;
    let rows = client
        .query(
            "SELECT a.public_handle, 'owner'::text AS role, TRUE AS owner, p.created_at
             FROM issuer_profiles p
             JOIN accounts a ON a.id = p.account_id
             WHERE p.id = $1
             UNION ALL
             SELECT a.public_handle, m.role, FALSE AS owner, m.joined_at
             FROM issuer_members m
             JOIN accounts a ON a.id = m.account_id
             WHERE m.issuer_profile_id = $1
             ORDER BY owner DESC, created_at ASC",
            &[&access.profile_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let mut members = Vec::with_capacity(rows.len());
    for row in rows {
        let zerant_id: Option<String> = row.get(0);
        members.push(IssuerMemberView {
            zerant_id: zerant_id.ok_or(ApiError::Unavailable)?,
            role: row.get(1),
            owner: row.get(2),
            joined_at: row.get(3),
        });
    }
    Ok(Json(members))
}

async fn list_issuer_team_invitations(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<IssuerInvitationView>>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    let access = issuer_access(&state.db, account).await?;
    require_issuer_role(&access, &["owner", "admin"])?;
    let client = db_client(&state.db).await?;
    client
        .execute(
            "UPDATE issuer_invitations
             SET status = 'expired', decided_at = NOW()
             WHERE issuer_profile_id = $1
               AND status = 'pending'
               AND expires_at <= NOW()",
            &[&access.profile_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let rows = client
        .query(
            "SELECT i.id, p.display_name, p.issuer_id, a.public_handle, i.role,
                    i.status, i.created_at, i.expires_at
             FROM issuer_invitations i
             JOIN issuer_profiles p ON p.id = i.issuer_profile_id
             JOIN accounts a ON a.id = i.invited_account_id
             WHERE i.issuer_profile_id = $1
               AND i.status = 'pending'
             ORDER BY i.created_at DESC
             LIMIT 128",
            &[&access.profile_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let mut invitations = Vec::with_capacity(rows.len());
    for row in rows {
        let handle: Option<String> = row.get(3);
        invitations.push(IssuerInvitationView {
            id: row.get(0),
            issuer_name: row.get(1),
            issuer_id: row.get(2),
            invited_zerant_id: handle.ok_or(ApiError::Unavailable)?,
            role: row.get(4),
            status: row.get(5),
            created_at: row.get(6),
            expires_at: row.get(7),
        });
    }
    Ok(Json(invitations))
}

async fn invite_issuer_member(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<InviteIssuerMember>,
) -> Result<(StatusCode, Json<IssuerInvitationView>), ApiError> {
    let account = account_id(&headers, &state.db).await?;
    enforce_account_rate_limit(&state.db, account, "issuer_team_invite", 20).await?;
    let access = issuer_access(&state.db, account).await?;
    require_issuer_role(&access, &["owner", "admin"])?;
    require_active_issuer(&access)?;

    let zerant_id = input.zerant_id.trim().to_owned();
    let role = input.role.trim().to_owned();
    if !zerant_id.starts_with("zr_")
        || !valid_short_text(&zerant_id, 27, 27)
        || !valid_issuer_member_role(&role)
    {
        return Err(ApiError::Invalid);
    }

    let mut client = db_client(&state.db).await?;
    let target = client
        .query_opt(
            "SELECT id FROM accounts WHERE public_handle = $1",
            &[&zerant_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::NotFound)?;
    let target_account: Uuid = target.get(0);
    if target_account == account {
        return Err(ApiError::Invalid);
    }

    let affiliation = client
        .query_one(
            "SELECT EXISTS(SELECT 1 FROM issuer_profiles WHERE account_id = $1),
                    EXISTS(SELECT 1 FROM issuer_members WHERE account_id = $1)",
            &[&target_account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let owns_issuer: bool = affiliation.get(0);
    let member_elsewhere: bool = affiliation.get(1);
    if owns_issuer || member_elsewhere {
        return Err(ApiError::Conflict);
    }

    let id = Uuid::new_v4();
    let expires_at = OffsetDateTime::now_utc() + Duration::days(7);
    let tx = client
        .transaction()
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let row = tx
        .query_one(
            "INSERT INTO issuer_invitations
             (id, issuer_profile_id, invited_account_id, invited_by_account_id, role, expires_at)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (issuer_profile_id, invited_account_id)
                 WHERE status = 'pending'
             DO UPDATE SET role = EXCLUDED.role,
                           invited_by_account_id = EXCLUDED.invited_by_account_id,
                           created_at = NOW(),
                           expires_at = EXCLUDED.expires_at
             RETURNING id, status, created_at, expires_at",
            &[
                &id,
                &access.profile_id,
                &target_account,
                &account,
                &role,
                &expires_at,
            ],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let profile = tx
        .query_one(
            "SELECT display_name, issuer_id FROM issuer_profiles WHERE id = $1",
            &[&access.profile_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let invitation_id: Uuid = row.get(0);
    record_issuer_event(
        &tx,
        access.profile_id,
        account,
        IssuerEvent {
            event_type: "team_invited",
            object_id: &invitation_id.to_string(),
            label: &role,
            context: None,
            counterparty: Some(&zerant_id),
        },
    )
    .await?;
    tx.commit().await.map_err(|_| ApiError::Unavailable)?;

    Ok((
        StatusCode::CREATED,
        Json(IssuerInvitationView {
            id: row.get(0),
            issuer_name: profile.get(0),
            issuer_id: profile.get(1),
            invited_zerant_id: zerant_id,
            role,
            status: row.get(1),
            created_at: row.get(2),
            expires_at: row.get(3),
        }),
    ))
}

async fn list_my_issuer_invitations(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<IssuerInvitationView>>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    let client = db_client(&state.db).await?;
    client
        .execute(
            "UPDATE issuer_invitations
             SET status = 'expired', decided_at = NOW()
             WHERE invited_account_id = $1
               AND status = 'pending'
               AND expires_at <= NOW()",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let rows = client
        .query(
            "SELECT i.id, p.display_name, p.issuer_id, a.public_handle, i.role,
                    i.status, i.created_at, i.expires_at
             FROM issuer_invitations i
             JOIN issuer_profiles p ON p.id = i.issuer_profile_id
             JOIN accounts a ON a.id = i.invited_account_id
             WHERE i.invited_account_id = $1
               AND i.status = 'pending'
               AND i.expires_at > NOW()
             ORDER BY i.created_at DESC
             LIMIT 64",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let mut invitations = Vec::with_capacity(rows.len());
    for row in rows {
        let handle: Option<String> = row.get(3);
        invitations.push(IssuerInvitationView {
            id: row.get(0),
            issuer_name: row.get(1),
            issuer_id: row.get(2),
            invited_zerant_id: handle.ok_or(ApiError::Unavailable)?,
            role: row.get(4),
            status: row.get(5),
            created_at: row.get(6),
            expires_at: row.get(7),
        });
    }
    Ok(Json(invitations))
}

async fn decide_issuer_invitation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(input): Json<DecideIssuerInvitation>,
) -> Result<Json<IssuerInvitationView>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    if !matches!(input.decision.as_str(), "accept" | "decline") {
        return Err(ApiError::Invalid);
    }

    let mut client = db_client(&state.db).await?;
    let tx = client
        .transaction()
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let row = tx
        .query_opt(
            "SELECT i.issuer_profile_id, i.role, i.status, i.created_at, i.expires_at,
                    p.display_name, p.issuer_id, a.public_handle, p.retired_at
             FROM issuer_invitations i
             JOIN issuer_profiles p ON p.id = i.issuer_profile_id
             JOIN accounts a ON a.id = i.invited_account_id
             WHERE i.id = $1 AND i.invited_account_id = $2
             FOR UPDATE OF i, p",
            &[&id, &account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::NotFound)?;

    let status: String = row.get(2);
    let expires_at: OffsetDateTime = row.get(4);
    if status != "pending" || expires_at <= OffsetDateTime::now_utc() {
        return Err(ApiError::Conflict);
    }
    let issuer_retired_at: Option<OffsetDateTime> = row.get(8);
    if issuer_retired_at.is_some() {
        return Err(ApiError::Conflict);
    }

    if input.decision == "decline" {
        tx.execute(
            "UPDATE issuer_invitations
             SET status = 'declined', decided_at = NOW()
             WHERE id = $1 AND status = 'pending'",
            &[&id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    } else {
        let affiliation = tx
            .query_one(
                "SELECT EXISTS(SELECT 1 FROM issuer_profiles WHERE account_id = $1),
                        EXISTS(SELECT 1 FROM issuer_members WHERE account_id = $1)",
                &[&account],
            )
            .await
            .map_err(|_| ApiError::Unavailable)?;
        if affiliation.get::<_, bool>(0) || affiliation.get::<_, bool>(1) {
            return Err(ApiError::Conflict);
        }

        tx.execute(
            "INSERT INTO issuer_members(issuer_profile_id, account_id, role)
             VALUES ($1, $2, $3)",
            &[&row.get::<_, Uuid>(0), &account, &row.get::<_, String>(1)],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
        tx.execute(
            "UPDATE issuer_invitations
             SET status = 'accepted', decided_at = NOW()
             WHERE id = $1 AND status = 'pending'",
            &[&id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    }

    let handle: Option<String> = row.get(7);
    let handle_ref = handle.as_deref().ok_or(ApiError::Unavailable)?;
    let event_type = if input.decision == "accept" {
        "team_joined"
    } else {
        "team_declined"
    };
    let role: String = row.get(1);
    record_issuer_event(
        &tx,
        row.get(0),
        account,
        IssuerEvent {
            event_type,
            object_id: &id.to_string(),
            label: &role,
            context: None,
            counterparty: Some(handle_ref),
        },
    )
    .await?;
    tx.commit().await.map_err(|_| ApiError::Unavailable)?;
    Ok(Json(IssuerInvitationView {
        id,
        issuer_name: row.get(5),
        issuer_id: row.get(6),
        invited_zerant_id: handle_ref.to_owned(),
        role: row.get(1),
        status: if input.decision == "accept" {
            "accepted".into()
        } else {
            "declined".into()
        },
        created_at: row.get(3),
        expires_at,
    }))
}

async fn remove_issuer_member(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(zerant_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    let access = issuer_access(&state.db, account).await?;
    require_issuer_role(&access, &["owner", "admin"])?;
    let mut client = db_client(&state.db).await?;
    let tx = client
        .transaction()
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let row = tx
        .query_opt(
            "SELECT m.account_id, m.role
             FROM issuer_members m
             JOIN accounts a ON a.id = m.account_id
             WHERE m.issuer_profile_id = $1 AND a.public_handle = $2",
            &[&access.profile_id, &zerant_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::NotFound)?;
    let target_role: String = row.get(1);
    if access.role == "admin" && target_role == "admin" {
        return Err(ApiError::Forbidden);
    }

    let affected = tx
        .execute(
            "DELETE FROM issuer_members
             WHERE issuer_profile_id = $1 AND account_id = $2",
            &[&access.profile_id, &row.get::<_, Uuid>(0)],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    if affected != 1 {
        return Err(ApiError::Conflict);
    }
    record_issuer_event(
        &tx,
        access.profile_id,
        account,
        IssuerEvent {
            event_type: "team_member_removed",
            object_id: &zerant_id,
            label: &target_role,
            context: None,
            counterparty: Some(&zerant_id),
        },
    )
    .await?;
    tx.commit().await.map_err(|_| ApiError::Unavailable)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn transfer_issuer_ownership(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<TransferIssuerOwnership>,
) -> Result<Json<IssuerMemberView>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    let access = issuer_access(&state.db, account).await?;
    require_issuer_role(&access, &["owner"])?;

    let zerant_id = input.zerant_id.trim().to_owned();
    if !zerant_id.starts_with("zr_") || !valid_short_text(&zerant_id, 27, 27) {
        return Err(ApiError::Invalid);
    }

    let mut client = db_client(&state.db).await?;
    let tx = client
        .transaction()
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let target = tx
        .query_opt(
            "SELECT m.account_id, m.joined_at
             FROM issuer_members m
             JOIN accounts a ON a.id = m.account_id
             WHERE m.issuer_profile_id = $1 AND a.public_handle = $2
             FOR UPDATE OF m",
            &[&access.profile_id, &zerant_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::NotFound)?;
    let new_owner: Uuid = target.get(0);
    if new_owner == access.owner_account_id {
        return Err(ApiError::Invalid);
    }

    let owns_other: bool = tx
        .query_one(
            "SELECT EXISTS(
                 SELECT 1 FROM issuer_profiles
                 WHERE account_id = $1 AND id <> $2
             )",
            &[&new_owner, &access.profile_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .get(0);
    if owns_other {
        return Err(ApiError::Conflict);
    }

    let profile_secret = tx
        .query_one(
            "SELECT ciphertext, data_nonce, wrapped_dek, wrap_nonce, key_version
             FROM issuer_profiles
             WHERE id = $1 AND account_id = $2
             FOR UPDATE",
            &[&access.profile_id, &access.owner_account_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let now = OffsetDateTime::now_utc();
    let profile_row = CredentialRow {
        id: access.profile_id,
        ciphertext: profile_secret.get(0),
        data_nonce: profile_secret.get(1),
        wrapped_dek: profile_secret.get(2),
        wrap_nonce: profile_secret.get(3),
        key_version: profile_secret.get(4),
        created_at: now,
        updated_at: now,
    };
    let mut profile_plaintext = state
        .cipher
        .decrypt(access.owner_account_id, &profile_row)?;
    let profile_rewrapped =
        state
            .cipher
            .encrypt(new_owner, access.profile_id, &profile_plaintext)?;
    profile_plaintext.fill(0);

    let key_rows = tx
        .query(
            "SELECT id, ciphertext, data_nonce, wrapped_dek, wrap_nonce, key_version
             FROM issuer_signing_keys
             WHERE issuer_profile_id = $1
             FOR UPDATE",
            &[&access.profile_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let mut rewrapped = Vec::with_capacity(key_rows.len());
    for row in key_rows {
        let key_id: Uuid = row.get(0);
        let encrypted_row = CredentialRow {
            id: key_id,
            ciphertext: row.get(1),
            data_nonce: row.get(2),
            wrapped_dek: row.get(3),
            wrap_nonce: row.get(4),
            key_version: row.get(5),
            created_at: now,
            updated_at: now,
        };
        let mut plaintext = state
            .cipher
            .decrypt(access.owner_account_id, &encrypted_row)?;
        let encrypted = state.cipher.encrypt(new_owner, key_id, &plaintext)?;
        plaintext.fill(0);
        rewrapped.push((key_id, encrypted));
    }

    tx.execute(
        "UPDATE issuer_profiles
         SET account_id = $2,
             ciphertext = $3,
             data_nonce = $4,
             wrapped_dek = $5,
             wrap_nonce = $6,
             key_version = $7
         WHERE id = $1 AND account_id = $8",
        &[
            &access.profile_id,
            &new_owner,
            &profile_rewrapped.ciphertext,
            &profile_rewrapped.data_nonce,
            &profile_rewrapped.wrapped_dek,
            &profile_rewrapped.wrap_nonce,
            &profile_rewrapped.key_version,
            &access.owner_account_id,
        ],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;

    for (key_id, encrypted) in rewrapped {
        tx.execute(
            "UPDATE issuer_signing_keys
             SET ciphertext = $3,
                 data_nonce = $4,
                 wrapped_dek = $5,
                 wrap_nonce = $6,
                 key_version = $7
             WHERE id = $1 AND issuer_profile_id = $2",
            &[
                &key_id,
                &access.profile_id,
                &encrypted.ciphertext,
                &encrypted.data_nonce,
                &encrypted.wrapped_dek,
                &encrypted.wrap_nonce,
                &encrypted.key_version,
            ],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    }

    tx.execute(
        "DELETE FROM issuer_members
         WHERE issuer_profile_id = $1 AND account_id = $2",
        &[&access.profile_id, &new_owner],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;
    tx.execute(
        "INSERT INTO issuer_members(issuer_profile_id, account_id, role)
         VALUES ($1, $2, 'admin')
         ON CONFLICT (issuer_profile_id, account_id)
         DO UPDATE SET role = 'admin'",
        &[&access.profile_id, &access.owner_account_id],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;

    record_issuer_event(
        &tx,
        access.profile_id,
        account,
        IssuerEvent {
            event_type: "ownership_transferred",
            object_id: &access.profile_id.to_string(),
            label: "Ownership transferred",
            context: None,
            counterparty: Some(&zerant_id),
        },
    )
    .await?;

    tx.commit().await.map_err(|_| ApiError::Unavailable)?;

    Ok(Json(IssuerMemberView {
        zerant_id,
        role: "owner".into(),
        owner: true,
        joined_at: target.get(1),
    }))
}

async fn retire_issuer_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let account = recent_account_id(&headers, &state.db).await?;
    enforce_account_rate_limit(&state.db, account, "issuer_retire", 3).await?;
    let access = issuer_access(&state.db, account).await?;
    require_issuer_role(&access, &["owner"])?;

    let mut client = db_client(&state.db).await?;
    let tx = client
        .transaction()
        .await
        .map_err(|_| ApiError::Unavailable)?;
    let row = tx
        .query_opt(
            "SELECT display_name, retired_at
             FROM issuer_profiles
             WHERE id = $1 AND account_id = $2
             FOR UPDATE",
            &[&access.profile_id, &account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::NotFound)?;
    let retired_at: Option<OffsetDateTime> = row.get(1);
    if retired_at.is_some() {
        return Err(ApiError::Conflict);
    }
    let display_name: String = row.get(0);

    tx.execute(
        "UPDATE issuer_profiles SET retired_at = NOW() WHERE id = $1 AND retired_at IS NULL",
        &[&access.profile_id],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;
    tx.execute(
        "UPDATE credential_schemas
         SET active = FALSE, retired_at = COALESCE(retired_at, NOW()), updated_at = NOW()
         WHERE issuer_profile_id = $1 AND active = TRUE",
        &[&access.profile_id],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;
    tx.execute(
        "UPDATE issuer_invitations
         SET status = 'cancelled', decided_at = NOW()
         WHERE issuer_profile_id = $1 AND status = 'pending'",
        &[&access.profile_id],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;
    record_issuer_event(
        &tx,
        access.profile_id,
        account,
        IssuerEvent {
            event_type: "issuer_retired",
            object_id: &access.profile_id.to_string(),
            label: &display_name,
            context: None,
            counterparty: None,
        },
    )
    .await?;
    tx.commit().await.map_err(|_| ApiError::Unavailable)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn get_issuer_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<IssuerProfileView>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    let access = issuer_access(&state.db, account).await?;
    let client = db_client(&state.db).await?;
    let row = client
        .query_opt(
            "SELECT display_name, issuer_id, retired_at, created_at
             FROM issuer_profiles
             WHERE id = $1",
            &[&access.profile_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::NotFound)?;

    Ok(Json(IssuerProfileView {
        display_name: row.get(0),
        issuer_id: row.get(1),
        retired_at: row.get(2),
        created_at: row.get(3),
    }))
}

async fn list_issuer_keys(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<IssuerKeyView>>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    let access = issuer_access(&state.db, account).await?;
    let client = db_client(&state.db).await?;
    let rows = client
        .query(
            "SELECT k.valid_from, k.retired_at, k.compromised_at
             FROM issuer_signing_keys k
             WHERE k.issuer_profile_id = $1
             ORDER BY k.valid_from DESC
             LIMIT 32",
            &[&access.profile_id],
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
    let access = issuer_access(&state.db, account).await?;
    require_issuer_role(&access, &["owner", "admin"])?;
    require_active_issuer(&access)?;

    let new_key_row_id = Uuid::new_v4();
    let new_key_id = format!("key-{}", Uuid::new_v4().simple());
    let mut new_private =
        Jwk::generate_ed_key(EdCurve::Ed25519).map_err(|_| ApiError::Unavailable)?;
    new_private.set_key_id(new_key_id.clone());
    let new_public = public_jwk(&new_private)?;
    let public_value = serde_json::to_value(&new_public).map_err(|_| ApiError::Unavailable)?;
    let mut plaintext = serde_json::to_vec(&new_private).map_err(|_| ApiError::Unavailable)?;
    let encrypted = state
        .cipher
        .encrypt(access.owner_account_id, new_key_row_id, &plaintext)?;
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
             WHERE p.id = $1
               AND k.retired_at IS NULL
               AND k.compromised_at IS NULL
             FOR UPDATE OF k",
            &[&access.profile_id],
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

    if input.compromise_current {
        record_issuer_event(
            &tx,
            profile_id,
            account,
            IssuerEvent {
                event_type: "issuer_key_compromised",
                object_id: &current_key_id,
                label: "Signing key marked compromised",
                context: None,
                counterparty: None,
            },
        )
        .await?;
    }
    record_issuer_event(
        &tx,
        profile_id,
        account,
        IssuerEvent {
            event_type: "issuer_key_rotated",
            object_id: &new_key_id,
            label: "Signing key rotated",
            context: None,
            counterparty: None,
        },
    )
    .await?;

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
    let affiliation = client
        .query_one(
            "SELECT EXISTS(SELECT 1 FROM issuer_profiles WHERE account_id = $1),
                    EXISTS(SELECT 1 FROM issuer_members WHERE account_id = $1)",
            &[&account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    if affiliation.get::<_, bool>(0) || affiliation.get::<_, bool>(1) {
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
            retired_at: None,
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
    let access = issuer_access(&state.db, account).await?;
    require_issuer_role(&access, &["owner", "admin", "issuer"])?;
    require_active_issuer(&access)?;
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
             WHERE p.id = $1
               AND k.retired_at IS NULL
               AND k.compromised_at IS NULL",
            &[&access.profile_id],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::Unauthorized)?;

    let (credential_schema_id, schema_display_name, claim_type, context, expires_days) =
        if let Some(schema_id) = input.credential_schema_id {
            let schema = resolve_issuer_schema(&state.db, access.profile_id, schema_id).await?;
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
    let mut private_bytes = state.cipher.decrypt(access.owner_account_id, &secret_row)?;
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

    let audit_label = schema_display_name.as_deref().unwrap_or(&claim_type);
    record_issuer_event(
        &tx,
        profile_id,
        account,
        IssuerEvent {
            event_type: "credential_issued",
            object_id: &payload.credential_id,
            label: audit_label,
            context: Some(&context),
            counterparty: Some(&holder_zerant_id),
        },
    )
    .await?;

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
    let access = issuer_access(&state.db, account).await?;
    require_issuer_role(&access, &["owner", "admin", "issuer"])?;
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
             FROM accounts a
             WHERE c.issuer_profile_id = $1
               AND c.credential_id = $2
               AND c.subject_account_id = a.id
               AND c.revoked_at IS NULL
             RETURNING c.issuer_profile_id, c.credential_id, a.public_handle,
                       c.credential_schema_id, c.claim_type, c.context, c.issued_at, c.expires_at",
            &[&access.profile_id, &credential_id],
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

    let holder: Option<String> = row.get(2);
    let holder_ref = holder.as_deref().ok_or(ApiError::Unavailable)?;
    let claim_type: String = row.get(4);
    let claim_context: String = row.get(5);
    record_issuer_event(
        &tx,
        issuer_profile_id,
        account,
        IssuerEvent {
            event_type: "credential_revoked",
            object_id: &credential_id,
            label: &claim_type,
            context: Some(&claim_context),
            counterparty: Some(holder_ref),
        },
    )
    .await?;

    tx.commit().await.map_err(|_| ApiError::Unavailable)?;
    Ok(Json(IssuedCredentialView {
        credential_id: row.get(1),
        holder_zerant_id: holder_ref.to_owned(),
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
    let access = issuer_access(&state.db, account).await?;
    let client = db_client(&state.db).await?;
    let rows = client
        .query(
            "SELECT c.credential_id, a.public_handle, c.credential_schema_id, c.claim_type, c.context,
                    c.issued_at, c.expires_at, c.revoked_at IS NOT NULL
             FROM issued_credentials c
             JOIN accounts a ON a.id = c.subject_account_id
             WHERE c.issuer_profile_id = $1
             ORDER BY c.created_at DESC
             LIMIT 256",
            &[&access.profile_id],
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

async fn list_account_sessions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<SessionView>>, ApiError> {
    let token = session_token(&headers)?;
    let token_hash = Sha256::digest(token.as_bytes()).to_vec();
    let account = account_id(&headers, &state.db).await?;
    let client = db_client(&state.db).await?;
    let rows = client
        .query(
            "SELECT id, auth_method, token_hash = $2 AS current,
                    created_at, last_seen_at, expires_at
             FROM sessions
             WHERE account_id = $1 AND expires_at > NOW()
             ORDER BY last_seen_at DESC, created_at DESC
             LIMIT $3",
            &[&account, &token_hash, &MAX_ACTIVE_SESSIONS_PER_ACCOUNT],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    Ok(Json(
        rows.into_iter()
            .map(|row| SessionView {
                id: row.get(0),
                auth_method: row.get(1),
                current: row.get(2),
                created_at: row.get(3),
                last_seen_at: row.get(4),
                expires_at: row.get(5),
            })
            .collect(),
    ))
}

async fn revoke_account_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Response, ApiError> {
    let token = session_token(&headers)?;
    let token_hash = Sha256::digest(token.as_bytes()).to_vec();
    let account = account_id(&headers, &state.db).await?;
    enforce_account_rate_limit(&state.db, account, "session_revoke", 30).await?;
    let client = db_client(&state.db).await?;
    let row = client
        .query_opt(
            "SELECT token_hash = $3 AS current
             FROM sessions
             WHERE id = $1 AND account_id = $2 AND expires_at > NOW()",
            &[&id, &account, &token_hash],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .ok_or(ApiError::NotFound)?;
    let current: bool = row.get(0);

    if !current {
        let recent = recent_account_id(&headers, &state.db).await?;
        if recent != account {
            return Err(ApiError::Unauthorized);
        }
    }

    let removed = client
        .execute(
            "DELETE FROM sessions WHERE id = $1 AND account_id = $2",
            &[&id, &account],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    if removed != 1 {
        return Err(ApiError::NotFound);
    }

    let mut response = StatusCode::NO_CONTENT.into_response();
    if current {
        response.headers_mut().insert(
            SET_COOKIE,
            HeaderValue::from_str(&clear_cookie_header(SESSION_COOKIE))
                .map_err(|_| ApiError::Unavailable)?,
        );
    }
    Ok(response)
}

async fn revoke_other_account_sessions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<SessionRevokeResult>, ApiError> {
    let token = session_token(&headers)?;
    let token_hash = Sha256::digest(token.as_bytes()).to_vec();
    let account = recent_account_id(&headers, &state.db).await?;
    enforce_account_rate_limit(&state.db, account, "session_revoke_others", 10).await?;
    let client = db_client(&state.db).await?;
    let removed = client
        .execute(
            "DELETE FROM sessions
             WHERE account_id = $1
               AND token_hash <> $2",
            &[&account, &token_hash],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    Ok(Json(SessionRevokeResult { revoked: removed }))
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
            "WITH found AS (
                 SELECT id, account_id, scopes
                 FROM sessions
                 WHERE token_hash = $1 AND expires_at > NOW()
             ),
             touched AS (
                 UPDATE sessions s
                 SET last_seen_at = NOW()
                 FROM found f
                 WHERE s.id = f.id
                   AND s.last_seen_at < NOW() - ($2::bigint * INTERVAL '1 minute')
                 RETURNING s.id
             )
             SELECT a.public_handle, f.scopes
             FROM found f
             JOIN accounts a ON a.id = f.account_id",
            &[&hash, &SESSION_TOUCH_INTERVAL_MINUTES],
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
    if !payment_request_matches_network(&summary, &state.zcash_chain) {
        return Err(ApiError::Invalid);
    }
    Ok(Json(summary))
}

fn payment_request_matches_network(
    summary: &zerant_zcash::zip321::PaymentRequestSummary,
    chain: &str,
) -> bool {
    let Some(expected_network) = chain.strip_prefix("zcash:") else {
        return false;
    };
    summary.payments.iter().all(|payment| {
        zerant_zcash::address::inspect_address(&payment.recipient)
            .map(|address| address.network == expected_network)
            .unwrap_or(false)
    })
}

async fn create_zcash_payment_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreatePaymentRequest>,
) -> Result<Json<zerant_zcash::zip321::PaymentRequestSummary>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    enforce_account_rate_limit(&state.db, account, "zcash_payment_create", 60).await?;
    let expected_network = match state.zcash_chain.as_str() {
        "zcash:mainnet" => "mainnet",
        "zcash:testnet" => "testnet",
        _ => return Err(ApiError::Unavailable),
    };
    let summary = zerant_zcash::zip321::create_payment_request(
        &input.recipient,
        &input.amount_zec,
        expected_network,
    )
    .map_err(|_| ApiError::Invalid)?;
    Ok(Json(summary))
}

fn safe_network_height(value: i64) -> Result<u64, ApiError> {
    let value = u64::try_from(value).map_err(|_| ApiError::Unavailable)?;
    if value > MAX_SAFE_INTEGER {
        return Err(ApiError::Unavailable);
    }
    Ok(value)
}

fn network_state_for_fresh_readiness(synced: bool) -> ZcashNetworkState {
    if synced {
        ZcashNetworkState::Ready
    } else {
        ZcashNetworkState::Syncing
    }
}

fn format_network_timestamp(value: OffsetDateTime) -> Result<String, ApiError> {
    value.format(&Rfc3339).map_err(|_| ApiError::Unavailable)
}

fn as_degraded_readiness(mut readiness: ZcashNetworkReadiness) -> ZcashNetworkReadiness {
    readiness.state = ZcashNetworkState::Degraded;
    readiness.network_actions_enabled = false;
    readiness.synced = false;
    readiness
}

fn last_good_is_recent(last_success_at: OffsetDateTime, now: OffsetDateTime) -> bool {
    now >= last_success_at && now - last_success_at < LIGHT_CLIENT_DEGRADED_DISPLAY_TTL
}

fn synced_from_row(row: &Row) -> Result<bool, ApiError> {
    let synced: Option<bool> = row.get("synced");
    synced.ok_or(ApiError::Unavailable)
}

async fn load_shared_light_client_readiness(
    client: &deadpool_postgres::Client,
    network: &str,
    endpoint_fingerprint: &str,
) -> Result<Option<PersistedLightClientReadiness>, ApiError> {
    let Some(row) = client
        .query_opt(
            "SELECT available, synced, block_height, estimated_height, lag, checked_at,
                    last_success_at
             FROM zcash_network_readiness
             WHERE network = $1 AND endpoint_fingerprint = $2",
            &[&network, &endpoint_fingerprint],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
    else {
        return Ok(None);
    };

    let available: bool = row.get("available");
    let checked_at: OffsetDateTime = row.get("checked_at");
    if !available {
        return Ok(Some(PersistedLightClientReadiness {
            checked_at,
            readiness: None,
        }));
    }

    let synced = synced_from_row(&row)?;
    let block_height: Option<i64> = row.get("block_height");
    let estimated_height: Option<i64> = row.get("estimated_height");
    let lag: Option<i64> = row.get("lag");
    let last_success_at: Option<OffsetDateTime> = row.get("last_success_at");
    let confirmed_at = last_success_at.unwrap_or(checked_at);

    let readiness = ZcashNetworkReadiness {
        configured: true,
        network: network.to_owned(),
        state: network_state_for_fresh_readiness(synced),
        network_actions_enabled: synced,
        synced,
        block_height: Some(safe_network_height(
            block_height.ok_or(ApiError::Unavailable)?,
        )?),
        estimated_height: Some(safe_network_height(
            estimated_height.ok_or(ApiError::Unavailable)?,
        )?),
        lag: Some(safe_network_height(lag.ok_or(ApiError::Unavailable)?)?),
        last_confirmed_at: Some(format_network_timestamp(confirmed_at)?),
    };

    Ok(Some(PersistedLightClientReadiness {
        checked_at,
        readiness: Some(readiness),
    }))
}

async fn load_last_good_light_client_readiness(
    client: &deadpool_postgres::Client,
    network: &str,
    now: OffsetDateTime,
) -> Result<Option<ZcashNetworkReadiness>, ApiError> {
    let Some(row) = client
        .query_opt(
            "SELECT last_success_block_height, last_success_estimated_height,
                    last_success_lag, last_success_synced, last_success_at
             FROM zcash_network_readiness
             WHERE network = $1",
            &[&network],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
    else {
        return Ok(None);
    };

    let last_success_at: Option<OffsetDateTime> = row.get("last_success_at");
    let Some(last_success_at) = last_success_at else {
        return Ok(None);
    };
    if !last_good_is_recent(last_success_at, now) {
        return Ok(None);
    }

    let block_height: Option<i64> = row.get("last_success_block_height");
    let estimated_height: Option<i64> = row.get("last_success_estimated_height");
    let lag: Option<i64> = row.get("last_success_lag");
    let last_success_synced: Option<bool> = row.get("last_success_synced");

    if block_height.is_none()
        || estimated_height.is_none()
        || lag.is_none()
        || last_success_synced.is_none()
    {
        return Ok(None);
    }

    Ok(Some(ZcashNetworkReadiness {
        configured: true,
        network: network.to_owned(),
        state: ZcashNetworkState::Degraded,
        network_actions_enabled: false,
        synced: false,
        block_height: Some(safe_network_height(
            block_height.ok_or(ApiError::Unavailable)?,
        )?),
        estimated_height: Some(safe_network_height(
            estimated_height.ok_or(ApiError::Unavailable)?,
        )?),
        lag: Some(safe_network_height(lag.ok_or(ApiError::Unavailable)?)?),
        last_confirmed_at: Some(format_network_timestamp(last_success_at)?),
    }))
}

async fn load_light_client_high_water(
    client: &deadpool_postgres::Client,
    network: &str,
) -> Result<Option<u64>, ApiError> {
    let row = client
        .query_opt(
            "SELECT high_water_block_height
             FROM zcash_network_readiness
             WHERE network = $1",
            &[&network],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let Some(row) = row else {
        return Ok(None);
    };
    let value: Option<i64> = row.get(0);
    value.map(safe_network_height).transpose()
}

async fn store_shared_light_client_success(
    client: &deadpool_postgres::Client,
    network: &str,
    endpoint_fingerprint: &str,
    readiness: &ZcashNetworkReadiness,
) -> Result<(), ApiError> {
    let block_height = i64::try_from(readiness.block_height.ok_or(ApiError::Unavailable)?)
        .map_err(|_| ApiError::Unavailable)?;
    let estimated_height = i64::try_from(readiness.estimated_height.ok_or(ApiError::Unavailable)?)
        .map_err(|_| ApiError::Unavailable)?;
    let lag = i64::try_from(readiness.lag.ok_or(ApiError::Unavailable)?)
        .map_err(|_| ApiError::Unavailable)?;

    client
        .execute(
            "INSERT INTO zcash_network_readiness
             (network, endpoint_fingerprint, available, synced, block_height,
              estimated_height, lag, checked_at, failure_count, last_error_at,
              high_water_block_height, last_success_at,
              last_success_block_height, last_success_estimated_height,
              last_success_lag, last_success_synced)
             VALUES ($1, $2, true, $3, $4, $5, $6, NOW(), 0, NULL, $4, NOW(),
                     $4, $5, $6, $3)
             ON CONFLICT (network)
             DO UPDATE SET endpoint_fingerprint = EXCLUDED.endpoint_fingerprint,
                           available = true,
                           synced = EXCLUDED.synced,
                           block_height = EXCLUDED.block_height,
                           estimated_height = EXCLUDED.estimated_height,
                           lag = EXCLUDED.lag,
                           checked_at = NOW(),
                           failure_count = 0,
                           last_error_at = NULL,
                           high_water_block_height = GREATEST(
                               COALESCE(zcash_network_readiness.high_water_block_height, 0),
                               EXCLUDED.block_height
                           ),
                           last_success_at = NOW(),
                           last_success_block_height = EXCLUDED.block_height,
                           last_success_estimated_height = EXCLUDED.estimated_height,
                           last_success_lag = EXCLUDED.lag,
                           last_success_synced = EXCLUDED.synced",
            &[
                &network,
                &endpoint_fingerprint,
                &readiness.synced,
                &block_height,
                &estimated_height,
                &lag,
            ],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    Ok(())
}

async fn store_shared_light_client_failure(
    client: &deadpool_postgres::Client,
    network: &str,
    endpoint_fingerprint: &str,
) -> Result<(), ApiError> {
    client
        .execute(
            "INSERT INTO zcash_network_readiness
             (network, endpoint_fingerprint, available, synced, block_height,
              estimated_height, lag, checked_at, failure_count, last_error_at)
             VALUES ($1, $2, false, NULL, NULL, NULL, NULL, NOW(), 1, NOW())
             ON CONFLICT (network)
             DO UPDATE SET endpoint_fingerprint = EXCLUDED.endpoint_fingerprint,
                           available = false,
                           synced = NULL,
                           block_height = NULL,
                           estimated_height = NULL,
                           lag = NULL,
                           checked_at = NOW(),
                           failure_count = CASE
                               WHEN zcash_network_readiness.endpoint_fingerprint =
                                    EXCLUDED.endpoint_fingerprint
                               THEN LEAST(zcash_network_readiness.failure_count + 1, 1000000)
                               ELSE 1
                           END,
                           last_error_at = NOW()",
            &[&network, &endpoint_fingerprint],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;
    Ok(())
}

async fn zcash_network_readiness(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ZcashNetworkReadiness>, ApiError> {
    let account = account_id(&headers, &state.db).await?;
    enforce_account_rate_limit(&state.db, account, "zcash_network_readiness", 60).await?;

    let expected_network = match state.zcash_chain.as_str() {
        "zcash:mainnet" => LightClientNetwork::Mainnet,
        "zcash:testnet" => LightClientNetwork::Testnet,
        _ => return Err(ApiError::Unavailable),
    };
    let network = light_client_network_label(expected_network);

    if state.light_client_endpoints.is_empty() {
        return Ok(Json(ZcashNetworkReadiness {
            configured: false,
            network: network.into(),
            state: ZcashNetworkState::NotConfigured,
            network_actions_enabled: false,
            synced: false,
            block_height: None,
            estimated_height: None,
            lag: None,
            last_confirmed_at: None,
        }));
    }

    let mut local_cache = state.light_client_cache.lock().await;
    if let Some(entry) = local_cache.as_ref()
        && entry.is_fresh()
    {
        return entry
            .readiness
            .clone()
            .map(Json)
            .ok_or(ApiError::Unavailable);
    }

    let endpoint_fingerprint =
        light_client_configuration_fingerprint(&state.light_client_endpoints);
    let now = OffsetDateTime::now_utc();
    let client = db_client(&state.db).await?;
    let persisted =
        load_shared_light_client_readiness(&client, network, &endpoint_fingerprint).await?;

    if let Some(entry) = persisted.as_ref()
        && entry.is_fresh(now)
    {
        if let Some(readiness) = entry.readiness.clone() {
            *local_cache = Some(LightClientCacheEntry {
                checked_at: Instant::now(),
                readiness: Some(readiness.clone()),
            });
            return Ok(Json(readiness));
        }

        if let Some(degraded) = load_last_good_light_client_readiness(&client, network, now).await?
        {
            *local_cache = Some(LightClientCacheEntry {
                checked_at: Instant::now(),
                readiness: Some(degraded.clone()),
            });
            return Ok(Json(degraded));
        }

        *local_cache = Some(LightClientCacheEntry {
            checked_at: Instant::now(),
            readiness: None,
        });
        return Err(ApiError::Unavailable);
    }

    let lock_id = light_client_refresh_lock(expected_network);
    let acquired: bool = client
        .query_one("SELECT pg_try_advisory_lock($1)", &[&lock_id])
        .await
        .map_err(|_| ApiError::Unavailable)?
        .get(0);

    if !acquired {
        if let Some(entry) = persisted
            && entry.within_stale_grace(now)
            && let Some(readiness) = entry.readiness
        {
            let degraded = as_degraded_readiness(readiness);
            *local_cache = Some(LightClientCacheEntry {
                checked_at: Instant::now(),
                readiness: Some(degraded.clone()),
            });
            return Ok(Json(degraded));
        }

        if let Some(degraded) = load_last_good_light_client_readiness(&client, network, now).await?
        {
            *local_cache = Some(LightClientCacheEntry {
                checked_at: Instant::now(),
                readiness: Some(degraded.clone()),
            });
            return Ok(Json(degraded));
        }
        return Err(ApiError::Unavailable);
    }

    let refresh_result = async {
        let refreshed =
            load_shared_light_client_readiness(&client, network, &endpoint_fingerprint).await?;
        let refreshed_now = OffsetDateTime::now_utc();
        if let Some(entry) = refreshed.as_ref()
            && entry.is_fresh(refreshed_now)
        {
            if let Some(readiness) = entry.readiness.clone() {
                return Ok(readiness);
            }
            return load_last_good_light_client_readiness(&client, network, refreshed_now)
                .await?
                .ok_or(ApiError::Unavailable);
        }

        let high_water = load_light_client_high_water(&client, network).await?;
        let mut successful_readiness = None;
        for endpoint in &state.light_client_endpoints {
            if let Ok(readiness) = fetch_light_client_readiness(
                endpoint,
                expected_network,
                state.light_client_allow_loopback,
            )
            .await
            {
                if !light_client_height_is_acceptable(readiness.block_height, high_water) {
                    continue;
                }
                let last_confirmed_at = OffsetDateTime::now_utc()
                    .format(&Rfc3339)
                    .map_err(|_| ApiError::Unavailable)?;
                successful_readiness = Some(ZcashNetworkReadiness {
                    configured: true,
                    network: readiness.network,
                    state: network_state_for_fresh_readiness(readiness.synced),
                    network_actions_enabled: readiness.synced,
                    synced: readiness.synced,
                    block_height: Some(readiness.block_height),
                    estimated_height: Some(readiness.estimated_height),
                    lag: Some(readiness.lag),
                    last_confirmed_at: Some(last_confirmed_at),
                });
                break;
            }
        }

        match successful_readiness {
            Some(readiness) => {
                store_shared_light_client_success(
                    &client,
                    network,
                    &endpoint_fingerprint,
                    &readiness,
                )
                .await?;
                Ok(readiness)
            }
            None => {
                store_shared_light_client_failure(&client, network, &endpoint_fingerprint).await?;
                load_last_good_light_client_readiness(&client, network, OffsetDateTime::now_utc())
                    .await?
                    .ok_or(ApiError::Unavailable)
            }
        }
    }
    .await;

    let _ = client
        .query_one("SELECT pg_advisory_unlock($1)", &[&lock_id])
        .await;

    match refresh_result {
        Ok(readiness) => {
            *local_cache = Some(LightClientCacheEntry {
                checked_at: Instant::now(),
                readiness: Some(readiness.clone()),
            });
            Ok(Json(readiness))
        }
        Err(error) => {
            *local_cache = Some(LightClientCacheEntry {
                checked_at: Instant::now(),
                readiness: None,
            });
            Err(error)
        }
    }
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

async fn public_zcash_config(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "chain": state.zcash_chain.clone() }))
}

async fn operational_health_internal(
    State(state): State<AppState>,
) -> Result<Json<OperationalHealthSnapshot>, ApiError> {
    let client = db_client(&state.db).await?;
    let counts = client
        .query_one(
            "SELECT
                (SELECT COUNT(*) FROM verification_requests
                 WHERE status = 'pending' AND expires_at > NOW()) AS pending_verifications,
                (SELECT COUNT(*) FROM verification_requests
                 WHERE status = 'pending' AND expires_at <= NOW()) AS overdue_verifications,
                (SELECT COUNT(*) FROM webhook_deliveries
                 WHERE status IN ('pending', 'delivering')) AS webhook_pending,
                (SELECT COUNT(*) FROM webhook_deliveries
                 WHERE status = 'dead') AS webhook_dead,
                (SELECT last_success_at FROM maintenance_job_state
                 WHERE job = 'retention') AS maintenance_last_success_at",
            &[],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let pending_verifications: i64 = counts.get(0);
    let overdue_verifications: i64 = counts.get(1);
    let webhook_pending: i64 = counts.get(2);
    let webhook_dead: i64 = counts.get(3);
    let maintenance_last_success_at: Option<OffsetDateTime> = counts.get(4);
    if pending_verifications < 0
        || overdue_verifications < 0
        || webhook_pending < 0
        || webhook_dead < 0
    {
        return Err(ApiError::Unavailable);
    }

    let now = OffsetDateTime::now_utc();
    let maintenance_stale = maintenance_last_success_at
        .is_none_or(|last| last > now || now - last > Duration::hours(36));

    let configured = !state.light_client_endpoints.is_empty();
    let network = match state.zcash_chain.as_str() {
        "zcash:mainnet" => "mainnet",
        "zcash:testnet" => "testnet",
        _ => return Err(ApiError::Unavailable),
    }
    .to_owned();

    let zcash_row = if configured {
        client
            .query_opt(
                "SELECT available, synced, checked_at, last_success_at
                 FROM zcash_network_readiness
                 WHERE network = $1",
                &[&network],
            )
            .await
            .map_err(|_| ApiError::Unavailable)?
    } else {
        None
    };

    let (available, synced, checked_at, last_success_at, stale) = match zcash_row {
        Some(row) => {
            let available: bool = row.get(0);
            let synced: Option<bool> = row.get(1);
            let checked_at: OffsetDateTime = row.get(2);
            let last_success_at: Option<OffsetDateTime> = row.get(3);
            let stale = checked_at > now || now - checked_at > LIGHT_CLIENT_DEGRADED_DISPLAY_TTL;
            (
                Some(available),
                synced,
                Some(checked_at),
                last_success_at,
                stale,
            )
        }
        None => (None, None, None, None, configured),
    };

    let zcash_unhealthy = configured && (stale || available != Some(true) || synced != Some(true));
    let (healthy, attention_required) = operational_health_flags(
        maintenance_stale,
        zcash_unhealthy,
        overdue_verifications,
        webhook_pending,
        webhook_dead,
    );

    Ok(Json(OperationalHealthSnapshot {
        healthy,
        attention_required,
        pending_verifications,
        overdue_verifications,
        webhook_pending,
        webhook_dead,
        maintenance_last_success_at,
        maintenance_stale,
        zcash: OperationalZcashHealth {
            configured,
            network,
            available,
            synced,
            checked_at,
            last_success_at,
            stale,
        },
    }))
}

async fn retention_maintenance_internal(
    State(state): State<AppState>,
) -> Result<Json<RetentionMaintenanceSummary>, ApiError> {
    let mut client = db_client(&state.db).await?;
    let tx = client
        .transaction()
        .await
        .map_err(|_| ApiError::Unavailable)?;
    tx.batch_execute("SET LOCAL lock_timeout = '5s'; SET LOCAL statement_timeout = '60s'")
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let locked: bool = tx
        .query_one(
            "SELECT pg_try_advisory_xact_lock($1)",
            &[&RETENTION_LOCK_ID],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?
        .get(0);
    if !locked {
        tx.rollback().await.map_err(|_| ApiError::Unavailable)?;
        return Ok(Json(RetentionMaintenanceSummary {
            skipped: true,
            expired_requests: 0,
            sessions: 0,
            zecauth_challenges: 0,
            passkey_challenges: 0,
            rate_limits: 0,
            webhook_deliveries: 0,
            proof_material: 0,
            expired_payments: 0,
            deleted_expired_payments: 0,
        }));
    }

    let expired_requests = tx
        .execute(
            "WITH doomed AS (
                SELECT id
                FROM verification_requests
                WHERE status = 'pending' AND expires_at <= NOW()
                ORDER BY expires_at
                LIMIT $1
                FOR UPDATE SKIP LOCKED
             )
             UPDATE verification_requests r
             SET status = 'expired', decided_at = COALESCE(r.decided_at, NOW())
             FROM doomed d
             WHERE r.id = d.id",
            &[&RETENTION_BATCH_LIMIT],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let sessions = tx
        .execute(
            "WITH doomed AS (
                SELECT id
                FROM sessions
                WHERE expires_at <= NOW() - INTERVAL '1 day'
                ORDER BY expires_at
                LIMIT $1
             )
             DELETE FROM sessions s USING doomed d WHERE s.id = d.id",
            &[&RETENTION_BATCH_LIMIT],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let zecauth_challenges = tx
        .execute(
            "WITH doomed AS (
                SELECT id
                FROM zecauth_challenges
                WHERE expires_at <= NOW() - INTERVAL '1 day'
                ORDER BY expires_at
                LIMIT $1
             )
             DELETE FROM zecauth_challenges c USING doomed d WHERE c.id = d.id",
            &[&RETENTION_BATCH_LIMIT],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let passkey_challenges = tx
        .execute(
            "WITH doomed AS (
                SELECT id
                FROM passkey_challenges
                WHERE expires_at <= NOW() - INTERVAL '1 day'
                ORDER BY expires_at
                LIMIT $1
             )
             DELETE FROM passkey_challenges c USING doomed d WHERE c.id = d.id",
            &[&RETENTION_BATCH_LIMIT],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let rate_limits = tx
        .execute(
            "WITH doomed AS (
                SELECT account_id, action
                FROM account_rate_limits
                WHERE window_started_at <= NOW() - INTERVAL '10 minutes'
                ORDER BY window_started_at
                LIMIT $1
             )
             DELETE FROM account_rate_limits r
             USING doomed d
             WHERE r.account_id = d.account_id AND r.action = d.action",
            &[&RETENTION_BATCH_LIMIT],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let webhook_deliveries = tx
        .execute(
            "WITH doomed AS (
                SELECT id
                FROM webhook_deliveries
                WHERE status IN ('delivered', 'dead')
                  AND created_at <= NOW() - INTERVAL '30 days'
                ORDER BY created_at
                LIMIT $1
             )
             DELETE FROM webhook_deliveries w USING doomed d WHERE w.id = d.id",
            &[&RETENTION_BATCH_LIMIT],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let proof_material = tx
        .execute(
            "WITH doomed AS (
                SELECT id
                FROM verification_requests
                WHERE status = 'approved'
                  AND expires_at <= NOW() - INTERVAL '1 day'
                  AND (
                    response_ciphertext IS NOT NULL
                    OR proof_revocation_jws IS NOT NULL
                    OR proof_issuer_id IS NOT NULL
                    OR proof_issuer_key_id IS NOT NULL
                  )
                ORDER BY expires_at
                LIMIT $1
                FOR UPDATE SKIP LOCKED
             )
             UPDATE verification_requests r
             SET response_ciphertext = NULL,
                 response_data_nonce = NULL,
                 response_wrapped_dek = NULL,
                 response_wrap_nonce = NULL,
                 response_key_version = NULL,
                 proof_issuer_id = NULL,
                 proof_issuer_key_id = NULL,
                 proof_revocation_jws = NULL
             FROM doomed d
             WHERE r.id = d.id",
            &[&RETENTION_BATCH_LIMIT],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let expired_payments = tx
        .execute(
            "WITH due AS (
            SELECT id FROM zcash_payments
            WHERE state = 'prepared' AND expires_at <= NOW()
            ORDER BY expires_at LIMIT $1 FOR UPDATE SKIP LOCKED
         )
         UPDATE zcash_payments p SET state = 'expired'
         FROM due WHERE p.id = due.id",
            &[&RETENTION_BATCH_LIMIT],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let deleted_expired_payments = tx
        .execute(
            "WITH due AS (
            SELECT id FROM zcash_payments
            WHERE state = 'expired' AND expires_at <= NOW() - INTERVAL '30 days'
            ORDER BY expires_at LIMIT $1 FOR UPDATE SKIP LOCKED
         )
         DELETE FROM zcash_payments p USING due WHERE p.id = due.id",
            &[&RETENTION_BATCH_LIMIT],
        )
        .await
        .map_err(|_| ApiError::Unavailable)?;

    let summary = serde_json::json!({
        "expired_requests": expired_requests,
        "sessions": sessions,
        "zecauth_challenges": zecauth_challenges,
        "passkey_challenges": passkey_challenges,
        "rate_limits": rate_limits,
        "webhook_deliveries": webhook_deliveries,
        "proof_material": proof_material,
        "expired_payments": expired_payments,
        "deleted_expired_payments": deleted_expired_payments,
    });
    tx.execute(
        "INSERT INTO maintenance_job_state(job, last_success_at, last_summary, updated_at)
         VALUES ('retention', NOW(), $1, NOW())
         ON CONFLICT (job)
         DO UPDATE SET last_success_at = EXCLUDED.last_success_at,
                       last_summary = EXCLUDED.last_summary,
                       updated_at = EXCLUDED.updated_at",
        &[&summary],
    )
    .await
    .map_err(|_| ApiError::Unavailable)?;

    tx.commit().await.map_err(|_| ApiError::Unavailable)?;

    Ok(Json(RetentionMaintenanceSummary {
        skipped: false,
        expired_requests,
        sessions,
        zecauth_challenges,
        passkey_challenges,
        rate_limits,
        webhook_deliveries,
        proof_material,
        expired_payments,
        deleted_expired_payments,
    }))
}

fn app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/public/zcash/config", get(public_zcash_config))
        .route("/v1/public/issuers", get(public_issuer_directory))
        .route(
            "/v1/public/issuers/{issuer_id}",
            get(public_issuer_metadata),
        )
        .route(
            "/v1/public/issuers/{issuer_id}/revocation",
            get(public_issuer_revocation),
        )
        .route(
            "/v1/auth/passkey/register/start",
            post(passkey_registration_start),
        )
        .route(
            "/v1/auth/passkey/register/finish",
            post(passkey_registration_finish),
        )
        .route(
            "/v1/auth/passkey/authenticate/start",
            post(passkey_authentication_start),
        )
        .route(
            "/v1/auth/passkey/authenticate/finish",
            post(passkey_authentication_finish),
        )
        .route(
            "/v1/auth/passkey/discoverable/start",
            post(passkey_discoverable::start),
        )
        .route(
            "/v1/auth/passkey/discoverable/finish",
            post(passkey_discoverable::finish),
        )
        .route("/v1/auth/zecauth/challenge", get(zecauth_challenge))
        .route("/v1/auth/zecauth/verify", post(zecauth_verify))
        .route("/v1/auth/wallet/verify", post(wallet_message_verify))
        .route("/v1/auth/session", get(redeem_zecauth_session))
        .route("/v1/auth/zecauth/session", get(redeem_zecauth_session))
        .route("/v1/session", get(session_info).delete(logout))
        .route("/v1/account", get(account_summary).delete(delete_account))
        .route("/v1/account/export", get(export_account))
        .route("/v1/account/passkeys", get(list_account_passkeys))
        .route("/v1/account/zcash/methods", get(list_linked_zcash_methods))
        .route("/v1/account/zcash/zecauth", delete(remove_zecauth_method))
        .route(
            "/v1/account/zcash/wallet/{network}",
            delete(remove_wallet_message_method),
        )
        .route("/v1/account/zcash/challenge", post(account_zcash_challenge))
        .route(
            "/v1/account/zcash/zecauth/callback",
            post(account_zecauth_link_callback),
        )
        .route(
            "/v1/account/zcash/zecauth/complete",
            post(account_zecauth_link_complete),
        )
        .route(
            "/v1/account/zcash/zecauth/verify",
            post(account_zecauth_verify),
        )
        .route(
            "/v1/account/zcash/wallet/verify",
            post(account_wallet_verify),
        )
        .route(
            "/v1/account/passkeys/register/start",
            post(account_passkey_registration_start),
        )
        .route(
            "/v1/account/passkeys/register/finish",
            post(account_passkey_registration_finish),
        )
        .route(
            "/v1/account/passkeys/{id}",
            axum::routing::delete(delete_account_passkey),
        )
        .route("/v1/account/sessions", get(list_account_sessions))
        .route(
            "/v1/account/sessions/revoke-others",
            post(revoke_other_account_sessions),
        )
        .route(
            "/v1/account/sessions/{id}",
            axum::routing::delete(revoke_account_session),
        )
        .route("/v1/activity", get(list_activity))
        .route(
            "/v1/issuer",
            get(get_issuer_profile)
                .post(register_issuer)
                .delete(retire_issuer_profile),
        )
        .route("/v1/issuer/team", get(list_issuer_team))
        .route("/v1/issuer/activity", get(list_issuer_activity))
        .route(
            "/v1/issuer/team/invitations",
            get(list_issuer_team_invitations).post(invite_issuer_member),
        )
        .route(
            "/v1/issuer/team/{zerant_id}",
            axum::routing::delete(remove_issuer_member),
        )
        .route(
            "/v1/issuer/ownership/transfer",
            post(transfer_issuer_ownership),
        )
        .route("/v1/issuer/invitations", get(list_my_issuer_invitations))
        .route(
            "/v1/issuer/invitations/{id}/decision",
            post(decide_issuer_invitation),
        )
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
        .route(
            "/v1/issuer/schemas/{schema_id}/versions",
            post(create_issuer_schema_version),
        )
        .route("/v1/issuers", get(issuer_directory))
        .route(
            "/v1/verifier",
            get(get_verifier_profile)
                .post(register_verifier)
                .delete(retire_verifier_profile),
        )
        .route("/v1/verifier/keys", get(list_verifier_keys))
        .route("/v1/verifier/keys/rotate", post(rotate_verifier_key))
        .route(
            "/v1/verifier/api-keys",
            get(list_verifier_api_keys).post(create_verifier_api_key),
        )
        .route(
            "/v1/verifier/api-keys/{id}/revoke",
            post(revoke_verifier_api_key),
        )
        .route(
            "/v1/verifier/webhooks",
            get(list_verifier_webhooks).post(create_verifier_webhook),
        )
        .route(
            "/v1/verifier/webhooks/{id}/disable",
            post(disable_verifier_webhook),
        )
        .route(
            "/v1/internal/webhooks/requests/{request_id}/dispatch",
            post(dispatch_request_webhooks_internal),
        )
        .route(
            "/v1/internal/maintenance/retention",
            post(retention_maintenance_internal),
        )
        .route("/v1/internal/ops/health", get(operational_health_internal))
        .route(
            "/v1/verifier/policies",
            get(list_verification_policies).post(create_verification_policy),
        )
        .route(
            "/v1/verifier/policies/{policy_id}/versions",
            post(create_verification_policy_version),
        )
        .route(
            "/v1/verifier/policies/{policy_id}/retire",
            post(retire_verification_policy),
        )
        .route(
            "/v1/verifier/policies/{policy_id}/requests",
            post(create_policy_verification_request),
        )
        .route(
            "/v1/verifier/requests",
            get(list_verifier_requests).post(create_verification_request),
        )
        .route("/v1/verifier/requests/{id}/proof", get(get_verifier_proof))
        .route(
            "/v1/integrations/verifier/requests",
            post(create_integration_verification_request),
        )
        .route(
            "/v1/integrations/verifier/policies/{policy_id}/requests",
            post(create_integration_policy_verification_request),
        )
        .route(
            "/v1/integrations/verifier/requests/{id}",
            get(get_integration_verification_request),
        )
        .route(
            "/v1/integrations/verifier/requests/{id}/proof",
            get(get_integration_verifier_proof),
        )
        .route("/v1/holder/requests", get(list_holder_requests))
        .route(
            "/v1/holder/requests/{id}/preview",
            get(preview_holder_request),
        )
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
        .route("/v1/zcash/network/readiness", get(zcash_network_readiness))
        .route("/v1/zcash/address/inspect", post(inspect_zcash_address))
        .route(
            "/v1/zcash/payment-request/inspect",
            post(inspect_zcash_payment_request),
        )
        .route(
            "/v1/zcash/payment-request/create",
            post(create_zcash_payment_request),
        )
        .route(
            "/v1/zcash/payments",
            get(payments::list).post(payments::prepare),
        )
        .route("/v1/zcash/payments/{id}/submit", post(payments::submit))
        .route("/v1/zcash/payments/{id}/observe", post(payments::observe))
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
            include_str!("../migrations/0015_credential_schema_versions.sql"),
            include_str!("../migrations/0016_public_trust_metadata.sql"),
            include_str!("../migrations/0017_verifier_api_keys.sql"),
            include_str!("../migrations/0018_issuer_teams.sql"),
            include_str!("../migrations/0019_issuer_audit.sql"),
            include_str!("../migrations/0020_verifier_webhooks.sql"),
            include_str!("../migrations/0021_verification_policies.sql"),
            include_str!("../migrations/0022_passkeys.sql"),
            include_str!("../migrations/0023_session_management.sql"),
            include_str!("../migrations/0024_zcash_identity_link.sql"),
            include_str!("../migrations/0025_zecauth_link_handoff.sql"),
            include_str!("../migrations/0026_zcash_network_readiness.sql"),
            include_str!("../migrations/0027_zcash_network_high_water.sql"),
            include_str!("../migrations/0028_zcash_network_last_good.sql"),
            include_str!("../migrations/0029_verifier_proof_packages.sql"),
            include_str!("../migrations/0030_operational_health.sql"),
            include_str!("../migrations/0031_zcash_payments.sql"),
            include_str!("../migrations/0032_discoverable_passkeys.sql"),
            include_str!("../migrations/0033_zcash_payment_observation.sql"),
            include_str!("../migrations/0034_zcash_payment_activity.sql"),
            include_str!("../migrations/0035_issuer_retirement.sql"),
            include_str!("../migrations/0036_safe_verifier_retirement.sql"),
            include_str!("../migrations/0037_verifier_retirement_serialization.sql"),
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
    let zcash_chain = env::var("ZERANT_ZCASH_CHAIN").unwrap_or_else(|_| "zcash:testnet".into());
    if !matches!(zcash_chain.as_str(), "zcash:testnet" | "zcash:mainnet") {
        return Err(ApiError::Unavailable.to_string().into());
    }

    let light_client_allow_loopback = env::var("ZERANT_LIGHT_CLIENT_ALLOW_LOOPBACK")
        .map(|value| value == "true")
        .unwrap_or(false);
    let light_client_endpoints_value = env::var("ZERANT_LIGHT_CLIENT_ENDPOINTS").ok();
    let light_client_legacy_endpoint = env::var("ZERANT_LIGHT_CLIENT_ENDPOINT").ok();
    let light_client_endpoints = parse_light_client_endpoints(
        light_client_endpoints_value.as_deref(),
        light_client_legacy_endpoint.as_deref(),
        light_client_allow_loopback,
    )
    .map_err(|error| error.to_string())?;

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

    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        if args[1] != "vault-rotation" {
            return Err(ApiError::Invalid.to_string().into());
        }
        vault_rotation::run(&db, &args[1..])
            .await
            .map_err(|error| error.to_string())?;
        return Ok(());
    }

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

    let rp_origin = url::Url::parse(&public_origin)?;
    let rp_id = rp_origin
        .domain()
        .ok_or_else(|| ApiError::Unavailable.to_string())?
        .to_owned();
    let webauthn = WebauthnBuilder::new(&rp_id, &rp_origin)?
        .rp_name("Zerant")
        .build()?;

    let state = AppState {
        db,
        cipher: Arc::new(VaultCipher::from_env().map_err(|error| error.to_string())?),
        webauthn: Arc::new(webauthn),
        public_origin,
        zcash_chain,
        light_client_endpoints,
        light_client_allow_loopback,
        light_client_cache: Arc::new(tokio::sync::Mutex::new(None)),
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
    fn stored_credential_expiry_is_derived_from_private_credential_timestamp() {
        let now = OffsetDateTime::from_unix_timestamp(1_000).unwrap();
        assert!(credential_expired_at(
            &serde_json::json!({"expires_at": 999}),
            now
        ));
        assert!(credential_expired_at(
            &serde_json::json!({"expires_at": 1_000}),
            now
        ));
        assert!(!credential_expired_at(
            &serde_json::json!({"expires_at": 1_001}),
            now
        ));
        assert!(!credential_expired_at(
            &serde_json::json!({"value": "member"}),
            now
        ));
    }

    #[test]
    fn approval_requires_the_exact_previewed_issuer_and_claim() {
        let mut input: DecideRequest = serde_json::from_str(r#"{"decision":"approve"}"#).unwrap();
        assert!(!approval_matches_preview(&input, "issuer-a", "member"));
        input.expected_issuer_id = Some("issuer-a".into());
        input.expected_value = Some("member".into());
        assert!(approval_matches_preview(&input, "issuer-a", "member"));
        assert!(!approval_matches_preview(&input, "issuer-b", "member"));
        assert!(!approval_matches_preview(&input, "issuer-a", "admin"));
    }

    async fn removal_account(
        client: &deadpool_postgres::Client,
        passkey: bool,
        zecauth: bool,
        chains: &[&str],
    ) -> (Uuid, String, Option<Uuid>, Option<Vec<u8>>) {
        let account = Uuid::new_v4();
        let handle = zerant_public_handle(account.as_bytes());
        client
            .execute(
                "INSERT INTO accounts(id, public_handle) VALUES ($1, $2)",
                &[&account, &handle],
            )
            .await
            .unwrap();
        let passkey_id = if passkey {
            let id = Uuid::new_v4();
            let credential_id = format!("credential_{id}");
            client
                .execute(
                    "INSERT INTO passkey_credentials(id, account_id, credential_id, passkey)
                     VALUES ($1, $2, $3, '{}'::jsonb)",
                    &[&id, &account, &credential_id],
                )
                .await
                .unwrap();
            Some(id)
        } else {
            None
        };
        let zecauth_key = if zecauth {
            let key = Sha256::digest(Uuid::new_v4().as_bytes()).to_vec();
            client
                .execute(
                    "INSERT INTO zecauth_identities(account_id, verification_key) VALUES ($1, $2)",
                    &[&account, &key],
                )
                .await
                .unwrap();
            Some(key)
        } else {
            None
        };
        for chain in chains {
            let mut key = vec![2_u8];
            key.extend_from_slice(&Sha256::digest(Uuid::new_v4().as_bytes()));
            client
                .execute(
                    "INSERT INTO wallet_message_identities(account_id, chain, public_key)
                     VALUES ($1, $2, $3)",
                    &[&account, chain, &key],
                )
                .await
                .unwrap();
        }
        (account, handle, passkey_id, zecauth_key)
    }

    async fn removal_session(
        client: &deadpool_postgres::Client,
        account: Uuid,
        method: &str,
    ) -> (String, Vec<u8>) {
        let token = format!("session_{}", Uuid::new_v4());
        let hash = Sha256::digest(token.as_bytes()).to_vec();
        client
            .execute(
                "INSERT INTO sessions(id, account_id, token_hash, auth_method, expires_at)
                 VALUES ($1, $2, $3, $4, NOW() + INTERVAL '1 day')",
                &[&Uuid::new_v4(), &account, &hash, &method],
            )
            .await
            .unwrap();
        (token, hash)
    }

    fn removal_headers(token: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::COOKIE,
            HeaderValue::from_str(&format!("{SESSION_COOKIE}={token}")).unwrap(),
        );
        headers
    }

    // Run with ZERANT_TEST_DATABASE_URL pointing at a disposable PostgreSQL database.
    #[test]
    fn light_client_endpoint_list_is_ordered_bounded_and_strict() {
        let endpoints = parse_light_client_endpoints(
            Some("https://primary.example, https://backup.example,https://primary.example"),
            None,
            false,
        )
        .unwrap();
        assert_eq!(
            endpoints,
            vec![
                "https://primary.example/".to_owned(),
                "https://backup.example/".to_owned()
            ]
        );

        assert!(
            parse_light_client_endpoints(
                Some("https://a.example,https://b.example,https://c.example,https://d.example,https://e.example"),
                None,
                false,
            )
            .is_err()
        );
        assert!(
            parse_light_client_endpoints(
                Some("https://a.example"),
                Some("https://legacy.example"),
                false,
            )
            .is_err()
        );
        assert!(parse_light_client_endpoints(Some("http://remote.example"), None, false).is_err());
        assert!(parse_light_client_endpoints(Some("http://127.0.0.1:9067"), None, true).is_ok());
    }

    #[test]
    fn light_client_high_water_rejects_material_rollback() {
        assert!(light_client_height_is_acceptable(1_000, None));
        assert!(light_client_height_is_acceptable(1_000, Some(1_000)));
        assert!(light_client_height_is_acceptable(980, Some(1_000)));
        assert!(!light_client_height_is_acceptable(979, Some(1_000)));
        assert!(light_client_height_is_acceptable(1_001, Some(1_000)));
    }

    #[test]
    fn zcash_high_water_migration_preserves_only_bounded_network_state() {
        let schema = include_str!("../migrations/0027_zcash_network_high_water.sql");
        assert!(schema.contains("high_water_block_height"));
        assert!(schema.contains("last_success_at"));
        for forbidden in ["wallet_address", "balance", "memo", "seed", "txid"] {
            assert!(!schema.contains(forbidden), "{forbidden}");
        }
    }

    #[test]
    fn light_client_configuration_fingerprint_binds_failover_order() {
        let primary_first = light_client_configuration_fingerprint(&[
            "https://primary.example/".into(),
            "https://backup.example/".into(),
        ]);
        let backup_first = light_client_configuration_fingerprint(&[
            "https://backup.example/".into(),
            "https://primary.example/".into(),
        ]);
        assert_ne!(primary_first, backup_first);
    }

    #[test]
    fn light_client_endpoint_fingerprint_is_stable_and_isolated() {
        let first = light_client_configuration_fingerprint(&["https://zaino-a.example/".into()]);
        let same = light_client_configuration_fingerprint(&["https://zaino-a.example/".into()]);
        let second = light_client_configuration_fingerprint(&["https://zaino-b.example/".into()]);
        assert_eq!(first.len(), 64);
        assert_eq!(first, same);
        assert_ne!(first, second);
        assert!(!first.contains("zaino"));
    }

    #[test]
    fn persisted_light_client_cache_has_bounded_freshness() {
        let now = OffsetDateTime::now_utc();
        let readiness = ZcashNetworkReadiness {
            configured: true,
            network: "testnet".into(),
            state: ZcashNetworkState::Syncing,
            network_actions_enabled: false,
            synced: false,
            block_height: Some(100),
            estimated_height: Some(101),
            lag: Some(1),
            last_confirmed_at: Some("2026-10-05T00:00:00Z".into()),
        };
        let fresh = PersistedLightClientReadiness {
            checked_at: now - Duration::seconds(1),
            readiness: Some(readiness.clone()),
        };
        assert!(fresh.is_fresh(now));
        assert!(fresh.within_stale_grace(now));

        let stale_but_usable = PersistedLightClientReadiness {
            checked_at: now - Duration::seconds(30),
            readiness: Some(readiness),
        };
        assert!(!stale_but_usable.is_fresh(now));
        assert!(stale_but_usable.within_stale_grace(now));

        let expired = PersistedLightClientReadiness {
            checked_at: now - Duration::seconds(91),
            readiness: Some(ZcashNetworkReadiness {
                configured: true,
                network: "testnet".into(),
                state: ZcashNetworkState::Ready,
                network_actions_enabled: true,
                synced: true,
                block_height: Some(100),
                estimated_height: Some(100),
                lag: Some(0),
                last_confirmed_at: Some("2026-10-05T00:00:00Z".into()),
            }),
        };
        assert!(!expired.within_stale_grace(now));

        let recent_failure = PersistedLightClientReadiness {
            checked_at: now - Duration::seconds(30),
            readiness: None,
        };
        assert!(recent_failure.is_fresh(now));
        assert!(!recent_failure.within_stale_grace(now));
    }

    #[test]
    fn light_client_database_heights_are_javascript_safe() {
        assert_eq!(safe_network_height(0).unwrap(), 0);
        assert_eq!(
            safe_network_height(i64::try_from(MAX_SAFE_INTEGER).unwrap()).unwrap(),
            MAX_SAFE_INTEGER
        );
        assert!(safe_network_height(-1).is_err());
        assert!(safe_network_height(i64::try_from(MAX_SAFE_INTEGER + 1).unwrap()).is_err());
    }

    #[test]
    fn retirement_migrations_are_wired_into_startup_order() {
        let source = include_str!("main.rs");
        let issuer = source
            .find("0035_issuer_retirement.sql")
            .expect("0035 wired");
        let verifier = source
            .find("0036_safe_verifier_retirement.sql")
            .expect("0036 wired");
        let serialized = source
            .find("0037_verifier_retirement_serialization.sql")
            .expect("0037 wired");
        assert!(issuer < verifier && verifier < serialized);
    }

    #[test]
    fn retired_issuer_blocks_new_trust_but_allows_maintenance() {
        let retired = IssuerAccess {
            profile_id: Uuid::nil(),
            owner_account_id: Uuid::nil(),
            role: "owner".into(),
            retired_at: Some(OffsetDateTime::UNIX_EPOCH),
        };
        assert!(require_active_issuer(&retired).is_err());
        assert!(require_issuer_role(&retired, &["owner", "admin"]).is_ok());
    }

    #[test]
    fn verifier_retirement_guard_serializes_new_work() {
        let schema = include_str!("../migrations/0037_verifier_retirement_serialization.sql");
        assert!(schema.contains("FOR SHARE"));
        assert!(schema.contains("verifier is retired"));
        assert!(schema.contains("verifier profile not found"));
    }

    #[test]
    fn verifier_retirement_blocks_inflight_work() {
        assert!(!verifier_retirement_blocked(false, false));
        assert!(verifier_retirement_blocked(true, false));
        assert!(verifier_retirement_blocked(false, true));
        assert!(verifier_retirement_blocked(true, true));
    }

    #[test]
    fn zcash_payment_activity_migration_is_metadata_only() {
        let schema = include_str!("../migrations/0034_zcash_payment_activity.sql");
        for required in [
            "zcash_payment_prepared",
            "zcash_payment_submitted",
            "NEW.id::text",
            "NEW.network",
        ] {
            assert!(schema.contains(required), "{required}");
        }
        for forbidden in [
            "recipient",
            "amount_zat",
            "txid",
            "wallet_address",
            "balance",
            "transaction_history",
        ] {
            assert!(!schema.contains(forbidden), "{forbidden}");
        }
    }

    #[test]
    fn zcash_readiness_migration_stores_no_endpoint_or_wallet_data() {
        let schema = include_str!("../migrations/0026_zcash_network_readiness.sql");
        for required in [
            "endpoint_fingerprint",
            "checked_at",
            "failure_count",
            "block_height",
            "estimated_height",
        ] {
            assert!(schema.contains(required), "{required}");
        }
        for forbidden in [
            "wallet_address",
            "balance",
            "memo",
            "seed",
            "transaction_history",
        ] {
            assert!(!schema.contains(forbidden), "{forbidden}");
        }
    }

    #[test]
    fn degraded_network_state_never_enables_actions() {
        let ready = ZcashNetworkReadiness {
            configured: true,
            network: "testnet".into(),
            state: ZcashNetworkState::Ready,
            network_actions_enabled: true,
            synced: true,
            block_height: Some(200),
            estimated_height: Some(200),
            lag: Some(0),
            last_confirmed_at: Some("2026-10-05T00:00:00Z".into()),
        };
        let degraded = as_degraded_readiness(ready);
        assert_eq!(degraded.state, ZcashNetworkState::Degraded);
        assert!(!degraded.network_actions_enabled);
        assert!(!degraded.synced);
        assert_eq!(degraded.block_height, Some(200));

        let cached = LightClientCacheEntry {
            checked_at: Instant::now(),
            readiness: Some(degraded),
        };
        assert_eq!(cached.ttl(), LIGHT_CLIENT_FAILURE_TTL);
    }

    #[test]
    fn payment_review_rejects_another_zcash_network() {
        let summary = zerant_zcash::zip321::create_payment_request(
            "tmEZhbWHTpdKMw5it8YDspUXSMGQyFwovpU",
            "1.25",
            "testnet",
        )
        .unwrap();
        assert!(payment_request_matches_network(&summary, "zcash:testnet"));
        assert!(!payment_request_matches_network(&summary, "zcash:mainnet"));
        assert!(!payment_request_matches_network(&summary, "invalid"));
    }

    #[test]
    fn degraded_last_good_state_has_bounded_display_window() {
        let now = OffsetDateTime::now_utc();
        assert!(last_good_is_recent(now - Duration::minutes(14), now));
        assert!(!last_good_is_recent(now - Duration::minutes(15), now));
        assert!(!last_good_is_recent(now + Duration::seconds(1), now));
    }

    #[test]
    fn last_good_migration_contains_no_wallet_private_data() {
        let schema = include_str!("../migrations/0028_zcash_network_last_good.sql");
        for required in [
            "last_success_block_height",
            "last_success_estimated_height",
            "last_success_lag",
            "last_success_synced",
        ] {
            assert!(schema.contains(required), "{required}");
        }
        for forbidden in ["wallet_address", "balance", "memo", "seed", "txid"] {
            assert!(!schema.contains(forbidden), "{forbidden}");
        }
    }

    #[test]
    fn light_client_success_cache_is_short_lived() {
        let readiness = ZcashNetworkReadiness {
            configured: true,
            network: "testnet".into(),
            state: ZcashNetworkState::Ready,
            network_actions_enabled: true,
            synced: true,
            block_height: Some(100),
            estimated_height: Some(100),
            lag: Some(0),
            last_confirmed_at: Some("2026-10-05T00:00:00Z".into()),
        };
        let fresh = LightClientCacheEntry {
            checked_at: Instant::now(),
            readiness: Some(readiness.clone()),
        };
        assert!(fresh.is_fresh());
        assert_eq!(fresh.ttl(), LIGHT_CLIENT_SUCCESS_TTL);
        assert_eq!(fresh.readiness.as_ref().unwrap().block_height, Some(100));

        let stale = LightClientCacheEntry {
            checked_at: Instant::now() - LIGHT_CLIENT_SUCCESS_TTL - StdDuration::from_millis(1),
            readiness: Some(readiness),
        };
        assert!(!stale.is_fresh());
    }

    #[test]
    fn light_client_failure_cache_backs_off_longer() {
        let fresh_failure = LightClientCacheEntry {
            checked_at: Instant::now(),
            readiness: None,
        };
        assert!(fresh_failure.is_fresh());
        assert_eq!(fresh_failure.ttl(), LIGHT_CLIENT_FAILURE_TTL);
        assert!(LIGHT_CLIENT_FAILURE_TTL > LIGHT_CLIENT_SUCCESS_TTL);

        let stale_failure = LightClientCacheEntry {
            checked_at: Instant::now() - LIGHT_CLIENT_FAILURE_TTL - StdDuration::from_millis(1),
            readiness: None,
        };
        assert!(!stale_failure.is_fresh());
    }

    #[test]
    fn operational_health_fails_closed_on_actionable_conditions() {
        assert_eq!(
            operational_health_flags(false, false, 0, 0, 0),
            (true, false)
        );
        assert_eq!(
            operational_health_flags(true, false, 0, 0, 0),
            (false, true)
        );
        assert_eq!(
            operational_health_flags(false, true, 0, 0, 0),
            (false, true)
        );
        assert_eq!(
            operational_health_flags(false, false, 1, 0, 0),
            (false, true)
        );
        assert_eq!(
            operational_health_flags(false, false, 0, 101, 0),
            (false, true)
        );
        assert_eq!(
            operational_health_flags(false, false, 0, 100, 0),
            (true, false)
        );
        assert_eq!(
            operational_health_flags(false, false, 0, 0, 1),
            (false, true)
        );
    }

    #[test]
    fn operational_health_migration_stores_only_bounded_job_state() {
        let schema = include_str!("../migrations/0030_operational_health.sql");
        for required in [
            "maintenance_job_state",
            "last_success_at",
            "last_summary",
            "updated_at",
        ] {
            assert!(schema.contains(required), "{required}");
        }
        for forbidden in [
            "credential",
            "wallet_address",
            "public_key",
            "private_key",
            "session_token",
            "memo",
        ] {
            assert!(!schema.contains(forbidden), "{forbidden}");
        }
    }

    #[tokio::test]
    async fn zcash_removal_database_contract() {
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
                    BTreeMap::from([(1, Aes256Gcm::new_from_slice(&[7_u8; 32]).unwrap())]),
                    1,
                )
                .unwrap(),
            ),
            webauthn: Arc::new(webauthn),
            public_origin: "https://zerant.example".to_owned(),
            zcash_chain: "zcash:testnet".to_owned(),
            light_client_endpoints: Vec::new(),
            light_client_allow_loopback: false,
            light_client_cache: Arc::new(tokio::sync::Mutex::new(None)),
            allowed_scopes: BTreeSet::from(["auth".to_owned()]),
        };
        let client = db_client(&db).await.unwrap();
        let mut accounts = Vec::new();

        // Unauthenticated and stale requests leave identity and sessions intact.
        let (account, handle, passkey, key) =
            removal_account(&client, true, true, &["zcash:testnet", "zcash:mainnet"]).await;
        accounts.push(account);
        assert!(passkey.is_some());
        let key = key.unwrap();
        let (passkey_token, passkey_hash) = removal_session(&client, account, "passkey").await;
        let (_other_zcash_token, other_zcash_hash) =
            removal_session(&client, account, "zcash").await;
        let (zcash_token, zcash_hash) = removal_session(&client, account, "zcash").await;
        assert!(matches!(
            remove_zecauth_method(State(state.clone()), HeaderMap::new()).await,
            Err(ApiError::Unauthorized)
        ));
        client.execute(
            "UPDATE sessions SET created_at = NOW() - INTERVAL '16 minutes' WHERE token_hash = $1",
            &[&passkey_hash],
        ).await.unwrap();
        assert!(matches!(
            remove_zecauth_method(State(state.clone()), removal_headers(&passkey_token)).await,
            Err(ApiError::Forbidden)
        ));
        assert_eq!(
            client
                .query_one(
                    "SELECT COUNT(*) FROM zecauth_identities WHERE account_id = $1",
                    &[&account]
                )
                .await
                .unwrap()
                .get::<_, i64>(0),
            1
        );
        client
            .execute(
                "UPDATE sessions SET created_at = NOW() WHERE token_hash = $1",
                &[&passkey_hash],
            )
            .await
            .unwrap();
        assert!(matches!(
            remove_wallet_message_method(
                State(state.clone()),
                removal_headers(&passkey_token),
                Path("regtest".to_owned())
            )
            .await,
            Err(ApiError::Invalid)
        ));
        assert!(matches!(
            remove_wallet_message_method(
                State(state.clone()),
                removal_headers(&passkey_token),
                Path("testnet-extra".to_owned())
            )
            .await,
            Err(ApiError::Invalid)
        ));
        assert!(matches!(
            remove_wallet_message_method(
                State(state.clone()),
                removal_headers(&passkey_token),
                Path("testnet".to_owned())
            )
            .await
            .unwrap()
            .status(),
            StatusCode::NO_CONTENT
        ));
        assert_eq!(client.query_one(
            "SELECT COUNT(*) FROM wallet_message_identities WHERE account_id = $1 AND chain = 'zcash:mainnet'",
            &[&account]
        ).await.unwrap().get::<_, i64>(0), 1);
        assert_eq!(client.query_one(
            "SELECT COUNT(*) FROM wallet_message_identities WHERE account_id = $1 AND chain = 'zcash:testnet'",
            &[&account]
        ).await.unwrap().get::<_, i64>(0), 0);
        // The wallet removal revoked both Zcash sessions, including the caller-independent one.
        for hash in [&zcash_hash, &other_zcash_hash] {
            assert_eq!(
                client
                    .query_one(
                        "SELECT COUNT(*) FROM sessions WHERE account_id = $1 AND token_hash = $2",
                        &[&account, hash]
                    )
                    .await
                    .unwrap()
                    .get::<_, i64>(0),
                0
            );
        }
        let sessions_before: i64 = client
            .query_one(
                "SELECT COUNT(*) FROM sessions WHERE account_id = $1",
                &[&account],
            )
            .await
            .unwrap()
            .get(0);
        assert!(matches!(
            remove_wallet_message_method(
                State(state.clone()),
                removal_headers(&passkey_token),
                Path("testnet".to_owned())
            )
            .await,
            Err(ApiError::NotFound)
        ));
        assert_eq!(
            client
                .query_one(
                    "SELECT COUNT(*) FROM sessions WHERE account_id = $1",
                    &[&account]
                )
                .await
                .unwrap()
                .get::<_, i64>(0),
            sessions_before
        );
        assert!(matches!(
            remove_zecauth_method(State(state.clone()), removal_headers(&zcash_token)).await,
            Err(ApiError::Unauthorized)
        ));
        let response = remove_zecauth_method(State(state.clone()), removal_headers(&passkey_token))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert!(response.headers().get(SET_COOKIE).is_none());
        assert_eq!(client.query_one(
            "SELECT COUNT(*) FROM zecauth_identities WHERE account_id = $1 OR verification_key = $2",
            &[&account, &key]
        ).await.unwrap().get::<_, i64>(0), 0);
        assert_eq!(
            client
                .query_one(
                    "SELECT public_handle FROM accounts WHERE id = $1",
                    &[&account]
                )
                .await
                .unwrap()
                .get::<_, String>(0),
            handle
        );
        assert_eq!(client.query_one(
            "SELECT COUNT(*) FROM sessions WHERE account_id = $1 AND auth_method = 'passkey'",
            &[&account]
        ).await.unwrap().get::<_, i64>(0), 1);
        assert_eq!(
            client
                .query_one(
                    "SELECT COUNT(*) FROM sessions WHERE account_id = $1 AND auth_method = 'zcash'",
                    &[&account]
                )
                .await
                .unwrap()
                .get::<_, i64>(0),
            0
        );
        assert!(matches!(
            remove_zecauth_method(State(state.clone()), removal_headers(&passkey_token)).await,
            Err(ApiError::NotFound)
        ));
        assert_eq!(
            client
                .query_one(
                    "SELECT COUNT(*) FROM sessions WHERE account_id = $1",
                    &[&account]
                )
                .await
                .unwrap()
                .get::<_, i64>(0),
            1
        );

        // A sole ZecAuth or wallet-message method cannot be removed.
        let (sole_zec, _, _, _) = removal_account(&client, false, true, &[]).await;
        accounts.push(sole_zec);
        let (sole_zec_token, sole_zec_hash) = removal_session(&client, sole_zec, "zcash").await;
        assert!(matches!(
            remove_zecauth_method(State(state.clone()), removal_headers(&sole_zec_token)).await,
            Err(ApiError::Conflict)
        ));
        assert_eq!(
            client
                .query_one(
                    "SELECT COUNT(*) FROM zecauth_identities WHERE account_id = $1",
                    &[&sole_zec]
                )
                .await
                .unwrap()
                .get::<_, i64>(0),
            1
        );
        assert_eq!(
            client
                .query_one(
                    "SELECT COUNT(*) FROM sessions WHERE account_id = $1 AND token_hash = $2",
                    &[&sole_zec, &sole_zec_hash]
                )
                .await
                .unwrap()
                .get::<_, i64>(0),
            1
        );
        let (sole_wallet, _, _, _) =
            removal_account(&client, false, false, &["zcash:testnet"]).await;
        accounts.push(sole_wallet);
        let (sole_wallet_token, sole_wallet_hash) =
            removal_session(&client, sole_wallet, "zcash").await;
        assert!(matches!(
            remove_wallet_message_method(
                State(state.clone()),
                removal_headers(&sole_wallet_token),
                Path("testnet".to_owned())
            )
            .await,
            Err(ApiError::Conflict)
        ));
        assert_eq!(
            client
                .query_one(
                    "SELECT COUNT(*) FROM wallet_message_identities WHERE account_id = $1",
                    &[&sole_wallet]
                )
                .await
                .unwrap()
                .get::<_, i64>(0),
            1
        );
        assert_eq!(
            client
                .query_one(
                    "SELECT COUNT(*) FROM sessions WHERE account_id = $1 AND token_hash = $2",
                    &[&sole_wallet, &sole_wallet_hash]
                )
                .await
                .unwrap()
                .get::<_, i64>(0),
            1
        );

        // Removing ZecAuth from its own recent Zcash session clears that cookie.
        let (current_zec, current_handle, _, current_key) =
            removal_account(&client, true, true, &[]).await;
        accounts.push(current_zec);
        let (current_token, current_hash) = removal_session(&client, current_zec, "zcash").await;
        let (_, second_current_hash) = removal_session(&client, current_zec, "zcash").await;
        let (_, preserved_hash) = removal_session(&client, current_zec, "passkey").await;
        let response = remove_zecauth_method(State(state.clone()), removal_headers(&current_token))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert!(
            response
                .headers()
                .get(SET_COOKIE)
                .unwrap()
                .to_str()
                .unwrap()
                .contains("Max-Age=0")
        );
        for hash in [&current_hash, &second_current_hash, &preserved_hash] {
            let expected = if hash == &preserved_hash { 1 } else { 0 };
            assert_eq!(
                client
                    .query_one(
                        "SELECT COUNT(*) FROM sessions WHERE account_id = $1 AND token_hash = $2",
                        &[&current_zec, hash]
                    )
                    .await
                    .unwrap()
                    .get::<_, i64>(0),
                expected
            );
        }
        assert_eq!(
            client
                .query_one(
                    "SELECT COUNT(*) FROM zecauth_identities WHERE verification_key = $1",
                    &[&current_key.unwrap()]
                )
                .await
                .unwrap()
                .get::<_, i64>(0),
            0
        );
        assert_eq!(
            client
                .query_one(
                    "SELECT public_handle FROM accounts WHERE id = $1",
                    &[&current_zec]
                )
                .await
                .unwrap()
                .get::<_, String>(0),
            current_handle
        );

        // Both deletion paths lock the account; one of two last-method removals wins.
        let (race_account, _, race_passkey, _) = removal_account(&client, true, true, &[]).await;
        accounts.push(race_account);
        let race_passkey = race_passkey.unwrap();
        let (race_passkey_token, _) = removal_session(&client, race_account, "passkey").await;
        let (race_zec_token, _) = removal_session(&client, race_account, "passkey").await;
        let (passkey_result, zec_result) = tokio::join!(
            delete_account_passkey(
                State(state.clone()),
                removal_headers(&race_passkey_token),
                Path(race_passkey)
            ),
            remove_zecauth_method(State(state.clone()), removal_headers(&race_zec_token)),
        );
        let successes = i64::from(passkey_result.is_ok()) + i64::from(zec_result.is_ok());
        assert_eq!(successes, 1);
        assert!(matches!(
            passkey_result,
            Ok(StatusCode::NO_CONTENT) | Err(ApiError::Conflict)
        ));
        assert!(matches!(zec_result, Ok(_) | Err(ApiError::Conflict)));
        let remaining: i64 = client
            .query_one(
                "SELECT (SELECT COUNT(*) FROM passkey_credentials WHERE account_id = $1)
                  + (SELECT COUNT(*) FROM zecauth_identities WHERE account_id = $1)",
                &[&race_account],
            )
            .await
            .unwrap()
            .get(0);
        assert!(remaining >= 1);

        // Revoke the session while passkey deletion waits on the account row.
        let (revoked_account, _, revoked_passkey, _) =
            removal_account(&client, true, true, &[]).await;
        accounts.push(revoked_account);
        let revoked_passkey = revoked_passkey.unwrap();
        let (revoked_token, revoked_hash) =
            removal_session(&client, revoked_account, "zcash").await;
        let mut lock_client = db_client(&db).await.unwrap();
        let lock = lock_client.transaction().await.unwrap();
        lock.query_one(
            "SELECT id FROM accounts WHERE id = $1 FOR NO KEY UPDATE",
            &[&revoked_account],
        )
        .await
        .unwrap();
        let pending = tokio::spawn(delete_account_passkey(
            State(state.clone()),
            removal_headers(&revoked_token),
            Path(revoked_passkey),
        ));
        tokio::time::timeout(StdDuration::from_secs(5), async {
            loop {
                if client
                    .query_opt(
                        "SELECT 1 FROM account_rate_limits
                         WHERE account_id = $1 AND action = 'passkey_remove'",
                        &[&revoked_account],
                    )
                    .await
                    .unwrap()
                    .is_some()
                {
                    break;
                }
                tokio::time::sleep(StdDuration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        lock.execute(
            "DELETE FROM zecauth_identities WHERE account_id = $1",
            &[&revoked_account],
        )
        .await
        .unwrap();
        lock.execute(
            "DELETE FROM sessions WHERE account_id = $1 AND token_hash = $2",
            &[&revoked_account, &revoked_hash],
        )
        .await
        .unwrap();
        lock.commit().await.unwrap();
        assert!(matches!(
            pending.await.unwrap(),
            Err(ApiError::Unauthorized)
        ));
        assert_eq!(
            client
                .query_one(
                    "SELECT COUNT(*) FROM passkey_credentials WHERE id = $1",
                    &[&revoked_passkey]
                )
                .await
                .unwrap()
                .get::<_, i64>(0),
            1
        );

        for account in accounts {
            client
                .execute("DELETE FROM accounts WHERE id = $1", &[&account])
                .await
                .unwrap();
        }
    }

    async fn challenge_message(response: Response) -> String {
        let body = axum::body::to_bytes(response.into_body(), 1 << 20)
            .await
            .unwrap();
        serde_json::from_slice::<serde_json::Value>(&body).unwrap()["message"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    // Run with ZERANT_TEST_DATABASE_URL pointing at a disposable PostgreSQL database.
    #[tokio::test]
    async fn zcash_link_database_contract() {
        use reddsa::SigningKey;
        use secp256k1::SecretKey;

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
                    BTreeMap::from([(1, Aes256Gcm::new_from_slice(&[7_u8; 32]).unwrap())]),
                    1,
                )
                .unwrap(),
            ),
            webauthn: Arc::new(webauthn),
            public_origin: "https://zerant.example".to_owned(),
            zcash_chain: "zcash:testnet".to_owned(),
            light_client_endpoints: Vec::new(),
            light_client_allow_loopback: false,
            light_client_cache: Arc::new(tokio::sync::Mutex::new(None)),
            allowed_scopes: BTreeSet::from(["auth".to_owned()]),
        };
        let client = db_client(&db).await.unwrap();
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let first_handle = zerant_public_handle(first.as_bytes());
        let second_handle = zerant_public_handle(second.as_bytes());
        for (account, handle) in [(first, &first_handle), (second, &second_handle)] {
            client
                .execute(
                    "INSERT INTO accounts(id, public_handle) VALUES ($1, $2)",
                    &[&account, handle],
                )
                .await
                .unwrap();
        }
        let first_token = format!("session_{}", Uuid::new_v4());
        let second_token = format!("session_{}", Uuid::new_v4());
        let first_session = Uuid::new_v4();
        let second_session = Uuid::new_v4();
        for (id, account, token) in [
            (first_session, first, &first_token),
            (second_session, second, &second_token),
        ] {
            let hash = Sha256::digest(token.as_bytes()).to_vec();
            client
                .execute(
                    "INSERT INTO sessions(id, account_id, token_hash, expires_at)
                 VALUES ($1, $2, $3, NOW() + INTERVAL '1 day')",
                    &[&id, &account, &hash],
                )
                .await
                .unwrap();
        }
        let sessions_before: i64 = client
            .query_one(
                "SELECT COUNT(*) FROM sessions WHERE account_id IN ($1, $2)",
                &[&first, &second],
            )
            .await
            .unwrap()
            .get(0);
        let cookie = |token: &str| {
            let mut headers = HeaderMap::new();
            headers.insert(
                axum::http::header::COOKIE,
                HeaderValue::from_str(&format!("{SESSION_COOKIE}={token}")).unwrap(),
            );
            headers
        };
        let first_headers = cookie(&first_token);
        let second_headers = cookie(&second_token);
        assert!(matches!(
            account_zcash_challenge(State(state.clone()), HeaderMap::new()).await,
            Err(ApiError::Unauthorized)
        ));
        client
            .execute(
                "UPDATE sessions SET created_at = NOW() - INTERVAL '16 minutes' WHERE id = $1",
                &[&first_session],
            )
            .await
            .unwrap();
        assert!(matches!(
            account_zcash_challenge(State(state.clone()), first_headers.clone()).await,
            Err(ApiError::Forbidden)
        ));
        client
            .execute(
                "UPDATE sessions SET created_at = NOW() WHERE id = $1",
                &[&first_session],
            )
            .await
            .unwrap();
        let new_challenge = |headers: HeaderMap| async {
            challenge_message(
                account_zcash_challenge(State(state.clone()), headers)
                    .await
                    .unwrap(),
            )
            .await
        };

        let red = SigningKey::<SpendAuth>::new(OsRng);
        let red_key: [u8; 32] = VerificationKey::from(&red).into();
        let sign_red = |message: String| VerifyZecAuth {
            pubkey: hex::encode(red_key),
            signature: hex::encode(<[u8; 64]>::from(red.sign(OsRng, message.as_bytes()))),
            message,
            granted: vec!["auth".to_owned()],
        };
        let granted = vec!["auth".to_owned()];
        let message = new_challenge(first_headers.clone()).await;
        let challenge_row = client.query_one(
            "SELECT link_account_id, link_session_id FROM zecauth_challenges WHERE message = $1",
            &[&message],
        ).await.unwrap();
        assert_eq!(challenge_row.get::<_, Uuid>(0), first);
        assert_eq!(challenge_row.get::<_, Uuid>(1), first_session);
        let accounts_before: i64 = client
            .query_one(
                "SELECT COUNT(*) FROM accounts WHERE id IN ($1, $2)",
                &[&first, &second],
            )
            .await
            .unwrap()
            .get(0);
        let handoff = account_zcash_challenge(State(state.clone()), first_headers.clone())
            .await
            .unwrap();
        let set_cookie = handoff
            .headers()
            .get(SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        let attempt = set_cookie.split(';').next().unwrap().to_owned();
        let handoff_message = challenge_message(handoff).await;
        let handoff_row = client.query_one(
            "SELECT link_attempt_hash, pending_link_key FROM zecauth_challenges WHERE message = $1",
            &[&handoff_message],
        ).await.unwrap();
        let stored_hash: Vec<u8> = handoff_row.get(0);
        assert_eq!(
            stored_hash,
            Sha256::digest(attempt.split('=').nth(1).unwrap().as_bytes()).to_vec()
        );
        assert_ne!(stored_hash, attempt.as_bytes());
        assert!(handoff_row.get::<_, Option<Vec<u8>>>(1).is_none());
        let with_attempt = |token: &str| {
            let mut headers = cookie(token);
            headers.insert(
                axum::http::header::COOKIE,
                HeaderValue::from_str(&format!("{SESSION_COOKIE}={token}; {attempt}")).unwrap(),
            );
            headers
        };
        assert!(matches!(
            account_zecauth_link_complete(State(state.clone()), first_headers.clone()).await,
            Err(ApiError::Unauthorized)
        ));
        assert!(matches!(
            account_zecauth_link_complete(State(state.clone()), with_attempt(&second_token)).await,
            Err(ApiError::Unauthorized)
        ));
        let waiting =
            account_zecauth_link_complete(State(state.clone()), with_attempt(&first_token))
                .await
                .unwrap();
        assert_eq!(waiting.status(), StatusCode::ACCEPTED);
        assert!(
            client
                .query_one(
                    "SELECT consumed_at IS NULL FROM zecauth_challenges WHERE message = $1",
                    &[&handoff_message],
                )
                .await
                .unwrap()
                .get::<_, bool>(0)
        );
        let race_message = new_challenge(first_headers.clone()).await;
        let racing = SigningKey::<SpendAuth>::new(OsRng);
        let racing_key: [u8; 32] = VerificationKey::from(&racing).into();
        let first_response = sign_red(race_message.clone());
        let second_response = VerifyZecAuth {
            pubkey: hex::encode(racing_key),
            signature: hex::encode(<[u8; 64]>::from(
                racing.sign(OsRng, race_message.as_bytes()),
            )),
            message: race_message.clone(),
            granted: vec!["auth".to_owned()],
        };
        let first_state = state.clone();
        let first_input = first_response.clone();
        let first_task = tokio::spawn(async move {
            account_zecauth_link_callback(State(first_state), Json(first_input)).await
        });
        let second_state = state.clone();
        let second_input = second_response.clone();
        let second_task = tokio::spawn(async move {
            account_zecauth_link_callback(State(second_state), Json(second_input)).await
        });
        let (first_result, second_result) = tokio::join!(first_task, second_task);
        let first_result = first_result.unwrap();
        let second_result = second_result.unwrap();
        assert!(first_result.is_ok() != second_result.is_ok());
        assert!(matches!(
            first_result.as_ref().err().or(second_result.as_ref().err()),
            Some(ApiError::Conflict)
        ));
        let (winner_key, winner_response) = if first_result.is_ok() {
            (red_key, first_response)
        } else {
            (racing_key, second_response)
        };
        let pending: Vec<u8> = client
            .query_one(
                "SELECT pending_link_key FROM zecauth_challenges WHERE message = $1",
                &[&race_message],
            )
            .await
            .unwrap()
            .get(0);
        assert_eq!(pending, winner_key);
        assert!(
            account_zecauth_link_callback(State(state.clone()), Json(winner_response))
                .await
                .is_ok()
        );
        client
            .execute(
                "UPDATE zecauth_challenges SET expires_at = NOW() - INTERVAL '1 second' WHERE message = $1",
                &[&race_message],
            )
            .await
            .unwrap();
        assert!(matches!(
            account_zecauth_link_callback(
                State(state.clone()),
                Json(sign_red(race_message.clone()))
            )
            .await,
            Err(ApiError::Unauthorized)
        ));
        let _ = account_zecauth_link_callback(
            State(state.clone()),
            Json(sign_red(handoff_message.clone())),
        )
        .await
        .unwrap();
        let _ = account_zecauth_link_callback(
            State(state.clone()),
            Json(sign_red(handoff_message.clone())),
        )
        .await
        .unwrap();
        let conflicting = SigningKey::<SpendAuth>::new(OsRng);
        let conflicting_key: [u8; 32] = VerificationKey::from(&conflicting).into();
        let conflicting_response = VerifyZecAuth {
            pubkey: hex::encode(conflicting_key),
            signature: hex::encode(<[u8; 64]>::from(
                conflicting.sign(OsRng, handoff_message.as_bytes()),
            )),
            message: handoff_message.clone(),
            granted: vec!["auth".to_owned()],
        };
        assert!(matches!(
            account_zecauth_link_callback(State(state.clone()), Json(conflicting_response)).await,
            Err(ApiError::Conflict)
        ));
        assert_eq!(
            client
                .query_one(
                    "SELECT COUNT(*) FROM zecauth_identities WHERE verification_key = $1",
                    &[&red_key.as_slice()]
                )
                .await
                .unwrap()
                .get::<_, i64>(0),
            0
        );
        assert!(matches!(
            zecauth_verify(
                State(state.clone()),
                Json(sign_red(handoff_message.clone()))
            )
            .await,
            Err(ApiError::Unauthorized)
        ));
        client
            .execute(
                "UPDATE sessions SET created_at = NOW() - INTERVAL '16 minutes' WHERE id = $1",
                &[&first_session],
            )
            .await
            .unwrap();
        assert!(matches!(
            account_zecauth_link_complete(State(state.clone()), with_attempt(&first_token)).await,
            Err(ApiError::Unauthorized)
        ));
        assert_eq!(
            client
                .query_one(
                    "SELECT COUNT(*) FROM zecauth_identities WHERE verification_key = $1",
                    &[&red_key.as_slice()]
                )
                .await
                .unwrap()
                .get::<_, i64>(0),
            0
        );
        client.execute("UPDATE sessions SET created_at = NOW(), expires_at = NOW() - INTERVAL '1 second' WHERE id = $1", &[&first_session]).await.unwrap();
        assert!(matches!(
            account_zecauth_link_complete(State(state.clone()), with_attempt(&first_token)).await,
            Err(ApiError::Unauthorized)
        ));
        client
            .execute(
                "UPDATE sessions SET expires_at = NOW() + INTERVAL '1 day' WHERE id = $1",
                &[&first_session],
            )
            .await
            .unwrap();
        let completed =
            account_zecauth_link_complete(State(state.clone()), with_attempt(&first_token))
                .await
                .unwrap();
        assert_eq!(completed.status(), StatusCode::OK);
        assert!(
            completed
                .headers()
                .get(SET_COOKIE)
                .unwrap()
                .to_str()
                .unwrap()
                .contains("Max-Age=0")
        );
        assert_eq!(
            client
                .query_one(
                    "SELECT account_id FROM zecauth_identities WHERE verification_key = $1",
                    &[&red_key.as_slice()]
                )
                .await
                .unwrap()
                .get::<_, Uuid>(0),
            first
        );
        assert_eq!(
            client
                .query_one(
                    "SELECT public_handle FROM accounts WHERE id = $1",
                    &[&first]
                )
                .await
                .unwrap()
                .get::<_, Option<String>>(0)
                .unwrap(),
            first_handle
        );
        assert_eq!(
            client
                .query_one(
                    "SELECT COUNT(*) FROM accounts WHERE id IN ($1, $2)",
                    &[&first, &second],
                )
                .await
                .unwrap()
                .get::<_, i64>(0),
            accounts_before
        );
        assert_eq!(
            client
                .query_one(
                    "SELECT COUNT(*) FROM sessions WHERE account_id IN ($1, $2)",
                    &[&first, &second],
                )
                .await
                .unwrap()
                .get::<_, i64>(0),
            sessions_before
        );
        let completed_row = client
            .query_one(
                "SELECT consumed_at, link_attempt_hash, pending_link_key, pending_link_at FROM zecauth_challenges WHERE message = $1",
                &[&handoff_message],
            )
            .await
            .unwrap();
        assert!(completed_row.get::<_, Option<OffsetDateTime>>(0).is_some());
        assert!(completed_row.get::<_, Option<Vec<u8>>>(1).is_none());
        assert!(completed_row.get::<_, Option<Vec<u8>>>(2).is_none());
        assert!(completed_row.get::<_, Option<OffsetDateTime>>(3).is_none());
        assert!(matches!(
            account_zecauth_link_complete(State(state.clone()), with_attempt(&first_token)).await,
            Err(ApiError::Unauthorized)
        ));
        assert!(matches!(
            account_zecauth_link_callback(State(state.clone()), Json(sign_red(handoff_message)))
                .await,
            Err(ApiError::Unauthorized)
        ));
        assert!(matches!(
            account_zecauth_verify(
                State(state.clone()),
                second_headers.clone(),
                Json(sign_red(message.clone()))
            )
            .await,
            Err(ApiError::Unauthorized)
        ));
        assert!(matches!(
            zecauth_verify(State(state.clone()), Json(sign_red(message.clone()))).await,
            Err(ApiError::Unauthorized)
        ));
        let secp = Secp256k1::new();
        let secret = loop {
            let mut bytes = [0_u8; 32];
            OsRng.fill_bytes(&mut bytes);
            if let Ok(secret) = SecretKey::from_byte_array(bytes) {
                break secret;
            }
        };
        let key = SecpPublicKey::from_secret_key(&secp, &secret);
        let sign_wallet = |message: String| {
            let digest = zcash_signed_message_hash(&message).unwrap();
            let signature = secp.sign_ecdsa_recoverable(SecpMessage::from_digest(digest), &secret);
            let (recovery, compact) = signature.serialize_compact();
            let mut bytes = vec![31 + i32::from(recovery) as u8];
            bytes.extend_from_slice(&compact);
            VerifyWalletMessage {
                pubkey: hex::encode(key.serialize()),
                signature: hex::encode(bytes),
                message,
                granted: vec!["auth".to_owned()],
                signing_mode: "derived".to_owned(),
            }
        };
        assert!(matches!(
            wallet_message_verify(State(state.clone()), Json(sign_wallet(message.clone()))).await,
            Err(ApiError::Unauthorized)
        ));
        assert_eq!(
            client
                .query_one(
                    "SELECT COUNT(*) FROM accounts WHERE id IN ($1, $2)",
                    &[&first, &second],
                )
                .await
                .unwrap()
                .get::<_, i64>(0),
            accounts_before
        );
        assert!(
            client
                .query_one(
                    "SELECT consumed_at IS NULL FROM zecauth_challenges WHERE message = $1",
                    &[&message]
                )
                .await
                .unwrap()
                .get::<_, bool>(0)
        );
        let linked_response = account_zecauth_verify(
            State(state.clone()),
            first_headers.clone(),
            Json(sign_red(message.clone())),
        )
        .await
        .unwrap();
        assert_eq!(
            linked_response
                .headers()
                .get(SET_COOKIE)
                .unwrap()
                .to_str()
                .unwrap(),
            clear_cookie_header(LINK_ATTEMPT_COOKIE)
        );
        let linked: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(linked_response.into_body(), 1 << 20)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(linked["zerant_id"], first_handle);
        assert_eq!(
            linked
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["linked", "zerant_id"])
        );
        let duplicate = account_zecauth_verify(
            State(state.clone()),
            first_headers.clone(),
            Json(sign_red(message.clone())),
        )
        .await
        .unwrap_err();
        assert!(matches!(duplicate, ApiError::Unauthorized));
        assert!(
            duplicate
                .into_response()
                .headers()
                .get(SET_COOKIE)
                .is_none()
        );
        assert!(
            !client
                .query_one(
                    "SELECT consumed_at IS NULL FROM zecauth_challenges WHERE message = $1",
                    &[&message]
                )
                .await
                .unwrap()
                .get::<_, bool>(0)
        );
        let message = new_challenge(first_headers.clone()).await;
        let _ = account_zecauth_verify(
            State(state.clone()),
            first_headers.clone(),
            Json(sign_red(message)),
        )
        .await
        .unwrap(); // Same account is idempotent.
        let message = new_challenge(second_headers.clone()).await;
        let conflict = account_zecauth_verify(
            State(state.clone()),
            second_headers.clone(),
            Json(sign_red(message.clone())),
        )
        .await
        .unwrap_err();
        assert!(matches!(conflict, ApiError::Conflict));
        assert!(conflict.into_response().headers().get(SET_COOKIE).is_none());
        assert!(
            client
                .query_one(
                    "SELECT consumed_at IS NULL FROM zecauth_challenges WHERE message = $1",
                    &[&message]
                )
                .await
                .unwrap()
                .get::<_, bool>(0)
        );
        let other_red = SigningKey::<SpendAuth>::new(OsRng);
        let other_key: [u8; 32] = VerificationKey::from(&other_red).into();
        let message = new_challenge(first_headers.clone()).await;
        assert!(matches!(
            finalize_zcash_link(
                &state,
                &first_headers,
                &message,
                &granted,
                LinkIdentity::ZecAuth(other_key)
            )
            .await,
            Err(ApiError::Conflict)
        ));
        assert!(
            client
                .query_one(
                    "SELECT consumed_at IS NULL FROM zecauth_challenges WHERE message = $1",
                    &[&message]
                )
                .await
                .unwrap()
                .get::<_, bool>(0)
        );
        let message = new_challenge(first_headers.clone()).await;
        let linked_response = account_wallet_verify(
            State(state.clone()),
            first_headers.clone(),
            Json(sign_wallet(message)),
        )
        .await
        .unwrap();
        assert_eq!(
            linked_response
                .headers()
                .get(SET_COOKIE)
                .unwrap()
                .to_str()
                .unwrap(),
            clear_cookie_header(LINK_ATTEMPT_COOKIE)
        );
        let linked: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(linked_response.into_body(), 1 << 20)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(linked["zerant_id"], first_handle);
        let message = new_challenge(first_headers.clone()).await;
        let _ = account_wallet_verify(
            State(state.clone()),
            first_headers.clone(),
            Json(sign_wallet(message)),
        )
        .await
        .unwrap();
        let message = new_challenge(second_headers.clone()).await;
        let conflict = account_wallet_verify(
            State(state.clone()),
            second_headers.clone(),
            Json(sign_wallet(message.clone())),
        )
        .await
        .unwrap_err();
        assert!(matches!(conflict, ApiError::Conflict));
        assert!(conflict.into_response().headers().get(SET_COOKIE).is_none());
        assert!(
            client
                .query_one(
                    "SELECT consumed_at IS NULL FROM zecauth_challenges WHERE message = $1",
                    &[&message]
                )
                .await
                .unwrap()
                .get::<_, bool>(0)
        );
        let message = new_challenge(first_headers.clone()).await;
        assert!(matches!(
            finalize_zcash_link(
                &state,
                &first_headers,
                &message,
                &granted,
                LinkIdentity::WalletMessage(
                    SecpPublicKey::from_secret_key(
                        &secp,
                        &SecretKey::from_byte_array([8_u8; 32]).unwrap()
                    )
                    .serialize()
                )
            )
            .await,
            Err(ApiError::Conflict)
        ));
        assert!(
            client
                .query_one(
                    "SELECT consumed_at IS NULL FROM zecauth_challenges WHERE message = $1",
                    &[&message]
                )
                .await
                .unwrap()
                .get::<_, bool>(0)
        );

        // Keep this integration fixture from conflating link security cases with quota exhaustion.
        client
            .execute(
                "DELETE FROM account_rate_limits WHERE account_id = $1 AND action = 'zcash_link_start'",
                &[&first],
            )
            .await
            .unwrap();
        let mut mainnet = state.clone();
        mainnet.zcash_chain = "zcash:mainnet".to_owned();
        let mainnet_challenge = |headers: HeaderMap| async {
            challenge_message(
                account_zcash_challenge(State(mainnet.clone()), headers)
                    .await
                    .unwrap(),
            )
            .await
        };
        let mainnet_message = mainnet_challenge(first_headers.clone()).await;
        let _ = account_wallet_verify(
            State(mainnet.clone()),
            first_headers.clone(),
            Json(sign_wallet(mainnet_message)),
        )
        .await
        .unwrap();
        let mainnet_message = mainnet_challenge(first_headers.clone()).await;
        let _ = account_wallet_verify(
            State(mainnet.clone()),
            first_headers.clone(),
            Json(sign_wallet(mainnet_message)),
        )
        .await
        .unwrap();
        let mainnet_message = mainnet_challenge(second_headers.clone()).await;
        assert!(matches!(
            account_wallet_verify(
                State(mainnet.clone()),
                second_headers.clone(),
                Json(sign_wallet(mainnet_message.clone()))
            )
            .await,
            Err(ApiError::Conflict)
        ));
        assert!(
            client
                .query_one(
                    "SELECT consumed_at IS NULL FROM zecauth_challenges WHERE message = $1",
                    &[&mainnet_message]
                )
                .await
                .unwrap()
                .get::<_, bool>(0)
        );
        let mainnet_message = mainnet_challenge(first_headers.clone()).await;
        assert!(matches!(
            finalize_zcash_link(
                &mainnet,
                &first_headers,
                &mainnet_message,
                &granted,
                LinkIdentity::WalletMessage(
                    SecpPublicKey::from_secret_key(
                        &secp,
                        &SecretKey::from_byte_array([8_u8; 32]).unwrap()
                    )
                    .serialize()
                )
            )
            .await,
            Err(ApiError::Conflict)
        ));
        assert!(
            client
                .query_one(
                    "SELECT consumed_at IS NULL FROM zecauth_challenges WHERE message = $1",
                    &[&mainnet_message]
                )
                .await
                .unwrap()
                .get::<_, bool>(0)
        );

        let conflict_handoff = account_zcash_challenge(State(state.clone()), first_headers.clone())
            .await
            .unwrap();
        let conflict_attempt = conflict_handoff
            .headers()
            .get(SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_owned();
        let conflict_message = challenge_message(conflict_handoff).await;
        let conflict_signer = SigningKey::<SpendAuth>::new(OsRng);
        let conflict_key: [u8; 32] = VerificationKey::from(&conflict_signer).into();
        let conflict_input = VerifyZecAuth {
            pubkey: hex::encode(conflict_key),
            signature: hex::encode(<[u8; 64]>::from(
                conflict_signer.sign(OsRng, conflict_message.as_bytes()),
            )),
            message: conflict_message.clone(),
            granted: vec!["auth".to_owned()],
        };
        let _ = account_zecauth_link_callback(State(state.clone()), Json(conflict_input.clone()))
            .await
            .unwrap();
        let mut conflict_headers = cookie(&first_token);
        conflict_headers.insert(
            axum::http::header::COOKIE,
            HeaderValue::from_str(&format!(
                "{SESSION_COOKIE}={first_token}; {conflict_attempt}"
            ))
            .unwrap(),
        );
        for _ in 0..2 {
            let conflict =
                account_zecauth_link_complete(State(state.clone()), conflict_headers.clone())
                    .await
                    .unwrap_err();
            assert!(matches!(conflict, ApiError::Conflict));
            assert!(conflict.into_response().headers().get(SET_COOKIE).is_none());
        }
        let conflict_row = client
            .query_one(
                "SELECT consumed_at, link_attempt_hash, pending_link_key FROM zecauth_challenges WHERE message = $1",
                &[&conflict_message],
            )
            .await
            .unwrap();
        assert!(conflict_row.get::<_, Option<OffsetDateTime>>(0).is_none());
        assert!(conflict_row.get::<_, Option<Vec<u8>>>(1).is_some());
        assert_eq!(
            conflict_row.get::<_, Option<Vec<u8>>>(2),
            Some(conflict_key.to_vec())
        );
        assert_eq!(
            client
                .query_one(
                    "SELECT COUNT(*) FROM zecauth_identities WHERE verification_key = $1",
                    &[&conflict_key.as_slice()],
                )
                .await
                .unwrap()
                .get::<_, i64>(0),
            0
        );
        let _ = account_zecauth_link_callback(State(state.clone()), Json(conflict_input))
            .await
            .unwrap();

        let expired_handoff = account_zcash_challenge(State(state.clone()), first_headers.clone())
            .await
            .unwrap();
        let expired_attempt = expired_handoff
            .headers()
            .get(SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_owned();
        let expired_message = challenge_message(expired_handoff).await;
        let _ = account_zecauth_link_callback(
            State(state.clone()),
            Json(sign_red(expired_message.clone())),
        )
        .await
        .unwrap();
        client
            .execute(
                "UPDATE zecauth_challenges SET expires_at = NOW() - INTERVAL '1 second' WHERE message = $1",
                &[&expired_message],
            )
            .await
            .unwrap();
        let mut expired_headers = cookie(&first_token);
        expired_headers.insert(
            axum::http::header::COOKIE,
            HeaderValue::from_str(&format!(
                "{SESSION_COOKIE}={first_token}; {expired_attempt}"
            ))
            .unwrap(),
        );
        assert!(matches!(
            account_zecauth_link_complete(State(state.clone()), expired_headers).await,
            Err(ApiError::Unauthorized)
        ));

        let methods = list_linked_zcash_methods(State(state.clone()), first_headers.clone())
            .await
            .unwrap()
            .0;
        let serialized = serde_json::to_string(&methods).unwrap();
        assert_eq!(methods.len(), 3);
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&serialized).unwrap();
        for method in &parsed {
            let keys: BTreeSet<_> = method
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect();
            assert_eq!(keys, BTreeSet::from(["method", "chain", "created_at"]));
        }
        assert!(!serialized.contains(&hex::encode(red_key)));
        assert!(!serialized.contains(&hex::encode(key.serialize())));
        assert!(!serialized.contains("signature"));
        let handle: String = client
            .query_one(
                "SELECT public_handle FROM accounts WHERE id = $1",
                &[&first],
            )
            .await
            .unwrap()
            .get::<_, Option<String>>(0)
            .unwrap();
        assert_eq!(handle, first_handle);
        assert_eq!(
            client
                .query_one(
                    "SELECT COUNT(*) FROM accounts WHERE id IN ($1, $2)",
                    &[&first, &second],
                )
                .await
                .unwrap()
                .get::<_, i64>(0),
            accounts_before
        );

        // The original sign-in endpoints still create and resolve accounts for ordinary challenges.
        let normal_challenge = || async {
            let response =
                zecauth_challenge(State(state.clone()), Query(ChallengeQuery { scopes: None }))
                    .await
                    .unwrap();
            let body = axum::body::to_bytes(response.into_body(), 1 << 20)
                .await
                .unwrap();
            serde_json::from_slice::<serde_json::Value>(&body).unwrap()["message"]
                .as_str()
                .unwrap()
                .to_owned()
        };
        let normal_red = SigningKey::<SpendAuth>::new(OsRng);
        let normal_red_key: [u8; 32] = VerificationKey::from(&normal_red).into();
        let sign_normal_red = |message: String| VerifyZecAuth {
            pubkey: hex::encode(normal_red_key),
            signature: hex::encode(<[u8; 64]>::from(normal_red.sign(OsRng, message.as_bytes()))),
            message,
            granted: vec!["auth".to_owned()],
        };
        let normal_for_link = normal_challenge().await;
        assert!(matches!(
            account_zecauth_link_callback(
                State(state.clone()),
                Json(sign_normal_red(normal_for_link.clone()))
            )
            .await,
            Err(ApiError::Unauthorized)
        ));
        let normal_red_message = normal_challenge().await;
        zecauth_verify(
            State(state.clone()),
            Json(sign_normal_red(normal_red_message)),
        )
        .await
        .unwrap();
        let red_account: Uuid = client
            .query_one(
                "SELECT account_id FROM zecauth_identities WHERE verification_key = $1",
                &[&normal_red_key.as_slice()],
            )
            .await
            .unwrap()
            .get(0);
        assert_ne!(red_account, first);
        let normal_red_message = normal_challenge().await;
        zecauth_verify(
            State(state.clone()),
            Json(sign_normal_red(normal_red_message)),
        )
        .await
        .unwrap();
        assert_eq!(
            client
                .query_one(
                    "SELECT account_id FROM zecauth_identities WHERE verification_key = $1",
                    &[&normal_red_key.as_slice()]
                )
                .await
                .unwrap()
                .get::<_, Uuid>(0),
            red_account
        );
        let normal_wallet_message = normal_challenge().await;
        wallet_message_verify(
            State(state.clone()),
            Json(sign_wallet(normal_wallet_message)),
        )
        .await
        .unwrap();
        assert_eq!(client.query_one("SELECT account_id FROM wallet_message_identities WHERE chain = $1 AND public_key = $2", &[&state.zcash_chain, &key.serialize().as_slice()]).await.unwrap().get::<_, Uuid>(0), first);
        let other_wallet_secret = SecretKey::from_byte_array([9_u8; 32]).unwrap();
        let other_wallet_key = SecpPublicKey::from_secret_key(&secp, &other_wallet_secret);
        let normal_wallet_message = normal_challenge().await;
        let digest = zcash_signed_message_hash(&normal_wallet_message).unwrap();
        let signature =
            secp.sign_ecdsa_recoverable(SecpMessage::from_digest(digest), &other_wallet_secret);
        let (recovery, compact) = signature.serialize_compact();
        let mut bytes = vec![31 + i32::from(recovery) as u8];
        bytes.extend_from_slice(&compact);
        wallet_message_verify(
            State(state.clone()),
            Json(VerifyWalletMessage {
                pubkey: hex::encode(other_wallet_key.serialize()),
                signature: hex::encode(bytes),
                message: normal_wallet_message,
                granted: vec!["auth".to_owned()],
                signing_mode: "derived".to_owned(),
            }),
        )
        .await
        .unwrap();
        let other_wallet_account: Uuid = client.query_one("SELECT account_id FROM wallet_message_identities WHERE chain = $1 AND public_key = $2", &[&state.zcash_chain, &other_wallet_key.serialize().as_slice()]).await.unwrap().get(0);
        assert_ne!(other_wallet_account, first);

        let revoked_handoff = account_zcash_challenge(State(state.clone()), second_headers.clone())
            .await
            .unwrap();
        let revoked_attempt = revoked_handoff
            .headers()
            .get(SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_owned();
        let revoked_message = challenge_message(revoked_handoff).await;
        let _ = account_zecauth_link_callback(
            State(state.clone()),
            Json(VerifyZecAuth {
                pubkey: hex::encode(other_key),
                signature: hex::encode(<[u8; 64]>::from(
                    other_red.sign(OsRng, revoked_message.as_bytes()),
                )),
                message: revoked_message.clone(),
                granted: vec!["auth".to_owned()],
            }),
        )
        .await
        .unwrap();
        let mut revoked_headers = cookie(&second_token);
        revoked_headers.insert(
            axum::http::header::COOKIE,
            HeaderValue::from_str(&format!(
                "{SESSION_COOKIE}={second_token}; {revoked_attempt}"
            ))
            .unwrap(),
        );
        let revoked = new_challenge(second_headers.clone()).await;
        client
            .execute("DELETE FROM sessions WHERE id = $1", &[&second_session])
            .await
            .unwrap();
        assert!(matches!(
            account_zecauth_link_complete(State(state.clone()), revoked_headers).await,
            Err(ApiError::Unauthorized)
        ));
        assert!(
            client
                .query_one(
                    "SELECT consumed_at IS NULL FROM zecauth_challenges WHERE message = $1",
                    &[&revoked_message],
                )
                .await
                .unwrap()
                .get::<_, bool>(0)
        );
        assert!(matches!(
            finalize_zcash_link(
                &state,
                &second_headers,
                &revoked,
                &granted,
                LinkIdentity::ZecAuth(other_key)
            )
            .await,
            Err(ApiError::Unauthorized)
        ));
        assert!(
            client
                .query_one(
                    "SELECT consumed_at IS NULL FROM zecauth_challenges WHERE message = $1",
                    &[&revoked]
                )
                .await
                .unwrap()
                .get::<_, bool>(0)
        );
        let expired = new_challenge(first_headers.clone()).await;
        client
            .execute(
                "UPDATE sessions SET expires_at = NOW() - INTERVAL '1 second' WHERE id = $1",
                &[&first_session],
            )
            .await
            .unwrap();
        assert!(matches!(
            finalize_zcash_link(
                &state,
                &first_headers,
                &expired,
                &granted,
                LinkIdentity::ZecAuth(red_key)
            )
            .await,
            Err(ApiError::Unauthorized)
        ));
        assert!(
            client
                .query_one(
                    "SELECT consumed_at IS NULL FROM zecauth_challenges WHERE message = $1",
                    &[&expired]
                )
                .await
                .unwrap()
                .get::<_, bool>(0)
        );
        assert!(matches!(
            account_zcash_challenge(State(state.clone()), first_headers.clone()).await,
            Err(ApiError::Forbidden)
        ));
        client.execute("UPDATE sessions SET expires_at = NOW() + INTERVAL '1 day', created_at = NOW() - INTERVAL '16 minutes' WHERE id = $1", &[&first_session]).await.unwrap();
        assert!(matches!(
            finalize_zcash_link(
                &state,
                &first_headers,
                &expired,
                &granted,
                LinkIdentity::ZecAuth(red_key)
            )
            .await,
            Err(ApiError::Unauthorized)
        ));
        assert!(
            client
                .query_one(
                    "SELECT consumed_at IS NULL FROM zecauth_challenges WHERE message = $1",
                    &[&expired]
                )
                .await
                .unwrap()
                .get::<_, bool>(0)
        );
        client
            .execute(
                "UPDATE sessions SET created_at = NOW() WHERE id = $1",
                &[&first_session],
            )
            .await
            .unwrap();
        let _ = finalize_zcash_link(
            &state,
            &first_headers,
            &expired,
            &granted,
            LinkIdentity::ZecAuth(red_key),
        )
        .await
        .unwrap();
    }

    #[test]
    fn issuer_team_roles_follow_least_privilege() {
        let access = |role: &str| IssuerAccess {
            profile_id: Uuid::from_u128(1),
            owner_account_id: Uuid::from_u128(2),
            role: role.to_owned(),
            retired_at: None,
        };

        for role in ["owner", "admin"] {
            assert!(require_issuer_role(&access(role), &["owner", "admin"]).is_ok());
        }
        for role in ["issuer", "auditor"] {
            assert!(require_issuer_role(&access(role), &["owner", "admin"]).is_err());
        }
        for role in ["owner", "admin", "issuer"] {
            assert!(require_issuer_role(&access(role), &["owner", "admin", "issuer"]).is_ok());
        }
        assert!(require_issuer_role(&access("auditor"), &["owner", "admin", "issuer"]).is_err());
        assert!(require_issuer_role(&access("owner"), &["owner"]).is_ok());
        for role in ["admin", "issuer", "auditor"] {
            assert!(require_issuer_role(&access(role), &["owner"]).is_err());
        }
        assert!(require_active_issuer(&access("owner")).is_ok());
        let retired = IssuerAccess {
            retired_at: Some(OffsetDateTime::now_utc()),
            ..access("owner")
        };
        assert!(require_active_issuer(&retired).is_err());
    }

    #[test]
    fn issuer_ownership_rewrap_changes_owner_binding() {
        let old_owner = Uuid::from_u128(20);
        let new_owner = Uuid::from_u128(21);
        let object_id = Uuid::from_u128(22);
        let cipher = VaultCipher::from_local_keys(
            BTreeMap::from([(1, Aes256Gcm::new_from_slice(&[7_u8; 32]).unwrap())]),
            1,
        )
        .unwrap();

        let encrypted = cipher
            .encrypt(old_owner, object_id, b"issuer-secret")
            .unwrap();
        let row = CredentialRow {
            id: object_id,
            ciphertext: encrypted.ciphertext,
            data_nonce: encrypted.data_nonce,
            wrapped_dek: encrypted.wrapped_dek,
            wrap_nonce: encrypted.wrap_nonce,
            key_version: encrypted.key_version,
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        };

        let plaintext = cipher.decrypt(old_owner, &row).unwrap();
        let rewrapped = cipher.encrypt(new_owner, object_id, &plaintext).unwrap();
        let new_row = CredentialRow {
            id: object_id,
            ciphertext: rewrapped.ciphertext,
            data_nonce: rewrapped.data_nonce,
            wrapped_dek: rewrapped.wrapped_dek,
            wrap_nonce: rewrapped.wrap_nonce,
            key_version: rewrapped.key_version,
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        };

        assert_eq!(
            cipher.decrypt(new_owner, &new_row).unwrap(),
            b"issuer-secret"
        );
        assert!(cipher.decrypt(old_owner, &new_row).is_err());
    }

    #[test]
    fn issuer_invitation_roles_are_bounded() {
        for role in ["admin", "issuer", "auditor"] {
            assert!(valid_issuer_member_role(role));
        }
        for role in ["owner", "viewer", "", "ADMIN"] {
            assert!(!valid_issuer_member_role(role));
        }
    }

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
    fn passkey_removal_preserves_at_least_one_access_method() {
        assert!(!can_remove_passkey(1, false));
        assert!(can_remove_passkey(2, false));
        assert!(can_remove_passkey(1, true));
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
        let cipher = VaultCipher::from_local_keys(
            BTreeMap::from([(1, Aes256Gcm::new_from_slice(&[7_u8; 32]).unwrap())]),
            1,
        )
        .unwrap();
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
        let cipher = VaultCipher::from_local_keys(
            BTreeMap::from([(1, Aes256Gcm::new_from_slice(&[9_u8; 32]).unwrap())]),
            1,
        )
        .unwrap();
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
    fn verifier_api_scopes_are_strict_and_canonical() {
        assert_eq!(
            normalize_verifier_api_scopes(vec![
                "requests:read".into(),
                "proofs:read".into(),
                "requests:create".into(),
            ])
            .unwrap(),
            vec![
                "proofs:read".to_string(),
                "requests:create".to_string(),
                "requests:read".to_string(),
            ]
        );
        assert_eq!(
            normalize_verifier_api_scopes(vec![
                "requests:read".into(),
                "requests:read".into(),
                "proofs:read".into(),
            ])
            .unwrap(),
            vec!["proofs:read".to_string(), "requests:read".to_string()]
        );
        assert!(normalize_verifier_api_scopes(vec![]).is_err());
        assert!(normalize_verifier_api_scopes(vec!["admin".into()]).is_err());
        assert!(
            normalize_verifier_api_scopes(vec![
                "requests:create".into(),
                "requests:read".into(),
                "proofs:read".into(),
                "requests:create".into(),
            ])
            .is_err()
        );
    }

    #[test]
    fn verified_proof_result_exposes_only_bounded_claim_values() {
        let ordinary = Claim::Ordinary(OrdinaryClaim {
            claim_type: "membership".into(),
            value: ClaimValue::String("active".into()),
            context: Some("community".into()),
        });
        let ordinary_view = verified_proof_result(&ordinary).unwrap();
        assert_eq!(ordinary_view.claim_type, "membership");
        assert_eq!(ordinary_view.value, Value::String("active".into()));
        assert_eq!(ordinary_view.context.as_deref(), Some("community"));

        let threshold = Claim::Threshold(zerant_credential::ThresholdAttestationClaim {
            claim_type: "reputation.threshold".into(),
            value: true,
            context: "grant".into(),
            policy_id: "policy".into(),
            policy_version: "1".into(),
            policy_digest: "digest".into(),
            as_of: 1,
            threshold: 10,
            operator: ">=".into(),
        });
        let threshold_view = verified_proof_result(&threshold).unwrap();
        assert_eq!(threshold_view.value, Value::Bool(true));
        assert_eq!(threshold_view.context.as_deref(), Some("grant"));

        let too_large = Claim::Ordinary(OrdinaryClaim {
            claim_type: "count".into(),
            value: ClaimValue::Integer((MAX_SAFE_INTEGER + 1) as i64),
            context: None,
        });
        assert!(verified_proof_result(&too_large).is_err());
    }

    #[test]
    fn proof_package_migration_stores_only_bounded_verification_evidence() {
        let schema = include_str!("../migrations/0029_verifier_proof_packages.sql");
        for required in [
            "proof_issuer_id",
            "proof_issuer_key_id",
            "proof_revocation_jws",
            "status = 'approved'",
        ] {
            assert!(schema.contains(required), "{required}");
        }
        for forbidden in [
            "holder_zerant_id",
            "source_credential",
            "wallet_address",
            "balance",
            "transaction_history",
            "seed",
        ] {
            assert!(!schema.contains(forbidden), "{forbidden}");
        }
    }

    #[test]
    fn verifier_api_secrets_are_random_and_bearer_only() {
        let first = generate_verifier_api_secret();
        let second = generate_verifier_api_secret();
        assert!(first.starts_with(VERIFIER_API_KEY_PREFIX));
        assert!(second.starts_with(VERIFIER_API_KEY_PREFIX));
        assert_ne!(first, second);
        assert!((48..=128).contains(&first.len()));

        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {first}")).unwrap(),
        );
        assert_eq!(bearer_token(&headers).unwrap(), first);

        headers.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_static("Bearer session-cookie-looking-value"),
        );
        assert!(bearer_token(&headers).is_err());
        headers.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_static("Basic dXNlcjpwYXNz"),
        );
        assert!(bearer_token(&headers).is_err());
    }

    #[test]
    fn verifier_api_key_input_rejects_unknown_fields() {
        let valid = serde_json::from_str::<CreateVerifierApiKey>(
            r#"{"name":"Production","scopes":["requests:read"],"expires_in_days":90}"#,
        );
        assert!(valid.is_ok());
        let unknown = serde_json::from_str::<CreateVerifierApiKey>(
            r#"{"name":"Production","scopes":["requests:read"],"expires_in_days":90,"admin":true}"#,
        );
        assert!(unknown.is_err());
    }

    #[test]
    fn webhook_urls_are_strict_and_do_not_accept_embedded_authority() {
        let valid = parse_webhook_url("https://hooks.example.com/zerant").unwrap();
        assert_eq!(valid.scheme(), "https");
        assert_eq!(valid.host_str(), Some("hooks.example.com"));

        for invalid in [
            "http://hooks.example.com/zerant",
            "https://user:pass@hooks.example.com/zerant",
            "https://hooks.example.com:8443/zerant",
            "https://hooks.example.com/zerant?token=secret",
            "https://hooks.example.com/zerant#fragment",
            "https://localhost/zerant",
        ] {
            assert!(
                parse_webhook_url(invalid).is_err(),
                "{invalid} was accepted"
            );
        }
    }

    #[test]
    fn webhook_ip_filter_blocks_internal_and_documentation_networks() {
        for blocked in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.10.20",
            "100.64.1.1",
            "192.0.2.10",
            "198.51.100.10",
            "203.0.113.10",
            "::1",
            "fc00::1",
            "fe80::1",
            "2001:db8::1",
        ] {
            let ip = blocked.parse::<IpAddr>().unwrap();
            assert!(!webhook_ip_is_public(ip), "{blocked} was treated as public");
        }

        for public in ["1.1.1.1", "8.8.8.8", "2606:4700:4700::1111"] {
            let ip = public.parse::<IpAddr>().unwrap();
            assert!(webhook_ip_is_public(ip), "{public} was rejected");
        }
    }

    #[test]
    fn webhook_signature_binds_timestamp_and_exact_body() {
        let secret = b"zrt_whsec_test_secret_value";
        let first = webhook_signature(secret, 1_700_000_000, br#"{"status":"approved"}"#).unwrap();
        let second = webhook_signature(secret, 1_700_000_001, br#"{"status":"approved"}"#).unwrap();
        let changed = webhook_signature(secret, 1_700_000_000, br#"{"status":"denied"}"#).unwrap();

        assert!(first.starts_with("v1="));
        assert_ne!(first, second);
        assert_ne!(first, changed);
    }

    #[test]
    fn webhook_retry_backoff_is_bounded_and_non_decreasing() {
        let schedule: Vec<i64> = (1..=MAX_WEBHOOK_ATTEMPTS)
            .map(webhook_retry_seconds)
            .collect();
        assert!(schedule.windows(2).all(|pair| pair[0] <= pair[1]));
        assert_eq!(schedule[0], 60);
        assert_eq!(*schedule.last().unwrap(), 86_400);
    }

    #[test]
    fn webhook_input_rejects_unknown_fields() {
        assert!(
            serde_json::from_str::<CreateVerifierWebhook>(
                r#"{"name":"Production","url":"https://hooks.example.com/zerant"}"#,
            )
            .is_ok()
        );
        assert!(
            serde_json::from_str::<CreateVerifierWebhook>(
                r#"{"name":"Production","url":"https://hooks.example.com/zerant","secret":"caller-controlled"}"#,
            )
            .is_err()
        );
    }

    #[test]
    fn public_issuer_cursor_roundtrips() {
        let created_at = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let id = Uuid::from_u128(42);
        let cursor = public_issuer_cursor(created_at, id);
        assert_eq!(
            parse_public_issuer_cursor(&cursor).unwrap(),
            (created_at, id)
        );
        assert!(parse_public_issuer_cursor(&"x".repeat(257)).is_err());
        assert!(parse_public_issuer_cursor("not-base64***").is_err());
    }

    #[test]
    fn public_issuer_ids_are_strictly_bounded() {
        assert!(validate_public_issuer_id("zerant:issuer:abc123").is_ok());
        assert!(validate_public_issuer_id("issuer:abc123").is_err());
        assert!(validate_public_issuer_id("zerant:issuer:bad\nvalue").is_err());
        assert!(validate_public_issuer_id(&format!("zerant:issuer:{}", "x".repeat(120))).is_err());
    }

    #[test]
    fn public_cache_headers_are_explicit() {
        let headers = public_cache_headers(120).unwrap();
        assert_eq!(
            headers.get(axum::http::header::CACHE_CONTROL).unwrap(),
            "public, max-age=120, stale-while-revalidate=60"
        );
    }

    #[test]
    fn public_issuer_metadata_shape_excludes_private_material() {
        let metadata = PublicIssuerMetadata {
            issuer_id: "zerant:issuer:test".into(),
            display_name: "Test Issuer".into(),
            keys: vec![PublicIssuerKeyView {
                key_id: "key-test".into(),
                public_jwk: PublicJwk {
                    kty: "OKP".into(),
                    crv: "Ed25519".into(),
                    x: "11qYAYKxCrfVS_7TyWQHOg7hcvPapiMlrwIaaPcHURo".into(),
                },
                valid_from: OffsetDateTime::UNIX_EPOCH,
                retired_at: None,
                compromised_at: None,
            }],
            credential_schemas: vec![PublicCredentialSchemaView {
                id: Uuid::from_u128(9),
                display_name: "Membership".into(),
                description: "Confirms active membership.".into(),
                claim_type: "membership".into(),
                context: "community".into(),
                default_expiry_days: 90,
                version: 1,
                active: true,
                supersedes_schema_id: None,
                retired_at: None,
                created_at: OffsetDateTime::UNIX_EPOCH,
            }],
            revocation_version: 1,
        };

        let json = serde_json::to_string(&metadata).unwrap();
        for forbidden in [
            "account_id",
            "subject_account",
            "holder_zerant_id",
            "ciphertext",
            "wrapped_dek",
            "verification_key",
            "wallet",
            "signed_credential",
        ] {
            assert!(
                !json.contains(forbidden),
                "public metadata leaked {forbidden}"
            );
        }
        assert!(json.contains("public_jwk"));
        assert!(json.contains("credential_schemas"));
    }

    #[test]
    fn envelope_key_rotation_keeps_old_records_readable() {
        let account = Uuid::from_u128(10);
        let old_id = Uuid::from_u128(11);
        let old_cipher = VaultCipher::from_local_keys(
            BTreeMap::from([(1, Aes256Gcm::new_from_slice(&[1_u8; 32]).unwrap())]),
            1,
        )
        .unwrap();
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

        let rotated = VaultCipher::from_local_keys(
            BTreeMap::from([
                (1, Aes256Gcm::new_from_slice(&[1_u8; 32]).unwrap()),
                (2, Aes256Gcm::new_from_slice(&[2_u8; 32]).unwrap()),
            ]),
            2,
        )
        .unwrap();

        assert_eq!(rotated.decrypt(account, &row).unwrap(), b"old-record");

        let new_record = rotated
            .encrypt(account, Uuid::from_u128(12), b"new-record")
            .unwrap();
        assert_eq!(new_record.key_version, 2);

        let missing_old_key = VaultCipher::from_local_keys(
            BTreeMap::from([(2, Aes256Gcm::new_from_slice(&[2_u8; 32]).unwrap())]),
            2,
        )
        .unwrap();
        assert!(missing_old_key.decrypt(account, &row).is_err());
    }
}
