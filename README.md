# Zerant Protocol

A proposed privacy-preserving credential and contextual reputation protocol for the Zcash ecosystem.

**Status: architecture foundation only.** No application code, runnable demo, Zcash testnet implementation, or implemented privacy guarantees exist yet. M1 specifies local signed minimal disclosure; it is not zero knowledge.

Issuers sign credentials, holders store them in an encrypted client-side vault, and verifiers request domain-bound claims with explicit holder consent. Threshold requests reveal an issuer-attested boolean rather than an exact reputation score. Reputation is contextual.

Start with [PRODUCT.md](PRODUCT.md), [DESIGN.md](DESIGN.md), and [Milestone 1](docs/scope/milestone-01.md). Technical contracts: [architecture](docs/ARCHITECTURE.md), [protocol](docs/PROTOCOL.md), [privacy](docs/PRIVACY.md), [threat model](docs/THREAT_MODEL.md), [credentials](docs/specs/credential-v0.1.md), [disclosure](docs/specs/disclosure-v0.1.md), and [reputation](docs/specs/reputation-v0.1.md).

The planned monorepo separates a web app, Rust protocol crates, a JS SDK, and a later Zcash adapter. There are no setup or test commands until implementation starts.
