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

## Current capability matrix and repeatable commands

The running loopback router was re-probed in this session on 2026-10-04. Its read-only
methods also accept requests without authentication. That is an explicitly selected
local deployment mode (`Z3_REGTEST_UNAUTHENTICATED=1`), never an automatic auth fallback.
Do not expose this router publicly. No credentials were recovered from another project.

| Capability | Running router | Zerant support | Evidence |
| --- | --- | --- | --- |
| rpc.discover | available | bounded names / per-method state | exercised Python and Rust |
| getblockchaininfo / getwalletinfo | available | minimal status | exercised Python and Rust; wallet RPC reachability is not safe sync |
| z_viewtransaction | advertised | exact intent observer and verified local receipts | deterministic mocks; prior live named transaction record retained |
| z_sendmany | advertised | path selection and strict broadcast-result parser | no fully shielded send success in this session |
| z_sendfromaccount | absent | capability detection only | live discovery |
| pczt_create / inspect / prove / sign / combine / extract | absent | native backend/review interfaces | live discovery and deterministic review tests |
| FROST | no integrated coordinator | authenticated shared-control adapter boundary | unit tests only; no threshold cryptography |

Current [upstream Zallet PCZT source](https://github.com/zcash/zallet/tree/main/zallet-core/src/components/json_rpc/methods)
contains those six methods, while the running build does not advertise them.
[pczt_inspect](https://github.com/zcash/zallet/blob/main/zallet-core/src/components/json_rpc/methods/pczt_inspect.rs)
reports creator-claimed metadata; extraction must verify it. The older
[RPC book page](https://github.com/zcash/zallet/blob/main/book/src/zcashd/json_rpc.md)
still links to pending PCZT work. Runtime discovery wins. No guessed PCZT calls or
privacy downgrade were used. [FROST](https://github.com/ZcashFoundation/frost) remains
optional reviewed external signing; Zerant supplies no participant shares or algorithm.

```sh
# Use this flag only if your isolated router is deliberately configured without auth.
Z3_REGTEST_UNAUTHENTICATED=1 make z3-check
Z3_REGTEST_UNAUTHENTICATED=1 python3 scripts/z3-capabilities.py
cargo run -p zerant-zcash --example verify_payment -- --fixture < fixtures/payment-verification-input.json
# Canonical stdin with one named txid, expected intent, healthy clock and authenticated origin:
cargo run -p zerant-zcash --example verify_payment -- --live-regtest < private-canonical-input.json
# Explicit synthetic funding / coinbase shielding, never mainnet:
Z3_REGTEST_UNAUTHENTICATED=1 python3 scripts/z3-payment-demo.py --execute-regtest
```

The last shielding helper attempt generated synthetic regtest funding but failed
closed after `z_getoperationstatus`, before reporting a payment condition. No new confirmed settlement or shielded
send is claimed. The helper now requires broadcast=true rather than accepting missing
broadcast metadata. Wallet scan/sync and beta selectable-funds limitations remain.
Only sanitized method signatures are captured in z3-current-capabilities.json.

For booting an isolated stack use the official
[Z3 regtest guide](https://github.com/ZcashFoundation/z3/blob/main/docs/regtest.md)
and its regtest initialization scripts. CI uses fixtures and does not boot a node.
