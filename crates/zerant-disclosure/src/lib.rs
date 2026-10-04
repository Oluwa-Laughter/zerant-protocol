//! Signed, single-result disclosure for Zerant.
//!
//! This crate authenticates protocol messages and enforces request/response bindings.
//! It does not authenticate a browser transport, store holder secrets, or implement ZK.
#![forbid(unsafe_code)]

pub mod compound;

use josekit::{
    jwk::Jwk,
    jws::{self, EdDSA, JwsHeader},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::Mutex, time::Duration};
use zerant_core::{
    Error, MAX_JSON_BYTES, MAX_SAFE_INTEGER, Result, canonicalize, decode_base64url,
    encode_base64url, parse_canonical, validate_challenge, validate_id, validate_interval,
    validate_origin, validate_timestamp,
};
use zerant_credential::{
    Claim, CredentialKind, CredentialPayload, IssuerTrustManifest, PublicJwk, verify_credential,
};

pub const REQUEST_SCHEMA: &str = "zerant.disclosure.request.v0.2";
pub const RESPONSE_SCHEMA: &str = "zerant.disclosure.response.v0.2";
pub const REQUEST_TYP: &str = "zerant-request-v0.2";
pub const RESPONSE_TYP: &str = "zerant-response-v0.2";
pub const MAX_REQUEST_LIFETIME: u64 = 300;
const MAX_PURPOSE_LEN: usize = 1024;
const MAX_ACCEPTED_ISSUERS: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Predicate {
    pub context: String,
    pub policy_id: String,
    pub policy_version: String,
    pub policy_digest: String,
    pub threshold: u64,
    pub operator: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub schema: String,
    pub request_id: String,
    pub verifier_id: String,
    pub verifier_key_id: String,
    pub verifier_origin: String,
    pub purpose: String,
    pub accepted_issuer_ids: Vec<String>,
    pub claim_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub predicate: Option<Predicate>,
    pub challenge: String,
    pub nonce: String,
    pub issued_at: u64,
    pub expires_at: u64,
}

impl Request {
    pub fn validate(
        &self,
        now: u64,
        authenticated_origin: &str,
        allowed_loopback: &[String],
    ) -> Result<()> {
        validate_origin(authenticated_origin, allowed_loopback)?;
        validate_origin(&self.verifier_origin, allowed_loopback)?;
        validate_interval(self.issued_at, self.expires_at, now)?;
        validate_id(&self.request_id)?;
        validate_challenge(&self.challenge)?;
        validate_challenge(&self.nonce)?;

        if self.schema != REQUEST_SCHEMA
            || self.verifier_origin != authenticated_origin
            || self.expires_at - self.issued_at > MAX_REQUEST_LIFETIME
            || self.verifier_id.is_empty()
            || self.verifier_key_id.is_empty()
            || self.purpose.trim().is_empty()
            || self.purpose.len() > MAX_PURPOSE_LEN
            || self.claim_type.is_empty()
            || self.accepted_issuer_ids.is_empty()
            || self.accepted_issuer_ids.len() > MAX_ACCEPTED_ISSUERS
            || self
                .accepted_issuer_ids
                .iter()
                .any(|issuer| issuer.is_empty() || issuer.len() > 256)
            || self
                .accepted_issuer_ids
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.context.as_deref().is_some_and(str::is_empty)
        {
            return Err(Error::Credential);
        }

        match (&self.predicate, self.claim_type.as_str()) {
            (Some(predicate), "reputation.threshold") => {
                if self.context.is_some()
                    || predicate.context.is_empty()
                    || predicate.policy_id.is_empty()
                    || predicate.policy_version.is_empty()
                    || predicate.operator != "gte"
                    || predicate.threshold > MAX_SAFE_INTEGER
                {
                    return Err(Error::Credential);
                }
                zerant_core::decode_fixed::<32>(&predicate.policy_digest)
                    .map_err(|_| Error::Credential)?;
            }
            (None, claim_type) if claim_type != "reputation.threshold" => {}
            _ => return Err(Error::Credential),
        }

        Ok(())
    }

