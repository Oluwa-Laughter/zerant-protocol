//! Generic vertical vectors use ephemeral keys; no private keys are persisted.
use josekit::jwk::{Jwk, alg::ed::EdCurve};
use zerant_core::{canonicalize, encode_base64url, parse_canonical};
use zerant_credential::*;
use zerant_disclosure::*;
use zerant_policy::{Policy, evaluate};

fn public(key: &Jwk) -> PublicJwk {
    let v = serde_json::to_value(key.to_public_key().unwrap()).unwrap();
    serde_json::from_value(serde_json::json!({"kty":v["kty"],"crv":v["crv"],"x":v["x"]})).unwrap()
}

/// Test-only enrollment boundary: an issuer may bind a fresh audience key only
/// after authenticating it belongs to the same enrolled source subject.
/// Production enrollment/challenge transport remains application responsibility.
fn bind_audience_key(
    enrolled_source: &PublicJwk,
    presented_source: &PublicJwk,
    audience_key: &Jwk,
) -> zerant_core::Result<PublicJwk> {
    if enrolled_source != presented_source {
        return Err(zerant_core::Error::Trust);
    }
    Ok(public(audience_key))
}

#[test]
fn foreign_source_subject_cannot_receive_an_enrolled_audience_binding() {
    let enrolled_source = public(&Jwk::generate_ed_key(EdCurve::Ed25519).unwrap());
    let foreign_source = public(&Jwk::generate_ed_key(EdCurve::Ed25519).unwrap());
    let audience = Jwk::generate_ed_key(EdCurve::Ed25519).unwrap();
    assert!(bind_audience_key(&enrolled_source, &foreign_source, &audience).is_err());
}

