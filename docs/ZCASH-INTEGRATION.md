# Zcash integration

Status: read-only adapter implemented and exercised against the official local Z3 regtest router. A synthetic coinbase-shielding payment was confirmed locally. Fully shielded `z_sendmany` and production settlement remain unimplemented.

## Workshop-derived architecture

The September 30 / October 1 / October 3, 2026 Zcash workshop sequence covered fundamentals, Z3 architecture/RPC, and FROST/wallet/payment infrastructure. Zerant uses those layers without making wallet state part of generic identity.

Current Z3 platform contract:

- Zebra validates the chain/full-node state.
- Zallet is the operator wallet/RPC boundary.
- Zaino is optional light-client/indexing infrastructure.
- Regtest exposes the merged RPC router on 127.0.0.1:8181, Zebra on 29232, Zallet on 50232, and optional Zaino gRPC on 28137.
- rpc.discover is the capability source for the merged regtest RPC schema.
- Z3 router tests currently expose Zallet methods including getwalletinfo and z_sendmany.

Primary references:
- https://github.com/ZcashFoundation/z3/blob/main/docs/contract.md
- https://github.com/ZcashFoundation/z3/blob/main/docs/regtest.md
- https://github.com/ZcashFoundation/z3/tree/main/rpc-router
- https://github.com/ZcashFoundation/frost

## Zerant boundary

zerant-zcash currently provides a transport-independent, regtest-oriented adapter for rpc.discover capability discovery, getblockchaininfo minimal chain status, getwalletinfo minimal readiness projection, detection of whether z_sendmany appears in the live discovered method set, named-transaction confirmation and exact payment-condition checks, and construction of a minimal payment.invoice_paid=true claim shape after an authorized issuer has performed its own business/payment verification.

The adapter deliberately does not return wallet balances, addresses, seed fingerprints, transaction history or memos to generic credential code. No seed phrase/private spending key is accepted by the adapter.

## Payment status

A discovered `z_sendmany` method is only a capability flag. The native adapter does not send funds. Explicit local exploratory calls were rejected; no successful fully shielded send is claimed.

