# Disclosure v0.1

Status: proposed M1 wire contract. One request asks for one result and yields at most one atomic credential.

## Request

The signed `request` object has exactly these fields: `schema: "zerant.disclosure.request.v0.1"`, random 256-bit base64url `request_id`, `verifier_id`, `verifier_key_id`, canonical `origin`, nonempty `purpose`, nonempty sorted `accepted_issuer_ids`, `claim_type`, optional `context`, optional `policy_id`/`policy_version`/integer `threshold`/`operator: "gte"`, random 256-bit `challenge`, distinct random 256-bit `nonce`, `created_at`, and `expires_at`. A threshold request has `claim_type: "reputation.threshold"` and all policy fields. Other requests omit the policy fields. Request lifetime is at most five minutes.

The request envelope is `{ "request": ..., "signature": { "alg": "Ed25519", "value": string } }`. The verifier signs UTF-8 `zerant-disclosure-request-v0.1\n` plus RFC 8785 JCS serialization of `request`. Binary values use unpadded base64url. Unknown fields, duplicate JSON keys, and alternate algorithms are rejected.

Holder and verifier both require the signed `origin` to equal an independently authenticated verifier origin and the verifier key registered for that origin. A claimed origin string alone is insufficient. The verifier stores the outstanding request, challenge, nonce, and expiration before presenting it. No wildcard origins; HTTPS is required outside loopback local development.

## Consent and response

The holder UI displays the authenticated origin, purpose, exact claim or predicate, issuer, and full response preview. Approval is an explicit action for this request only. Denial sends either nothing or a constant-shape `declined` status without credential, subject key, claim, score, or explanatory reason. No analytics event may include denied data.

On approval, `response` contains `schema: "zerant.disclosure.response.v0.1"`, `request_id`, `origin`, `challenge`, `nonce`, `request_digest`, `credential` (the full atomic signed envelope), `responded_at`, and `holder_signature`. `request_digest` is unpadded base64url SHA-256 of UTF-8 `zerant-disclosure-request-v0.1\n` plus JCS of the request. `holder_signature` is an Ed25519 signature with the credential's `subject_key` over UTF-8 `zerant-disclosure-response-v0.1\n` plus JCS of the response excluding `holder_signature`. Unknown fields and duplicate JSON keys are rejected.

The response contains no other credentials, local score, source events, wallet, or Zcash data. A threshold response's `credential.payload.claim.value` is only `true`. A holder who cannot match the exact requested credential denies or reports a generic unavailable state without naming other credentials.

## Verifier validation

Check authenticated own origin and outstanding signed request; compare request ID, digest, origin, challenge, nonce, and expiration; verify response signature and subject binding; verify issuer allowlist and request's accepted issuer IDs, credential signature, audience, exact claim selector/policy/threshold, credential time, and current signed revocation list; then atomically consume the nonce before accepting. Every failure is nonacceptance. Consumed or unknown nonces fail, including on replay to another domain. Keep nonce state at least through request expiry plus a documented clock-skew window; local M1 may use a persistent store. An unsigned denial is never a successful proof.
