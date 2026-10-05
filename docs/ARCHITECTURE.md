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

The issuer UI snapshots the recipient Zerant ID, active credential type, claim,
context, and validity for a separate review step before issuance. Editing returns
to the draft. The service still rechecks issuer role, recipient existence, type,
claim bounds, and signing authority at issuance; the UI review is an error
prevention step and never grants authority. It requests no wallet information.

## Load-bearing decisions

### Holder proof preview binding (2026-10-05)

Before an authenticated holder approves a pending verification request, the
service returns a holder-only preview of the exact claim value and responsible
issuer selected under the request's issuer allowlist and credential context. The
preview includes the requester, authenticated origin, purpose, and expiry; it
returns no source credential, signature, wallet information, or other credential
history. The decision request must echo the previewed value and issuer ID. The
service repeats credential eligibility, revocation, request-expiry, audience,
issuer-trust, and replay checks, then rejects approval if the currently selected
value or issuer differs. Denial requires no preview and emits no proof. Preview
does not reserve consent, persist another credential copy, or grant the verifier
access. A changed request or available credential requires another holder review.

### Explicit Noir connection gesture (2026-10-05)

The user-initiated Noir connection calls `zcash_requestAccounts` directly. Silent
`zcash_getAccounts` is reserved for restoring an existing site authorization; a
preapproval rejection from that read must not block the wallet's approval prompt.
Connection grants only the wallet capabilities the user approves and never grants
Zerant sign-in or spending consent. No wallet address, balance, history, or key is
persisted by this change. Testnet settlement remains unsupported without an
authorized named-transaction observer.

Direct browser wallet sessions are admitted only after the returned account
addresses identify the configured Zcash network locally. A mainnet Noir extension
may be detectable on a testnet site because the injected SDK has no safe preapproval
network query; detection is not compatibility. An unrecognized or mixed-network
account fails closed before Zerant offers wallet payment actions. This check reads
only the wallet's connection result in browser memory and never sends the addresses
to the service or persists them. It does not validate spending capability or prove
settlement. Other wallets may use the complete canonical ZIP-321 request without a
live connection; URI handoff is available only after server validation and exact
network matching. Adding arbitrary wallet providers requires reviewed adapters,
not broad discovery of browser globals.

### Discoverable passkey sign-in (2026-10-05)

Zerant may offer account sign-in without typing a Zerant ID when the device supplies a discoverable passkey. The WebAuthn challenge is generated and persisted by the service for five minutes using a distinct ceremony kind and a one-time HttpOnly attempt cookie. Its account UUID is a nil sentinel until the authenticator response identifies an account UUID and credential ID; the service looks up that exact account and stored credential, then validates the signed assertion with `webauthn-rs` before creating a session. The challenge is locked and consumed transactionally, and credential counter updates use the same path as account-selected passkey login. The existing Zerant-ID sign-in remains available for non-discoverable passkeys. This access change grants no wallet or payment authority and does not change credential subject binding, consent, key custody, revocation, or verifier replay handling. The service stores no additional identity or device profile data beyond the existing passkey record and short-lived ceremony.

### Server-backed Zcash payment tracking (2026-10-05)

Payment-request review rejects destinations on a network other than the configured Zcash chain, so a testnet review cannot hand off a mainnet destination.

The authenticated Zerant service owns a payment record for one validated, exact-amount ZIP-321 payment on the configured Zcash network. Preparation binds the account, a random record ID, SHA-256 of the canonical request, normalized recipient, integer zatoshis, and network. The canonical URI, memo, wallet account, balance, and history are not stored. Requests with multiple payments, unspecified amounts, memo, label, message, or extra parameters remain reviewable but cannot create a tracked payment in this profile. A submitted txid is bound once to the account-owned prepared record and is unique across records; retries with the same txid are idempotent. A wallet-returned txid is untrusted submission metadata, never settlement evidence. Only a future server-side named-transaction observer may advance beyond submitted; the existing native verified receipt path is regtest-only and must not be used to assert testnet settlement. Browser input never authorizes arbitrary transaction lookup.

Prepared records expire after 24 hours. The service lists only records owned by the authenticated account and deletes expired prepared records after 30 days; submitted records remain pending until a reviewed observation and retention policy is implemented. This interim retention favors preserving the sole pending payment reference over silently losing it. Future confirmation/credit needs a durable txid tombstone and explicit reorg/retention rules before any submitted record is deleted. This tracking does not change credential subject binding, consent, key lifecycle, revocation, or replay contracts. It creates account-to-payment metadata within the protected service, and therefore does not confer anonymity or unlinkability.

