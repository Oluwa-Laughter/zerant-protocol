# Verifier integration API

Zerant exposes a server-to-server API for applications that need to create private verification requests programmatically.

The integration API is intentionally narrower than the signed-in verifier dashboard. Machine clients can create a request from an immutable managed credential definition and poll the resulting request status. They cannot read holder credential contents, vault records, wallet state, source evidence or holder signing material.

## Create an integration key

A signed-in verifier operator creates keys from the **Developer integration** section of the verifier workspace.

Each key has:

- a human-readable name;
- explicit scopes;
- optional expiry;
- a short display prefix;
- revocation state and last-used timestamp.

The full secret is returned exactly once at creation. Zerant stores only its SHA-256 hash.

Never place a verifier integration key in browser JavaScript, a mobile bundle, a public repository or client-side storage. Treat it as a server credential.

## Scopes

Supported scopes are deliberately small:

- `requests:create` — create a verification request;
- `requests:read` — read the status of a request created by the verifier account.

Unknown scopes fail closed. A key can be revoked immediately from the verifier workspace.

## Authentication

Send the key as an HTTP bearer token:

```http
Authorization: Bearer <verifier-integration-secret>
```

Session cookies are not accepted by the integration routes. The public Next.js integration proxy forwards the bearer token and request body only; it does not forward a Zerant browser session.

## Discover credential definitions

Applications should discover issuer and credential-definition metadata through the public trust endpoints:

```text
GET /api/zerant/public/issuers
GET /api/zerant/public/issuers/<issuer-id>
```

Use the immutable credential-definition `id` when creating machine verification requests. Programmatic verification requests are managed-schema only; free-text claim/context requests are rejected.

## Create a verification request

```http
POST /api/zerant/integrations/verifier/requests
Content-Type: application/json
Authorization: Bearer <secret>
```

Body:

```json
{
  "holder_zerant_id": "<recipient Zerant ID>",
  "purpose": "<why the application needs this proof>",
  "credential_schema_id": "<immutable credential-definition UUID>"
}
```

The schema pins its issuer, claim type and context. The request still goes through the normal Zerant request-signing, expiry, rate-limit, holder-consent and proof-generation path.

Successful creation returns the request ID, holder Zerant ID, purpose, credential definition, current status and five-minute expiry.

## Poll request status

```http
GET /api/zerant/integrations/verifier/requests/<request-id>
Authorization: Bearer <secret with requests:read>
```

Status is one of:

- `pending`
- `approved`
- `denied`
- `expired`

`verified` is true only after the holder approved and Zerant successfully revalidated the source credential, issuer trust, revocation state, request binding and holder response.

The status API does not return the holder's original credential or unrelated account data.

## Retrieve the signed proof package

Status polling deliberately does not return proof material. Create an integration key with the
separate `proofs:read` permission when your backend needs the signed verification result.

```http
GET /api/zerant/integrations/verifier/requests/<request-id>/proof
Authorization: Bearer <secret with proofs:read>
```

The endpoint succeeds only for an approved request that was completed after proof-package
evidence storage was introduced. Pending, denied, expired, or older incomplete requests fail
closed instead of returning a synthetic package.

The response uses `zerant.verifier-proof-package.v0.1` and contains:

- the database request UUID and protocol request ID;
- the original verifier-signed `request_jws`;
- the holder-signed verifier-specific `response_jws`;
- the issuer-signed revocation snapshot used when Zerant approved the proof;
- the exact issuer and verifier public signing keys used, including their lifecycle timestamps
  and compromise state;
- immutable managed credential-definition identity when the request used one;
- verifier origin, decision time, and the proof expiry.

It does **not** contain the holder Zerant ID, source credential, credential portfolio, account UUID,
wallet address, balance, transaction history, memo, seed, or private signing material.

A verifier that wants to validate the package independently should perform the same profile checks
as Zerant's native disclosure verifier:

1. verify `request_jws` with the packaged verifier key and confirm the expected verifier origin;
2. verify the packaged revocation snapshot with the packaged issuer key;
3. read the untrusted response payload only to locate its embedded `attestation_jws`;
4. verify that attestation against the issuer key, revocation snapshot, expected audience,
   credential definition, claim type/context, and validity interval;
5. verify `response_jws` with the pairwise holder public key carried by the now-verified
   attestation;
6. confirm request ID, challenge, nonce, request digest, origin, and response time bindings;
7. reject compromised signing keys and expired proof material.

The approval-time revocation snapshot proves what Zerant validated when the holder responded.
While a proof is still within its short validity window, a verifier that requires the freshest
issuer status can also fetch the issuer's current public revocation snapshot from the public trust
directory. The package never substitutes for the verifier's own replay/idempotency tracking.

## Limits and lifecycle

- Verification-request creation shares the verifier account's durable database-backed request quota.
- A verifier can have at most 20 active, unexpired integration keys.
- Key creation is serialized per verifier so concurrent requests cannot bypass that cap.
- Key use updates its last-used timestamp at most once every five minutes to avoid unnecessary database churn.
- Creation and revocation are written to the verifier's append-only Zerant activity history.
- Revoked or expired keys fail authentication immediately.
- Integration endpoints are `no-store` and are intended for server-to-server calls.

## Errors

Typical responses:

- `400` — malformed request, unmanaged/invalid schema input;
- `401` — missing, invalid, expired, revoked or insufficient-scope API key;
- `404` — holder, schema or request not found for that verifier;
- `409` — lifecycle conflict or active-key limit reached;
- `429` — account request quota exceeded;
- `503` — Zerant service or required dependency unavailable.

## Result webhooks

Verifiers can register public HTTPS webhook endpoints from the signed-in verifier workspace. Zerant emits final approved/denied request status through a durable outbox and Vercel Queue-backed retry path. Webhook payloads contain request status and managed credential-definition identity only; they do not contain holder credentials or wallet data.

See [Verifier result webhooks](VERIFIER_WEBHOOKS.md) for payload shape, HMAC verification, SSRF protections, idempotency and retry behavior.

Polling remains supported as a reconciliation path. The signed proof-package endpoint is the
cryptographic integration surface for approved requests; webhooks remain notification-only and
intentionally contain no holder proof material.
