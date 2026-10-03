use josekit::{
    jwk::Jwk,
    jws::{self, EdDSA},
};
use serde_json::{Value, json};
use zerant_core::{Error, canonicalize, decode_base64url, encode_base64url, parse_canonical};
use zerant_credential::*;

const NOW: u64 = 1_700_000_010;
const ISSUER: &str = "zerant:issuer:local-demo";
const KID: &str = "demo-1";

fn private_key() -> Jwk {
    Jwk::from_bytes(include_bytes!("fixtures/issuer-private.json")).unwrap()
}

fn source() -> CredentialPayload {
    parse_canonical(include_bytes!("fixtures/source.json")).unwrap()
}

fn threshold() -> CredentialPayload {
    parse_canonical(include_bytes!("fixtures/threshold.json")).unwrap()
}

fn snapshot() -> RevocationSnapshot {
    parse_canonical(include_bytes!("fixtures/revocation.json")).unwrap()
}

fn trust() -> IssuerTrustManifest {
    serde_json::from_slice(include_bytes!("fixtures/trust.json")).unwrap()
}

fn credential_token(payload: &CredentialPayload) -> String {
    sign_credential(payload, &private_key()).unwrap()
}

fn revocation_token(payload: &RevocationSnapshot) -> String {
    sign_revocation_snapshot(payload, &private_key()).unwrap()
}

fn verify(payload: &CredentialPayload, audience: &str) -> zerant_core::Result<VerifiedCredential> {
    verify_credential(
        &credential_token(payload),
        &revocation_token(&snapshot()),
        &trust(),
        ISSUER,
        audience,
        &[],
        NOW,
        7,
    )
}

fn replace_payload(token: &str, value: &Value) -> String {
    let parts: Vec<_> = token.split('.').collect();
    format!(
        "{}.{}.{}",
        parts[0],
        encode_base64url(&canonicalize(value).unwrap()),
        parts[2]
    )
}

fn replace_header(token: &str, bytes: &[u8]) -> String {
    let parts: Vec<_> = token.split('.').collect();
    format!("{}.{}.{}", encode_base64url(bytes), parts[1], parts[2])
}

#[test]
fn rfc8037_ed25519_jws_vector_verifies_with_josekit() {
    let public = Jwk::from_bytes(
        br#"{"crv":"Ed25519","kty":"OKP","x":"11qYAYKxCrfVS_7TyWQHOg7hcvPapiMlrwIaaPcHURo"}"#,
    )
    .unwrap();
    let verifier = EdDSA.verifier_from_jwk(&public).unwrap();
    let token = concat!(
        "eyJhbGciOiJFZERTQSJ9.",
        "RXhhbXBsZSBvZiBFZDI1NTE5IHNpZ25pbmc.",
        "hgyY0il_MGCjP0JzlnLWG1PPOt7-09PGcvMg3AIbQR6dWbhijcNR4ki4iylGjg5BhVsPt",
        "9g7sVvpAr_MuM0KAg"
    );
    let (payload, header) = jws::deserialize_compact(token, &verifier).unwrap();
    assert_eq!(payload, b"Example of Ed25519 signing");
    assert_eq!(header.algorithm(), Some("EdDSA"));
}

#[test]
fn fixture_source_and_threshold_verify_end_to_end() {
    let source = verify(&source(), HOLDER_LOCAL_AUDIENCE).unwrap();
    assert_eq!(source.accepted_revocation_version, 7);

    let threshold = verify(&threshold(), "https://verifier.example").unwrap();
    assert!(matches!(threshold.payload.claim, Claim::Threshold(_)));
}

#[test]
fn signing_is_deterministic_and_payload_is_canonical() {
    let payload = source();
    let first = credential_token(&payload);
    assert_eq!(first, credential_token(&payload));

    let parts: Vec<_> = first.split('.').collect();
    assert_eq!(
        decode_base64url(parts[1]).unwrap(),
        include_bytes!("fixtures/source.json")
    );

    let header: Value = serde_json::from_slice(&decode_base64url(parts[0]).unwrap()).unwrap();
    assert_eq!(header.as_object().unwrap().len(), 3);
    assert_eq!(header["alg"], "EdDSA");
    assert_eq!(header["kid"], KID);
    assert_eq!(header["typ"], CREDENTIAL_TYP);
}

