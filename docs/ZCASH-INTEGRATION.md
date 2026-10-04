# Zcash integration

Status: read-only adapter implemented and exercised against the official local Z3 regtest router. No ZEC payment send/settlement flow has been exercised.

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

zerant-zcash currently provides a transport-independent, regtest-oriented adapter for rpc.discover capability discovery, getblockchaininfo minimal chain status, getwalletinfo minimal readiness projection, detection of whether z_sendmany appears in the live discovered method set, and construction of a minimal payment.invoice_paid=true claim shape after an authorized issuer has performed its own business/payment verification.

The adapter deliberately does not return wallet balances, addresses, seed fingerprints, transaction history or memos to generic credential code. No seed phrase/private spending key is accepted by the adapter.

## Payment status

A discovered z_sendmany method is only a capability flag. Zerant does not call it yet.

The official Z3 regtest stack was started during this implementation run using a temporary Compose provider over the local Podman API. That temporary Compose helper is no longer installed, while the resulting Z3 regtest containers remain available for inspection. Read-only calls to rpc.discover, getblockchaininfo and getwalletinfo succeeded through the local router. A legacy z_viewtransaction assumption was removed because it is not present in the current Z3 contract/source inspection.

No payment was sent. Before Zerant enables a payment-send path, it must use only the currently discovered/documented Zallet methods and validate recipient, amount, network, transaction binding and reorg/confirmation policy before an issuer signs a payment attestation. Captured fixtures must remain sanitized so no mnemonic/private wallet material is committed.

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
