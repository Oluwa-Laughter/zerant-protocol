# Documentation

This directory contains Zerant's product architecture, protocol contracts, security boundaries, integration references, and development notes.

Start with the [project README](../README.md) for the overview, technology stack, and local development commands.

## System design

- [Architecture](ARCHITECTURE.md) — services, data ownership, trust boundaries, and request lifecycle.
- [Protocol](PROTOCOL.md) — issuer, holder, and verifier contracts.
- [Privacy](PRIVACY.md) — disclosure guarantees and practical limits.
- [Threat model](THREAT_MODEL.md) — adversarial cases and residual risks.
- [Account access](ACCOUNT_ACCESS.md) — passkeys, authentication, and session behavior.
- [Product and implementation roadmap](ROADMAP.md) — completed and planned milestones.

## Integrations

- [Zcash integration](ZCASH-INTEGRATION.md) — addresses, ZIP-321, payment review, network state, and custody separation.
- [Wallet compatibility](WALLET_COMPATIBILITY.md) — supported actions and boundaries of retained adapters.
- [Verifier API](VERIFIER_INTEGRATION_API.md) — request and result integrations.
- [Verifier webhooks](VERIFIER_WEBHOOKS.md) — signed delivery, retries, and trust.
- [Public trust discovery](PUBLIC_TRUST_DISCOVERY.md) — published issuer and key information.
- [Zcash resources](ZCASH-RESOURCES.md) — useful protocol and ecosystem references.
- [Deployment](VERCEL_DEPLOYMENT.md) — private services, environment configuration, and operations.

## Protocol specs and examples

- [Credential specification](specs/credential-v0.1.md)
- [Disclosure specifications](specs/disclosure-v0.1.md), [v0.2](specs/disclosure-v0.2.md), [v0.3](specs/disclosure-v0.3.md)
- [Payment specification](specs/payment-v0.1.md)
- [Reputation and policy specification](specs/reputation-v0.1.md)
- [Policy examples](examples/README.md)
- [Original milestone scope](scope/milestone-01.md) — historical reference, not the current feature list.

## Design and engineering

- [Design principles](../DESIGN.md)
- [Product principles](../PRODUCT.md)
- [Animation and accessibility audit](ANIMATION_AUDIT.md)
- [Security reporting](../SECURITY.md)

Documentation should distinguish implemented behavior from experimental work. In particular, a credential disclosure is not a zero-knowledge proof, and a prepared Zcash payment is not a confirmed settlement.
