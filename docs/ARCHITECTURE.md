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

## Zcash boundary

Zcash remains isolated behind `zerant-zcash`; generic credentials and contextual policies do not depend on wallet state. The current adapter is regtest-oriented and read-only: it can discover the local Z3 RPC contract and project minimal Zebra/Zallet readiness without exporting balances, addresses, seed fingerprints or transaction history. The official Z3 regtest router has been exercised for read-only capability/status calls; no payment-send path is enabled.

Any future wallet-identity or payment authorization feature must separately define possession, observability, confirmation/reorg policy and key separation. [zcashd is deprecated](https://z.cash/support/zcashd-deprecation/); do not assume its retired embedded-wallet RPCs. Using Zcash tooling does not confer Zcash transaction-privacy properties on Zerant credentials.

## Decision gate before code

Validate maintained JOSE/JCS and Web Crypto library compatibility, finalize exact public metadata fixtures and vault format, and record key rotation/compromise and storage implementation choices here. Preserve the contracts below or explicitly version a change; unresolved library choices are not permission to weaken disclosure or replay rules.

## M1A product shell (implemented scope)

M1A introduces `apps/web`, a statically exportable Next.js interface deployed as public assets on Vercel. Its role views use invented, typed public fixtures; they do not implement protocol parsers, origin authentication, signing, encrypted storage, issuance, verification, replay state, or revocation. Review and decline controls change only transient React state. Reloading resets that state; no browser persistence, analytics, wallet connection, backend, or credential transport is introduced. All displayed origins, keys and identifiers are illustrative placeholders, not authenticated evidence.

The hosting boundary serves public product copy and demo assets only. No holder secrets are accepted or rendered. Static hosting is not a protocol verifier or an isolation boundary between the demo roles. Native Rust code implements credential, policy, disclosure and replay rules, but production browser integration still requires secure key/vault lifecycle, authenticated transport, consent wiring and deployment review.

## M1B Rust credential core (implemented)

The first protocol-critical implementation now lives in Rust. `zerant-core` owns strict encoding, canonicalization, time, and origin primitives. `zerant-credential` owns atomic credential parsing, issuer trust, EdDSA/Ed25519 compact-JWS validation, audience/expiry checks, and signed revocation snapshots.

The cryptographic boundary uses maintained libraries: `josekit 0.10.3` for JOSE/JWS EdDSA and `serde_json_canonicalizer 0.3.2` for RFC 8785 payload canonicalization. Zerant does not implement Ed25519 or JWS itself. Protected JOSE headers are restricted to `alg=EdDSA`, exact `kid`, and exact message-specific `typ`; signed Zerant payloads are JCS canonical.

Protocol-critical logic remains native Rust. Contextual policy evaluation, disclosure v0.2, replay persistence and the read-only Z3 adapter are implemented and covered by shared vectors/tests. Browser/WASM integration, holder vault storage, enrollment/key-possession UX, authenticated browser transport, payment sending/settlement and live FROST wiring remain unimplemented.

See [M1B implementation](specs/implementation-m1b.md).

## General-purpose executable boundaries (2026-10-04)

Policies remain immutable contextual rules, with bounded integer category weights;
OSS is one fixture among service, business, community, grant, marketplace, role and
payment examples. The evaluator requires caller-verified credentials against pinned
issuer trust and fresh signed revocation at the explicit evaluation time. Conflicting
IDs, foreign contexts/subjects and unsupported evidence fail unavailable. Local scores
are private calculations, never verifier proof.

Disclosure reuses the existing JOSE/JCS profile. A pinned verifier key authenticates a
single-result request; the caller separately supplies an authenticated transport origin.
Approval signs exactly one verified matching attestation with its audience subject key.
No source fallback, denial reason, score, wallet history or portfolio is transmitted.
Request policy digest is explicitly included to prevent immutable policy substitution;
this is the versioned disclosure v0.2 profile. Issuer authorization remains mandatory.
Keys and transport authentication remain application responsibilities; no vault is implied.

Replay stores register the exact signed request digest before delivery and atomically
consume only after every verification check. A SQLite implementation uses conditional
UPDATE for process/restart safety; no response bodies are stored. Expired rows may be
purged; absent state fails closed. Revocation watermarks must be persisted by callers.
SQLite is a necessary local persistence dependency for this requested replay boundary.

Zcash is isolated in zerant-zcash. Only regtest adapters are enabled here. Documented
read-only RPCs and OpenRPC discovery are allowlisted. Wallet RPC output is projected
into readiness/status without exporting fingerprints, balances or history. Payment
confirmation alone does not establish recipient, amount or invoice fulfillment; these
must be checked by an authorized issuer before signing a payment claim. No seeds are
accepted. Sending and production payment settlement are separate capabilities, enabled
only after a demonstrated supported contract. Optional organizational signing has an
interface only; no FROST algorithm or compatibility claim is implemented.

The browser playground uses shared public policy examples and transient consent state.
It is a simulation, not a native verifier, authenticated transport or encrypted vault.
No real credentials or wallet secrets are accepted; scenario changes reset consent.

## Policy and disclosure implementation contract (2026-10-04)

The native policy API accepts already-verified CredentialPayload values; callers must
verify issuer authorization, signature and fresh revocation at as_of before calling.
It rechecks structure, time, context, source schema, issuer and subject consistency.
It exposes a local count/exclusion summary without payloads or identifiers. Empty
evidence is zero; conflicts and invalid evidence are unavailable, never silently zero.
Weights may be zero; every rule has a nonzero bounded count. Supported thresholds
are unique safe integers within the cap (including zero); output sorts thresholds.

Disclosure v0.2 requires an immutable policy digest for threshold requests. Native
libraries accept pinned verifier trust and an independently authenticated origin;
they do not authenticate a browser session. Holder signing is a low-level operation
requiring application consent. Enrollment, key generation/rotation and encrypted
key storage remain application responsibilities; compromise flags reject known bad
keys. Issuer trust and signed revocation remain required at response acceptance.

Replay rows contain ID, exact ASCII request digest, origin, expiry and status only.
The exact signed request must additionally be retained by the caller and supplied
when verifying; its hash must match the persisted row. SQLite conditional updates
provide single acceptance across threads, connections and restart. Never overwrite
an existing request ID, even consumed or expired. No response bodies or scores are
stored. Caller-controlled deletion of expired rows must never restore old requests;
register only freshly issued, independently random IDs. Revocation watermarks and
clock health are caller responsibilities. No vault, authenticated browser transport
or transaction privacy is implemented by the policy/disclosure components.

The concrete regtest HTTP transport uses reqwest blocking HTTP without TLS (fixed
loopback router only), disables redirects and proxies, enforces a 15-second deadline
and 1 MiB response limit, and allowlists three read-only RPCs: rpc.discover,
getblockchaininfo and getwalletinfo. Authentication stays
inside the client and errors omit raw wallet responses. No remote endpoint or spending
method is exposed. A missing Zebra IBD field is unknown, never inferred ready/false.
