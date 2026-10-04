# Compound disclosure v0.3

Implemented native all-of extension; v0.2 remains unchanged. This is signed minimal
disclosure, not zero knowledge, anonymity or unlinkability.

## Request and consent

The strict JCS payload has exactly `schema: zerant.disclosure.request.v0.3`,
`primary` (one atomic v0.2 Request), `additional` (1–7 atomic v0.2 Requests), and
`expected_values` (one entry per ordered requirement). Ordinary requirements specify
an exact string, boolean or safe integer; threshold entries are null and match the
complete pinned predicate. Payment requirements must request boolean true.

All atomic descriptions have identical verifier identity/key, authenticated origin,
purpose, challenge, nonce, issuance and expiry, and distinct requirement request IDs.
The primary ID is the compound replay ID. All conditions are mandatory. The JWS uses
exact protected alg=EdDSA, pinned verifier kid, typ=zerant-request-v0.3.

Consent covers the exact signed ordered list and selected evidence. Changes invalidate
approval. Denial emits no response. Missing evidence never triggers a source fallback.
All evidence uses the same audience subject key, with distinct credential IDs. This
links approved attestations within this request; pairwise keys do not guarantee
anonymity. Transport origin authentication remains separate from signed origin syntax.

## Response and verification

The response JCS has exactly schema=zerant.disclosure.response.v0.3, request_id,
request_digest, verifier_origin, challenge, nonce, ordered attestations, responded_at.
The holder signs using typ=zerant-response-v0.3 and kid=audience subject key x.
Verifier checks pinned request trust and persisted replay state, every authorized
issuer signature and fresh signed revocation snapshot, exact requirements/values,
common holder key, holder signature and every response binding before one atomic
replay consumption. Failed/partial satisfaction leaves the request pending. Oversized,
unknown, noncanonical and tampered data fail closed under existing JOSE/JCS limits.

## Payment binding

An invoice requirement is `payment.invoice_paid` with context
`payment-intent:<canonical intent SHA-256 base64url digest>` and expected value true.
The responsible issuer observes the exact recipient/amount/network/confirmations,
checks its ledger, and bounds attestation expiry to the invoice policy. Verifiers pin
that issuer's payment context authorization. Native receipt JSON alone is not proof.
Only the paid boolean and intent digest plus ordinary verification metadata are
presented; recipient, amount, txid, memo and wallet history are absent. The issuer
must revoke on reorgs and define retention; no eternal settlement guarantee exists.