#[test]
fn every_shared_scenario_uses_same_signed_policy_and_disclosure() {
    const NOW: u64 = 1_800_000_000;
    let scenarios: serde_json::Value =
        serde_json::from_str(include_str!("../../../fixtures/scenarios.json")).unwrap();
    for scenario in scenarios.as_array().unwrap() {
        let p: Policy = parse_canonical(&canonicalize(&scenario["policy"]).unwrap()).unwrap();
        let issuer = Jwk::generate_ed_key(EdCurve::Ed25519).unwrap();
        let holder = Jwk::generate_ed_key(EdCurve::Ed25519).unwrap();
        let source_holder = Jwk::generate_ed_key(EdCurve::Ed25519).unwrap();
        let verifier = Jwk::generate_ed_key(EdCurve::Ed25519).unwrap();
        let origin = format!("https://{}.example", scenario["id"].as_str().unwrap());
        let digest = p.digest().unwrap();
        let trust = IssuerTrustManifest {
            issuers: vec![TrustedIssuer {
                issuer_id: p.issuer_id.clone(),
                keys: vec![TrustedIssuerKey {
                    issuer_key_id: "issuer-1".into(),
                    public_key: public(&issuer),
                    valid_from: NOW - 100,
                    valid_until: NOW + 10000,
                    compromised: false,
                }],
                allowed_claim_types: vec![
                    p.source_schema_id.clone(),
                    "reputation.threshold".into(),
                ],
                allowed_contexts: vec![p.context_id.clone()],
                source_schemas: vec![SourceSchemaAuthorization {
                    source_schema_id: p.source_schema_id.clone(),
                    version: p.source_schema_version.clone(),
                    context: p.context_id.clone(),
                    categories: p.category_rules.keys().cloned().collect(),
                }],
                policies: vec![PolicyAuthorization {
                    policy_id: p.policy_id.clone(),
                    version: p.version.clone(),
                    context: p.context_id.clone(),
                    digest: digest.clone(),
                    supported_thresholds: p.supported_thresholds.clone(),
                }],
            }],
        };
        let snapshot = RevocationSnapshot {
            schema: REVOCATION_SCHEMA.into(),
            issuer_id: p.issuer_id.clone(),
            issuer_key_id: "issuer-1".into(),
            version: 1,
            issued_at: NOW - 10,
            next_update: NOW + 1000,
            revoked_digests: vec![],
        };
        let rev = sign_revocation_snapshot(&snapshot, &issuer).unwrap();
        let enrolled_source = public(&source_holder);
        let source = CredentialPayload {
            schema: CREDENTIAL_SCHEMA.into(),
            kind: CredentialKind::Source,
            credential_id: encode_base64url(&[1; 16]),
            issuer_id: p.issuer_id.clone(),
            issuer_key_id: "issuer-1".into(),
            subject_key: enrolled_source.clone(),
            audience: HOLDER_LOCAL_AUDIENCE.into(),
            issued_at: NOW - 10,
            expires_at: NOW + 1000,
            revocation_handle: encode_base64url(&[2; 16]),
            claim: Claim::Source(SourceEventClaim {
                claim_type: p.source_schema_id.clone(),
                value: p.category_rules.keys().next().unwrap().clone(),
                source_schema_version: p.source_schema_version.clone(),
                context: p.context_id.clone(),
                occurred_at: NOW - 5,
            }),
        };
        let source_jws = sign_credential(&source, &issuer).unwrap();
        let verified = verify_credential(
            &source_jws,
            &rev,
            &trust,
            &p.issuer_id,
            HOLDER_LOCAL_AUDIENCE,
            &[],
            NOW,
            1,
        )
        .unwrap();
        let verified_source = verified.payload;
        let evaluation =
            evaluate(&p, &[verified_source.clone(), verified_source.clone()], NOW).unwrap();
        let threshold = p.supported_thresholds[0];
        assert!(evaluation.score >= threshold);
        let audience_subject =
            bind_audience_key(&enrolled_source, &verified_source.subject_key, &holder).unwrap();
        let attestation = CredentialPayload {
            schema: CREDENTIAL_SCHEMA.into(),
            kind: CredentialKind::Attestation,
            credential_id: encode_base64url(&[3; 16]),
            issuer_id: p.issuer_id.clone(),
            issuer_key_id: "issuer-1".into(),
            subject_key: audience_subject,
            audience: origin.clone(),
            issued_at: NOW,
            expires_at: evaluation.next_change_at.unwrap().min(NOW + 300),
            revocation_handle: encode_base64url(&[4; 16]),
            claim: Claim::Threshold(ThresholdAttestationClaim {
                claim_type: "reputation.threshold".into(),
                value: true,
                context: p.context_id.clone(),
                policy_id: p.policy_id.clone(),
                policy_version: p.version.clone(),
                policy_digest: digest.clone(),
                as_of: NOW,
                threshold,
                operator: "gte".into(),
            }),
        };
        let attestation_jws = sign_credential(&attestation, &issuer).unwrap();
        let request = Request {
            schema: REQUEST_SCHEMA.into(),
            request_id: encode_base64url(&[5; 16]),
            verifier_id: "verifier".into(),
            verifier_key_id: "verifier-1".into(),
            verifier_origin: origin.clone(),
            purpose: scenario["purpose"].as_str().unwrap().into(),
            accepted_issuer_ids: vec![p.issuer_id.clone()],
            claim_type: "reputation.threshold".into(),
            context: None,
            predicate: Some(Predicate {
                context: p.context_id,
                policy_id: p.policy_id,
                policy_version: p.version,
                policy_digest: digest,
                threshold,
                operator: "gte".into(),
            }),
            challenge: encode_base64url(&[6; 32]),
            nonce: encode_base64url(&[7; 32]),
            issued_at: NOW,
            expires_at: NOW + 300,
        };
        let request_jws = sign_request(&request, &verifier, &[]).unwrap();
        let pin = VerifierPin {
            verifier_id: "verifier".into(),
            key_id: "verifier-1".into(),
            key: public(&verifier),
            allowed_origins: vec![origin.clone()],
            valid_from: NOW - 100,
            valid_until: NOW + 1000,
            compromised: false,
        };
        let context = Context {
            pin: &pin,
            origin: &origin,
            trust: &trust,
            now: NOW,
            allowed_loopback: &[],
        };
        let evidence = Evidence {
            attestation_jws: &attestation_jws,
            revocation_jws: &rev,
            issuer_id: &p.issuer_id,
            minimum_revocation_version: 1,
        };
        assert!(
            respond(&request_jws, &context, Decision::Deny, None, &holder)
                .unwrap()
                .is_none()
        );
        let response = respond(
            &request_jws,
            &context,
            Decision::Approve,
            Some(&evidence),
            &holder,
        )
        .unwrap()
        .unwrap();
        let store = MemoryReplay::default();
        register_request(&request_jws, &context, &store).unwrap();
        assert_eq!(
            accept(&request_jws, &response, &context, &evidence, &store).unwrap(),
            attestation
        );
        assert!(accept(&request_jws, &response, &context, &evidence, &store).is_err());
    }
}
