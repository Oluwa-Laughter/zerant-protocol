# Architecture

Status: active implementation architecture. The Rust protocol crates and public web console described below exist; future boundaries are labeled explicitly.

## Current monorepo

| Boundary | Responsibility |
| --- | --- |
| apps/web | Customer product surfaces for credentials, issuers, verifiers, consent and Zcash review |
| services/zerant-api | Authenticated account state, protected credential storage, issuer/verifier workflows and native integrations |
| crates/zerant-core | Strict parsing, origin/time rules, IDs and canonicalization |
| crates/zerant-credential | Credential signing/validation, issuer trust and revocation snapshots |
| crates/zerant-policy | Deterministic contextual policy evaluation |
| crates/zerant-disclosure | Signed verifier requests, holder responses, binding and replay contracts |
| crates/zerant-zcash | Zcash address/payment parsing, Z3/Zallet observation, PCZT/FROST capability boundaries |

The browser is a product and consent surface, not the protocol authority. Durable credential, session, issuer and verifier state is held by the service. Protocol-critical signing and verification reuse the Rust crates so the web layer does not implement a second version of the security rules.

Issuer governance actions are written to an append-only `issuer_events` ledger in the same PostgreSQL transaction as the protected change. Owners, admins and auditors can review the paginated organization history from the issuer workspace.

## Load-bearing decisions

1. **Minimal disclosure through atomic attestations.** A normal signature cannot survive deleting signed claims. Private source credentials stay local. Separately signed audience-bound attestations carry one result. This is signed selection, not cryptographic selective disclosure or ZK.
2. **Threshold trust.** A trusted issuer evaluates its own evidence under the same pinned policy and signs a boolean. Holder computation gives transparency, not a verifiable hidden-input predicate. No issuer receives a complete holder portfolio. Supporting other issuers' aggregation requires a new trust/privacy design.
3. **Subject and audience binding.** Each account has a protected credential key for private source credentials. Approved presentations use a separate protected holder key scoped to the verifier relationship, so two verifiers do not receive the same stable holder proof key. This reduces direct cross-verifier correlation but is not a full unlinkability guarantee.
4. **Server credential vault.** `/vault` is backed by `zerant-api` and PostgreSQL. Every credential uses a fresh random AES-256 data key; the payload ciphertext is stored with that DEK wrapped by a versioned server key-encryption key. Sessions use opaque random tokens stored only as hashes and delivered as HttpOnly secure cookies. Browser storage is not a source of truth. The KEK is server-only and is designed to move behind a managed KMS/HSM in production. Zcash seeds, spending keys, PCZT artifacts and FROST shares are outside this vault.

### Service vault KEK migration decision

The existing v1 data and wrap AAD both bind the account, record ID and KEK version. A wrapped-DEK-only update would invalidate the data authentication tag. Rotation therefore decrypts and re-encrypts each record with a fresh DEK and nonces under the active version, retaining the existing on-disk format. An operator-only maintenance command changes at most one bounded PostgreSQL row-locking batch per invocation. It covers credential envelopes, account credential keys, issuer profiles and signing keys, verifier profiles and signing keys, holder pairwise keys, verifier webhook secrets, and approved verification responses. The command derives each AAD account from the same ownership relation as runtime decryption. A failed decrypt or write rolls back the batch; historical KEKs remain available until a zero-reference inventory is verified after all writers use the new active version. Plaintext and key material never enter output or logs. This trades additional data encryption work and a brief row lock for no format migration or dual-AAD compatibility mode. Concurrent writes can create old-version rows until every service instance has switched; the final inventory is therefore an operational gate, not a cryptographic proof against a later stale writer.
5. **Revocation is issuer-authoritative.** Issuers maintain a monotonic revocation version and revoked credential digests. Proof construction signs the current snapshot and revoked credentials are excluded immediately. Public signed snapshots are cached and published through the bounded issuer-discovery surface; production cache/CDN behavior and key custody still require operational hardening.
6. **Durable replay state.** Verifier owns pending requests and atomically consumes them after full verification; expiry and restart rules are in disclosure spec.
7. **No generic wallet identity.** No wallet addresses, balances or transactions enter generic schemas.

## Zcash boundary

Zcash remains isolated behind `zerant-zcash`; generic credentials and contextual policies do not depend on wallet state. The current adapter is regtest-oriented and read-only: it can discover the local Z3 RPC contract and project minimal Zebra/Zallet readiness without exporting balances, addresses, seed fingerprints or transaction history. The official Z3 regtest router has been exercised for read-only capability/status calls; no payment-send path is enabled.

