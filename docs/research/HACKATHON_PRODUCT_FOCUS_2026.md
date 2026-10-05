# Zerant product focus for the 2026 Zcash hackathons

Reviewed 2026-10-05. This is a scope decision, not a claim that future payment
verification or wallet interoperability is already implemented.

## User problem and demo

A community, grant program, or organization needs to attest to one fact it knows;
a person needs to receive that fact without publishing a complete profile; and a
verifier needs a bounded answer with the person's consent. The complete demo path
is organization setup → credential type → issuance to a holder Zerant ID → holder
reviews a verifier request → verifier receives one narrow signed result. Wallet
connection must not be a prerequisite. A Zcash payment is an optional second path,
with wallet approval and server-observed settlement kept separate from credential
identity.

## What the supplied resources establish

- [ZIP 316](https://zips.z.cash/zip-0316) defines Unified Addresses and receiver
  handling; a Zerant ID is not one of these addresses.
- [ZIP 321](https://zips.z.cash/zip-0321) defines portable payment-request URIs.
  Zerant already parses and prepares exact requests. A URI hands work to a wallet;
  it is not evidence that a payment settled.
- [librustzcash](https://github.com/zcash/librustzcash) and
  [Zebra](https://github.com/ZcashFoundation/zebra) are maintained native wallet
  and node foundations. Zerant's existing `zerant-zcash` crate and private network
  boundary should continue to own Zcash logic.
- [Zingo](https://z.cash/ecosystem/zingo/) and
  [ZODL](https://github.com/zodl-inc) demonstrate separate wallet products.
  Their existence does not establish a tested browser connection or ZIP-321
  handoff for Zerant. Do not advertise one without interoperability evidence.
- [NEAR Intents](https://docs.near-intents.org/) documents a cross-chain swap
  system. It does not solve Zerant's credential consent or exact Zcash shielded
  settlement verification. It is outside the first complete Zerant flow.
- The [Zcash testnet guide](https://zcash.readthedocs.io/en/latest/rtd_pages/testnet_guide.html)
  identifies testnet coins as distinct from ZEC. The hosted product stays on
  `zcash:testnet`; no mainnet claim follows from a testnet demo.

## Hackathon fit and deadlines

[Colosseum Crypto World's Fair](https://colosseum.com/worldsfair) lists an
October 12, 2026 submission deadline and ecosystem tracks. Zerant's credible
submission is a working Zcash ecosystem trust product with a complete issuer,
holder, consent, and verifier walkthrough. The product must show real state and
security boundaries rather than more protocol scaffolding.

[ZECATHON](https://thezecathon.com/) lists a $100,000 pool, five tracks, and an
October 28, 2026 submission deadline in its event information. Contextual
identity/credential disclosure is a plausible Wildcard or Core & Tooling story;
Shielded Payments becomes a credible story only after trustworthy settlement
observation. Treat prizes and track details as event metadata to recheck before
submission.

## Near-term build order

1. Make passkey access, Zerant ID delivery, issuer creation, holder consent, and
   verifier requests understandable and reliable without a wallet.
2. Keep the saved payment lifecycle honest: prepared and submitted are supported;
   observed and confirmed require a narrowly authorized named-transaction view.
3. Exercise the actual Testnet Noir extension connection and a reviewed
   ZIP-321 wallet handoff with real user environments; add only verified adapters.
4. Complete a trustworthy testnet observer before payment-derived credentials or
   settlement claims.

Privacy constraint: neither a wallet connection nor a payment address becomes a
Zerant account identifier. No global reputation score or wallet-history import.
