---
name: zerant-protocol
description: Credential, disclosure, reputation, trust, revocation, and protocol-format rules for Zerant.
---
# Zerant Protocol
Use when changing credential formats, reputation rules, disclosure requests/responses, signatures, revocation, or trust policies.

Read docs/specs before code.

Rules:
- M1 uses atomic signed credentials/attestations, not ZK selective disclosure.
- Do not invent cryptographic primitives.
- Signed bytes, canonical serialization, domain labels, and versioning must be explicit.
- Requests bind verifier origin, request ID, challenge, nonce, issued/expiry times, and requested result.
- Responses contain only approved requested data.
- Threshold reputation verification requires a verifiable issuer-backed attestation in M1.
- Exact local score stays private for threshold-only requests.
- Revocation freshness failures fail closed.
- Protocol changes require a versioned spec update.
