# Zerant product roadmap

**Status: active Zcash testnet development.** This page separates **implemented product behavior** from **native protocol research** and **future work**. The [README](../README.md) links the live app and demo; neither the hosted site nor this roadmap claims production readiness.

## Implemented in the hosted product

| Area | Available behavior | Boundary |
| --- | --- | --- |
| Account access | Passkey registration/sign-in, sessions, account controls, optional scoped wallet-auth methods | A Zerant ID is not a wallet or spending key |
| Organizations | Issuer profiles, scoped owner/admin/issuer/auditor roles, invitations, lifecycle controls, signing-key rotation | Authority is account- and role-scoped |
| Credentials | Schema definitions/versions, signing, issuance to a Zerant account, vault view, expiry and revocation | Issuer honesty remains a trust assumption |
| Verification | Scoped verifier requests, holder consent, signature/trust checks, revocation, expiry, replay safeguards | Signed result is not zero knowledge |
| Integrations | Verifier API keys, signed webhooks, public issuer trust metadata | Secret keys remain private and access is scoped |
| Payout destinations | Encrypted, temporary, organization-scoped, shielded-capable Zcash testnet address sharing | Does not make the holder anonymous to the organization |
| Zcash payments | Address/network validation, ZIP-321 review, external-wallet handoff, prepared/submitted tracking | No exact shielded-recipient settlement proof |
| Usability | Responsive workspace, navigation, issuer history filtering, visible request states and errors | Test on real browsers and devices for accessibility |

## Native crates and local experimentation

The Rust workspace implements and tests canonical credential and disclosure profiles, contextual policies, and Zcash adapter utilities. Some Z3/Zallet regtest tools and fixture-based settlement logic are deliberately limited to **local experiment/test contexts**, not the live testnet application.

- [Native protocol overview](PROTOCOL.md)
- [Credential format](specs/credential-v0.1.md)
- [Single-claim and compound disclosures](specs/disclosure-v0.3.md)
- [Zcash integration](ZCASH-INTEGRATION.md)

## Next engineering priorities

1. **End-to-end reliability:** repeated issuer → holder → verifier tests across two independent real browser profiles, including expired, revoked, rejected, stale and replayed requests.
2. **Operations and security:** monitor backend availability and database migrations, improve recovery guidance and key-rotation runbooks, and exercise failure modes before wider use.
3. **Wallet interoperability:** verify explicit testnet wallet capabilities, real permission flows and wallet-directed payment handoff without pretending all wallets offer the same APIs.
4. **Settlement observability:** research an accurate exact-payment verification path before labeling funds “confirmed,” including the limitations of observing shielded Zcash recipients.
5. **Usability and accessibility:** test mobile account creation, navigation, issuer/verifier forms, consent and empty states with keyboard, screen readers and real devices.
6. **Stronger privacy research:** evaluate established anonymous-credential/selective-disclosure and ZK systems. Any stronger property must be implemented, tested, threat-modelled and documented before being advertised.

These priorities are **not** all implemented. Specific Zcash mainnet custody, proprietary cryptography, custom ZK circuits or unlinkable credential claims are not in scope for the current hosted testnet product.

## Historical milestones

Earlier internal phases were called **M1A** (visual/product shell), **M1B** (native Rust credential core), **M1C** (native disclosure and policy engine), and **M2** (Zcash identity/interoperability evaluation). Later server-backed issuer/holder/verifier operations superseded early static frontend prototypes.

For engineering history only, consult the [initial milestone scope](scope/milestone-01.md), [native implementation notes](specs/implementation-m1b.md), and versioned specifications under [`docs/specs/`](specs/). They are snapshots, not independent evidence that a listed browser or mainnet feature is deployed.
