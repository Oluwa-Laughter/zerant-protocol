# Zcash Resources Used by Zerant

Zerant treats maintained Zcash specifications and Rust libraries as implementation dependencies, not as inspiration for custom replacements.

## Current integrations

### ZIP 316 / Zcash addresses

`zcash_address 0.14.0-pre.0` is used by `zerant-zcash::address` for canonical address parsing and bounded capability inspection.

The API returns only:

- canonical encoded address;
- network;
- high-level address kind;
- whether the address is Unified;
- memo capability;
- transparent-only status;
- Orchard, Sapling, and transparent receive capability.

Receiver bytes and wallet key material are not returned to the browser. Transaction receiver selection remains a wallet responsibility.

### ZIP 321 payment requests

`zip321 0.10.0-pre.0` from librustzcash is used by `zerant-zcash::zip321`.

Zerant accepts a bounded payment-request URI, delegates parsing and canonical rendering to the reference crate, and returns a consent-oriented summary:

- canonical ZIP-321 URI;
- recipient address;
- requested amount in zatoshis;
- human-readable label and message;
- whether a memo is present;
- names of additional parameters.

Raw memo bytes are intentionally not returned by the generic review endpoint. The browser does not implement ZIP-321 parsing.

### Z3 / Zebra / Zallet

The existing native adapter performs runtime RPC discovery and bounded readiness projections. Browser code never receives Z3 credentials or direct wallet RPC access.

The current concrete HTTP transport remains authenticated, loopback-only, read-only, and regtest-oriented. Production wallet sending is therefore not claimed.

### PCZT

PCZT is treated as the preferred review-first transaction path when the running Zallet advertises the complete create / inspect / prove / sign / combine / extract capability set.

Zerant does not reimplement PCZT cryptography or serialize PCZT material into generic browser state.

### FROST

FROST remains an external threshold-signing boundary for organizations, teams, and shared treasuries. Zerant coordinates policy and approval state but does not implement custom FROST cryptography.

### Browser wallet connectivity

Zerant now includes a lightweight injected-wallet adapter boundary. The current concrete browser extension integration follows the public Noir Zcash provider RPC contract, while Zerant's internal wallet interface remains vendor-neutral.

Authentication uses only connection approval plus a derived Zcash signed-message identity. Zerant intentionally does not query balances or transaction history during sign-in. Other wallets can integrate through the same capability interface or through the ZecAuth wallet-app path.

### ZecAuth

The Rust API implements the server-verification side of the ZecAuth v1 draft model:

1. create a short-lived domain / chain / nonce challenge;
2. verify the RedPallas signature;
3. consume the challenge once;
4. bind the authenticated account to a purpose-specific Zcash authentication key;
5. issue an opaque HttpOnly server session.

Authentication identity is kept separate from payment addresses and spending authority.

## Next maintained integrations

The next wallet/network work should build on maintained Zcash components rather than bespoke code:

- `zcash_client_backend` for shielded light-client synchronization and wallet APIs;
- Zaino or lightwalletd for compact-block / light-client service access;
- Tor support through maintained Zcash client tooling when network privacy is required;
- native mobile SDKs only when Zerant ships platform-specific wallet surfaces.

Any new integration must preserve the current rule: protocol and wallet authority stay behind native/server boundaries; the browser remains a presentation and consent surface.
