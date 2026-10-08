# Zerant Protocol — contributor and agent guidance

This is the Zerant monorepo: a Next.js product, private Rust/Axum API, and Rust protocol crates for credentials, consent-bound verification, and Zcash integration. Treat the deployed testnet workflow and current implementation as the source of truth; historical milestones are not current feature guarantees.

## Before changing behavior

Read the relevant areas of [PRODUCT.md](PRODUCT.md), [DESIGN.md](DESIGN.md), [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md), [docs/PRIVACY.md](docs/PRIVACY.md), [docs/THREAT_MODEL.md](docs/THREAT_MODEL.md), and the applicable [protocol specification](docs/README.md). Check the working tree before editing and preserve unrelated changes.

## Non-negotiable boundaries

- **Identity is not a wallet.** Zerant IDs, credential claims, and payment destinations have distinct authority.
- **Consent is request-specific.** A verifier receives only an approved result, never a holder's entire credential collection.
- **Signed disclosures are not zero-knowledge proofs.** Do not claim anonymity, guaranteed unlinkability, or unimplemented cryptography.
- **Payment preparation is not settlement.** External wallets retain spending keys; a submitted transaction ID is not exact shielded payment proof.
- **Server-owned secrets stay on the server.** Do not expose database, key-provider, RPC, or credential secrets through browser variables, logs, fixtures, or commits.
- **Trust checks fail closed.** Test malformed data, nonce/audience mismatch, expiry, revocation, replay, missing authority, and negative wallet capabilities.
- **Protocol and implementation stay aligned.** Document breaking contract changes, privacy implications, and lifecycle decisions before changing supported behavior.

## Work practices

Keep new code in the appropriate layer: generic credential and policy logic must not depend on Zcash wallet state; Zcash-specific parsing and RPC logic belongs in the Zcash adapter and backend. Prefer maintained libraries and meaningful regression tests. Do not fabricate proof, wallet approvals, or network settlement for screenshots or tests.

Use the checks in [README.md](README.md), including web lint, type checking, tests, Rust formatting, Clippy, and protocol tests as appropriate. Report any unavailable or failing checks rather than describing them as passing. Do not commit, push, publish, or deploy unless the user explicitly requests it.