Any future wallet-identity or payment authorization feature must separately define possession, observability, confirmation/reorg policy and key separation. [zcashd is deprecated](https://z.cash/support/zcashd-deprecation/); do not assume its retired embedded-wallet RPCs. Using Zcash tooling does not confer Zcash transaction-privacy properties on Zerant credentials.

## Decision gate before code

Validate maintained JOSE/JCS and server envelope-encryption compatibility, finalize exact public metadata contracts, and record KEK rotation/KMS and storage implementation choices here. Preserve the contracts below or explicitly version a change; unresolved library choices are not permission to weaken disclosure or replay rules.

## Issuer key lifecycle

Issuer signing authority is versioned instead of being replaced in place. Every issued credential records the issuer key that signed it. Routine rotation retires the current key for new issuance while retaining it only for validating and revoking credentials it already signed until those credentials expire. A compromise replacement marks the affected key compromised, invalidates credentials signed by it, and activates a fresh key for future issuance.

Historical signing keys remain encrypted under the issuer account boundary and participate in the same vault-key versioning model as other protected records.

## Verifier key lifecycle

Verifier request signing authority is versioned. Every verification request records the verifier key that signed it. Routine rotation retires the current key for new requests while already-sent requests remain valid until their existing short expiry. If a verifier reports the current key compromised, Zerant marks that key compromised, immediately expires pending requests signed by it, and activates a replacement for future requests.

Holder approval always validates the request against the exact historical verifier key recorded on the request, so rotation cannot silently reinterpret an existing request.

## Product application (implemented)

The web application now uses authenticated service state for real credentials, issuer profiles, verifier profiles and consent requests. It contains no seeded credential or verification data. A holder can receive issuer-created private credentials, review short-lived verifier requests, approve or deny them, and use Zcash-native identity/payment review surfaces.

## M1B Rust credential core (implemented)

The first protocol-critical implementation now lives in Rust. `zerant-core` owns strict encoding, canonicalization, time, and origin primitives. `zerant-credential` owns atomic credential parsing, issuer trust, EdDSA/Ed25519 compact-JWS validation, audience/expiry checks, and signed revocation snapshots.

The cryptographic boundary uses maintained libraries: `josekit 0.10.3` for JOSE/JWS EdDSA and `serde_json_canonicalizer 0.3.2` for RFC 8785 payload canonicalization. Zerant does not implement Ed25519 or JWS itself. Protected JOSE headers are restricted to `alg=EdDSA`, exact `kid`, and exact message-specific `typ`; signed Zerant payloads are JCS canonical.

Account-visible issuance, revocation and verification lifecycle events are captured by append-only database triggers and exposed through bounded cursor pagination. Authenticated write rate limits are stored in PostgreSQL so quotas remain consistent across horizontally scaled service instances.

Credential definitions are immutable once used. Issuers publish a new database row for each version; the previous active version is retired and remains referenced by already-issued credentials and in-flight verification requests. Only one active version per issuer/credential meaning is available for new issuance and new verifier requests.

Issuer authority is organization-scoped rather than account-shared. One Zerant account can belong to one issuer organization at a time. The organization owner controls ownership transfer, owners/admins manage security and credential definitions, owners/admins/issuers may issue or revoke, and auditors are read-only. Invitations target Zerant IDs and require explicit acceptance. Ownership transfer preserves the public issuer identity while rewrapping issuer signing-key ciphertext under the new owner's authenticated encryption context; the previous owner becomes an admin member.

Public trust discovery exposes only organization-level verification material: issuer identity, public signing-key lifecycle, immutable credential-definition versions and signed revocation state. Directory enumeration is cursor-paginated and bounded. Holder IDs, recipients, credentials, sessions and wallet data are excluded by contract and regression tests.

Verifier integrations use separately revocable bearer credentials whose full secrets are shown once and stored only as SHA-256 hashes. Integration keys have explicit request-create/read scopes and optional expiry. Machine request creation reuses the same account-scoped request core as the signed-in verifier dashboard and requires immutable managed credential definitions. Browser sessions are not forwarded through the integration proxy.

Protocol-critical logic remains native Rust. Contextual policy evaluation, disclosure v0.2/v0.3, replay persistence, issuer issuance, verifier requests, holder consent, verifier-scoped holder proof keys, audience-bound proof construction, payment-intent/settlement verification, the read-only Z3 transport, protected credential storage, ZecAuth authentication and server sessions are implemented. Production wallet spending, managed KMS/HSM custody, stronger unlinkable credential systems, invoice automation and live FROST signing remain future work.

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


## Observed payment condition boundary (2026-10-04)

Live discovery supports `z_viewtransaction`. The adapter now checks a single named
transaction against a caller-pinned recipient, minimum integer zatoshis and minimum
confirmations, rejects change/transparent/unknown outputs, and returns only success
or unavailable/error. This is local issuer evidence, not a signed receipt by itself.
The issuer must bind receipts to invoices, prevent double credit, and revoke/reissue
on reorgs. The explicit regtest helper exposes transparent coinbase funding and
shielded receipt; it does not weaken `FullPrivacy` to make fully shielded sends work.
No wallet seeds/keys, transaction IDs, addresses or memos are written to public
fixtures. Generic credentials do not acquire wallet dependencies.

## Compound disclosure / invoice lifecycle decisions

The v0.3 extension signs 2–8 ordered atomic request descriptions with identical
origin, purpose, challenge, nonce and validity. It keeps v0.2 unchanged and verifies
all issuer attestations with one audience subject key before consuming one durable
replay row. Payment requirements use payment.invoice_paid with the canonical intent
digest as context; approval requirements use the same ordinary attestation primitive.
Payment receipt observations remain native issuer evidence, never verifier proof.

Exact settlement checks sum only external shielded outputs to a pinned recipient.
The payment ledger retains only intent ID/digest, txid and lifecycle status, uses a
unique transaction constraint, and rejects stale/cancelled/expired observations.
A receipt requires a mined block identity/time and a fresh named-transaction query;
reorg revocation and invoice retention/deletion remain issuer responsibilities.
No whole-wallet response, participant secrets or PCZT bytes enter browser data.
SQLite reuses the existing maintained rusqlite dependency for durable single-credit
protection. No cryptographic profile, trust root or subject enrollment changes.

PCZT interfaces bind explicit review to the exact plan and intent digest; inspection
is creator-claimed metadata, not cryptographic proof. External adapters must validate
extracted transactions against their original proposals. Shared control delegates
approval verification to reviewed external tooling. Native HTTP remains read-only;
discovery alone never authorizes spending. FROST is an interface, not live signing.
