# Credential v0.1

Status: proposed M1 wire contract. A signed credential contains exactly one atomic claim; there is no hidden-field proof.

## Canonical payload

The `payload` object contains only the fields below. Unknown fields, duplicate JSON keys, floats, and invalid encodings are rejected. IDs and nonces are cryptographically random 256-bit bytes encoded as unpadded base64url. Times are UTC RFC 3339 strings; implementations compare parsed instants.

| Field | Type | Meaning |
| --- | --- | --- |
| `schema` | string | Exactly `zerant.credential.v0.1` |
| `credential_id` | string | Unique random identifier; never reused |
| `issuer_id` | string | Stable identifier in the local issuer trust manifest |
| `issuer_key_id` | string | Exact Ed25519 key reference in that manifest |
| `subject_key` | string | Unpadded base64url Ed25519 public key controlled by holder, unique per verifier origin |
| `audience` | string | Canonical verifier origin (`scheme://host[:port]`); no path or wildcard |
| `issued_at`, `expires_at` | string | Issuance and expiration; expiry strictly after issuance |
| `revocation_handle` | string | Random 256-bit private-to-credential revocation handle |
| `claim` | object | Exactly one typed claim, defined below |

`claim` has required `type` and `value` fields; `value` is a string, boolean, or bounded integer. It may also have `context`, `policy_id`, and `policy_version` strings. Claim type names are registry-controlled. Source event claims require `context` and an integer `value`; other ordinary claims omit irrelevant fields. A `reputation.threshold` claim requires `value: true`, `context`, `policy_id`, `policy_version`, integer `threshold`, and `operator: "gte"`. No other fields are allowed for that variant. False predicates are not issued; an absent credential is not proof of false. Numeric scores and source events are forbidden in threshold credentials.

The signed envelope is `{ "payload": ..., "signature": { "alg": "Ed25519", "value": string } }`. `value` is the unpadded base64url signature over UTF-8 bytes of `zerant-credential-v0.1\n` followed by RFC 8785 JCS serialization of `payload`. The signature does not cover the envelope wrapper. Reject any alternative algorithm, malformed key, or signature. Ed25519 and JCS are specified by [RFC 8032](https://www.rfc-editor.org/rfc/rfc8032.html) and [RFC 8785](https://www.rfc-editor.org/rfc/rfc8785.html). Do not implement either primitive from scratch.

## Subject binding and trust

Issuer checks control of `subject_key` before issuance (challenge signature). A verifier requires a holder response signature by that exact key. Issuer ID and key ID resolve through an explicit allowlist; self-asserted keys are never enough. The audience and subject key are per origin, including distinct credentials for distinct verifiers. This incurs issuance overhead and does not conceal the target origin from the issuer.

## MVP revocation

The issuer publishes a signed, monotonic revocation-list snapshot with `issuer_id`, `issuer_key_id`, `version`, `issued_at`, `next_update`, and sorted digests. Each digest is SHA-256 of UTF-8 `zerant-revocation-v0.1\n` followed by the decoded 32-byte `revocation_handle`. The list envelope uses Ed25519 over `zerant-revocation-list-v0.1\n` plus JCS of the snapshot. The verifier checks list signature/key, version rollback protection, `next_update`, and absence of the digest. A missing or stale list fails closed. The random handle prevents guessing another credential's digest, but revocation metadata is public and not private lookup infrastructure.
