# Zerant

**Prove trust. Preserve privacy.**

Zerant is a privacy-conscious trust and credential platform for applications in the Zcash ecosystem. Organizations issue signed credentials, holders control which claims they disclose, and verifiers receive only the specific result they requested and the holder approved.

**[Open Zerant](https://zerant.vercel.app)** · **[Documentation](docs/README.md)** · **[Architecture](docs/ARCHITECTURE.md)** · **[Security](SECURITY.md)**

Zerant is currently deployed on **Zcash testnet**. It is experimental software, not an audited wallet, an anonymous-credential system, or a zero-knowledge proof protocol.

## Why Zerant?

An application may need to know whether someone is eligible, belongs to a community, or holds a particular role. It should not have to collect the person's full identity, unrelated credentials, wallet balance, or transaction history to answer that question.

Zerant keeps **account access**, **credential disclosure**, and **payment authorization** separate. A Zerant ID identifies an account, not a Zcash payment address or an on-chain reputation profile.

## Capabilities

| Area | Current functionality |
| --- | --- |
| **Credential vault** | Receive and manage signed issuer credentials in an encrypted, authenticated workspace. |
| **Issuer tools** | Manage organizations, credential definitions, issuance, revocation, signing-key rotation, and roles. |
| **Consent and verification** | Create purpose- and audience-bound requests; let holders preview the exact claim and approve or deny it. |
| **Verifier integrations** | Retrieve narrowly scoped results through authenticated APIs and signed webhook delivery. |
| **Account security** | Access the workspace with passkeys, maintain opaque sessions, and revoke sessions. |
| **Private payouts** | Share a temporary, encrypted, shielded-capable Zcash testnet payout destination with a selected organization. |
| **Zcash payments** | Validate and review ZIP-321 payment requests, present QR/link handoff, and record prepared or submitted status. |

A credential's meaning comes from its **issuer, claim, and context**. Zerant does not assign a universal public reputation score.

## How it works

1. **Issue:** An authorized organization signs a credential and sends it to the holder's Zerant vault.
2. **Request:** A verifier asks for one specific fact, stating its purpose, audience, and expiry.
3. **Consent:** The holder inspects the exact requested disclosure and approves or rejects it.
4. **Verify:** Zerant checks issuer trust, signatures, revocation, expiry, request binding, and replay protection, then returns the approved result.
5. **Pay separately, when needed:** A validated ZIP-321 request is handed to an external wallet. The wallet, not Zerant, controls spending approval.

![Private credential vault in Zerant](docs/assets/screenshots/05-vault.png)

The [architecture documentation](docs/ARCHITECTURE.md) details the service boundaries, and the [protocol documentation](docs/PROTOCOL.md) covers verification and disclosure rules.

## Architecture

~~~text
apps/web/                Next.js application, consent UI, and server proxy
services/zerant-api/     Private Rust/Axum API, sessions, and persistence
crates/zerant-core/      Shared identifiers and canonical validation
crates/zerant-credential/ Signed credential and issuer trust primitives
crates/zerant-policy/    Contextual policy evaluation
crates/zerant-disclosure/ Request, consent, and replay protection
crates/zerant-zcash/     Zcash addresses, ZIP-321, and network integration
fixtures/                Protocol test vectors and fixtures
docs/                    Product, architecture, security, and API documentation
scripts/                 Development and validation helpers
~~~

The frontend uses **Next.js, React, TypeScript, and Tailwind CSS**. A private **Rust/Axum** service owns authorization and protocol operations, backed by **PostgreSQL/Neon**. The deployed web app connects to the backend through server-side routes and a private service binding.

### Zcash integration

Zerant uses Zcash testnet for payment-related functionality: address validation, exact-amount ZIP-321 requests, QR/link handoff to compatible wallets, and bounded transaction observation. Credentials and their consent workflow run in the Zerant service; they are **not recorded on-chain**.

Payment preparation is not payment submission, and a wallet-reported transaction ID is **not proof of exact shielded settlement**. Zerant does not hold wallet spending keys.

## Development

**Requirements:** Node.js 22.x, pnpm 10.34.6, Rust, and a PostgreSQL database for the private API. Review [deployment and configuration](docs/VERCEL_DEPLOYMENT.md) before starting the backend; do not expose server secrets through public frontend environment variables.

~~~bash
pnpm install --frozen-lockfile
pnpm --filter @zerant/web dev
~~~

**Checks:**

~~~bash
pnpm --filter @zerant/web lint
pnpm --filter @zerant/web typecheck
pnpm --filter @zerant/web test
pnpm --filter @zerant/web build

cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

python3 scripts/secret-check.py
~~~

Backend deployment and runtime configuration are described in [docs/VERCEL_DEPLOYMENT.md](docs/VERCEL_DEPLOYMENT.md). The repository's [documentation index](docs/README.md) links the protocol, privacy, verifier, and Zcash integration references.

## Privacy and security boundaries

Zerant currently provides **signed minimal disclosure with holder consent**, not zero-knowledge proofs, anonymity, or guaranteed unlinkability. Credential records are encrypted at rest behind the service's key-provider boundary; this is not holder-only end-to-end encryption. Issuer honesty, device compromise, traffic correlation, and coercion remain important risks.

Temporary private payout destinations are scoped to a chosen organization and expire or can be withdrawn; they are not public identity attributes. An external wallet must independently approve spending.

See [Privacy](docs/PRIVACY.md), [Threat Model](docs/THREAT_MODEL.md), and [Security](SECURITY.md) for implementation limits and security reporting.

## Status

Zerant is an actively developed Zcash testnet product. Interfaces and protocol details may evolve as additional interoperability and privacy features are reviewed and tested.
