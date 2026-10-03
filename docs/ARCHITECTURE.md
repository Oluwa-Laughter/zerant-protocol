# Architecture

Status: proposed local foundation. No directories below are implemented components.

## Planned monorepo

| Boundary | Responsibility |
| --- | --- |
| `apps/web` | Next.js / React / TypeScript / Tailwind / shadcn/ui; holder vault and consent, distinct local issuer/verifier demo roles |
| `crates/zerant-core` | Types, strict parsing, origin/time rules, canonicalization and library-backed crypto interfaces |
| `crates/zerant-credential` | Credential signing/validation, issuer trust and revocation snapshots |
| `crates/zerant-reputation` | Deterministic context policies and local audit trace |
| `crates/zerant-disclosure` | Request matching, response construction, verification and replay contracts |
| `crates/zerant-zcash` | Future Zcash adapter; depends on generic interfaces, never the reverse |
| `packages/sdk` | Future issuer/verifier JS API; no holder profile service |

Start with one repository and a local demo, not distributed services. Rust execution through WASM versus native local tooling requires a compatibility decision and shared test vectors before implementation. Do not duplicate divergent scoring or serialization logic in JS. TanStack Query may manage public metadata; private holder state stays outside server-rendered data and shared caches.

Axum/PostgreSQL are optional later choices only if public issuer keys, schemas, policies or revocation services actually need hosting. M1 uses pinned local public metadata files and a local durable verifier request/replay store. These files are not holder databases. Local issuer keys, holder secrets and verifier state use separate storage namespaces and roles; a same-machine demo is not a production isolation boundary.

## Load-bearing decisions

1. **Minimal disclosure through atomic attestations.** A normal signature cannot survive deleting signed claims. Private source credentials stay local. Separately signed audience-bound attestations carry one result. This is signed selection, not cryptographic selective disclosure or ZK.
2. **Threshold trust.** A trusted issuer evaluates its own evidence under the same pinned policy and signs a boolean. Holder computation gives transparency, not a verifiable hidden-input predicate. No issuer receives a complete holder portfolio. Supporting other issuers' aggregation requires a new trust/privacy design.
3. **Pairwise subject binding.** Holder creates an independent Ed25519 key per verifier origin. Each attestation uses fresh IDs and that key. Private source credentials use a separate local holder key never presented. Issuer authenticates enrollment and binds the audience key to its subject; holder self-assertion alone is insufficient.
4. **Local encrypted vault.** Use browser Web Crypto AES-256-GCM with fresh random 96-bit IVs per encryption and authenticated vault version/record ID. Persist ciphertext in IndexedDB; no server sync by default. Derive the wrapping key from a holder passphrase using PBKDF2-HMAC-SHA-256 with a random 128-bit salt and at least 600,000 iterations; store KDF parameters, benchmark usability, and review strength before implementation. Encrypt random vault data keys and signing-key bytes, never persist plaintext passphrases/keys. Auto-lock clears accessible secrets on a best-effort basis; JS memory erasure is not guaranteed. Weak passphrases, compromised devices and malicious same-origin code remain risks. Encrypted manual export is optional; loss without backup means loss/reissuance. No server recovery escrow.
5. **Offline public revocation.** Issuer-signed snapshots expire within 24 hours. Missing or stale status fails closed. No credential-specific online lookups in M1. See credential spec for identifiers and validity.
6. **Durable replay state.** Verifier owns pending requests and atomically consumes them after full verification; expiry and restart rules are in disclosure spec.
7. **No generic wallet identity.** No wallet addresses, balances or transactions enter generic schemas.

## Future Zcash boundary

M2 must choose what testnet identity means, who verifies possession, the observable data, and how wallet keys remain separate from holder keys. No architectural dependency on a Zcash node exists in M1. Evaluate supported 2026 [librustzcash](https://github.com/zcash/librustzcash) libraries and current wallet interfaces when M2 starts, pin versions, and document capabilities. [zcashd is deprecated](https://z.cash/support/zcashd-deprecation/); do not assume its retired embedded-wallet RPCs or build around it. Evaluate maintained node/wallet tooling such as Zebra and Zallet only as needed. Using Zcash tooling does not confer its privacy properties on Zerant credentials.

## Decision gate before code

Validate maintained JOSE/JCS and Web Crypto library compatibility, finalize exact public metadata fixtures and vault format, and record key rotation/compromise and storage implementation choices here. Preserve the contracts below or explicitly version a change; unresolved library choices are not permission to weaken disclosure or replay rules.

## M1A product shell (implemented scope)

M1A introduces `apps/web`, a statically exportable Next.js interface deployed as public assets on Vercel. Its role views use invented, typed public fixtures; they do not implement protocol parsers, origin authentication, signing, encrypted storage, issuance, verification, replay state, or revocation. Review and decline controls change only transient React state. Reloading resets that state; no browser persistence, analytics, wallet connection, backend, or credential transport is introduced. All displayed origins, keys and identifiers are illustrative placeholders, not authenticated evidence.

The hosting boundary serves public product copy and demo assets only. No holder secrets are accepted or rendered. Existing cryptographic profiles, subject binding, consent, storage, key lifecycle, revocation, replay and retention decisions remain requirements for M1B/M1C. Static hosting is not a protocol verifier or an isolation boundary between the demo roles. Production protocol implementation still requires the decision gate above. M1A does not complete M1 security acceptance criteria.

## M1B Rust credential core (implemented)

The first protocol-critical implementation now lives in Rust. `zerant-core` owns strict encoding, canonicalization, time, and origin primitives. `zerant-credential` owns atomic credential parsing, issuer trust, EdDSA/Ed25519 compact-JWS validation, audience/expiry checks, and signed revocation snapshots.

The cryptographic boundary uses maintained libraries: `josekit 0.10.3` for JOSE/JWS EdDSA and `serde_json_canonicalizer 0.3.2` for RFC 8785 payload canonicalization. Zerant does not implement Ed25519 or JWS itself. Protected JOSE headers are restricted to `alg=EdDSA`, exact `kid`, and exact message-specific `typ`; signed Zerant payloads are JCS canonical.

M1B remains native Rust. Browser/WASM integration is intentionally deferred until there is a concrete M1C product boundary and shared vectors can prove parity. Holder vault storage, key enrollment/possession, reputation evaluation, disclosure responses, replay persistence, authenticated transport, and Zcash remain unimplemented.

See [M1B implementation](specs/implementation-m1b.md).
