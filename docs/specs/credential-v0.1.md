# Credential v0.1

Status: proposed M1 wire contract. A credential is one atomic signed claim. This is signed minimal disclosure, not a hidden-field proof.

## Common encoding and envelope

The payload is UTF-8 JSON canonicalized with RFC 8785 JCS and signed as a compact JWS. The protected header contains exactly `alg: "EdDSA"`, `kid`, and `typ: "zerant-credential-v0.1"`; use the standard JWS signing input and a maintained JOSE library. The `kid` equals `issuer_key_id`. The key is an allowlisted Ed25519 public key. Reject noncanonical payloads, duplicate keys, floats, unsafe integers, unknown fields, alternate algorithms, and malformed encodings. See [protocol encoding](../PROTOCOL.md) and [RFC 9864](https://www.rfc-editor.org/rfc/rfc9864.html).

All times are integer UTC Unix seconds. IDs are independent 128-bit CSPRNG bytes encoded unpadded base64url. Public keys are Ed25519 public JWKs with `kty: "OKP"`, `crv: "Ed25519"`, and `x`; private `d` is forbidden on the wire.

| Payload field | Meaning |
| --- | --- |
| `schema` | Exactly `zerant.credential.v0.1` |
| `kind` | `source` or `attestation` |
| `credential_id` | Fresh random identifier; never reused |
| `issuer_id`, `issuer_key_id` | Exact entries in a pinned issuer trust manifest |
| `subject_key` | Holder Ed25519 public JWK verified at issuance |
| `audience` | `holder-local` for private source evidence, or a canonical verifier origin for an attestation |
| `issued_at`, `expires_at` | Validity interval with `expires_at > issued_at`; `issued_at` is the start of validity |
| `revocation_handle` | Fresh random 128-bit handle for this credential |
| `claim` | Exactly one typed claim |

`claim` requires `type` and `value` (string, boolean, or safe integer). For a source event, `type` is the policy's `source_schema_id`; `value` is the registered category string; and `source_schema_version`, `context`, and integer `occurred_at` are required. The source `credential_id` is its stable event ID. An ordinary attestation contains only its required claim fields. A `reputation.threshold` attestation requires `value: true`, `context`, `policy_id`, `policy_version`, unpadded base64url `policy_digest` (SHA-256 of pinned JCS policy bytes), integer `as_of` equal to `issued_at`, safe integer `threshold` from that policy's supported thresholds, and `operator: "gte"`. False predicates are not issued; absence is not proof of false. It contains no numeric score or source event ID.

Source credentials use a separate local holder key and `audience: "holder-local"`. They MUST NOT appear in verifier responses. Attestations use a fresh key per verifier origin and that origin as audience. The issuer checks holder control of the relevant key by challenge signature and checks the audience enrollment before issuance. Verifier validation requires a holder response signature by the exact attestation subject key. Issuer IDs alone do not establish trust.

## Revocation

The issuer publishes a compact JWS with protected `alg: "EdDSA"`, issuer `kid`, and `typ: "zerant-revocation-v0.1"`. Its JCS payload contains `schema: "zerant.revocation.v0.1"`, `issuer_id`, `issuer_key_id`, monotonically increasing integer `version`, `issued_at`, `next_update`, and sorted unique `revoked_digests`. Each digest is unpadded base64url SHA-256 of the decoded 16-byte random handle. Require `issued_at < next_update <= issued_at + 86400`. A verifier validates the JWS and issuer key, rejects future/expired/rolled-back snapshots, and checks that the presented handle digest is absent. Missing or stale snapshots fail closed. Publishing whole signed snapshots locally avoids credential-specific online lookups; a revocation digest can still correlate reuse of the same attestation within one origin.
