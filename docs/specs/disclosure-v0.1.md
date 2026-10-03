# Disclosure v0.1

Status: proposed M1 wire contract. One request asks for one result and approval returns one atomic attestation. Private source credentials are never response material.

## Request

`request_jws` is compact JWS over a JCS payload. Its protected header has exactly `alg: "Ed25519"`, the allowlisted verifier `kid`, and `typ: "zerant-request-v0.1"`. The payload has `schema: "zerant.disclosure.request.v0.1"`, independent random 128-bit `request_id`, `verifier_id`, `verifier_key_id` matching `kid`, canonical `verifier_origin`, nonempty `purpose`, nonempty sorted `accepted_issuer_ids`, `claim_type`, independent random 256-bit `challenge` and `nonce`, integer `issued_at`, and integer `expires_at`. `expires_at` is no more than 300 seconds after `issued_at`.

An ordinary claim request may include `context`; it omits policy fields. A threshold request sets `claim_type: "reputation.threshold"` and requires `context`, `policy_id`, `policy_version`, safe integer `threshold`, and `operator: "gte"`. All other fields are rejected. Binary IDs/challenge/nonce are unpadded base64url. The verifier persists the exact JWS and outstanding request before sending it.

`verifier_origin` is the browser's ASCII serialized origin: scheme and lowercase host, plus a nondefault port if present, with no path, query, fragment, credentials, or trailing slash. The holder verifies the request signature and pinned verifier key, then requires byte-for-byte equality with an independently authenticated origin from the browser transport. A string in a pasted request is not origin authentication. Production requires HTTPS; M1 allows explicitly configured loopback origins. No wildcard origin or untrusted remote key URL is accepted.

## Consent and response

The holder displays the authenticated origin, purpose, exact requested claim or threshold, selected issuer, attestation expiry, and full outbound payload. Approval is explicit for this request only. Denial sends no response and no credential data. Missing matching evidence yields a local generic unavailable state; it never triggers source disclosure.

On approval, `response_jws` is compact JWS signed with the attestation's audience-specific holder key. Its protected header has exactly `alg: "Ed25519"`, `kid` equal to that key's public JWK `x`, and `typ: "zerant-response-v0.1"`. Its JCS payload has `schema: "zerant.disclosure.response.v0.1"`, `request_id`, `verifier_origin`, `challenge`, `nonce`, `request_digest`, `attestation_jws`, and integer `responded_at`. `request_digest` is unpadded base64url SHA-256 of the exact ASCII compact `request_jws`. The response contains no other credentials, exact score, source events, wallet data, or explanatory denial reason. For threshold requests the attestation has only the signed `true` predicate and necessary protocol metadata.

## Verification and replay

The verifier checks its authenticated own origin and persisted request; validates request JWS; compares request ID, digest, origin, challenge, nonce, and time; validates response JWS against the attestation's subject key and header `kid`; validates the attestation JWS against a pinned issuer key and requested accepted issuer; checks audience, exact claim/policy/threshold and pinned policy digest, validity, and a fresh signed revocation snapshot. It then atomically transitions the request from pending to consumed before accepting. Any failed check rejects the response. Concurrent duplicates and responses after verifier restart cannot both succeed. Persist nonce/request state through expiration plus clock tolerance; after a state reset, all prior requests fail closed. Cross-origin replay fails origin, audience, request digest, and key checks.
