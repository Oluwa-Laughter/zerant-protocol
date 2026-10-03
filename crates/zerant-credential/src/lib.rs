//! Strict credential and revocation verification for Zerant v0.1.
//!
//! This crate implements signed atomic credentials. It does not implement a
//! holder vault, consent, reputation evaluation, disclosure responses, ZK, or
//! Zcash integration.
#![forbid(unsafe_code)]

use josekit::{
    jwk::Jwk,
    jws::{self, EdDSA, JwsHeader},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zerant_core::{
    Error, MAX_JSON_BYTES, MAX_SAFE_INTEGER, Result, canonicalize, decode_base64url, decode_fixed,
    encode_base64url, parse_canonical, validate_id, validate_interval, validate_origin,
    validate_timestamp,
};

pub const CREDENTIAL_SCHEMA: &str = "zerant.credential.v0.1";
pub const CREDENTIAL_TYP: &str = "zerant-credential-v0.1";
pub const REVOCATION_SCHEMA: &str = "zerant.revocation.v0.1";
pub const REVOCATION_TYP: &str = "zerant-revocation-v0.1";
pub const JOSE_ALG: &str = "EdDSA";
pub const HOLDER_LOCAL_AUDIENCE: &str = "holder-local";
pub const MAX_REVOCATION_WINDOW: u64 = 86_400;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicJwk {
    pub kty: String,
    pub crv: String,
    pub x: String,
}

impl PublicJwk {
    pub fn validate(&self) -> Result<()> {
        if self.kty != "OKP" || self.crv != "Ed25519" {
            return Err(Error::Key);
        }
        decode_fixed::<32>(&self.x)
            .map(|_| ())
            .map_err(|_| Error::Key)
    }

    fn to_josekit(&self) -> Result<Jwk> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|_| Error::Key)?;
        Jwk::from_bytes(bytes).map_err(|_| Error::Key)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CredentialKind {
    Source,
    Attestation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceEventClaim {
    #[serde(rename = "type")]
    pub claim_type: String,
    pub value: String,
    pub source_schema_version: String,
    pub context: String,
    pub occurred_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThresholdAttestationClaim {
    #[serde(rename = "type")]
    pub claim_type: String,
    pub value: bool,
    pub context: String,
    pub policy_id: String,
    pub policy_version: String,
    pub policy_digest: String,
    pub as_of: u64,
    pub threshold: u64,
    pub operator: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ClaimValue {
    String(String),
    Boolean(bool),
    Integer(i64),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrdinaryClaim {
    #[serde(rename = "type")]
    pub claim_type: String,
    pub value: ClaimValue,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Claim {
    Source(SourceEventClaim),
    Threshold(ThresholdAttestationClaim),
    Ordinary(OrdinaryClaim),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CredentialPayload {
    pub schema: String,
    pub kind: CredentialKind,
    pub credential_id: String,
    pub issuer_id: String,
    pub issuer_key_id: String,
    pub subject_key: PublicJwk,
    pub audience: String,
    pub issued_at: u64,
    pub expires_at: u64,
    pub revocation_handle: String,
    pub claim: Claim,
}

impl CredentialPayload {
    pub fn validate(
        &self,
        now: u64,
        expected_audience: &str,
        allowed_loopback: &[String],
    ) -> Result<()> {
        if self.schema != CREDENTIAL_SCHEMA
            || self.issuer_id.is_empty()
            || self.issuer_key_id.is_empty()
        {
            return Err(Error::Credential);
        }
        validate_id(&self.credential_id)?;
        validate_id(&self.revocation_handle)?;
        self.subject_key.validate()?;
        validate_interval(self.issued_at, self.expires_at, now)?;

        match (&self.kind, &self.claim) {
            (CredentialKind::Source, Claim::Source(claim)) => {
                if self.audience != HOLDER_LOCAL_AUDIENCE
                    || expected_audience != HOLDER_LOCAL_AUDIENCE
                {
                    return Err(Error::Audience);
                }
                validate_source_claim(claim)?;
            }
            (CredentialKind::Attestation, Claim::Threshold(claim)) => {
                validate_origin(&self.audience, allowed_loopback)?;
                validate_origin(expected_audience, allowed_loopback)?;
                if self.audience != expected_audience {
                    return Err(Error::Audience);
                }
                validate_threshold_claim(claim, self.issued_at, self.expires_at)?;
            }
            (CredentialKind::Attestation, Claim::Ordinary(claim)) => {
                validate_origin(&self.audience, allowed_loopback)?;
                validate_origin(expected_audience, allowed_loopback)?;
                if self.audience != expected_audience {
                    return Err(Error::Audience);
                }
                validate_ordinary_claim(claim)?;
            }
            _ => return Err(Error::Credential),
        }
        Ok(())
    }
}

fn validate_source_claim(claim: &SourceEventClaim) -> Result<()> {
    if claim.claim_type.is_empty()
        || claim.value.is_empty()
        || claim.source_schema_version.is_empty()
        || claim.context.is_empty()
    {
        return Err(Error::Credential);
    }
    validate_timestamp(claim.occurred_at)
}

fn validate_threshold_claim(
    claim: &ThresholdAttestationClaim,
    issued_at: u64,
    expires_at: u64,
) -> Result<()> {
    if claim.claim_type != "reputation.threshold"
        || !claim.value
        || claim.context.is_empty()
        || claim.policy_id.is_empty()
        || claim.policy_version.is_empty()
        || claim.operator != "gte"
        || claim.as_of != issued_at
        || claim.threshold > MAX_SAFE_INTEGER
        || expires_at - issued_at > MAX_REVOCATION_WINDOW
    {
        return Err(Error::Credential);
    }
    decode_fixed::<32>(&claim.policy_digest)
        .map(|_| ())
        .map_err(|_| Error::Credential)
}

fn validate_ordinary_claim(claim: &OrdinaryClaim) -> Result<()> {
    if claim.claim_type.is_empty()
        || claim.claim_type == "reputation.threshold"
        || claim.context.as_deref().is_some_and(str::is_empty)
    {
        return Err(Error::Credential);
    }
    if let ClaimValue::String(value) = &claim.value
        && value.is_empty()
    {
        return Err(Error::Credential);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrustedIssuerKey {
    pub issuer_key_id: String,
    pub public_key: PublicJwk,
    pub valid_from: u64,
    pub valid_until: u64,
    pub compromised: bool,
}

impl TrustedIssuerKey {
    fn validate_for(&self, key_id: &str, now: u64) -> Result<()> {
        self.public_key.validate()?;
        validate_timestamp(self.valid_from)?;
        validate_timestamp(self.valid_until)?;
        validate_timestamp(now)?;
        if self.issuer_key_id != key_id
            || self.compromised
            || self.valid_from >= self.valid_until
            || now < self.valid_from
            || now >= self.valid_until
        {
            return Err(Error::Trust);
        }
        Ok(())
    }

    fn validate_issued_at(&self, issued_at: u64) -> Result<()> {
        validate_timestamp(issued_at)?;
        if issued_at < self.valid_from || issued_at >= self.valid_until {
            return Err(Error::Trust);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceSchemaAuthorization {
    pub source_schema_id: String,
    pub version: String,
    pub context: String,
    pub categories: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyAuthorization {
    pub policy_id: String,
    pub version: String,
    pub context: String,
    pub digest: String,
    pub supported_thresholds: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrustedIssuer {
    pub issuer_id: String,
    pub keys: Vec<TrustedIssuerKey>,
    pub allowed_claim_types: Vec<String>,
    pub allowed_contexts: Vec<String>,
    pub source_schemas: Vec<SourceSchemaAuthorization>,
    pub policies: Vec<PolicyAuthorization>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IssuerTrustManifest {
    pub issuers: Vec<TrustedIssuer>,
}

impl IssuerTrustManifest {
    fn issuer(&self, issuer_id: &str) -> Result<&TrustedIssuer> {
        let mut matches = self
            .issuers
            .iter()
            .filter(|issuer| issuer.issuer_id == issuer_id);
        let issuer = matches.next().ok_or(Error::Trust)?;
        if matches.next().is_some() {
            return Err(Error::Trust);
        }
        Ok(issuer)
    }

    pub fn find_key(
        &self,
        issuer_id: &str,
        key_id: &str,
        now: u64,
    ) -> Result<(&TrustedIssuer, &TrustedIssuerKey)> {
        let issuer = self.issuer(issuer_id)?;
        let mut matches = issuer.keys.iter().filter(|key| key.issuer_key_id == key_id);
        let key = matches.next().ok_or(Error::Trust)?;
        if matches.next().is_some() {
            return Err(Error::Trust);
        }
        key.validate_for(key_id, now)?;
        Ok((issuer, key))
    }

    fn authorize_payload(&self, payload: &CredentialPayload, now: u64) -> Result<()> {
        let (issuer, key) = self.find_key(&payload.issuer_id, &payload.issuer_key_id, now)?;
        key.validate_issued_at(payload.issued_at)?;

        let claim_type = match &payload.claim {
            Claim::Source(claim) => claim.claim_type.as_str(),
            Claim::Threshold(claim) => claim.claim_type.as_str(),
            Claim::Ordinary(claim) => claim.claim_type.as_str(),
        };
        if !issuer
            .allowed_claim_types
            .iter()
            .any(|value| value == claim_type)
        {
            return Err(Error::Trust);
        }

        match &payload.claim {
            Claim::Source(claim) => {
                if !issuer
                    .allowed_contexts
                    .iter()
                    .any(|value| value == &claim.context)
                {
                    return Err(Error::Trust);
                }
                let mut schemas = issuer.source_schemas.iter().filter(|schema| {
                    schema.source_schema_id == claim.claim_type
                        && schema.version == claim.source_schema_version
                        && schema.context == claim.context
                });
                let schema = schemas.next().ok_or(Error::Trust)?;
                if schemas.next().is_some()
                    || !schema.categories.iter().any(|value| value == &claim.value)
                {
                    return Err(Error::Trust);
                }
            }
            Claim::Threshold(claim) => {
                if !issuer
                    .allowed_contexts
                    .iter()
                    .any(|value| value == &claim.context)
                {
                    return Err(Error::Trust);
                }
                let mut policies = issuer.policies.iter().filter(|policy| {
                    policy.policy_id == claim.policy_id
                        && policy.version == claim.policy_version
                        && policy.context == claim.context
                        && policy.digest == claim.policy_digest
                });
                let policy = policies.next().ok_or(Error::Trust)?;
                if policies.next().is_some()
                    || !policy.supported_thresholds.contains(&claim.threshold)
                {
                    return Err(Error::Trust);
                }
            }
            Claim::Ordinary(claim) => {
                if let Some(context) = &claim.context
                    && !issuer.allowed_contexts.iter().any(|value| value == context)
                {
                    return Err(Error::Trust);
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevocationSnapshot {
    pub schema: String,
    pub issuer_id: String,
    pub issuer_key_id: String,
    pub version: u64,
    pub issued_at: u64,
    pub next_update: u64,
    pub revoked_digests: Vec<String>,
}

impl RevocationSnapshot {
    fn validate_structure(&self, now: u64, persisted_min_version: u64) -> Result<()> {
        if self.schema != REVOCATION_SCHEMA
            || self.issuer_id.is_empty()
            || self.issuer_key_id.is_empty()
        {
            return Err(Error::Revocation);
        }
        for value in [self.version, self.issued_at, self.next_update, now] {
            validate_timestamp(value).map_err(|_| Error::Revocation)?;
        }
        if self.version < persisted_min_version
            || self.issued_at >= self.next_update
            || self.next_update - self.issued_at > MAX_REVOCATION_WINDOW
            || now < self.issued_at
            || now >= self.next_update
        {
            return Err(Error::Revocation);
        }

        let mut previous: Option<&str> = None;
        for digest in &self.revoked_digests {
            decode_fixed::<32>(digest).map_err(|_| Error::Revocation)?;
            if previous.is_some_and(|prev| prev >= digest.as_str()) {
                return Err(Error::Revocation);
            }
            previous = Some(digest);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedCredential {
    pub payload: CredentialPayload,
    pub accepted_revocation_version: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StrictProtectedHeader {
    alg: String,
    kid: String,
    typ: String,
}

fn inspect_protected_header(token: &str, expected_typ: &str) -> Result<StrictProtectedHeader> {
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

    let bytes = decode_base64url(header).map_err(|_| Error::Signature)?;
    let parsed: StrictProtectedHeader =
        serde_json::from_slice(&bytes).map_err(|_| Error::Signature)?;
    if parsed.alg != JOSE_ALG || parsed.typ != expected_typ || parsed.kid.is_empty() {
        return Err(Error::Signature);
    }
    Ok(parsed)
}

fn verify_compact_jws(
    token: &str,
    expected_typ: &str,
    expected_issuer: &str,
    manifest: &IssuerTrustManifest,
    now: u64,
) -> Result<(Vec<u8>, StrictProtectedHeader)> {
    let protected = inspect_protected_header(token, expected_typ)?;
    let (_, trusted) = manifest.find_key(expected_issuer, &protected.kid, now)?;
    let jwk = trusted.public_key.to_josekit()?;
    let mut verifier = EdDSA.verifier_from_jwk(&jwk).map_err(|_| Error::Key)?;
    verifier.set_key_id(protected.kid.clone());

    let (payload, returned_header) =
        jws::deserialize_compact(token, &verifier).map_err(|_| Error::Signature)?;
    if returned_header.claims_set().len() != 3
        || returned_header.algorithm() != Some(JOSE_ALG)
        || returned_header.key_id() != Some(protected.kid.as_str())
        || returned_header.token_type() != Some(expected_typ)
    {
        return Err(Error::Signature);
    }
    Ok((payload, protected))
}

/// Signing exists for local issuers, demos, and test-vector generation. Callers own
/// private-key storage and key lifecycle.
pub fn sign_credential(payload: &CredentialPayload, private_jwk: &Jwk) -> Result<String> {
    payload.validate(payload.issued_at, &payload.audience, &[])?;
    let body = canonicalize(payload)?;
    let mut signer = EdDSA.signer_from_jwk(private_jwk).map_err(|_| Error::Key)?;
    signer.set_key_id(payload.issuer_key_id.clone());
    let mut header = JwsHeader::new();
    header.set_token_type(CREDENTIAL_TYP);
    jws::serialize_compact(&body, &header, &signer).map_err(|_| Error::Signature)
}

pub fn sign_revocation_snapshot(
    snapshot: &RevocationSnapshot,
    private_jwk: &Jwk,
) -> Result<String> {
    snapshot.validate_structure(snapshot.issued_at, 0)?;
    let body = canonicalize(snapshot)?;
    let mut signer = EdDSA.signer_from_jwk(private_jwk).map_err(|_| Error::Key)?;
    signer.set_key_id(snapshot.issuer_key_id.clone());
    let mut header = JwsHeader::new();
    header.set_token_type(REVOCATION_TYP);
    jws::serialize_compact(&body, &header, &signer).map_err(|_| Error::Signature)
}

pub fn revocation_digest(revocation_handle: &str) -> Result<String> {
    let handle = decode_fixed::<16>(revocation_handle)?;
    Ok(encode_base64url(&Sha256::digest(handle)))
}

pub fn verify_revocation_snapshot(
    token: &str,
    issuer_id: &str,
    manifest: &IssuerTrustManifest,
    now: u64,
    persisted_min_version: u64,
) -> Result<RevocationSnapshot> {
    let (payload_bytes, protected) =
        verify_compact_jws(token, REVOCATION_TYP, issuer_id, manifest, now)?;
    let snapshot: RevocationSnapshot = parse_canonical(&payload_bytes)?;
    if snapshot.issuer_id != issuer_id || snapshot.issuer_key_id != protected.kid {
        return Err(Error::Revocation);
    }
    let (_, key) = manifest.find_key(issuer_id, &snapshot.issuer_key_id, now)?;
    key.validate_issued_at(snapshot.issued_at)?;
    snapshot.validate_structure(now, persisted_min_version)?;
    Ok(snapshot)
}

#[allow(clippy::too_many_arguments)]
pub fn verify_credential(
    credential_jws: &str,
    revocation_jws: &str,
    manifest: &IssuerTrustManifest,
    expected_issuer_id: &str,
    expected_audience: &str,
    allowed_loopback: &[String],
    now: u64,
    persisted_min_revocation_version: u64,
) -> Result<VerifiedCredential> {
    let (payload_bytes, protected) = verify_compact_jws(
        credential_jws,
        CREDENTIAL_TYP,
        expected_issuer_id,
        manifest,
        now,
    )?;
    let payload: CredentialPayload = parse_canonical(&payload_bytes)?;
    if payload.issuer_id != expected_issuer_id || payload.issuer_key_id != protected.kid {
        return Err(Error::Trust);
    }
    manifest.authorize_payload(&payload, now)?;
    payload.validate(now, expected_audience, allowed_loopback)?;

    let snapshot = verify_revocation_snapshot(
        revocation_jws,
        expected_issuer_id,
        manifest,
        now,
        persisted_min_revocation_version,
    )?;
    if snapshot.issuer_key_id != payload.issuer_key_id {
        return Err(Error::Revocation);
    }
    let digest = revocation_digest(&payload.revocation_handle)?;
    if snapshot.revoked_digests.binary_search(&digest).is_ok() {
        return Err(Error::Revoked);
    }

    Ok(VerifiedCredential {
        payload,
        accepted_revocation_version: snapshot.version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use josekit::jwk::{Jwk, alg::ed::EdCurve};
    use serde_json::json;
    use zerant_core::{canonicalize, encode_base64url};

    const ISSUER: &str = "zerant:issuer:local-demo";
    const KID: &str = "issuer-key-01";
    const NOW: u64 = 1_800_000_000;

    fn key_pair() -> (Jwk, PublicJwk) {
        let mut private = Jwk::generate_ed_key(EdCurve::Ed25519).unwrap();
        private.set_key_id(KID);
        let public = private.to_public_key().unwrap();
        let value = serde_json::to_value(public).unwrap();
        let public = PublicJwk {
            kty: value["kty"].as_str().unwrap().to_owned(),
            crv: value["crv"].as_str().unwrap().to_owned(),
            x: value["x"].as_str().unwrap().to_owned(),
        };
        (private, public)
    }

    fn holder_public(seed: u8) -> PublicJwk {
        PublicJwk {
            kty: "OKP".into(),
            crv: "Ed25519".into(),
            x: encode_base64url(&[seed; 32]),
        }
    }

    fn manifest(public_jwk: PublicJwk) -> IssuerTrustManifest {
        IssuerTrustManifest {
            issuers: vec![TrustedIssuer {
                issuer_id: ISSUER.into(),
                keys: vec![TrustedIssuerKey {
                    issuer_key_id: KID.into(),
                    public_key: public_jwk,
                    valid_from: NOW - 1000,
                    valid_until: NOW + 1000,
                    compromised: false,
                }],
                allowed_claim_types: vec![
                    "zerant:source:oss-contribution".into(),
                    "reputation.threshold".into(),
                    "membership".into(),
                ],
                allowed_contexts: vec!["oss-community".into()],
                source_schemas: vec![SourceSchemaAuthorization {
                    source_schema_id: "zerant:source:oss-contribution".into(),
                    version: "0.1".into(),
                    context: "oss-community".into(),
                    categories: vec![
                        "merged_contribution".into(),
                        "review".into(),
                        "mentorship".into(),
                    ],
                }],
                policies: vec![PolicyAuthorization {
                    policy_id: "zerant:oss:contribution".into(),
                    version: "0.1".into(),
                    context: "oss-community".into(),
                    digest: encode_base64url(&[5; 32]),
                    supported_thresholds: vec![40],
                }],
            }],
        }
    }

    fn source_payload() -> CredentialPayload {
        CredentialPayload {
            schema: CREDENTIAL_SCHEMA.into(),
            kind: CredentialKind::Source,
            credential_id: encode_base64url(&[1; 16]),
            issuer_id: ISSUER.into(),
            issuer_key_id: KID.into(),
            subject_key: holder_public(7),
            audience: HOLDER_LOCAL_AUDIENCE.into(),
            issued_at: NOW - 20,
            expires_at: NOW + 200,
            revocation_handle: encode_base64url(&[2; 16]),
            claim: Claim::Source(SourceEventClaim {
                claim_type: "zerant:source:oss-contribution".into(),
                value: "merged_contribution".into(),
                source_schema_version: "0.1".into(),
                context: "oss-community".into(),
                occurred_at: NOW - 50,
            }),
        }
    }

    fn threshold_payload() -> CredentialPayload {
        let issued_at = NOW - 20;
        CredentialPayload {
            schema: CREDENTIAL_SCHEMA.into(),
            kind: CredentialKind::Attestation,
            credential_id: encode_base64url(&[3; 16]),
            issuer_id: ISSUER.into(),
            issuer_key_id: KID.into(),
            subject_key: holder_public(8),
            audience: "https://grants.example".into(),
            issued_at,
            expires_at: NOW + 200,
            revocation_handle: encode_base64url(&[4; 16]),
            claim: Claim::Threshold(ThresholdAttestationClaim {
                claim_type: "reputation.threshold".into(),
                value: true,
                context: "oss-community".into(),
                policy_id: "zerant:oss:contribution".into(),
                policy_version: "0.1".into(),
                policy_digest: encode_base64url(&[5; 32]),
                as_of: issued_at,
                threshold: 40,
                operator: "gte".into(),
            }),
        }
    }

    fn revocation_snapshot(revoked: Vec<String>, version: u64) -> RevocationSnapshot {
        let mut revoked_digests = revoked;
        revoked_digests.sort();
        RevocationSnapshot {
            schema: REVOCATION_SCHEMA.into(),
            issuer_id: ISSUER.into(),
            issuer_key_id: KID.into(),
            version,
            issued_at: NOW - 10,
            next_update: NOW + 100,
            revoked_digests,
        }
    }

    fn signed_bundle(
        payload: &CredentialPayload,
        revoked: Vec<String>,
        version: u64,
    ) -> (String, String, IssuerTrustManifest, Jwk) {
        let (private, public) = key_pair();
        let credential = sign_credential(payload, &private).unwrap();
        let revocation =
            sign_revocation_snapshot(&revocation_snapshot(revoked, version), &private).unwrap();
        (credential, revocation, manifest(public), private)
    }

    #[test]
    fn valid_source_credential() {
        let payload = source_payload();
        let (credential, revocation, trust, _) = signed_bundle(&payload, vec![], 7);
        let verified = verify_credential(
            &credential,
            &revocation,
            &trust,
            ISSUER,
            HOLDER_LOCAL_AUDIENCE,
            &[],
            NOW,
            7,
        )
        .unwrap();
        assert_eq!(verified.payload, payload);
        assert_eq!(verified.accepted_revocation_version, 7);
    }

    #[test]
    fn valid_audience_bound_threshold_attestation() {
        let payload = threshold_payload();
        let (credential, revocation, trust, _) = signed_bundle(&payload, vec![], 8);
        let verified = verify_credential(
            &credential,
            &revocation,
            &trust,
            ISSUER,
            "https://grants.example",
            &[],
            NOW,
            7,
        )
        .unwrap();
        assert!(matches!(verified.payload.claim, Claim::Threshold(_)));
    }

    #[test]
    fn tampered_token_fails() {
        let payload = source_payload();
        let (mut credential, revocation, trust, _) = signed_bundle(&payload, vec![], 1);
        let last = credential.pop().unwrap();
        credential.push(if last == 'A' { 'B' } else { 'A' });
        assert_eq!(
            verify_credential(
                &credential,
                &revocation,
                &trust,
                ISSUER,
                HOLDER_LOCAL_AUDIENCE,
                &[],
                NOW,
                0,
            ),
            Err(Error::Signature)
        );
    }

    #[test]
    fn wrong_or_unknown_issuer_key_fails() {
        let payload = source_payload();
        let (credential, revocation, mut trust, _) = signed_bundle(&payload, vec![], 1);
        trust.issuers[0].keys[0].issuer_key_id = "other-key".into();
        assert!(matches!(
            verify_credential(
                &credential,
                &revocation,
                &trust,
                ISSUER,
                HOLDER_LOCAL_AUDIENCE,
                &[],
                NOW,
                0,
            ),
            Err(Error::Trust)
        ));
    }

    #[test]
    fn expired_and_invalid_intervals_fail() {
        let mut expired = source_payload();
        expired.issued_at = NOW - 200;
        expired.expires_at = NOW;
        let (credential, revocation, trust, _) = signed_bundle(&expired, vec![], 1);
        assert!(
            verify_credential(
                &credential,
                &revocation,
                &trust,
                ISSUER,
                HOLDER_LOCAL_AUDIENCE,
                &[],
                NOW,
                0
            )
            .is_err()
        );

        let mut invalid = source_payload();
        invalid.issued_at = NOW;
        invalid.expires_at = NOW;
        let (private, _) = key_pair();
        assert_eq!(sign_credential(&invalid, &private), Err(Error::Time));
    }

    #[test]
    fn wrong_audience_fails() {
        let payload = threshold_payload();
        let (credential, revocation, trust, _) = signed_bundle(&payload, vec![], 1);
        assert_eq!(
            verify_credential(
                &credential,
                &revocation,
                &trust,
                ISSUER,
                "https://other.example",
                &[],
                NOW,
                0,
            ),
            Err(Error::Audience)
        );
    }

    #[test]
    fn malformed_or_private_subject_jwk_fails_strict_parsing() {
        let mut value = serde_json::to_value(source_payload()).unwrap();
        value["subject_key"]["x"] = json!("bad");
        let bytes = canonicalize(&value).unwrap();
        let parsed: CredentialPayload = parse_canonical(&bytes).unwrap();
        assert_eq!(
            parsed.validate(NOW, HOLDER_LOCAL_AUDIENCE, &[]),
            Err(Error::Key)
        );

        let mut value = serde_json::to_value(source_payload()).unwrap();
        value["subject_key"]["d"] = json!(encode_base64url(&[9; 32]));
        let bytes = canonicalize(&value).unwrap();
        assert!(parse_canonical::<CredentialPayload>(&bytes).is_err());
    }

    #[test]
    fn unknown_payload_fields_fail_strict_parsing() {
        let mut value = serde_json::to_value(source_payload()).unwrap();
        value["surprise"] = json!(true);
        let bytes = canonicalize(&value).unwrap();
        assert!(parse_canonical::<CredentialPayload>(&bytes).is_err());
    }

    #[test]
    fn revoked_handle_fails() {
        let payload = source_payload();
        let digest = revocation_digest(&payload.revocation_handle).unwrap();
        let (credential, revocation, trust, _) = signed_bundle(&payload, vec![digest], 2);
        assert_eq!(
            verify_credential(
                &credential,
                &revocation,
                &trust,
                ISSUER,
                HOLDER_LOCAL_AUDIENCE,
                &[],
                NOW,
                2,
            ),
            Err(Error::Revoked)
        );
    }

    #[test]
    fn stale_snapshot_and_rollback_fail() {
        let payload = source_payload();
        let (private, public) = key_pair();
        let credential = sign_credential(&payload, &private).unwrap();
        let trust = manifest(public);

        let mut stale = revocation_snapshot(vec![], 9);
        stale.issued_at = NOW - 200;
        stale.next_update = NOW;
        let stale = sign_revocation_snapshot(&stale, &private).unwrap();
        assert!(matches!(
            verify_credential(
                &credential,
                &stale,
                &trust,
                ISSUER,
                HOLDER_LOCAL_AUDIENCE,
                &[],
                NOW,
                9,
            ),
            Err(Error::Revocation)
        ));

        let rollback = sign_revocation_snapshot(&revocation_snapshot(vec![], 8), &private).unwrap();
        assert!(matches!(
            verify_credential(
                &credential,
                &rollback,
                &trust,
                ISSUER,
                HOLDER_LOCAL_AUDIENCE,
                &[],
                NOW,
                9,
            ),
            Err(Error::Revocation)
        ));
    }

    #[test]
    fn trust_scope_rejects_policy_context_and_threshold_substitution() {
        for mutation in 0..3 {
            let mut payload = threshold_payload();
            if let Claim::Threshold(claim) = &mut payload.claim {
                match mutation {
                    0 => claim.threshold = 41,
                    1 => claim.context = "other-context".into(),
                    _ => claim.policy_digest = encode_base64url(&[0; 32]),
                }
            }
            let (private, public) = key_pair();
            let credential = sign_credential(&payload, &private).unwrap();
            let revocation =
                sign_revocation_snapshot(&revocation_snapshot(vec![], 3), &private).unwrap();
            assert_eq!(
                verify_credential(
                    &credential,
                    &revocation,
                    &manifest(public),
                    ISSUER,
                    "https://grants.example",
                    &[],
                    NOW,
                    0,
                ),
                Err(Error::Trust)
            );
        }
    }

    #[test]
    fn threshold_attestation_lifetime_is_bounded_to_24_hours() {
        let mut payload = threshold_payload();
        payload.expires_at = payload.issued_at + MAX_REVOCATION_WINDOW + 1;
        let (private, _) = key_pair();
        assert_eq!(sign_credential(&payload, &private), Err(Error::Credential));
    }

    #[test]
    fn issuance_must_fall_inside_issuer_key_validity() {
        let mut payload = source_payload();
        payload.issued_at = NOW - 2_000;
        payload.expires_at = NOW + 100;
        let (private, public) = key_pair();
        let credential = sign_credential(&payload, &private).unwrap();
        let revocation =
            sign_revocation_snapshot(&revocation_snapshot(vec![], 4), &private).unwrap();
        assert_eq!(
            verify_credential(
                &credential,
                &revocation,
                &manifest(public),
                ISSUER,
                HOLDER_LOCAL_AUDIENCE,
                &[],
                NOW,
                0,
            ),
            Err(Error::Trust)
        );
    }

    #[test]
    fn compromised_issuer_key_fails_closed() {
        let payload = source_payload();
        let (credential, revocation, mut trust, _) = signed_bundle(&payload, vec![], 5);
        trust.issuers[0].keys[0].compromised = true;
        assert_eq!(
            verify_credential(
                &credential,
                &revocation,
                &trust,
                ISSUER,
                HOLDER_LOCAL_AUDIENCE,
                &[],
                NOW,
                0,
            ),
            Err(Error::Trust)
        );
    }

    #[test]
    fn future_unsorted_and_duplicate_revocation_snapshots_fail() {
        let (private, public) = key_pair();
        let trust = manifest(public);

        let mut future = revocation_snapshot(vec![], 6);
        future.issued_at = NOW + 1;
        future.next_update = NOW + 100;
        let token = sign_revocation_snapshot(&future, &private).unwrap();
        assert_eq!(
            verify_revocation_snapshot(&token, ISSUER, &trust, NOW, 0),
            Err(Error::Revocation)
        );

        let a = encode_base64url(&[1; 32]);
        let b = encode_base64url(&[2; 32]);
        let mut unsorted = revocation_snapshot(vec![], 6);
        unsorted.revoked_digests = vec![b.clone(), a.clone()];
        assert_eq!(
            sign_revocation_snapshot(&unsorted, &private),
            Err(Error::Revocation)
        );

        let mut duplicate = revocation_snapshot(vec![], 6);
        duplicate.revoked_digests = vec![a.clone(), a];
        assert_eq!(
            sign_revocation_snapshot(&duplicate, &private),
            Err(Error::Revocation)
        );
    }

    #[test]
    fn ordinary_atomic_attestation_is_supported_with_trust_scope() {
        let issued_at = NOW - 20;
        let payload = CredentialPayload {
            schema: CREDENTIAL_SCHEMA.into(),
            kind: CredentialKind::Attestation,
            credential_id: encode_base64url(&[9; 16]),
            issuer_id: ISSUER.into(),
            issuer_key_id: KID.into(),
            subject_key: holder_public(10),
            audience: "https://grants.example".into(),
            issued_at,
            expires_at: NOW + 200,
            revocation_handle: encode_base64url(&[11; 16]),
            claim: Claim::Ordinary(OrdinaryClaim {
                claim_type: "membership".into(),
                value: ClaimValue::String("member".into()),
                context: Some("oss-community".into()),
            }),
        };
        let (credential, revocation, trust, _) = signed_bundle(&payload, vec![], 7);
        let verified = verify_credential(
            &credential,
            &revocation,
            &trust,
            ISSUER,
            "https://grants.example",
            &[],
            NOW,
            0,
        )
        .unwrap();
        assert!(matches!(verified.payload.claim, Claim::Ordinary(_)));
    }

    #[test]
    fn threshold_serialization_has_no_exact_score_or_source_ids() {
        let bytes = canonicalize(&threshold_payload()).unwrap();
        let text = std::str::from_utf8(&bytes).unwrap();
        assert!(!text.contains(r#""score""#));
        assert!(!text.contains("source_id"));
        assert!(!text.contains("credential_ids"));
        assert!(text.contains(r#""threshold":40"#));
    }

    #[test]
    fn unsafe_jcs_integer_is_rejected() {
        assert_eq!(
            canonicalize(&json!({"too_large": MAX_SAFE_INTEGER + 1})),
            Err(Error::UnsafeNumber)
        );
    }

    #[test]
    fn protected_header_rejects_extra_fields() {
        let header = json!({"alg":"EdDSA","kid":KID,"typ":CREDENTIAL_TYP,"jku":"https://evil"});
        let header = encode_base64url(&serde_json::to_vec(&header).unwrap());
        let fake = format!("{header}.e30.AA");
        assert_eq!(
            inspect_protected_header(&fake, CREDENTIAL_TYP),
            Err(Error::Signature)
        );
    }
}