Testnet observation remains a separate trust boundary. [Lightwalletd's named `GetTransaction`](https://github.com/zcash/lightwalletd/blob/master/frontend/service.go) returns a raw transaction; this alone cannot reveal the recipient and exact amount of a shielded output. [Zallet's `z_viewtransaction`](https://github.com/zcash/zallet/blob/main/book/src/zcashd/json_rpc.md) is a wallet view and can include decoded outputs plus mined status and confirmations, but Zerant currently has no recipient viewing authority for arbitrary holder-initiated payments. A future payment issuer or recipient must explicitly supply an authorized, narrowly scoped viewing/observation service, and Zerant must verify its exact named transaction against the immutable request digest before advancing state. Do not turn general network readiness or a raw transaction lookup into settlement evidence; no testnet confirmation job is scheduled until this boundary is implemented and tested.

### Wallet connector discovery (2026-10-05)

Browser wallet selection is a client-side registry of explicit, reviewed connectors. Detection only reads known provider entry points and never requests accounts, addresses, balances, history, or remote metadata. Identity methods are offered only for connectors that can sign the exact server-issued challenge in the verified profile, or can complete the existing ZecAuth handoff. The server remains authoritative for challenge creation, signature verification, session redemption, and account linking. Connector metadata and capability flags do not grant authority. The ZecAuth callback contains only the public challenge and callback URL; the initiating browser retains the HttpOnly attempt cookie, and account/session IDs never enter the handoff. No connector discovery state or wallet address is persisted by Zerant.

`zcash-connectors.ts` is the sole product-facing registry for sign-in, account linking, general connection, and payment. Its capability contract requires an implemented operation for every advertised capability. `zcash-connection.ts` stores only the selected connector, normalized transient account/session, network configuration, pairing URI, and connection status; it does not discover wallets. The shared selector opens from the Vault sign-in or workspace wallet action and filters the registry by the current purpose. ZecAuth and ZIP-321 are portable auth and payment routes, respectively, rather than wallet brands. The optional WalletConnect route exists only with a configured public project ID on Zcash mainnet. Before pairing it advertises connection and restoration only. After a verified session with `zcash_transfer` and a transparent account, that connector may advertise transparent payment; disconnect removes it. It never gains Zerant identity or shielded-send authority. Payment routing preserves the full canonical ZIP-321 URI whenever a direct send cannot express every field. A direct shielded send requires an exact simple request to a shielded or Unified recipient and an adapter with that method. A transparent recipient requires a separately supported transparent method and explicit action; no shielded request is downgraded. Submitted transactions are not settlement evidence. These client changes do not alter subject binding, consent, storage, key lifecycle, revocation, replay handling, or retention.

The signed-out Vault uses the identity purpose, so its wallet choices can actually complete Zerant sign-in. It does not restore a payment-wallet connection on mount; wallet discovery starts when the person opens the selector. The authenticated workspace exposes general wallet connection beside the existing payment-request review; its portable ZIP-321 choice points to that review rather than posing as account access. ZecAuth sign-in remains a separate server-issued challenge and wallet-app approval. These are UI routing decisions only: neither a payment-only connection nor a WalletConnect session becomes Zerant identity, and payment review continues to require an authenticated session.

Nozy's developing extension has no verified stable derived-signature contract for Zerant, so no Nozy identity adapter is enabled. Noir's documented WalletConnect reference requests only address, balance, and transparent transfer methods; no WalletConnect sign-in or direct shielded payment is enabled. Adding either requires a reviewed wallet-side contract and negative tests. ZIP-321 remains the exact portable payment request. A direct injected send is permitted only for a request fully represented by the adapter's parameters; a wallet-submitted transaction ID is not settlement. This connector change does not change credential subject binding, server storage, key lifecycle, revocation, replay handling, or retention. Existing ZecAuth five-minute expiry and one-day challenge cleanup remain in force.

### Zcash sign-in identity linking

An account-link challenge is a separate purpose in the existing five-minute ZecAuth challenge store. Its row binds the target account UUID and initiating session UUID; neither is accepted from a signed callback or verification body. Issuance requires a session created within 15 minutes and sets a separate 256-bit link-attempt cookie (`HttpOnly`, `Secure`, `SameSite=Lax`); only its SHA-256 hash is stored. The cookie is distinct from the ordinary sign-in attempt. A wallet-app callback verifies the RedPallas signature for an unexpired, unconsumed link challenge and stores one bounded pending authentication public key on that exact row. It cannot create an account or session or link an identity. The first valid key wins; same-key retries are idempotent and a different key is rejected. The initiating browser completes with its exact attempt cookie and still-active, recent, matching session; the transaction consumes the challenge and clears the attempt cookie only after identity ownership and slot checks succeed. No account or session identifiers enter the wallet callback URL or request body. The injected-wallet path retains direct verification and the same transaction-bound session checks. Ordinary sign-in verification accepts only challenges without a link target, so responses cannot cross purposes. The session UUID is deliberately not a foreign key: deleting a session leaves an expiring challenge row, while completion fails closed because it must lock the live session with the same token, UUID, and account.

The identity tables retain one ZecAuth key per account and one derived wallet key per account/chain. Existing ownership by another account or a different key in the target slot causes conflict; matching keys are idempotent. Linking never updates the account's public handle. Raw authentication keys stay in the protected service database, and the account read model exposes only method, stored chain where applicable, and creation time. Challenges expire after five minutes and old rows follow the existing one-day cleanup. Session revocation/deletion, session expiry, and loss of recent authentication all fail closed. This is account authentication only: no wallet address, balance, history, seed or spending key is requested or stored. A compromised recent session plus control of an unlinked authentication key could add an access method, so session protection and short reauthentication age remain material controls.

### Zcash sign-in identity removal

The account owner may delete the single ZecAuth identity or the wallet-message identity for an explicit testnet/mainnet chain using a session created within 15 minutes. Removal takes the account row lock before counting the current passkeys, ZecAuth identity and per-chain wallet-message identities; passkey removal uses the same lock. Both removal paths recheck that the exact initiating session is still active and recent under that lock, so a session revoked while waiting cannot delete another method. The target must exist and at least one access method must remain. The identity deletion and deletion of every `auth_method = 'zcash'` session for that account commit together. Sessions do not record the specific Zcash key/chain that authenticated them, so all Zcash sessions are conservatively revoked; passkey sessions remain. A removed current Zcash session receives a clearing session cookie. This changes only account authentication state, not the public handle, credentials, memberships, or payment authority. Rotation requires another access method, explicit removal, then an explicit link under the existing ownership/conflict rules. Removed authentication keys have no tombstone and may later be used under ordinary sign-in/linking rules; removal does not revoke any wallet spending key.

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

### Browser Zcash wallet capability routing

The browser discovers reviewed injected providers through explicit detector functions. It also offers a ZecAuth authentication handoff and canonical ZIP-321 payment handoff without a live wallet session. A configured WalletConnect project ID enables a lazily loaded mainnet `bip122:00040fe8ec8471911baa1db1266ea15d` session connector. WalletConnect requests only `zcash_getAddress` to connect; `zcash_transfer` is optional and is enabled for direct transparent payments only when the approved session advertises it. Connection and restoration never query balances or history. There is no universal Zcash dApp discovery standard, so only registered providers appear as installed wallets. A wallet without compatible browser or WalletConnect interfaces can still use supported URI handoffs.

Connection state is transient browser state and contains only selected connector, capability flags, connection status and minimal account metadata. Addresses from a wallet response are never sent to Zerant authentication endpoints or treated as account identity. Only an adapter advertising safe derived identity signing may use the existing server challenge and verification flow. ZecAuth uses its existing, separate server-controlled handoff. WalletConnect currently carries no Zerant identity signing authority. A payment-only connection does not create a Zerant session; passkeys remain an independent account method. Disconnect ends the selected wallet session where the provider supports it and clears browser connection state; it does not revoke a Zerant account session.

A connector may submit a direct payment only for a single exact amount with no memo, label, message or extra ZIP-321 parameters, and only in the explicitly advertised funding mode matching the recipient address class. Transparent payment requires a separate user action and may reveal sender/recipient/amount on chain. Every validated request retains its canonical ZIP-321 URI for portable handoff. Wallet submission is not settlement verification. This browser routing introduces no new credential trust, revocation, replay or retention contract; the existing backend identity and payment verification boundaries remain authoritative.

### Vault key-provider boundary

Envelope encryption is split into data encryption and data-key wrapping. VaultCipher owns the stable record/AAD contract while a VaultKeyProvider supplies the active wrapping-key version and versioned wrap/unwrap operations. The current concrete provider is the local AES-256-GCM keyring loaded from server-only deployment secrets. Rotation and all encrypted record families use provider methods rather than direct key-map access.

This boundary deliberately does not claim managed KMS/HSM custody yet. A future adapter can replace the wrapping provider while preserving existing ciphertext rows and version migration rules.