#[test]
fn payload_tampering_and_header_substitution_fail() {
    let token = credential_token(&source());

    let mut payload = serde_json::to_value(source()).unwrap();
    payload["claim"]["value"] = json!("review");
    assert!(
        verify_credential(
            &replace_payload(&token, &payload),
            &revocation_token(&snapshot()),
            &trust(),
            ISSUER,
            HOLDER_LOCAL_AUDIENCE,
            &[],
            NOW,
            7,
        )
        .is_err()
    );

    let header: Value =
        serde_json::from_slice(&decode_base64url(token.split('.').next().unwrap()).unwrap())
            .unwrap();
    for (field, value) in [
        ("alg", json!("none")),
        ("typ", json!("other")),
        ("kid", json!("unknown")),
    ] {
        let mut changed = header.clone();
        changed[field] = value;
        let bytes = canonicalize(&changed).unwrap();
        assert!(
            verify_credential(
                &replace_header(&token, &bytes),
                &revocation_token(&snapshot()),
                &trust(),
                ISSUER,
                HOLDER_LOCAL_AUDIENCE,
                &[],
                NOW,
                7,
            )
            .is_err()
        );
    }
}

#[test]
fn duplicate_or_extra_protected_header_fields_fail() {
    let token = credential_token(&source());

    let duplicate =
        br#"{"alg":"EdDSA","alg":"EdDSA","kid":"demo-1","typ":"zerant-credential-v0.1"}"#;
    assert_eq!(
        verify_credential(
            &replace_header(&token, duplicate),
            &revocation_token(&snapshot()),
            &trust(),
            ISSUER,
            HOLDER_LOCAL_AUDIENCE,
            &[],
            NOW,
            7,
        ),
        Err(Error::Signature)
    );

    let extra = canonicalize(&json!({
        "alg":"EdDSA",
        "jku":"https://evil.example/jwks.json",
        "kid":KID,
        "typ":CREDENTIAL_TYP
    }))
    .unwrap();
    assert_eq!(
        verify_credential(
            &replace_header(&token, &extra),
            &revocation_token(&snapshot()),
            &trust(),
            ISSUER,
            HOLDER_LOCAL_AUDIENCE,
            &[],
            NOW,
            7,
        ),
        Err(Error::Signature)
    );
}

#[test]
fn duplicate_trust_entries_fail_closed() {
    let mut manifest = trust();
    let duplicate = manifest.issuers[0].keys[0].clone();
    manifest.issuers[0].keys.push(duplicate);
    assert_eq!(
        verify_credential(
            &credential_token(&source()),
            &revocation_token(&snapshot()),
            &manifest,
            ISSUER,
            HOLDER_LOCAL_AUDIENCE,
            &[],
            NOW,
            7,
        ),
        Err(Error::Trust)
    );
}

#[test]
fn revocation_digest_matches_fixture_vector() {
    let digest = revocation_digest(&source().revocation_handle).unwrap();
    assert_eq!(
        digest,
        include_str!("fixtures/revocation-digest.txt").trim()
    );
}

#[test]
fn threshold_fixture_has_no_score_or_source_identifiers() {
    let value = serde_json::to_value(threshold()).unwrap();
    let claim = value["claim"].as_object().unwrap();
    for forbidden in [
        "score",
        "exact_score",
        "source_ids",
        "source_event_id",
        "source_credential_id",
        "events",
        "wallet",
        "address",
        "balance",
        "history",
    ] {
        assert!(!claim.contains_key(forbidden));
    }
}

#[test]
fn oversized_compact_input_fails_before_jws_processing() {
    let oversized = "a".repeat(zerant_core::MAX_JSON_BYTES * 2 + 1);
    assert_eq!(
        verify_credential(
            &oversized,
            "",
            &trust(),
            ISSUER,
            HOLDER_LOCAL_AUDIENCE,
            &[],
            NOW,
            7,
        ),
        Err(Error::Size)
    );
}
