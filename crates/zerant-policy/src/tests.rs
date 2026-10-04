use super::*;
use zerant_core::{MAX_SAFE_INTEGER, decode_fixed};
use zerant_credential::{CREDENTIAL_SCHEMA, PublicJwk, SourceEventClaim};
const NOW: u64 = 1_800_000_000;
fn policy() -> Policy {
    serde_json::from_str(include_str!(
        "../../../docs/examples/policies/oss-contribution.json"
    ))
    .unwrap()
}
fn source(p: &Policy, id: u8, category: &str, occurred_at: u64) -> CredentialPayload {
    CredentialPayload {
        schema: CREDENTIAL_SCHEMA.into(),
        kind: CredentialKind::Source,
        credential_id: encode_base64url(&[id; 16]),
        issuer_id: p.issuer_id.clone(),
        issuer_key_id: "issuer-key".into(),
        subject_key: PublicJwk {
            kty: "OKP".into(),
            crv: "Ed25519".into(),
            x: encode_base64url(&[1; 32]),
        },
        audience: HOLDER_LOCAL_AUDIENCE.into(),
        issued_at: NOW - 20,
        expires_at: NOW + 200,
        revocation_handle: encode_base64url(&[id; 16]),
        claim: Claim::Source(SourceEventClaim {
            claim_type: p.source_schema_id.clone(),
            value: category.into(),
            source_schema_version: p.source_schema_version.clone(),
            context: p.context_id.clone(),
            occurred_at,
        }),
    }
}
#[test]
fn current_oss_fixture() {
    let p = policy();
    let mut e = Vec::new();
    for id in 1..=3 {
        e.push(source(&p, id, "merged_contribution", NOW - 50));
    }
    for id in 4..=5 {
        e.push(source(&p, id, "review", NOW - 50));
    }
    let r = evaluate(&p, &e, NOW).unwrap();
    assert_eq!(r.score, 40);
    assert_eq!(r.thresholds, BTreeMap::from([(40, true)]));
}
#[test]
fn freelance_service_fixture() {
    let p: Policy = serde_json::from_str(include_str!(
        "../../../docs/examples/policies/freelance-service.json"
    ))
    .unwrap();
    let e = source(&p, 1, "service_completed", NOW - 10);
    assert_eq!(evaluate(&p, &[e], NOW).unwrap().score, 20);
}
#[test]
fn context_separation() {
    let p = policy();
    let mut e = source(&p, 1, "review", NOW);
    if let Claim::Source(c) = &mut e.claim {
        c.context = "other".into();
    }
    assert!(evaluate(&p, &[e], NOW).is_err());
}
#[test]
fn subject_mismatch() {
    let p = policy();
    let e = source(&p, 1, "review", NOW);
    let mut other = source(&p, 2, "review", NOW);
    other.subject_key.x = encode_base64url(&[2; 32]);
    assert!(evaluate(&p, &[e, other], NOW).is_err());
}
#[test]
fn duplicate_identical() {
    let p = policy();
    let e = source(&p, 1, "review", NOW);
    let r = evaluate(&p, &[e.clone(), e], NOW).unwrap();
    assert_eq!(r.score, 5);
    assert_eq!(r.audit.excluded_by_reason["identical_duplicate"], 1);
}
#[test]
fn conflicting_duplicate() {
    let p = policy();
    let a = source(&p, 1, "review", NOW);
    let b = source(&p, 1, "mentorship", NOW);
    assert!(evaluate(&p, &[a, b], NOW).is_err());
}
#[test]
fn conflict_in_unsigned_metadata_also_rejected() {
    let p = policy();
    let a = source(&p, 1, "review", NOW);
    let mut b = a.clone();
    b.expires_at += 1;
    assert!(evaluate(&p, &[a, b], NOW).is_err());
}
#[test]
fn window_boundaries() {
    let p = policy();
    let a = source(&p, 1, "review", NOW - p.window_seconds);
    let b = source(&p, 2, "review", NOW - p.window_seconds + 1);
    let c = source(&p, 3, "review", NOW);
    let r = evaluate(&p, &[a, b, c], NOW).unwrap();
    assert_eq!(r.score, 10);
    assert_eq!(r.audit.excluded_by_reason["outside_window"], 1);
    assert!(evaluate(&p, &[source(&p, 4, "review", NOW + 1)], NOW).is_err());
}
#[test]
fn window_before_epoch() {
    let mut p = policy();
    p.window_seconds = NOW + 1;
    assert_eq!(
        evaluate(&p, &[source(&p, 1, "review", 0)], NOW)
            .unwrap()
            .score,
        5
    );
}
#[test]
fn unknown_category() {
    let p = policy();
    assert!(evaluate(&p, &[source(&p, 1, "untrusted", NOW)], NOW).is_err());
}
#[test]
fn wrong_issuer_schema_version_and_kind() {
    for field in 0..4 {
        let p = policy();
        let mut e = source(&p, 1, "review", NOW);
        match field {
            0 => e.issuer_id = "other".into(),
            1 => {
                if let Claim::Source(c) = &mut e.claim {
                    c.claim_type = "other".into();
                }
            }
            2 => {
                if let Claim::Source(c) = &mut e.claim {
                    c.source_schema_version = "2".into();
                }
            }
            _ => e.kind = CredentialKind::Attestation,
        }
        assert!(evaluate(&p, &[e], NOW).is_err());
    }
}
#[test]
fn credential_valid_at_as_of() {
    for field in 0..2 {
        let p = policy();
        let mut e = source(&p, 1, "review", NOW);
        if field == 0 {
            e.expires_at = NOW;
        } else {
            e.issued_at = NOW + 1;
        }
        assert!(evaluate(&p, &[e], NOW).is_err());
    }
}
#[test]
fn malformed_policy() {
    for field in 0..10 {
        let mut p = policy();
        match field {
            0 => p.context_id = " ".into(),
            1 => p.score_cap = 0,
            2 => p.window_seconds = 0,
            3 => p.operator = "lte".into(),
            4 => p.supported_thresholds = vec![],
            5 => p.supported_thresholds = vec![40, 40],
            6 => p.supported_thresholds = vec![101],
            7 => p.category_rules.clear(),
            8 => p.category_rules.values_mut().next().unwrap().max_events = 0,
            _ => p.issuer_id = "".into(),
        }
        assert!(p.validate().is_err());
    }
}
#[test]
fn safe_integer_overflow() {
    for field in 0..5 {
        let mut p = policy();
        match field {
            0 => p.score_cap = MAX_SAFE_INTEGER + 1,
            1 => p.window_seconds = MAX_SAFE_INTEGER + 1,
            2 => p.category_rules.values_mut().next().unwrap().weight = MAX_SAFE_INTEGER + 1,
            3 => p.category_rules.values_mut().next().unwrap().max_events = MAX_SAFE_INTEGER + 1,
            _ => p.supported_thresholds = vec![MAX_SAFE_INTEGER + 1],
        }
        assert!(p.validate().is_err());
    }
}
#[test]
fn bounded_products_cannot_overflow() {
    let mut p = policy();
    p.category_rules.get_mut("review").unwrap().weight = MAX_SAFE_INTEGER;
    p.category_rules.get_mut("review").unwrap().max_events = MAX_SAFE_INTEGER;
    let e: Vec<_> = (1..=10).map(|id| source(&p, id, "review", NOW)).collect();
    assert_eq!(evaluate(&p, &e, NOW).unwrap().score, 100);
}
#[test]
fn deterministic_output() {
    let p = policy();
    let mut e = vec![
        source(&p, 1, "review", NOW),
        source(&p, 2, "mentorship", NOW),
    ];
    let a = evaluate(&p, &e, NOW).unwrap();
    e.reverse();
    assert_eq!(a, evaluate(&p, &e, NOW).unwrap());
    assert_eq!(p.digest().unwrap(), p.digest().unwrap());
    decode_fixed::<32>(&p.digest().unwrap()).unwrap();
}
#[test]
fn threshold_set_and_empty_evidence() {
    let mut p = policy();
    p.supported_thresholds = vec![100, 0, 40, 5];
    let r = evaluate(&p, &[source(&p, 1, "review", NOW)], NOW).unwrap();
    assert_eq!(
        r.thresholds,
        BTreeMap::from([(0, true), (5, true), (40, false), (100, false)])
    );
    let empty = evaluate(&p, &[], NOW).unwrap();
    assert_eq!(empty.score, 0);
    assert!(empty.thresholds[&0]);
    assert!(!empty.thresholds[&5]);
}
#[test]
fn caps_and_zero_weight() {
    let mut p = policy();
    let e: Vec<_> = (1..=10).map(|id| source(&p, id, "review", NOW)).collect();
    let r = evaluate(&p, &e, NOW).unwrap();
    assert_eq!(r.score, 20);
    assert_eq!(r.audit.excluded_by_reason["category_cap"], 6);
    p.category_rules.get_mut("review").unwrap().weight = 0;
    assert_eq!(evaluate(&p, &e, NOW).unwrap().score, 0);
}
#[test]
fn strict_and_pinned() {
    let p = policy();
    let bytes = canonicalize(&p).unwrap();
    assert!(Policy::parse(&bytes, &p.digest().unwrap()).is_ok());
    assert!(Policy::parse(&bytes, "other").is_err());
    let mut v = serde_json::to_value(&p).unwrap();
    v["extra"] = true.into();
    assert!(parse_canonical::<Policy>(&canonicalize(&v).unwrap()).is_err());
    let mut v = serde_json::to_value(&p).unwrap();
    v["category_rules"]["review"]["extra"] = true.into();
    assert!(parse_canonical::<Policy>(&canonicalize(&v).unwrap()).is_err());
    let duplicate = String::from_utf8(bytes)
        .unwrap()
        .replace("\"review\":{", "\"review\":{\"weight\":5,\"weight\":5,");
    assert!(parse_canonical::<Policy>(duplicate.as_bytes()).is_err());
}
#[test]
fn all_vertical_examples() {
    for name in [
        "freelance-service",
        "business-vendor",
        "community-contribution",
        "grant-eligibility",
        "oss-contribution",
        "marketplace-fulfillment",
        "organization-membership",
        "payment-receipt",
    ] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../docs/examples/policies/{name}.json"));
        let p: Policy = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        p.validate().unwrap();
        let category = p.category_rules.keys().next().unwrap();
        assert!(
            evaluate(&p, &[source(&p, 1, category, NOW)], NOW)
                .unwrap()
                .score
                > 0
        );
    }
}
