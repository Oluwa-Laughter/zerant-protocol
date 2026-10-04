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

Production only:

- `ZERANT_PUBLIC_ORIGIN`: `https://zerant-protocol.vercel.app` or the final custom production origin.

If `ZERANT_PUBLIC_ORIGIN` is absent, preview deployments use Vercel's deployment URL automatically.

Do not create `ZERANT_API_ORIGIN` manually. Vercel injects it from the `web -> backend` service binding.

Do not expose any of these with a `NEXT_PUBLIC_` prefix.

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