The official Z3 regtest stack was started using a temporary Compose provider over
Podman. Read-only discovery, chain and wallet calls succeeded through the router.
`z_viewtransaction` is present in the **live merged OpenRPC** and was exercised
successfully against the confirmed synthetic transaction; it is also described by
[current Zallet source documentation](https://github.com/zcash/zallet/blob/main/book/src/zcashd/json_rpc.md).
It is not inferred from retired zcashd documentation.

The local coinbase-shielding path was exercised through `z_getnewaccount`,
`z_getaddressforaccount`, `z_listunifiedreceivers`, `generatetoaddress`,
`z_shieldcoinbase`, `z_getoperationstatus` and `z_viewtransaction`. The transaction
was mined with three confirmations. Its shielded output matched the expected
recipient and a minimum of 100,000,000 zatoshis, and was not wallet-internal change.
Only this named transaction was inspected. The sanitized fixture omits addresses,
transaction IDs, memos and account identifiers. Native `matches_payment` and
`confirmations` were exercised against it through the real HTTP router.

This path reveals the transparent coinbase sender; it is **not a fully shielded
send**. Regtest activates upgrades through NU6.3 at height 2. The shipped wallet
reported the shielded output pool as `ironwood`. A subsequent `z_sendmany` with
`FullPrivacy` was rejected with insufficient selectable balance for derived
Unified Addresses, despite the wallet reporting shielded pools. No weakened
privacy fallback or fully shielded transfer success is claimed. Receiver API accepts
p2pkh/sapling/orchard; asking for an ironwood receiver was explicitly rejected.
These are observed version-specific results, not general Zcash guarantees.

`matches_payment` validates one recipient, minimum amount, non-change shielded
output and confirmation count. The issuer still owns invoice/receipt binding,
double-credit prevention, reorg handling and credential revocation. It must not sign
paid-invoice claims solely from confirmation count. The generic claim carries only
a contextual boolean and verification metadata, never wallet-wide evidence.

Reproduce after official regtest initialization with
`Z3_REGTEST_RPC_ROUTER_PASSWORD=<local value> python3 scripts/z3-payment-demo.py --execute-regtest`.
This explicit helper proves regtest-only `generate` succeeds before wallet changes,
mines synthetic funds, shields one coinbase and checks the exact condition. It prints
only sanitized status. Failures report no successful condition. Wallet seeds stay in
Zallet; no real funds or private key export is involved.

## FROST

Zcash Foundation FROST is an optional organizational custody/signing capability for team/business treasury approval or issuer-key custody. Zerant currently exposes only signer interfaces and implements no FROST cryptography. FROST is not required for holder flows and must not be presented as integrated until an actual compatible signing path is exercised.

## Live execution record

On 2026-10-04, official `regtest-init.sh` succeeded after installing a temporary
Compose v2.39.4 provider and starting a local Podman API socket. Fedora SELinux required
`:Z` labels on the temporary checkout's config bind mounts. No changes to upstream
Z3 were committed. Zebra reported regtest activation height 2 and Compose readiness.
The full regtest stack started. Through router 8181, `rpc.discover`,
`getblockchaininfo` and `getwalletinfo` succeeded. Both `scripts/z3-check.py` and the
Rust `regtest_check` example exercised the actual router. Sanitized method signatures
and status live in `fixtures/z3-regtest-*.json`. Wallet info only establishes RPC
reachability on this shipped version; it does not establish spend readiness.

The concrete Rust transport uses maintained reqwest 0.13.5 with no TLS feature for
the fixed loopback HTTP endpoint. [Client builder documentation](https://docs.rs/reqwest/0.13.5/reqwest/blocking/struct.ClientBuilder.html)
covers timeout, redirect and proxy configuration. It rejects unallowlisted methods,
non-success HTTP, RPC errors, mismatched IDs and oversized responses.

Official Z3 checkout inspected at `e84ce9fd8e864ff0b2a8a62f6ce14392145db0fb`.
Method names and parameter order are recorded from live discovery in
`fixtures/z3-regtest-discovery.json`, not generated from guessed legacy RPCs.

## Workshop-aligned integration map

The September 30, October 1, and October 3, 2026 workshop sequence maps directly to the implementation:

- **Fundamentals:** Zerant keeps Zcash settlement separate from generic identity and explicitly distinguishes transparent versus shielded output pools.
- **Z3 / RPC:** the adapter uses runtime `rpc.discover`, Zebra/Zallet readiness projections, strict loopback RPC transport, and named-transaction verification instead of retired `zcashd` assumptions.
- **FROST / wallets / payments:** payment intents use explicit privacy policy and integer zatoshis; PCZT is modeled as review-first coordination; FROST remains an optional external shared-control signer boundary.

### Runtime capability matrix

| Capability | Zerant handling | Current claim |
| --- | --- | --- |
| `rpc.discover` | bounded method discovery | implemented and previously exercised on local Z3 regtest |
| `getblockchaininfo` | minimal regtest readiness projection | implemented and previously exercised |
| `getwalletinfo` | minimal readiness only; balances/seeds not exposed | implemented and previously exercised |
| `z_viewtransaction` | exact named-transaction settlement observation | implemented and previously exercised for synthetic regtest shielded receipt |
| `z_sendmany` | detected; result parser checks `broadcast` and unambiguous txid | capability-gated; not exposed by the read-only HTTP transport |
| `z_sendfromaccount` | detected independently | capability-gated; availability depends on Zallet build |
| `pczt_create/combine/inspect/prove/sign/extract` | detected per RPC; complete set enables review-first plan | capability-gated; the previously captured Z3 OpenRPC fixture did not advertise the complete PCZT set |
| FROST | external coordinator/signing interface only | no FROST cryptography or live threshold signing in Zerant |

Zallet is beta software, so runtime discovery wins over brand/version assumptions. A newer Zallet build can expose capabilities that an older captured Z3 fixture did not. Zerant must adapt to the actual discovered contract and still apply its own consent/security policy.

The concrete `HttpRegtestTransport` remains authenticated, loopback-only, proxy/redirect-disabled, bounded, and read-only. Spending RPCs are not allowlisted there. This lets the protocol reason about spend paths without turning a public demo or generic verifier into a wallet controller.

## Repeatable authenticated checks

The repository keeps the previously verified live discovery capture in
`fixtures/z3-regtest-discovery.json` and the sanitized historical payment observation
under `fixtures/z3-regtest-payment.json`. They describe the local build that was
actually exercised; they are not a substitute for runtime discovery on a newer Zallet.

```sh
Z3_REGTEST_RPC_ROUTER_PASSWORD=<local-regtest-password> make z3-check

cargo run -p zerant-zcash --example verify_payment -- --fixture \
  < fixtures/payment-verification-input.json

# Canonical private input for one named regtest transaction:
Z3_REGTEST_RPC_ROUTER_PASSWORD=<local-regtest-password> \
  cargo run -p zerant-zcash --example verify_payment -- --live-regtest \
  < private-canonical-input.json

# Explicit synthetic funding / coinbase shielding, never mainnet:
Z3_REGTEST_RPC_ROUTER_PASSWORD=<local-regtest-password> \
  python3 scripts/z3-payment-demo.py --execute-regtest
```

All concrete repository RPC helpers require authentication and use the fixed loopback
router. CI uses deterministic fixtures and never starts a node or sends funds. Runtime
`rpc.discover` remains authoritative for whether `z_sendmany`, `z_sendfromaccount`, or
individual PCZT methods are available on the operator's current Zallet build.

## ZecAuth server authentication

Legacy account-link endpoints use the existing five-minute ZecAuth challenge message. Their dedicated endpoint stores the target account and initiating recent session on the server and accepts either RedPallas wallet-app or derived injected-wallet verification. The ordinary sign-in verifier cannot consume a link challenge. A linked key cannot be moved from another account or silently replace a different key, and the account's public handle stays fixed. Current account settings do not offer new wallet linking; these server paths remain for compatibility and existing-account safeguards. Their public callback can store a pending key only and cannot create an account or session. Passkeys provide current wallet-independent account entry.

Linked ZecAuth and chain-specific derived wallet-message sign-in methods from earlier product versions can be removed through account settings with a session created within 15 minutes. The last account access method cannot be removed. The selected authentication identity and every Zcash-authenticated Zerant session are deleted together; sessions do not record the individual key, while passkey sessions remain. A current Zcash session is signed out. New linking is not exposed in the current UI. This neither revokes a Zcash spending key nor deletes a wallet account.

`zerant-api` implements the server-verification side of the ZecAuth v1 draft profile. It issues five-minute domain/chain/nonce challenges, verifies RedPallas public keys and signatures with `reddsa`, consumes each challenge once, and creates opaque HttpOnly sessions only after the browser redeems its completed server-side authentication attempt.

The default capability allow-list is `auth` plus `request_payment`. Broader viewing capabilities must be explicitly enabled by the operator. The authentication key is a pseudonymous login identity only and is never interpreted as a Zcash address or spending key.

Authenticated users can query bounded Z3/Zallet capability, chain-readiness and wallet-readiness state through the Rust API. Raw balances, seed fingerprints, address inventories and wallet history are not returned to the browser.

## Wallet interoperability model

Zerant is wallet-agnostic. It uses portable Zcash handoff formats wherever a wallet supports them and isolates browser-specific behavior behind capability-based adapters.

### Legacy portable wallet-app authentication

ZecAuth remains implemented as a compatibility authentication handoff for earlier linked accounts, but it is not exposed as the current sign-in path. The server creates a short-lived domain/chain/nonce challenge and can redeem a valid completed handoff into the normal opaque account session. Current product sign-in is passkey-first.

### Retained injected-wallet adapter research

The repository retains reviewed injected-adapter and capability tests from the earlier browser-wallet prototype. The current product UI does not render a wallet chooser, connection state, or Noir-specific flow. These modules are compatibility research and can inform future integrations without changing credential, disclosure, verifier, or passkey semantics.

### Wallets with payment support but no authentication extension

A wallet may support Zcash addresses and ZIP-321 payment requests without implementing ZecAuth or browser message signing. Zerant treats that as a capability difference, not as a non-Zcash wallet. Portable payment interoperability remains available. Passkeys provide wallet-independent Zerant account entry.

See `docs/WALLET_COMPATIBILITY.md` for the adapter and security contract.

## ZIP-321 wallet execution

The product now treats the canonical ZIP-321 URI as the portable wallet handoff format.

After the Rust parser validates and canonicalizes a request:

- any compatible wallet can receive the complete canonical `zcash:` URI through the external-wallet handoff;
- Zerant renders a QR code for the same reviewed URI;
- a single exact-amount request without extra fields may also expose the validated recipient and amount for deliberate manual entry;
- multi-recipient, memo-bearing, or otherwise richer requests never get reduced to a lossy manual form.

Zerant does not read a wallet balance or transaction history before offering this handoff. Spending approval happens outside Zerant.

## Legacy browser connector boundary

This section documents compatibility behavior retained in source and tests. It is not a current product interaction: the normal Vault uses passkeys, the payment workspace uses ZIP-321 handoff, and no product screen presents a persistent browser-wallet connection state.

The earlier browser prototype routed through explicit injected-wallet adapters, ZecAuth authentication handoff, and canonical ZIP-321 payment handoff. Those adapters remain isolated compatibility code rather than current navigation. WalletConnect is intentionally not used by product discovery because its current namespace does not solve the hosted testnet flow. The native Zcash payment parser remains the authority for canonical ZIP-321 validation, and any external wallet submission remains separate from settlement verification.

### Production light-client readiness

For hosted network readiness, Zerant uses the maintained `zcash_client_backend` lightwalletd-compatible gRPC client. Operators may configure an ordered primary plus up to three trusted HTTPS endpoints. The service validates the expected Zcash network, bounds requests and responses, caches successful checks briefly, backs off failures, and coordinates refreshes through PostgreSQL so autoscaled instances share one recent observation.

Only network-readiness metadata is persisted: network, a SHA-256 fingerprint of the ordered endpoint configuration, bounded heights/lag, timestamps and failure count. Zerant does not persist the endpoint URL, wallet addresses, balances, transaction history, memos or seed material in the readiness table. Endpoint failover is availability logic only; it is not wallet authority and does not authorize spending. Zerant also keeps the highest successfully observed block height for each network. A candidate endpoint may fall at most 20 blocks behind that high-water mark; a larger rollback is treated as stale/untrusted readiness and Zerant tries the next configured endpoint or fails closed. This is service-integrity protection, not consensus validation. During a transient outage Zerant may display the last confirmed good network heights for at most 15 minutes as a `degraded` status. Degraded observations never enable network-dependent actions; they are informational only. After the bounded display window expires, readiness fails unavailable until a fresh valid observation succeeds.
