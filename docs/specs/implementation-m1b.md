# M1B implementation — Rust credential core

Status: implemented and locally validated.

## What M1B implements

Two Rust crates now form the first executable Zerant protocol layer:

- `zerant-core` — strict encoding, RFC 8785 JCS canonicalization, safe-integer rules, base64url validation, Unix-second validity checks, and canonical verifier-origin syntax.
- `zerant-credential` — atomic source credentials, audience-bound threshold attestations, issuer trust, compact JWS signing/verification, and signed revocation snapshots.

Rust is the protocol source of truth. The web preview does not reimplement these verification semantics.

## Cryptographic profile

Zerant does not implement signature primitives itself.

- JWS implementation: `josekit 0.10.3`
- JOSE protected algorithm: `alg = "EdDSA"`
- key type: OKP / Ed25519
- signed payload serialization: RFC 8785 JCS via `serde_json_canonicalizer 0.3.2`
- revocation digest: SHA-256 of the decoded random 16-byte revocation handle

Protected JWS headers are deliberately restricted to exactly `alg`, `kid`, and message-specific `typ`. Unknown headers, alternate algorithms, malformed compact forms, and attacker-supplied key URLs are rejected. JOSE header JSON itself is not required to be JCS; JCS applies to Zerant signed payloads.

## Issuer authorization scope

A trusted signing key is not a blanket authorization. The trust manifest pins each issuer's allowed claim types, contexts, source-schema versions/categories, and reputation policies/digests/supported thresholds. Verification rejects a correctly signed credential that exceeds that authorization.

Credential and revocation issuance timestamps must also fall inside the signing key's configured validity interval. Threshold attestations are limited to 24 hours.

## Implemented fail-closed checks

Credential verification rejects:

- unknown, duplicate, compromised, inactive, or wrong issuer-key trust entries
- altered signatures or payloads
- unsupported schemas/kinds/claim shapes
- unknown wire fields
- private or malformed subject JWKs
- noncanonical signed payload JSON
- floats and integers outside the interoperable JSON safe-integer range
- expired/future/invalid credential intervals
- incorrect holder-local or verifier audience
- stale, malformed, rolled-back, or wrongly signed revocation snapshots
- revoked credential handles
- oversized compact-JWS input

Revocation snapshot version equal to the caller's persisted minimum is accepted when fresh; lower versions are rejected. This allows repeated verification against the current snapshot without requiring a new issuer publication.

## Deliberate limits

M1B does **not** implement:

- holder key generation or proof-of-possession enrollment
- encrypted holder vault/storage (implemented later in `/vault`; enrollment/key issuance remains separate)
- reputation-policy evaluation
- disclosure requests or holder response signing
- verifier replay-state persistence
- authenticated browser-origin transport
- multi-issuer private aggregation
- Zcash wallet/testnet integration
- zero-knowledge or anonymous credentials

Signing helpers exist for local issuer/demo/test-vector use. Applications remain responsible for private-key storage and lifecycle.

## Validation

The implementation passes:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Current Rust test coverage includes 5 core unit tests, 19 credential unit tests, and 9 integration/vector tests, including the independently specified RFC 8037 Ed25519 compact-JWS validation vector.

The fixtures under `crates/zerant-credential/tests/fixtures` are test material only and must never be reused as production keys.
