# Disclosure v0.2

Status: implemented native Rust protocol profile for signed single-result disclosure. It is signed minimal disclosure, not zero knowledge, anonymity, or cryptographic unlinkability.

## Request

A request is a compact JWS whose JCS payload has schema zerant.disclosure.request.v0.2. The protected header contains exactly alg=EdDSA, the pinned verifier kid, and typ=zerant-request-v0.2.

The payload contains a random 128-bit request_id, verifier_id, verifier_key_id, canonical verifier_origin, bounded purpose, sorted unique accepted_issuer_ids, one claim_type, independent random 256-bit challenge and nonce, and integer issued_at/expires_at with a lifetime no greater than 300 seconds.

An ordinary request may include context. A reputation.threshold request instead carries one predicate containing context, policy_id, policy_version, 32-byte policy_digest, supported integer threshold, and operator=gte. v0.2 adds policy_digest so an immutable policy cannot be substituted while reusing an ID/version label. v0.1 remains a historical draft and is not the executable Rust profile.

The native verifier trust input pins verifier ID, Ed25519 key, allowed origins, validity interval and compromise status. No request may nominate a remote key URL or new trust root. The caller must separately authenticate the browser/transport origin; the signed JSON origin string is not transport authentication.

## Matching and consent

A request can match only one already verified attestation. Private source credentials are not presentation material.

Ordinary requests require exact claim type and optional context. Threshold requests require exact claim type, context, policy ID, version, digest, threshold and operator. The attestation audience must equal the verifier origin and its issuer must be accepted by the request.

Denial produces no credential response. Missing or mismatched evidence is unavailable/error, never a false eligibility assertion and never a trigger to upload source evidence.

## Response

An approved response is compact JWS with alg=EdDSA, kid equal to the attestation subject public-key x, and typ=zerant-response-v0.2.

Its JCS payload contains exactly schema zerant.disclosure.response.v0.2, request ID, verifier origin, challenge, nonce, request_digest (SHA-256 of the exact ASCII compact request JWS), exactly one attestation_jws, and integer responded_at.

The holder signs with the exact audience-specific subject private key bound to the attestation. The response contains no exact local score, source event, wallet address, balance, transaction history, memo, or extra credential.

## Verification and replay

Acceptance verifies the signed request and independently authenticated origin; a persisted pending replay row with exact request ID/digest/origin/expiry; issuer credential signature, authorization, expiry and fresh signed revocation; exact request-to-attestation matching; holder response signature; exact response bindings; then atomically consumes the request.

Any failed check leaves the request unconsumed. Duplicate and concurrent consumption fail closed. The reference SQLite store persists only request ID, digest, origin, expiry and status; it stores no response body, score or credential portfolio.

Revocation watermarks, authenticated browser sessions, holder key/vault storage and clock health remain application/deployment responsibilities.
