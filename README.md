# Zerant Protocol

**Prove trust. Preserve privacy.**

Zerant is a privacy-preserving trust layer for Zcash applications. It lets organizations issue trusted credentials, lets people keep those credentials private, and lets applications verify a specific claim without collecting the holder's entire identity, credential history or wallet activity.

Zerant is designed for memberships, contributor history, roles, achievements, grants, marketplaces, business relationships, professional credentials, community access and other trust decisions. Payment conditions are optional and remain separate from generic identity.

## What Zerant solves

Most applications collect far more information than the decision requires. A service that only needs to know whether someone is eligible may end up storing names, accounts, wallet addresses, transaction history and unrelated credentials.

Zerant changes the exchange:

1. **Issuer** — an organization creates a trusted private credential.
2. **Holder** — the recipient keeps that credential private under a Zerant ID.
3. **Verifier** — an application asks for one specific fact and explains why it needs it.
4. **Consent** — the holder approves or denies the request.
5. **Proof** — Zerant returns a short-lived verifier-specific result. The original credential and unrelated evidence stay out of the response.

There is no universal social-credit score. Reputation and eligibility remain contextual.

## Product surfaces

- /vault — private credentials, Zerant ID and credential history.
- /issuer — create an issuer profile and deliver credentials to a Zerant ID.
- /verifier — register an application and request a narrow proof from a holder.
- /requests — holder inbox for approving or denying verification requests.
- /app — product workspace plus Zcash address/payment-request review tools.

The product does not seed demo credentials, identities, payments or verification results.

## Implemented

- Strict canonical encoding, IDs, timestamps, origin binding and safe-integer rules.
- Signed issuer credentials with audience, expiry, trust scope and revocation support.
- Contextual deterministic policy evaluation without a universal reputation score.
- Signed verifier requests with domain, purpose, challenge, nonce, expiry and replay binding.
- Holder consent with denial producing no credential response.
- Verifier-specific audience-bound attestations derived from private source credentials.
- Protected account credential storage and opaque authenticated sessions.
- Issuer registration, private credential issuance and recipient delivery by Zerant ID.
- Verifier registration, trusted-issuer selection, short-lived requests and verified status.
- Zcash-native authentication separation from payment authority.
- Canonical Zcash address inspection and ZIP-321 payment-request review.
- Read-only Z3/Zallet capability and settlement-observation boundaries.
- PCZT/FROST coordination interfaces remain capability-gated; Zerant does not implement custom threshold cryptography.

## Security and privacy status

The current server-first product encrypts credential records at rest with per-record data keys wrapped by a versioned service key. The browser is not the credential or session source of truth.

This is **not end-to-end holder-only encryption**: an authorized Zerant service runtime can decrypt a holder record in order to serve the holder and construct an approved proof. Production deployment therefore requires strict service isolation, managed KMS/HSM custody, audit controls and careful backup access. Verifiers and other Zerant users do not receive the holder's private credential portfolio.

No zero-knowledge, anonymity or unlinkability claim is made yet. Pairwise verifier identity, stronger unlinkable credentials, managed KMS/HSM deployment, production revocation distribution, production Zcash spending and live FROST signing remain future work.

Read architecture, disclosure profile, privacy limits, examples, Zcash integration, Zcash resource map and security documentation in docs/.

## Local setup

Use Node.js **22.13+** and pnpm **10+** for the web application. The public shell can run without external services; authenticated credentials, issuer/verifier workflows and requests require the Zerant service plus PostgreSQL.

```sh
# With pnpm available on your PATH:
pnpm install
pnpm dev
```

Open `http://localhost:3000`, then `/app`. If pnpm is unavailable, install it using your normal package-manager setup or use `npm exec --yes --package=pnpm -- pnpm install`. This requires registry access.

Dependencies are pinned to the stable versions resolved by pnpm: Next.js 16.3.8, React 19.3.0 and Tailwind CSS 4.3.3. ESLint 9.39.2 and TypeScript 5.9.3 stay within the supported lint-tool peer ranges. `pnpm-lock.yaml` records the full graph; use frozen installs for CI and deployment.

## Development gates

Run `make check` for Rust fmt/clippy/tests, web typecheck/lint/tests/build, diff and
source secret-pattern scan. `make integration` runs native integration/test targets.
`make z3-check` probes an already running official **local regtest** router; it never
sends funds or prints raw wallet data. CI runs Rust and web gates independently.


### Rust API environment

`zerant-api` is a separate server deployment. Required production configuration:

```text
DATABASE_URL
ZERANT_PUBLIC_ORIGIN
ZERANT_VAULT_KEK_B64
ZERANT_VAULT_KEY_VERSION
ZERANT_ZCASH_CHAIN
ZERANT_ZECAUTH_SCOPES
Z3_REGTEST_RPC_ROUTER_USER
Z3_REGTEST_RPC_ROUTER_PASSWORD
```

`ZERANT_VAULT_KEK_B64` is the bootstrap key-encryption key for envelope encryption. Production custody should move behind a managed KMS/HSM. `Z3_REGTEST_*` currently drives the exercised local Z3 adapter; the current concrete router transport is intentionally regtest/read-only and is not a mainnet spending backend.

The Vercel project needs only the server-side `ZERANT_API_ORIGIN` pointing at the deployed Rust API. Do not expose it with a `NEXT_PUBLIC_` prefix.

### Vercel deployment

The deployable Next.js project lives in `apps/web`. In Vercel, set **Root Directory**
to `apps/web` once; the repository now keeps the Next.js `vercel.json`, package
metadata, favicon and build configuration inside that directory. Vercel should use its
default Next.js build/output handling. The old repository-root static-export
configuration has been removed.

Public RFC test key material is labeled under credential fixtures. Never reuse it.


## Product integration surface

The web product is backed by real authenticated state and contains no seeded user data. /issuer delivers signed source credentials into a holder account. /verifier creates short-lived signed requests bound to the verifier website and trusted issuer set. /requests lets the holder approve or deny. Approval re-verifies the stored source credential, creates a fresh verifier-specific attestation, signs the holder response, verifies that response internally, and only then marks the request verified.

/vault shows human-readable credential cards; signed payloads and key material stay behind the product surface. The browser does not invoke wallet RPC or duplicate the protocol verifier.

The v0.3 compound profile remains available for multi-condition workflows. Payment conditions are optional and stay behind the Zcash boundary; generic verifiers receive only approved condition-bound assertions rather than wallet-wide data.
