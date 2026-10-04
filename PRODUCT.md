# Zerant Protocol

Zerant is general-purpose privacy-preserving trust infrastructure for applications in the Zcash ecosystem. It lets an issuer attest to a fact it can substantiate, a holder keep evidence privately and choose what to disclose, and a verifier ask for one narrowly defined result instead of collecting a person's complete history.

Zerant is not an OSS-only reputation product and it does not define a universal social score. The same protocol primitives can support freelancers, businesses and vendors, communities, grants, marketplaces, organizations, open-source ecosystems, paid-invoice receipts, and future autonomous software.

## Product model

The reusable primitives are atomic signed credentials and attestations; issuer authorization scoped to claims, contexts, schemas and policies; contextual immutable policies evaluated locally; exact verifier requests with purpose, audience, challenge, nonce and expiry; request-specific holder consent; minimal signed responses; revocation and replay protection; independent audience keys; and optional payment-specific claims without turning wallet history into identity.

A signature authenticates an assertion; it does not establish that an issuer is honest. A local score helps a holder understand a policy; it is not verifier proof. Threshold attestations are issuer-backed signed booleans, not hidden-input zero-knowledge proofs.

## Example use cases

A freelancer can prove one completed engagement without exposing other clients. A vendor can prove one qualification. A community can verify membership or a contribution threshold. A grant program can verify eligibility without receiving a full history. A marketplace can verify fulfillment. An organization can verify one role. A payment issuer can attest that one invoice condition is satisfied without returning wallet balances, addresses or transaction history.

These examples reuse one protocol. New use cases should add reviewed schemas/policies and trust configuration rather than fork the core.

## Zcash boundary

Zcash is an integration and settlement layer, not a generic identity field. The generic protocol does not require wallet addresses, balances or transactions.

The current Zcash adapter is regtest-oriented and read-only: it discovers current Z3 RPC capabilities and projects minimal chain/wallet readiness. Payment sending is deliberately disabled until the current Zallet payment RPC contract is exercised end-to-end on official Z3 regtest. Zcash privacy properties do not automatically apply to Zerant credentials.

FROST is an optional future organizational signing/custody capability for teams or issuer keys, not a requirement for holder flows.

## Success criteria

Zerant succeeds when applications can ask for a narrow fact, holders can understand and approve exactly what leaves their device, and verifiers can fail closed on tampering, expiry, revocation, domain substitution and replay—without creating a centralized holder profile.

No claim of anonymity, unlinkability, zero knowledge or production Zcash payment privacy should exceed the mechanism that is actually implemented and tested.
