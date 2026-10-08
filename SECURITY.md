# Security

Zerant is experimental software built for Zcash testnet. It is not an audited production wallet, custody provider, or anonymous-credential system. Its current credential protocol uses signed, consent-bound minimal disclosures—not zero-knowledge proofs, guaranteed unlinkability, or anonymity.

## Trust assumptions

- The private Rust service manages authenticated workflow state and encrypted credential records; storage is not holder-only end-to-end encrypted.
- A signature establishes who made an assertion, not whether the issuer is truthful.
- Account sessions, issuer key lifecycle, revocation freshness, enrollment, and accurate clocks are security-critical.
- Payment wallets keep spending keys and authorize transactions independently. A prepared request or wallet-reported transaction ID does not prove exact shielded settlement.
- Test vectors contain public, intentionally non-secret fixture material. Never reuse fixture keys or commit real credentials, spending keys, wallet mnemonics, RPC credentials, or database secrets.

Read the [threat model](docs/THREAT_MODEL.md), [privacy boundaries](docs/PRIVACY.md), and [architecture](docs/ARCHITECTURE.md) before extending security-sensitive functionality.

## Vulnerability reporting

Use this repository's **GitHub → Security → Advisories → Report a vulnerability** if private reporting is enabled. Otherwise, ask the maintainers to provide a private disclosure channel before sharing exploit details. No audit, support SLA, or paid security program is claimed.
