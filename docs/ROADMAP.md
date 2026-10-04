# Zerant roadmap

The roadmap is intentionally staged so the product never claims privacy or protocol properties ahead of implementation.

## M1A — Visual and product shell
**Status:** complete

- Brand system and provisional logo
- Next.js product shell
- Holder / Issuer / Verifier demo views
- Consent review UI
- Robust README and architecture documentation
- Vercel-ready deployment
- Static typed demo data only

No signing, secure vault, protocol verification, Zcash integration, ZK, or mainnet.

## M1B — Rust credential core
**Status:** credential core implemented

- Create `zerant-core` and `zerant-credential`
- Canonical formats and strict parsers
- Ed25519/JWS verification through maintained libraries
- Issuer trust metadata
- Credential validation, expiry, and revocation snapshot validation
- Shared interoperable fixtures/test vectors
- Rust as source of truth; browser integration via WASM only where appropriate

## M1C — Rust reputation and disclosure engine
- `zerant-reputation`
- `zerant-disclosure`
- Deterministic contextual reputation
- Request/domain/challenge/nonce binding
- Holder response validation
- Durable replay model
- Revocation handling
- Privacy-invariant and adversarial test suites

## M2 — Zcash testnet identity adapter
- `zerant-zcash`
- Define exactly what Zcash identity/authority means for Zerant
- Evaluate current `librustzcash`, Zebra/Zaino/Zallet interfaces as appropriate
- Keep wallet spending authority separate from Zerant identity authority
- Testnet only until threat model and privacy properties are demonstrated

## M3 — Stronger privacy predicates
Research established, reviewed anonymous credential/selective-disclosure/ZK systems for:
- hidden-input predicates
- reduced issuer/verifier linkability
- threshold proofs without exact score disclosure
- stronger unlinkable credential and presentation guarantees beyond pairwise proof keys

No custom cryptography or unsupported privacy claims.
