# Zerant documentation

Zerant is a **testnet-stage credential, trust and consent platform** with an independent Zcash payment-preparation integration. The [main README](../README.md) is the fastest overview and includes the **[live application](https://zerant.vercel.app)** and **[product demo](https://youtu.be/P36WelovsiU)**.

## Start here

| Question | Read |
| --- | --- |
| What does the product do, and how can I try it? | [Project README](../README.md) |
| What is live vs. experimental? | [Feature status and roadmap](ROADMAP.md) |
| Where do credentials, requests and keys flow? | [Architecture](ARCHITECTURE.md) |
| What data is exposed and what are the privacy limits? | [Privacy contract](PRIVACY.md) and [threat model](THREAT_MODEL.md) |
| What is actually integrated with Zcash? | [Zcash integration](ZCASH-INTEGRATION.md) and [wallet compatibility](WALLET_COMPATIBILITY.md) |
| How can a verifier integrate? | [Verifier API](VERIFIER_INTEGRATION_API.md) and [webhooks](VERIFIER_WEBHOOKS.md) |
| How is it deployed and configured? | [Deployment / operations](VERCEL_DEPLOYMENT.md) |

### Implementation boundaries

- **Hosted application:** Next.js frontend, private Rust/Axum service and database; passkey accounts; issuer, holder and verifier workflows; signed consent-bound disclosures; Zcash testnet payment preparation.
- **Native Rust / local tooling:** protocol modules, fixtures, and isolated Zcash/regtest verification experiments. These do **not** imply the hosted product executes those same operations with real wallet funds.
- **Not currently claimed:** zero-knowledge credential predicates, cryptographic anonymity, universal unlinkability, wallet custody, exact shielded recipient settlement verification, or a production mainnet payment service.

## Protocol and security

- [Protocol model](PROTOCOL.md) — the issuer/holder/verifier trust model.
- [Credential formats](specs/credential-v0.1.md) — canonical signed credential structure.
- [Minimal disclosure v0.1](specs/disclosure-v0.1.md), [v0.2](specs/disclosure-v0.2.md), [v0.3](specs/disclosure-v0.3.md) — versioned specifications, not interchangeable guarantees.
- [Policy/reputation primitives](specs/reputation-v0.1.md) — contextual evaluation rather than a global reputation score.
- [Payment specification](specs/payment-v0.1.md) — payment claim model and integration boundaries.
- [Public issuer trust discovery](PUBLIC_TRUST_DISCOVERY.md) — keys and issuer metadata.
- [Threat model](THREAT_MODEL.md), [privacy contract](PRIVACY.md), [security reporting](../SECURITY.md).
- [Account access](ACCOUNT_ACCESS.md) — passkeys, sessions, and optional wallet authentication.

## Integrations and developer references

- [Verifier integration API](VERIFIER_INTEGRATION_API.md).
- [Verifier webhooks](VERIFIER_WEBHOOKS.md).
- [Zcash integration](ZCASH-INTEGRATION.md).
- [Wallet compatibility](WALLET_COMPATIBILITY.md).
- [Ecosystem resources](ZCASH-RESOURCES.md).
- [Backend deployment](VERCEL_DEPLOYMENT.md).
- [Policy fixtures and examples](examples/README.md).

## Design and historical notes

- [Design system](../DESIGN.md) — current UI and accessibility principles.
- [Product model](../PRODUCT.md) — problems, primitives and trust boundaries.
- [Roadmap](ROADMAP.md) — delivered and future work.
- [Animation review](ANIMATION_AUDIT.md) — point-in-time UI audit.
- [Initial milestone scope](scope/milestone-01.md) and [native credential implementation note](specs/implementation-m1b.md) — historical snapshots; refer to the current README for hosted capability.

**Reading rule:** Versioned protocol drafts and earlier milestone notes are engineering references, not proof of a feature deployed on the live site. When documentation differs, distinguish the tested **hosted application** from the **native crate** or historical design proposal.
