# Vercel deployment

Zerant deploys as one Vercel project with two services and one managed Postgres database.

- `web`: Next.js customer application.
- `backend`: private Rust/Axum container service.
- Neon: managed PostgreSQL installed from the Vercel Marketplace.

The web service reaches the backend through a private Vercel service binding. The backend has no public rewrite.

## Vercel project settings

The Vercel project must use the repository root, not `apps/web`, as its Root Directory.

Set the Framework Preset to **Services**. The root `vercel.json` declares both services and the private binding.

## Database

Install **Neon** from the Vercel Marketplace and connect it to this project. The integration provides `DATABASE_URL` to the deployment.

The backend uses TLS by default and runs the Zerant schema migrations at startup under a PostgreSQL advisory lock so concurrent cold starts cannot race migrations.

## Required project environment variables

Set these in Vercel Project Settings -> Environment Variables.

- `ZERANT_VAULT_KEK_B64`: 32 random bytes encoded as URL-safe base64 without padding. Treat as a sensitive secret.
- `ZERANT_VAULT_KEY_VERSION`: `1` for the current key.
- `ZERANT_DATABASE_TLS`: `require`.
- `ZERANT_ZCASH_CHAIN`: `zcash:testnet` while the product remains on testnet.
- `ZERANT_ZECAUTH_SCOPES`: `auth,request_payment`.

Optional Zcash network connectivity:

- ZERANT_LIGHT_CLIENT_ENDPOINT: HTTPS endpoint for a trusted Zaino or lightwalletd-compatible service.
- ZERANT_LIGHT_CLIENT_ALLOW_LOOPBACK: keep false in Vercel; this exists only for deliberate local testing.

Zerant validates the configured endpoint at startup, requires HTTPS for remote services, rejects embedded credentials, query strings, fragments and paths, bounds gRPC response sizes, and checks that the remote chain matches ZERANT_ZCASH_CHAIN.

Production only:

- `ZERANT_PUBLIC_ORIGIN`: `https://zerant.vercel.app` or the final custom production origin.

If `ZERANT_PUBLIC_ORIGIN` is absent, preview deployments use Vercel's deployment URL automatically.

Do not create `ZERANT_API_ORIGIN` manually. Vercel injects it from the `web -> backend` service binding.

Do not expose any of these with a `NEXT_PUBLIC_` prefix.



## Vault key rotation

Zerant supports multiple decryption-key versions. New records use `ZERANT_VAULT_KEY_VERSION`; existing records keep their recorded version until migrated. Rotation is an operator maintenance task. It has no HTTP endpoint or customer setting.

For an old version `1` and new version `2`:

1. Preserve a secure backup of the database and current key material under the existing secret-management policy. Generate a new independent 32-byte key as described below. Do not print keys in maintenance output.
2. Set `ZERANT_VAULT_KEYS_B64` to a JSON object containing both versions, for example `{"1":"<old>","2":"<new>"}`. Keep the old key unchanged. Set `ZERANT_VAULT_KEY_VERSION=2`, deploy, and ensure **all** backend instances use version 2 before migrating. Confirm an old credential and a newly stored credential are readable.
3. From a trusted maintenance environment with the deployed backend binary, a TLS `DATABASE_URL`, and exactly the same `ZERANT_VAULT_KEYS_B64` and `ZERANT_VAULT_KEY_VERSION`, run `zerant-api vault-rotation status 1`. Restrict shell, process environment and database access to operators; do not run this through a public web route. The command prints counts only.
4. Run `zerant-api vault-rotation batch 1 100` repeatedly. Each invocation migrates at most 100 rows in one encrypted record family and commits one transaction. A smaller limit from 1 to 100 is allowed. If any invocation fails, preserve both keys, investigate the affected row without dumping its contents, and retry after repair. A failed batch rolls back; completed batches remain committed. Concurrent writers or a stopped command are safe to resume by repeating the command.
5. Run `zerant-api vault-rotation status 1` after the final batch. Verify **every** family reports zero, including `verification_responses` and retired signing keys. Repeat after a normal write interval to catch a stale backend still writing version 1. If a count rises, update the stale instance and resume batches.
6. Only after zero references remain and no old-version writer exists, remove version `1` from `ZERANT_VAULT_KEYS_B64` and redeploy. The active version must remain present. `ZERANT_VAULT_KEK_B64` may be removed once the versioned keyring contains the active key. Keep the secure backup according to retention policy.

The migration decrypts and re-encrypts each payload with a fresh DEK and nonces because v1 data authentication binds the old version. It covers credential envelopes, account credential keys, issuer profiles, issuer signing keys, verifier profiles, verifier signing keys, holder pairwise keys, webhook secrets, and encrypted verification responses. Missing historical keys or tampered records fail closed. There is no partial-row update: each batch is transactional. Counts are an operational retirement check; a stale instance can create new old-version rows after a check.

This keyring is an interim production-hardening mechanism. Managed KMS/HSM custody remains the target for stronger operational isolation.

## Not required yet

Do not configure `Z3_REGTEST_RPC_ROUTER_USER` or `Z3_REGTEST_RPC_ROUTER_PASSWORD` in production yet. They belong to the currently exercised local/regtest Z3 boundary, not a production Zcash operator.

## Verification after deployment

Open:

`https://<your-production-domain>/api/zerant/health`

A healthy deployment returns JSON indicating the Zerant service is available and using PostgreSQL-backed storage.

Then verify the product flow:

1. Connect a Zcash identity.
2. Copy the generated Zerant ID from Credentials.
3. Create an issuer profile and issue a credential to that Zerant ID.
4. Confirm the credential appears in the holder account.
5. Create a verifier profile and send a narrow verification request.
6. Approve the request from the holder Requests page.
7. Confirm the verifier request becomes Verified.

## Secret generation

Generate the vault key locally and put the result directly into Vercel. Do not commit it or send it through chat:

```bash
python3 - <<'PY'
import base64, secrets
print(base64.urlsafe_b64encode(secrets.token_bytes(32)).decode().rstrip('='))
PY
```

Production key custody should later move from an environment secret to a managed KMS/HSM. That is an operational-hardening step, not a reason to expose key material to the browser.

## Vercel Queues

Verifier result webhooks use a private queue-triggered Next.js route declared under the `web` service in `vercel.json`. Vercel provides queue authentication and delivery; no public consumer URL or `CRON_SECRET` is required.

The queue message contains only the verification request UUID. Webhook payloads and delivery state remain in PostgreSQL. This makes Queue a wake-up/retry layer rather than a second source of truth.

The current queue message retention is seven days, with bounded concurrency and application-level dead-delivery tracking.
