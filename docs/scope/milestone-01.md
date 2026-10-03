# Milestone 01: local foundation

Status: specified, not implemented. MUST/MUST NOT requirements describe future M1 behavior.

## Included

One local issuer, holder vault, and verifier exercise this complete flow:

1. Issuer creates and signs a private source credential; holder verifies and stores it encrypted.
2. Holder derives a contextual score locally using a pinned deterministic policy.
3. The local issuer validates its own issued evidence and signs a separate, audience-bound minimal attestation for a supported claim or threshold. Provision this before the verifier request; no presentation-time issuer callback or portfolio upload is required.
4. Verifier creates an authenticated-origin request containing one result, challenge, nonce, and expiry.
5. Holder reviews the exact outbound result and metadata, then approves or denies.
6. Approval produces holder-signed evidence; denial produces no response. Verifier validates issuer trust, signatures, subject possession, expiry, revocation, policy, challenge, nonce, domain and replay state.

M1 scoring uses one issuer's evidence per policy. Arbitrary thresholds, multi-issuer private aggregation, and generating authenticated predicates from a holder-only score are unsupported. Missing attestations produce `evidence_unavailable`, never source disclosure or an unsigned substitute.

## Excluded

Mainnet, payments, tokens, NFTs, governance, AI features, mobile apps, custom ZK circuits, production microservices, hosted credential profiles, and Zcash testnet identity. Testnet identity belongs to M2; only the adapter boundary is documented now.

## Acceptance criteria for later implementation

- Happy path demonstrates issuance, locked/unlocked encrypted persistence, deterministic score, consent and accepted minimal evidence.
- Unrequested source credentials never leave the vault; denial emits no response; threshold evidence contains no exact score or inputs.
- Different verifier origins use independently generated subject keys, credential IDs and revocation IDs.
- Reject tampering, unknown issuer/key, revoked/expired credentials, stale/invalid revocation snapshots, wrong policy/context, wrong holder signature, request mutation, expired request, wrong domain/challenge/nonce, and duplicate responses.
- Concurrent duplicate submissions accept at most once. Replay prevention survives verifier restart; reset invalidates all outstanding requests.
- Credential loss, key loss, clock failure and missing evidence fail closed with actionable local errors.
- Inspect outbound payloads, logs, browser storage and any metadata service to confirm no backend can reconstruct the holder's complete credential profile from protocol data.
- Verify keyboard use, focus, reduced motion and accessible consent states against DESIGN.md.

No runtime tests are available in this architecture-only milestone preparation. Implementation must supply independent fixtures and negative cases before declaring M1 complete.
