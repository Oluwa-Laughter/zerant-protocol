# Verifier result webhooks

Zerant can push final verification results to a verifier-owned HTTPS endpoint after a holder approves or denies a request.

Webhooks are intentionally narrow. They do **not** contain holder credentials, source evidence, wallet addresses, balances, transaction history, account sessions, or signing material.

## Registration

A signed-in verifier creates an endpoint from the **Result webhooks** section of the verifier workspace.

Each verifier can keep up to five active endpoints. The URL must:

- use HTTPS on port 443;
- have no embedded username or password;
- have no query string or fragment;
- resolve only to public IP addresses.

Zerant resolves and validates the destination again at delivery time and pins the HTTP client to the validated addresses. Redirects are disabled. This prevents a registered endpoint from becoming a generic internal-network request primitive.

The webhook signing secret is returned exactly once at creation and is encrypted at rest afterwards.

## Events

Current event types:

- `verification.approved`
- `verification.denied`

Example payload:

```json
{
  "schema": "zerant.webhook.event.v0.1",
  "event_id": "<delivery UUID>",
  "type": "verification.approved",
  "request_id": "<verification request UUID>",
  "credential_schema_id": "<credential definition UUID or null>",
  "status": "approved",
  "verified": true,
  "occurred_at": "2026-10-04T16:20:00Z"
}
```

A denied event has `status: "denied"` and `verified: false`.

Treat `event_id` as the delivery idempotency key.

## Delivery headers

Every delivery includes:

```text
Content-Type: application/json
User-Agent: Zerant-Webhook/1.0
X-Zerant-Event: verification.approved
X-Zerant-Delivery-Id: <delivery UUID>
X-Zerant-Timestamp: <unix seconds>
X-Zerant-Signature: v1=<hex HMAC-SHA256>
```

## Signature verification

The signed bytes are:

```text
<timestamp>.<exact raw request body>
```

Compute HMAC-SHA256 with the one-time webhook secret and compare it to the hexadecimal value after `v1=`.

Verification should:

1. read the raw request body before JSON parsing;
2. reject an old timestamp according to the application's replay window;
3. compute the HMAC over the exact body bytes;
4. compare signatures in constant time;
5. deduplicate by `X-Zerant-Delivery-Id`;
6. only then parse and process the JSON event.

Return any 2xx status after the event has been durably accepted by the verifier application.

## Reliability

Final holder decisions write webhook deliveries into PostgreSQL in the same transaction as the verification lifecycle change.

Production wake-up and retry execution uses Vercel Queues:

- durable at-least-once queue message;
- seven-day queue retention;
- idempotent publish per verification request;
- private queue-triggered consumer;
- bounded concurrent dispatch;
- database-backed delivery state;
- exponential retry schedule;
- eight application delivery attempts before the delivery becomes permanently failed.

PostgreSQL remains the audit/dead-letter source of truth. The verifier workspace shows pending and permanently failed delivery counts.

Because delivery is at-least-once, receivers must be idempotent even when Zerant normally succeeds on the first attempt.

## Failure behavior

- non-2xx responses are retried;
- network failures are retried;
- unsafe DNS/address changes disable the endpoint and permanently fail its queued deliveries;
- disabling an endpoint permanently stops its remaining pending deliveries;
- redirects are never followed;
- a delivery that reaches the maximum attempt count becomes `dead`.

Creating, disabling and delivery state are isolated per verifier. One failing webhook cannot block another verifier's request lifecycle.