    pub fn matches_attestation(&self, payload: &CredentialPayload) -> Result<()> {
        if payload.kind != CredentialKind::Attestation
            || payload.audience != self.verifier_origin
            || !self.accepted_issuer_ids.contains(&payload.issuer_id)
        {
            return Err(Error::Audience);
        }

        let matches = match (&payload.claim, &self.predicate) {
            (Claim::Threshold(claim), Some(predicate)) => {
                self.claim_type == claim.claim_type
                    && claim.context == predicate.context
                    && claim.policy_id == predicate.policy_id
                    && claim.policy_version == predicate.policy_version
                    && claim.policy_digest == predicate.policy_digest
                    && claim.threshold == predicate.threshold
                    && claim.operator == predicate.operator
            }
            (Claim::Ordinary(claim), None) => {
                self.claim_type == claim.claim_type && self.context == claim.context
            }
            _ => false,
        };

        if matches {
            Ok(())
        } else {
            Err(Error::Credential)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Response {
    pub schema: String,
    pub request_id: String,
    pub verifier_origin: String,
    pub challenge: String,
    pub nonce: String,
    pub request_digest: String,
    pub attestation_jws: String,
    pub responded_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifierPin {
    pub verifier_id: String,
    pub key_id: String,
    pub key: PublicJwk,
    pub allowed_origins: Vec<String>,
    pub valid_from: u64,
    pub valid_until: u64,
    pub compromised: bool,
}

impl VerifierPin {
    fn validate(
        &self,
        now: u64,
        authenticated_origin: &str,
        allowed_loopback: &[String],
    ) -> Result<()> {
        self.key.validate()?;
        validate_interval(self.valid_from, self.valid_until, now)?;
        if self.compromised
            || self.verifier_id.is_empty()
            || self.key_id.is_empty()
            || self.allowed_origins.is_empty()
            || !self
                .allowed_origins
                .iter()
                .any(|origin| origin == authenticated_origin)
        {
            return Err(Error::Trust);
        }
        for origin in &self.allowed_origins {
            validate_origin(origin, allowed_loopback)?;
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProtectedHeader {
    alg: String,
    kid: String,
    typ: String,
}

fn compact_parts(token: &str) -> Result<[&str; 3]> {
    if token.len() > MAX_JSON_BYTES * 2 {
        return Err(Error::Size);
    }
    let mut parts = token.split('.');
    let header = parts.next().ok_or(Error::Signature)?;
    let payload = parts.next().ok_or(Error::Signature)?;
    let signature = parts.next().ok_or(Error::Signature)?;
    if parts.next().is_some() || header.is_empty() || payload.is_empty() || signature.is_empty() {
        return Err(Error::Signature);
    }
    Ok([header, payload, signature])
}

fn inspect_header(token: &str, expected_typ: &str, expected_kid: &str) -> Result<()> {
    let [header, _, _] = compact_parts(token)?;
    let bytes = decode_base64url(header).map_err(|_| Error::Signature)?;
    let parsed: ProtectedHeader = serde_json::from_slice(&bytes).map_err(|_| Error::Signature)?;
    if parsed.alg != "EdDSA" || parsed.kid != expected_kid || parsed.typ != expected_typ {
        return Err(Error::Signature);
    }
    Ok(())
}

fn public_jwk_to_josekit(key: &PublicJwk) -> Result<Jwk> {
    key.validate()?;
    Jwk::from_bytes(serde_json::to_vec(key).map_err(|_| Error::Key)?).map_err(|_| Error::Key)
}

fn verify_jws<T: serde::de::DeserializeOwned>(
    token: &str,
    typ: &str,
    kid: &str,
    key: &PublicJwk,
) -> Result<T> {
    inspect_header(token, typ, kid)?;
    let jwk = public_jwk_to_josekit(key)?;
    let mut verifier = EdDSA.verifier_from_jwk(&jwk).map_err(|_| Error::Key)?;
    verifier.set_key_id(kid);
    let (payload, returned_header) =
        jws::deserialize_compact(token, &verifier).map_err(|_| Error::Signature)?;
    if returned_header.claims_set().len() != 3
        || returned_header.algorithm() != Some("EdDSA")
        || returned_header.key_id() != Some(kid)
        || returned_header.token_type() != Some(typ)
    {
        return Err(Error::Signature);
    }
    parse_canonical(&payload)
}

fn sign_jws<T: Serialize>(payload: &T, typ: &str, kid: &str, key: &Jwk) -> Result<String> {
    let mut signer = EdDSA.signer_from_jwk(key).map_err(|_| Error::Key)?;
    signer.set_key_id(kid);
    let mut header = JwsHeader::new();
    header.set_token_type(typ);
    jws::serialize_compact(&canonicalize(payload)?, &header, &signer).map_err(|_| Error::Signature)
}

fn untrusted_response(token: &str) -> Result<Response> {
    let [_, payload, _] = compact_parts(token)?;
    parse_canonical(&decode_base64url(payload).map_err(|_| Error::Signature)?)
}

pub fn request_digest(token: &str) -> String {
    encode_base64url(&Sha256::digest(token.as_bytes()))
}

pub fn sign_request(request: &Request, key: &Jwk, allowed_loopback: &[String]) -> Result<String> {
    request.validate(
        request.issued_at,
        &request.verifier_origin,
        allowed_loopback,
    )?;
    sign_jws(request, REQUEST_TYP, &request.verifier_key_id, key)
}

pub fn verify_request(
    token: &str,
    pin: &VerifierPin,
    authenticated_origin: &str,
    now: u64,
    allowed_loopback: &[String],
) -> Result<Request> {
    pin.validate(now, authenticated_origin, allowed_loopback)?;
    let request: Request = verify_jws(token, REQUEST_TYP, &pin.key_id, &pin.key)?;
    request.validate(now, authenticated_origin, allowed_loopback)?;
    if request.verifier_id != pin.verifier_id
        || request.verifier_key_id != pin.key_id
        || request.issued_at < pin.valid_from
        || request.issued_at >= pin.valid_until
    {
        return Err(Error::Trust);
    }
    Ok(request)
}

pub struct Evidence<'a> {
    pub attestation_jws: &'a str,
    pub revocation_jws: &'a str,
    pub issuer_id: &'a str,
    pub minimum_revocation_version: u64,
}

pub struct Context<'a> {
    pub pin: &'a VerifierPin,
    pub origin: &'a str,
    pub trust: &'a IssuerTrustManifest,
    pub now: u64,
    pub allowed_loopback: &'a [String],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Approve,
    Deny,
}

/// Denial produces no credential response. Missing evidence is unavailable, never false.
pub fn respond(
    request_jws: &str,
    context: &Context<'_>,
    decision: Decision,
    evidence: Option<&Evidence<'_>>,
    holder_private_key: &Jwk,
) -> Result<Option<String>> {
    let request = verify_request(
        request_jws,
        context.pin,
        context.origin,
        context.now,
        context.allowed_loopback,
    )?;

    if decision == Decision::Deny {
        return Ok(None);
    }

    let evidence = evidence.ok_or(Error::Credential)?;
    let attestation = verify_credential(
        evidence.attestation_jws,
        evidence.revocation_jws,
        context.trust,
        evidence.issuer_id,
        context.origin,
        context.allowed_loopback,
        context.now,
        evidence.minimum_revocation_version,
    )?
    .payload;
    request.matches_attestation(&attestation)?;

    let response = Response {
        schema: RESPONSE_SCHEMA.into(),
        request_id: request.request_id,
        verifier_origin: request.verifier_origin,
        challenge: request.challenge,
        nonce: request.nonce,
        request_digest: request_digest(request_jws),
        attestation_jws: evidence.attestation_jws.into(),
        responded_at: context.now,
    };

    let signed = sign_jws(
        &response,
        RESPONSE_TYP,
        &attestation.subject_key.x,
        holder_private_key,
    )?;
    // Fail closed if the caller supplied the wrong holder private key.
    let _: Response = verify_jws(
        &signed,
        RESPONSE_TYP,
        &attestation.subject_key.x,
        &attestation.subject_key,
    )?;
    Ok(Some(signed))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayStatus {
    Pending,
    Consumed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayEntry {
    pub request_id: String,
    pub digest: String,
    pub origin: String,
    pub expires_at: u64,
    pub status: ReplayStatus,
}

fn validate_replay_entry(entry: &ReplayEntry) -> Result<()> {
    validate_id(&entry.request_id)?;
    validate_challenge(&entry.digest)?;
    validate_timestamp(entry.expires_at)?;
    // Syntax only: verifier pin and transport authorize origins before registration.
    validate_origin(&entry.origin, std::slice::from_ref(&entry.origin))?;
    if entry.status != ReplayStatus::Pending {
        return Err(Error::Trust);
    }
    Ok(())
}

pub trait ReplayStore: Send + Sync {
    fn register(&self, entry: ReplayEntry) -> Result<()>;
    fn get(&self, request_id: &str) -> Result<Option<ReplayEntry>>;
    fn consume_if_pending(
        &self,
        request_id: &str,
        digest: &str,
        origin: &str,
        now: u64,
    ) -> Result<()>;
}

#[derive(Default)]
pub struct MemoryReplay(Mutex<BTreeMap<String, ReplayEntry>>);

impl ReplayStore for MemoryReplay {
    fn register(&self, entry: ReplayEntry) -> Result<()> {
        validate_replay_entry(&entry)?;
        let mut rows = self.0.lock().map_err(|_| Error::Trust)?;
        if rows.contains_key(&entry.request_id) {
            return Err(Error::Trust);
        }
        rows.insert(entry.request_id.clone(), entry);
        Ok(())
    }

    fn get(&self, request_id: &str) -> Result<Option<ReplayEntry>> {
        Ok(self
            .0
            .lock()
            .map_err(|_| Error::Trust)?
            .get(request_id)
            .cloned())
    }

    fn consume_if_pending(
        &self,
        request_id: &str,
        digest: &str,
        origin: &str,
        now: u64,
    ) -> Result<()> {
        validate_timestamp(now)?;
        let mut rows = self.0.lock().map_err(|_| Error::Trust)?;
        let row = rows.get_mut(request_id).ok_or(Error::Trust)?;
        if row.digest != digest
            || row.origin != origin
            || row.expires_at <= now
            || row.status != ReplayStatus::Pending
        {
            return Err(Error::Trust);
        }
        row.status = ReplayStatus::Consumed;
        Ok(())
    }
}

pub struct SqliteReplay(Mutex<rusqlite::Connection>);

fn sqlite_time(value: u64) -> Result<i64> {
    validate_timestamp(value)?;
    i64::try_from(value).map_err(|_| Error::Time)
}

impl SqliteReplay {
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let connection = rusqlite::Connection::open(path).map_err(|_| Error::Trust)?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(|_| Error::Trust)?;
        connection
            .execute_batch(
                "PRAGMA journal_mode=WAL;
                 PRAGMA synchronous=FULL;
                 CREATE TABLE IF NOT EXISTS requests(
                   id TEXT PRIMARY KEY,
                   digest TEXT NOT NULL,
                   origin TEXT NOT NULL,
                   expires INTEGER NOT NULL,
                   status INTEGER NOT NULL DEFAULT 0 CHECK(status IN (0,1))
                 );",
            )
            .map_err(|_| Error::Trust)?;
        Ok(Self(Mutex::new(connection)))
    }

    pub fn purge_expired(&self, now: u64) -> Result<()> {
        let now = sqlite_time(now)?;
        self.0
            .lock()
            .map_err(|_| Error::Trust)?
            .execute("DELETE FROM requests WHERE expires<=?1", [now])
            .map_err(|_| Error::Trust)?;
        Ok(())
    }
}

impl ReplayStore for SqliteReplay {
    fn register(&self, entry: ReplayEntry) -> Result<()> {
        validate_replay_entry(&entry)?;
        let expires = sqlite_time(entry.expires_at)?;
        self.0
            .lock()
            .map_err(|_| Error::Trust)?
            .execute(
                "INSERT INTO requests(id,digest,origin,expires,status) VALUES(?1,?2,?3,?4,0)",
                rusqlite::params![entry.request_id, entry.digest, entry.origin, expires],
            )
            .map_err(|_| Error::Trust)?;
        Ok(())
    }

    fn get(&self, request_id: &str) -> Result<Option<ReplayEntry>> {
        let connection = self.0.lock().map_err(|_| Error::Trust)?;
        let mut statement = connection
            .prepare("SELECT id,digest,origin,expires,status FROM requests WHERE id=?1")
            .map_err(|_| Error::Trust)?;
        let mut rows = statement.query([request_id]).map_err(|_| Error::Trust)?;
        let Some(row) = rows.next().map_err(|_| Error::Trust)? else {
            return Ok(None);
        };
        let expires_i64: i64 = row.get(3).map_err(|_| Error::Trust)?;
        let expires_at = u64::try_from(expires_i64).map_err(|_| Error::Trust)?;
        let status_i64: i64 = row.get(4).map_err(|_| Error::Trust)?;
        let status = match status_i64 {
            0 => ReplayStatus::Pending,
            1 => ReplayStatus::Consumed,
            _ => return Err(Error::Trust),
        };
        Ok(Some(ReplayEntry {
            request_id: row.get(0).map_err(|_| Error::Trust)?,
            digest: row.get(1).map_err(|_| Error::Trust)?,
            origin: row.get(2).map_err(|_| Error::Trust)?,
            expires_at,
            status,
        }))
    }

    fn consume_if_pending(
        &self,
        request_id: &str,
        digest: &str,
        origin: &str,
        now: u64,
    ) -> Result<()> {
        let now = sqlite_time(now)?;
        let affected = self
            .0
            .lock()
            .map_err(|_| Error::Trust)?
            .execute(
                "UPDATE requests
                 SET status=1
                 WHERE id=?1 AND digest=?2 AND origin=?3 AND status=0 AND expires>?4",
                rusqlite::params![request_id, digest, origin, now],
            )
            .map_err(|_| Error::Trust)?;
        if affected != 1 {
            return Err(Error::Trust);
        }
        Ok(())
    }
}

pub fn register_request(
    request_jws: &str,
    context: &Context<'_>,
    store: &dyn ReplayStore,
) -> Result<()> {
    let request = verify_request(
        request_jws,
        context.pin,
        context.origin,
        context.now,
        context.allowed_loopback,
    )?;
    store.register(ReplayEntry {
        request_id: request.request_id,
        digest: request_digest(request_jws),
        origin: request.verifier_origin,
        expires_at: request.expires_at,
        status: ReplayStatus::Pending,
    })
}

pub fn accept(
    request_jws: &str,
    response_jws: &str,
    context: &Context<'_>,
    evidence: &Evidence<'_>,
    store: &dyn ReplayStore,
) -> Result<CredentialPayload> {
    let request = verify_request(
        request_jws,
        context.pin,
        context.origin,
        context.now,
        context.allowed_loopback,
    )?;

    let persisted = store.get(&request.request_id)?.ok_or(Error::Trust)?;
    let expected_digest = request_digest(request_jws);
    if persisted.digest != expected_digest
        || persisted.origin != request.verifier_origin
        || persisted.expires_at != request.expires_at
        || persisted.status != ReplayStatus::Pending
    {
        return Err(Error::Trust);
    }

    // Read only enough untrusted response data to locate the signed attestation;
    // no acceptance decision is made until every signature and binding is verified.
    let untrusted = untrusted_response(response_jws)?;
    if untrusted.attestation_jws != evidence.attestation_jws {
        return Err(Error::Credential);
    }

    let attestation = verify_credential(
        &untrusted.attestation_jws,
        evidence.revocation_jws,
        context.trust,
        evidence.issuer_id,
        context.origin,
        context.allowed_loopback,
        context.now,
        evidence.minimum_revocation_version,
    )?
    .payload;
    request.matches_attestation(&attestation)?;

    let response: Response = verify_jws(
        response_jws,
        RESPONSE_TYP,
        &attestation.subject_key.x,
        &attestation.subject_key,
    )?;

    if response.schema != RESPONSE_SCHEMA
        || response.request_id != request.request_id
        || response.verifier_origin != request.verifier_origin
        || response.challenge != request.challenge
        || response.nonce != request.nonce
        || response.request_digest != expected_digest
        || response.attestation_jws != evidence.attestation_jws
        || response.responded_at < request.issued_at
        || response.responded_at > context.now
    {
        return Err(Error::Credential);
    }

    attestation.validate(
        response.responded_at,
        context.origin,
        context.allowed_loopback,
    )?;
    store.consume_if_pending(
        &request.request_id,
        &expected_digest,
        &request.verifier_origin,
        context.now,
    )?;
    Ok(attestation)
}

/// Optional organizational custody boundary. Implementations must produce a standard
/// profile-compatible compact JWS. No FROST algorithm is implemented by Zerant.
pub trait OrganizationalSigner {
    fn sign_approved(&self, canonical_payload: &[u8], typ: &str, key_id: &str) -> Result<String>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use josekit::jwk::{Jwk, alg::ed::EdCurve};
    use std::sync::Arc;
    use zerant_credential::{
        ClaimValue, OrdinaryClaim, PolicyAuthorization, RevocationSnapshot, TrustedIssuer,
        TrustedIssuerKey, sign_credential, sign_revocation_snapshot,
    };

    const NOW: u64 = 1_800_000_000;
    const ORIGIN: &str = "https://app.example";
    const ISSUER: &str = "zerant:issuer:test";
    const ISSUER_KID: &str = "issuer-1";
    const VERIFIER: &str = "zerant:verifier:test";
    const VERIFIER_KID: &str = "verifier-1";

    struct Fixture {
        issuer_private: Jwk,
        verifier_private: Jwk,
        holder_private: Jwk,
        holder_public: PublicJwk,
        trust: IssuerTrustManifest,
        pin: VerifierPin,
        revocation_jws: String,
    }

    fn public_from(private: &Jwk) -> PublicJwk {
        let public = private.to_public_key().unwrap();
        let value = serde_json::to_value(public).unwrap();
        PublicJwk {
            kty: value["kty"].as_str().unwrap().into(),
            crv: value["crv"].as_str().unwrap().into(),
            x: value["x"].as_str().unwrap().into(),
        }
    }

    fn fixture() -> Fixture {
        let mut issuer_private = Jwk::generate_ed_key(EdCurve::Ed25519).unwrap();
        issuer_private.set_key_id(ISSUER_KID);
        let mut verifier_private = Jwk::generate_ed_key(EdCurve::Ed25519).unwrap();
        verifier_private.set_key_id(VERIFIER_KID);
        let holder_private = Jwk::generate_ed_key(EdCurve::Ed25519).unwrap();
        let holder_public = public_from(&holder_private);

        let policy_digest = encode_base64url(&[9; 32]);
        let trust = IssuerTrustManifest {
            issuers: vec![TrustedIssuer {
                issuer_id: ISSUER.into(),
                keys: vec![TrustedIssuerKey {
                    issuer_key_id: ISSUER_KID.into(),
                    public_key: public_from(&issuer_private),
                    valid_from: NOW - 10_000,
                    valid_until: NOW + 10_000,
                    compromised: false,
                }],
                allowed_claim_types: vec!["membership".into(), "reputation.threshold".into()],
                allowed_contexts: vec!["community".into()],
                source_schemas: vec![],
                policies: vec![PolicyAuthorization {
                    policy_id: "zerant:community:policy".into(),
                    version: "0.1".into(),
                    context: "community".into(),
                    digest: policy_digest,
                    supported_thresholds: vec![10],
                }],
            }],
        };

        let revocation = RevocationSnapshot {
            schema: "zerant.revocation.v0.1".into(),
            issuer_id: ISSUER.into(),
            issuer_key_id: ISSUER_KID.into(),
            version: 1,
            issued_at: NOW - 100,
            next_update: NOW + 1_000,
            revoked_digests: vec![],
        };
        let revocation_jws = sign_revocation_snapshot(&revocation, &issuer_private).unwrap();

        Fixture {
            issuer_private,
            verifier_private: verifier_private.clone(),
            holder_private,
            holder_public,
            trust,
            pin: VerifierPin {
                verifier_id: VERIFIER.into(),
                key_id: VERIFIER_KID.into(),
                key: public_from(&verifier_private),
                allowed_origins: vec![ORIGIN.into()],
                valid_from: NOW - 10_000,
                valid_until: NOW + 10_000,
                compromised: false,
            },
            revocation_jws,
        }
    }

    fn ordinary_attestation(fixture: &Fixture, claim_type: &str, context: &str) -> String {
        let payload = CredentialPayload {
            schema: "zerant.credential.v0.1".into(),
            kind: CredentialKind::Attestation,
            credential_id: encode_base64url(&[1; 16]),
            issuer_id: ISSUER.into(),
            issuer_key_id: ISSUER_KID.into(),
            subject_key: fixture.holder_public.clone(),
            audience: ORIGIN.into(),
            issued_at: NOW - 100,
            expires_at: NOW + 1_000,
            revocation_handle: encode_base64url(&[2; 16]),
            claim: Claim::Ordinary(OrdinaryClaim {
                claim_type: claim_type.into(),
                value: ClaimValue::String("active".into()),
                context: Some(context.into()),
            }),
        };
        sign_credential(&payload, &fixture.issuer_private).unwrap()
    }

    fn ordinary_attestation_with_id(
        fixture: &Fixture,
        claim_type: &str,
        context: &str,
        credential_id: String,
        revocation_handle: String,
    ) -> String {
        let payload = CredentialPayload {
            schema: "zerant.credential.v0.1".into(),
            kind: CredentialKind::Attestation,
            credential_id,
            issuer_id: ISSUER.into(),
            issuer_key_id: ISSUER_KID.into(),
            subject_key: fixture.holder_public.clone(),
            audience: ORIGIN.into(),
            issued_at: NOW - 100,
            expires_at: NOW + 1_000,
            revocation_handle,
            claim: Claim::Ordinary(OrdinaryClaim {
                claim_type: claim_type.into(),
                value: ClaimValue::String("active".into()),
                context: Some(context.into()),
            }),
        };
        sign_credential(&payload, &fixture.issuer_private).unwrap()
    }

    fn boolean_attestation_with_id(
        fixture: &Fixture,
        claim_type: &str,
        context: &str,
        credential_id: String,
        revocation_handle: String,
    ) -> String {
        let payload = CredentialPayload {
            schema: "zerant.credential.v0.1".into(),
            kind: CredentialKind::Attestation,
            credential_id,
            issuer_id: ISSUER.into(),
            issuer_key_id: ISSUER_KID.into(),
            subject_key: fixture.holder_public.clone(),
            audience: ORIGIN.into(),
            issued_at: NOW - 100,
            expires_at: NOW + 1_000,
            revocation_handle,
            claim: Claim::Ordinary(OrdinaryClaim {
                claim_type: claim_type.into(),
                value: ClaimValue::Boolean(true),
                context: Some(context.into()),
            }),
        };
        sign_credential(&payload, &fixture.issuer_private).unwrap()
    }

    fn threshold_attestation(fixture: &Fixture, digest_value: &str, threshold: u64) -> String {
        let payload = CredentialPayload {
            schema: "zerant.credential.v0.1".into(),
            kind: CredentialKind::Attestation,
            credential_id: encode_base64url(&[3; 16]),
            issuer_id: ISSUER.into(),
            issuer_key_id: ISSUER_KID.into(),
            subject_key: fixture.holder_public.clone(),
            audience: ORIGIN.into(),
            issued_at: NOW - 100,
            expires_at: NOW + 1_000,
            revocation_handle: encode_base64url(&[4; 16]),
            claim: Claim::Threshold(zerant_credential::ThresholdAttestationClaim {
                claim_type: "reputation.threshold".into(),
                value: true,
                context: "community".into(),
                policy_id: "zerant:community:policy".into(),
                policy_version: "0.1".into(),
                policy_digest: digest_value.into(),
                as_of: NOW - 100,
                threshold,
                operator: "gte".into(),
            }),
        };
        sign_credential(&payload, &fixture.issuer_private).unwrap()
    }

    fn ordinary_request() -> Request {
        Request {
            schema: REQUEST_SCHEMA.into(),
            request_id: encode_base64url(&[10; 16]),
            verifier_id: VERIFIER.into(),
            verifier_key_id: VERIFIER_KID.into(),
            verifier_origin: ORIGIN.into(),
            purpose: "Verify active community membership".into(),
            accepted_issuer_ids: vec![ISSUER.into()],
            claim_type: "membership".into(),
            context: Some("community".into()),
            predicate: None,
            challenge: encode_base64url(&[11; 32]),
            nonce: encode_base64url(&[12; 32]),
            issued_at: NOW,
            expires_at: NOW + 300,
        }
    }

    fn threshold_request(digest_value: String, threshold: u64) -> Request {
        Request {
            schema: REQUEST_SCHEMA.into(),
            request_id: encode_base64url(&[13; 16]),
            verifier_id: VERIFIER.into(),
            verifier_key_id: VERIFIER_KID.into(),
            verifier_origin: ORIGIN.into(),
            purpose: "Check one contextual threshold".into(),
            accepted_issuer_ids: vec![ISSUER.into()],
            claim_type: "reputation.threshold".into(),
            context: None,
            predicate: Some(Predicate {
                context: "community".into(),
                policy_id: "zerant:community:policy".into(),
                policy_version: "0.1".into(),
                policy_digest: digest_value,
                threshold,
                operator: "gte".into(),
            }),
            challenge: encode_base64url(&[14; 32]),
            nonce: encode_base64url(&[15; 32]),
            issued_at: NOW,
            expires_at: NOW + 300,
        }
    }

    fn context<'a>(fixture: &'a Fixture, now: u64, origin: &'a str) -> Context<'a> {
        Context {
            pin: &fixture.pin,
            origin,
            trust: &fixture.trust,
            now,
            allowed_loopback: &[],
        }
    }

    fn evidence<'a>(attestation: &'a str, fixture: &'a Fixture) -> Evidence<'a> {
        Evidence {
            attestation_jws: attestation,
            revocation_jws: &fixture.revocation_jws,
            issuer_id: ISSUER,
            minimum_revocation_version: 1,
        }
    }

    #[test]
    fn ordinary_request_response_accepts_once_and_denial_is_silent() {
        let fixture = fixture();
        let request = ordinary_request();
        let request_jws = sign_request(&request, &fixture.verifier_private, &[]).unwrap();
        let attestation = ordinary_attestation(&fixture, "membership", "community");
        let valid_evidence = evidence(&attestation, &fixture);
        let ctx = context(&fixture, NOW + 10, ORIGIN);

        assert_eq!(
            respond(
                &request_jws,
                &ctx,
                Decision::Deny,
                Some(&valid_evidence),
                &fixture.holder_private
            )
            .unwrap(),
            None
        );

        let response = respond(
            &request_jws,
            &ctx,
            Decision::Approve,
            Some(&valid_evidence),
            &fixture.holder_private,
        )
        .unwrap()
        .unwrap();

        let store = MemoryReplay::default();
        register_request(&request_jws, &ctx, &store).unwrap();
        assert!(accept(&request_jws, &response, &ctx, &valid_evidence, &store).is_ok());
        assert!(accept(&request_jws, &response, &ctx, &valid_evidence, &store).is_err());

        let serialized =
            String::from_utf8(canonicalize(&untrusted_response(&response).unwrap()).unwrap())
                .unwrap();
        for forbidden in [
            "score",
            "source_event",
            "wallet",
            "address",
            "balance",
            "history",
        ] {
            assert!(!serialized.contains(forbidden));
        }
    }

    #[test]
    fn threshold_request_binds_policy_digest_and_threshold() {
        let fixture = fixture();
        let digest_value = encode_base64url(&[9; 32]);
        let request = threshold_request(digest_value.clone(), 10);
        let request_jws = sign_request(&request, &fixture.verifier_private, &[]).unwrap();
        let attestation = threshold_attestation(&fixture, &digest_value, 10);
        let valid_evidence = evidence(&attestation, &fixture);
        let ctx = context(&fixture, NOW + 10, ORIGIN);
        let response = respond(
            &request_jws,
            &ctx,
            Decision::Approve,
            Some(&valid_evidence),
            &fixture.holder_private,
        )
        .unwrap()
        .unwrap();
        let store = MemoryReplay::default();
        register_request(&request_jws, &ctx, &store).unwrap();
        assert!(accept(&request_jws, &response, &ctx, &valid_evidence, &store).is_ok());

        let wrong_digest_attestation =
            threshold_attestation(&fixture, &encode_base64url(&[8; 32]), 10);
        let wrong_digest = self::evidence(&wrong_digest_attestation, &fixture);
        assert!(
            respond(
                &request_jws,
                &ctx,
                Decision::Approve,
                Some(&wrong_digest),
                &fixture.holder_private
            )
            .is_err()
        );

        let wrong_threshold_attestation = threshold_attestation(&fixture, &digest_value, 9);
        let wrong_threshold = self::evidence(&wrong_threshold_attestation, &fixture);
        assert!(
            respond(
                &request_jws,
                &ctx,
                Decision::Approve,
                Some(&wrong_threshold),
                &fixture.holder_private
            )
            .is_err()
        );
    }

    #[test]
    fn request_origin_expiry_issuer_claim_and_holder_key_fail_closed() {
        let fixture = fixture();
        let request = ordinary_request();
        let request_jws = sign_request(&request, &fixture.verifier_private, &[]).unwrap();
        let attestation = ordinary_attestation(&fixture, "membership", "community");
        let evidence = evidence(&attestation, &fixture);

        assert!(
            verify_request(
                &request_jws,
                &fixture.pin,
                "https://other.example",
                NOW + 1,
                &[]
            )
            .is_err()
        );
        assert!(verify_request(&request_jws, &fixture.pin, ORIGIN, NOW + 300, &[]).is_err());

        let mut unsupported_issuer = ordinary_request();
        unsupported_issuer.accepted_issuer_ids = vec!["zerant:issuer:other".into()];
        let token = sign_request(&unsupported_issuer, &fixture.verifier_private, &[]).unwrap();
        assert!(
            respond(
                &token,
                &context(&fixture, NOW + 1, ORIGIN),
                Decision::Approve,
                Some(&evidence),
                &fixture.holder_private
            )
            .is_err()
        );

        let mut wrong_claim = ordinary_request();
        wrong_claim.claim_type = "role".into();
        let token = sign_request(&wrong_claim, &fixture.verifier_private, &[]).unwrap();
        assert!(
            respond(
                &token,
                &context(&fixture, NOW + 1, ORIGIN),
                Decision::Approve,
                Some(&evidence),
                &fixture.holder_private
            )
            .is_err()
        );

        let wrong_holder = Jwk::generate_ed_key(EdCurve::Ed25519).unwrap();
        assert!(
            respond(
                &request_jws,
                &context(&fixture, NOW + 1, ORIGIN),
                Decision::Approve,
                Some(&evidence),
                &wrong_holder
            )
            .is_err()
        );
    }

    #[test]
    fn source_credentials_cannot_be_disclosed() {
        let fixture = fixture();
        let source = CredentialPayload {
            schema: "zerant.credential.v0.1".into(),
            kind: CredentialKind::Source,
            credential_id: encode_base64url(&[20; 16]),
            issuer_id: ISSUER.into(),
            issuer_key_id: ISSUER_KID.into(),
            subject_key: fixture.holder_public.clone(),
            audience: "holder-local".into(),
            issued_at: NOW - 100,
            expires_at: NOW + 1_000,
            revocation_handle: encode_base64url(&[21; 16]),
            claim: Claim::Source(zerant_credential::SourceEventClaim {
                claim_type: "zerant:source:test".into(),
                value: "event".into(),
                source_schema_version: "0.1".into(),
                context: "community".into(),
                occurred_at: NOW - 100,
            }),
        };
        assert!(ordinary_request().matches_attestation(&source).is_err());
    }

    #[test]
    fn response_binding_mutations_fail_without_consuming_request() {
        let fixture = fixture();
        let request = ordinary_request();
        let request_jws = sign_request(&request, &fixture.verifier_private, &[]).unwrap();
        let attestation = ordinary_attestation(&fixture, "membership", "community");
        let evidence = evidence(&attestation, &fixture);
        let ctx = context(&fixture, NOW + 10, ORIGIN);
        let response = respond(
            &request_jws,
            &ctx,
            Decision::Approve,
            Some(&evidence),
            &fixture.holder_private,
        )
        .unwrap()
        .unwrap();

        for field in ["challenge", "nonce", "request_digest"] {
            let mut parsed = untrusted_response(&response).unwrap();
            match field {
                "challenge" => parsed.challenge = encode_base64url(&[90; 32]),
                "nonce" => parsed.nonce = encode_base64url(&[91; 32]),
                _ => parsed.request_digest = encode_base64url(&[92; 32]),
            }
            let mutated = sign_jws(
                &parsed,
                RESPONSE_TYP,
                &fixture.holder_public.x,
                &fixture.holder_private,
            )
            .unwrap();
            let store = MemoryReplay::default();
            register_request(&request_jws, &ctx, &store).unwrap();
            assert!(accept(&request_jws, &mutated, &ctx, &evidence, &store).is_err());
            assert_eq!(
                store.get(&request.request_id).unwrap().unwrap().status,
                ReplayStatus::Pending
            );
        }
    }

    #[test]
    fn request_mutation_and_cross_origin_replay_fail() {
        let fixture = fixture();
        let request = ordinary_request();
        let original = sign_request(&request, &fixture.verifier_private, &[]).unwrap();
        let attestation = ordinary_attestation(&fixture, "membership", "community");
        let evidence = evidence(&attestation, &fixture);
        let ctx = context(&fixture, NOW + 10, ORIGIN);
        let response = respond(
            &original,
            &ctx,
            Decision::Approve,
            Some(&evidence),
            &fixture.holder_private,
        )
        .unwrap()
        .unwrap();
        let store = MemoryReplay::default();
        register_request(&original, &ctx, &store).unwrap();

        let mut changed = request.clone();
        changed.purpose = "Different purpose".into();
        let changed_token = sign_request(&changed, &fixture.verifier_private, &[]).unwrap();
        assert!(accept(&changed_token, &response, &ctx, &evidence, &store).is_err());
        assert_eq!(
            store.get(&request.request_id).unwrap().unwrap().status,
            ReplayStatus::Pending
        );

        assert!(
            accept(
                &original,
                &response,
                &context(&fixture, NOW + 10, "https://other.example"),
                &evidence,
                &store
            )
            .is_err()
        );
    }

    #[test]
    fn memory_replay_is_atomic_under_concurrency() {
        let store = Arc::new(MemoryReplay::default());
        let entry = ReplayEntry {
            request_id: encode_base64url(&[30; 16]),
            digest: encode_base64url(&[31; 32]),
            origin: ORIGIN.into(),
            expires_at: NOW + 100,
            status: ReplayStatus::Pending,
        };
        store.register(entry).unwrap();

        let mut handles = vec![];
        for _ in 0..16 {
            let store = Arc::clone(&store);
            handles.push(std::thread::spawn(move || {
                store
                    .consume_if_pending(
                        &encode_base64url(&[30; 16]),
                        &encode_base64url(&[31; 32]),
                        ORIGIN,
                        NOW,
                    )
                    .is_ok()
            }));
        }
        let successes = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .filter(|ok| *ok)
            .count();
        assert_eq!(successes, 1);
    }

    #[test]
    fn sqlite_replay_survives_reopen_and_duplicate_register_fails() {
        let path = std::env::temp_dir().join(format!(
            "zerant-disclosure-{}-replay.sqlite",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);

        {
            let store = SqliteReplay::open(&path).unwrap();
            let entry = ReplayEntry {
                request_id: encode_base64url(&[32; 16]),
                digest: encode_base64url(&[33; 32]),
                origin: ORIGIN.into(),
                expires_at: NOW + 100,
                status: ReplayStatus::Pending,
            };
            store.register(entry.clone()).unwrap();
            assert!(store.register(entry).is_err());
            store
                .consume_if_pending(
                    &encode_base64url(&[32; 16]),
                    &encode_base64url(&[33; 32]),
                    ORIGIN,
                    NOW,
                )
                .unwrap();
        }

        {
            let store = SqliteReplay::open(&path).unwrap();
            assert_eq!(
                store
                    .get(&encode_base64url(&[32; 16]))
                    .unwrap()
                    .unwrap()
                    .status,
                ReplayStatus::Consumed
            );
            assert!(
                store
                    .consume_if_pending(
                        &encode_base64url(&[32; 16]),
                        &encode_base64url(&[33; 32]),
                        ORIGIN,
                        NOW
                    )
                    .is_err()
            );
        }

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("sqlite-wal"));
        let _ = std::fs::remove_file(path.with_extension("sqlite-shm"));
    }

    #[test]
    fn expired_replay_cannot_be_consumed() {
        let store = MemoryReplay::default();
        store
            .register(ReplayEntry {
                request_id: encode_base64url(&[34; 16]),
                digest: encode_base64url(&[35; 32]),
                origin: ORIGIN.into(),
                expires_at: NOW,
                status: ReplayStatus::Pending,
            })
            .unwrap();
        assert!(
            store
                .consume_if_pending(
                    &encode_base64url(&[34; 16]),
                    &encode_base64url(&[35; 32]),
                    ORIGIN,
                    NOW
                )
                .is_err()
        );
    }

    #[test]
    fn compound_all_of_requires_every_requirement_and_consumes_once() {
        let fixture = fixture();
        let primary = ordinary_request();
        let digest_value = encode_base64url(&[9; 32]);
        let mut threshold = threshold_request(digest_value.clone(), 10);
        threshold.verifier_id = primary.verifier_id.clone();
        threshold.verifier_key_id = primary.verifier_key_id.clone();
        threshold.verifier_origin = primary.verifier_origin.clone();
        threshold.purpose = primary.purpose.clone();
        threshold.challenge = primary.challenge.clone();
        threshold.nonce = primary.nonce.clone();
        threshold.issued_at = primary.issued_at;
        threshold.expires_at = primary.expires_at;

        let request = compound::CompoundRequest {
            schema: compound::SCHEMA.into(),
            primary,
            additional: vec![threshold],
            expected_values: vec![Some(ClaimValue::String("active".into())), None],
        };
        let ctx = context(&fixture, NOW + 10, ORIGIN);
        let request_jws = compound::sign(&request, &ctx, &fixture.verifier_private).unwrap();

        let first = ordinary_attestation(&fixture, "membership", "community");
        let second = threshold_attestation(&fixture, &digest_value, 10);
        let evidence = vec![evidence(&first, &fixture), evidence(&second, &fixture)];

        assert_eq!(
            compound::respond(
                &request_jws,
                &ctx,
                Decision::Deny,
                &evidence,
                &fixture.holder_private,
            )
            .unwrap(),
            None
        );
        assert!(
            compound::respond(
                &request_jws,
                &ctx,
                Decision::Approve,
                &evidence[..1],
                &fixture.holder_private,
            )
            .is_err()
        );

        let response = compound::respond(
            &request_jws,
            &ctx,
            Decision::Approve,
            &evidence,
            &fixture.holder_private,
        )
        .unwrap()
        .unwrap();
        let store = MemoryReplay::default();
        compound::register(&request_jws, &ctx, &store).unwrap();
        let accepted = compound::accept(&request_jws, &response, &ctx, &evidence, &store).unwrap();
        assert_eq!(accepted.len(), 2);
        assert!(compound::accept(&request_jws, &response, &ctx, &evidence, &store).is_err());
    }

    #[test]
    fn compound_payment_requirement_exposes_attestation_not_wallet_evidence() {
        let mut fixture = fixture();
        let intent_digest = encode_base64url(&[77; 32]);
        let payment_context = format!("payment-intent:{intent_digest}");
        fixture.trust.issuers[0]
            .allowed_claim_types
            .push("payment.invoice_paid".into());
        fixture.trust.issuers[0]
            .allowed_contexts
            .push(payment_context.clone());

        let mut request = ordinary_request();
        request.claim_type = "payment.invoice_paid".into();
        request.context = Some(payment_context.clone());
        request.purpose = "Confirm settlement of this private Zcash payment intent".into();

        let compound = compound::CompoundRequest {
            schema: compound::SCHEMA.into(),
            primary: request,
            additional: vec![{
                let mut role = ordinary_request();
                role.request_id = encode_base64url(&[88; 16]);
                role.purpose = "Confirm settlement of this private Zcash payment intent".into();
                role
            }],
            expected_values: vec![
                Some(ClaimValue::Boolean(true)),
                Some(ClaimValue::String("active".into())),
            ],
        };
        let ctx = context(&fixture, NOW + 10, ORIGIN);
        let request_jws = compound::sign(&compound, &ctx, &fixture.verifier_private).unwrap();
        let payment = boolean_attestation_with_id(
            &fixture,
            "payment.invoice_paid",
            &payment_context,
            encode_base64url(&[89; 16]),
            encode_base64url(&[90; 16]),
        );
        let role = ordinary_attestation_with_id(
            &fixture,
            "membership",
            "community",
            encode_base64url(&[91; 16]),
            encode_base64url(&[92; 16]),
        );
        let evidence = vec![evidence(&payment, &fixture), evidence(&role, &fixture)];

        let response = compound::respond(
            &request_jws,
            &ctx,
            Decision::Approve,
            &evidence,
            &fixture.holder_private,
        )
        .unwrap()
        .unwrap();

        let [_, payload, _] = compact_parts(&response).unwrap();
        let text = String::from_utf8(decode_base64url(payload).unwrap()).unwrap();
        assert!(text.contains("payment.invoice_paid") || text.contains("attestations"));
        for forbidden in ["valueZat", "recipient", "memo", "wallet", "balance", "txid"] {
            assert!(!text.contains(forbidden), "{forbidden}");
        }
    }

    #[test]
    fn strict_header_and_payload_reject_unknowns() {
        let extra_header = serde_json::json!({
            "alg":"EdDSA","kid":VERIFIER_KID,"typ":REQUEST_TYP,"jku":"https://evil.example"
        });
        let fake = format!(
            "{}.e30.AA",
            encode_base64url(&serde_json::to_vec(&extra_header).unwrap())
        );
        assert!(inspect_header(&fake, REQUEST_TYP, VERIFIER_KID).is_err());

        let alternate = serde_json::json!({
            "alg":"HS256","kid":VERIFIER_KID,"typ":REQUEST_TYP
        });
        let fake = format!(
            "{}.e30.AA",
            encode_base64url(&serde_json::to_vec(&alternate).unwrap())
        );
        assert!(inspect_header(&fake, REQUEST_TYP, VERIFIER_KID).is_err());

        let fixture = fixture();
        let request = ordinary_request();
        let mut value = serde_json::to_value(request).unwrap();
        value["extra"] = true.into();
        let token = sign_jws(&value, REQUEST_TYP, VERIFIER_KID, &fixture.verifier_private).unwrap();
        assert!(verify_request(&token, &fixture.pin, ORIGIN, NOW + 1, &[]).is_err());
    }
    #[test]
    fn compound_all_of_consent_bindings_and_replay() {
        let f = fixture();
        let c = context(&f, NOW, ORIGIN);
        let primary = ordinary_request();
        let mut second = threshold_request(encode_base64url(&[9; 32]), 10);
        second.request_id = encode_base64url(&[99; 16]);
        second.purpose = primary.purpose.clone();
        second.challenge = primary.challenge.clone();
        second.nonce = primary.nonce.clone();
        second.issued_at = primary.issued_at;
        second.expires_at = primary.expires_at;
        let request = compound::CompoundRequest {
            schema: compound::SCHEMA.into(),
            primary,
            additional: vec![second],
            expected_values: vec![Some(ClaimValue::String("active".into())), None],
        };
        let token = compound::sign(&request, &c, &f.verifier_private).unwrap();
        let ordinary = ordinary_attestation(&f, "membership", "community");
        let threshold = threshold_attestation(&f, &encode_base64url(&[9; 32]), 10);
        let items = [evidence(&ordinary, &f), evidence(&threshold, &f)];
        assert!(
            compound::respond(&token, &c, Decision::Deny, &[], &f.holder_private)
                .unwrap()
                .is_none()
        );
        assert!(
            compound::respond(
                &token,
                &c,
                Decision::Approve,
                &items[..1],
                &f.holder_private
            )
            .is_err()
        );
        let response = compound::respond(&token, &c, Decision::Approve, &items, &f.holder_private)
            .unwrap()
            .unwrap();
        let decoded = decode_base64url(response.split('.').nth(1).unwrap()).unwrap();
        let text = String::from_utf8(decoded).unwrap();
        for forbidden in [
            "score",
            "source_events",
            "wallet",
            "recipient",
            "amount_zat",
        ] {
            assert!(!text.contains(forbidden));
        }
        let store = MemoryReplay::default();
        compound::register(&token, &c, &store).unwrap();
        let swapped = [evidence(&threshold, &f), evidence(&ordinary, &f)];
        assert!(compound::accept(&token, &response, &c, &swapped, &store).is_err());
        assert_eq!(
            store
                .get(&request.primary.request_id)
                .unwrap()
                .unwrap()
                .status,
            ReplayStatus::Pending
        );
        assert_eq!(
            compound::accept(&token, &response, &c, &items, &store)
                .unwrap()
                .len(),
            2
        );
        assert!(compound::accept(&token, &response, &c, &items, &store).is_err());
        assert!(compound::verify(&token, &context(&f, NOW, "https://wrong.example")).is_err());
        assert!(compound::verify(&token, &context(&f, NOW + 301, ORIGIN)).is_err());
        let mut mutated = request.clone();
        mutated.additional[0].verifier_origin = "https://wrong.example".into();
        assert!(compound::sign(&mutated, &c, &f.verifier_private).is_err());
        mutated = request.clone();
        mutated.additional[0].request_id = mutated.primary.request_id.clone();
        assert!(compound::sign(&mutated, &c, &f.verifier_private).is_err());
        let other = Jwk::generate_ed_key(EdCurve::Ed25519).unwrap();
        assert!(compound::respond(&token, &c, Decision::Approve, &items, &other).is_err());
        let mut unknown = serde_json::to_value(&request).unwrap();
        unknown["whole_wallet"] = serde_json::json!(true);
        assert!(
            parse_canonical::<compound::CompoundRequest>(&canonicalize(&unknown).unwrap()).is_err()
        );
    }
}
