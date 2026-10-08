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
**Status:** implemented

- `zerant-policy`
- `zerant-disclosure`
- Deterministic contextual reputation
- Request/domain/challenge/nonce binding
- Holder response validation
- Durable replay model
- Revocation handling
- Privacy-invariant and adversarial test suites

## M2 — Zcash testnet identity adapter
**Status:** active implementation

- `zerant-zcash`
- Define exactly what Zcash identity/authority means for Zerant
- Evaluate current `librustzcash`, Zebra/Zaino/Zallet interfaces as appropriate
- Keep wallet spending authority separate from Zerant identity authority
- Testnet only until threat model and privacy properties are demonstrated


### Production trust lifecycle — implemented

- issuer and verifier signing-key rotation / compromise handling
- credential revocation lifecycle
- append-only activity history and bounded cursor pagination
- database-backed write quotas and authentication circuit breaker
- reusable credential definitions with immutable version history
- public issuer directory, signing-key metadata and signed revocation publication
- scoped verifier integration API keys and server-to-server request/status API
- verifier-scoped holder proof keys
- account export and deletion controls

### Zcash product integration — active

- passkey-first Zerant account access with wallet-independent identity
- ZIP-316 address validation and canonical ZIP-321 review/handoff
- external-wallet link, QR, and exact simple-payment handoff with no persistent browser-wallet connection state
- account-owned shareable Zcash invoice requests with public capability links, cancellation, expiry, bounded retention and export
- temporary encrypted organization-scoped private payout destinations
- prepared/submitted payment tracking with honest settlement boundaries
- Z3/Zallet local capability boundary
- zcash_client_backend light-client readiness for Zaino/lightwalletd-compatible services
- retained injected/ZecAuth adapter research kept outside current product navigation

## M3 — Stronger privacy predicates
Research established, reviewed anonymous credential/selective-disclosure/ZK systems for:
- hidden-input predicates
- reduced issuer/verifier linkability
- threshold proofs without exact score disclosure
- stronger unlinkable credential and presentation guarantees beyond pairwise proof keys

No custom cryptography or unsupported privacy claims.
