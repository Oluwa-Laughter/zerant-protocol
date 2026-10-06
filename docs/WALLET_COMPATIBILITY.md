# Zcash wallet compatibility

Zerant is wallet-agnostic. Wallet choice must not change credential, issuer, verifier or consent semantics.

## Compatibility layers

### 1. Portable wallet handoff

These paths do not depend on a browser-extension vendor:

- **Authentication:** Zerant can create a short-lived `zecauth:` request for wallet apps that implement the ZecAuth handoff draft.
- **Payments:** Zerant uses canonical ZIP-321 `zcash:` payment-request URIs. A wallet that understands ZIP-321 can receive the exact validated request without Zerant translating it into a vendor-specific transaction format.
- **QR and manual payment:** The payment page renders the complete URI as a QR code in the browser. For one exact-amount payment with no memo or extra fields, it also shows the validated recipient and decimal ZEC amount for manual entry in a testnet wallet. The holder must compare both fields in that wallet. Multi-output and memo-bearing requests keep the complete URI because manual entry would omit required details.
- **Addresses:** Zerant validates Zcash addresses through the maintained Zcash address libraries rather than a wallet-specific parser.

Portable handoff is the preferred interoperability layer because the wallet remains responsible for keys, transaction construction and user approval.

### 2. Injected wallet adapters

Browser wallets may expose richer capabilities. Zerant models those through `InjectedZcashWalletAdapter` rather than importing wallet behavior into the product core.

An adapter advertises capabilities independently:

- connection restoration;
- private identity-message signing;
- direct shielded payment submission.

A wallet can support any subset. Zerant must not assume that connection implies signing, or that signing implies payment submission.

Fresh connection discovery is passive. Opening Zerant's wallet chooser may detect a reviewed provider and load Zerant's public network configuration, but it must not call the provider's account methods before the user selects a wallet. For Noir, the first account RPC in a fresh connection flow is the user-triggered interactive connect request. Silent account lookup remains a restoration or recovery primitive and is not part of chooser discovery.

Noir Wallet is currently one concrete injected adapter. It is not Zerant's wallet protocol and it is not required for the portable wallet-app or ZIP-321 paths.

### 3. Wallets without Zerant authentication capabilities

Some Zcash wallets may understand Zcash addresses and ZIP-321 payments but not ZecAuth or browser message signing. Zerant must not misrepresent those wallets as unsupported Zcash wallets. They can still participate in portable Zcash payment flows.

Passkeys now provide wallet-independent Zerant account entry, so holders can use an account even when their wallet has no authentication extension. An authenticated holder can link supported Zcash sign-in to that same account. The account UI supports installed-wallet linking and a portable link handoff for supported ZecAuth wallet apps. The initiating browser checks approval and finalizes the link with its original recent session.

Account settings also allow removal of a linked ZecAuth or chain-specific wallet-message authentication method after recent sign-in, as long as another access method remains. Removal revokes all Zcash-authenticated Zerant sessions because they are not associated with individual keys; passkey sessions remain. Key rotation is explicit removal followed by linking. This affects Zerant sign-in only, not wallet spending authority or the wallet account.

## Security rules

- Never use a payment address as the Zerant account identity.
- Never request wallet balance, transaction history, seed phrases or spending keys for authentication.
- Never silently downgrade a rich ZIP-321 request into a simpler vendor transaction call.
- Direct browser payment is allowed only when the request can be represented exactly by the adapter's supported method.
- Multi-recipient, memo-bearing or otherwise richer requests stay in the portable wallet handoff path.
- Wallet-specific code stays under the adapter boundary.
- Provider detection and wallet-choice rendering must not silently request accounts or authorization.

## Adding another injected wallet

Implement `InjectedZcashWalletAdapter` in `apps/web/src/lib/zcash-wallet.ts` or a dedicated adapter module, advertise only the capabilities actually supported, add the adapter factory to `getInjectedZcashWallets()`, and add conformance tests.

No issuer, credential, verifier, disclosure or Zcash-native Rust code should need to change merely to add a wallet adapter.

## Current connection router

For the hosted `zcash:testnet` product, a detected mainnet Noir extension is not
a usable direct payment wallet. The SDK does not expose a safe network check before
the connection approval. Zerant checks the returned account's address network in
browser memory after approval and refuses unknown, mixed, or mainnet accounts.
The official [Testnet Noir build](https://github.com/NoirWallet/noir-wallet-sdk/releases) is separate
from the Chrome Web Store mainnet extension. Noir's SDK repository documents that the
testnet asset ends in `-testnet.zip`, installs as `[Testnet] Noir Wallet`, uses isolated
wallet data, and cannot be reached by switching the mainnet extension at runtime. A portable ZIP-321 request offers a
path for a wallet without a Zerant browser adapter.
[Zingo PC](https://github.com/zingolabs/zingo-pc) documents testnet wallets and a ZIP-321 URI handler;
this does not mean Zingo has a live Zerant connection. Keep the external wallet's
network and exact payment details visible to the user before approval.

The Zcash workspace starts with payment review and exposes the complete validated
ZIP-321 URI through an open or copy action. Its optional direct-connection chooser
lists only detected installed providers. WalletConnect is intentionally not offered
by product discovery: it does not solve the hosted testnet path. Sign-in and
payment choosers filter the same capability registry for their separate purposes.
"Supported Zcash wallet" means support for a specific action, not membership in a
brand allowlist. There is no universal Zcash dApp connector today. New injected
providers require an explicit safe detector and adapter; Zerant does not scan
arbitrary browser globals.

The historical WalletConnect adapter remains isolated in source for compatibility tests, but it is not registered in product discovery and is not a supported Zerant route. The testnet fallback is the exact ZIP-321 request, which a compatible wallet can open without a Zerant browser connection.

ZecAuth is an authentication-specific handoff, not the definition of wallet support. A browser wallet may authenticate only if it provides the reviewed derived-signing method. Neither wallet connection nor a payment address creates an account identity. Direct payment is limited to exact simple requests; all richer requests use canonical ZIP-321 handoff. Wallet submission remains separate from settlement verification.

## Evidence-based wallet matrix (2026-10-05)

`Yes` means the cited provider or protocol documents the capability and Zerant has a corresponding implementation or bounded test. `Unknown` means wallet-side support was not verified. A portable format being available in Zerant does not establish that a named wallet handles it.

| Wallet / route | Injected identity signing | ZecAuth auth handoff | WalletConnect Zcash transport | ZIP-321 payment URI | Direct shielded payment | Restore connection |
| --- | --- | --- | --- | --- | --- | --- |
| Noir injected provider | Yes, derived mode | Unknown | Not used by Zerant discovery | N/A for direct adapter | Yes, including the official separate testnet extension | Yes, `getAccounts` |
| Noir WalletConnect reference | No | Unknown | Not used by Zerant discovery | Unknown | No | Not used |
| NozyWallet extension | Unknown; contract not verified for Zerant | Unknown | Unknown | Unknown | Unknown | Unknown |
| ZecAuth-compatible wallet (unbranded) | N/A | Yes, if wallet implements draft | N/A | Unknown | Unknown | N/A |
| ZIP-321-compatible wallet (unbranded) | N/A | N/A | N/A | Yes, if wallet implements ZIP 321 | N/A | N/A |
| Zingo PC portable route | N/A | N/A | N/A | Yes; documents `zcash:` ZIP-321 handling and testnet wallets | N/A | N/A |
| Zodl/Zashi, YWallet, Unstoppable | Unknown | Unknown | Unknown | Unknown | Unknown | Unknown |

Sources: [Noir SDK and extension installation](https://github.com/NoirWallet/noir-wallet-sdk), [Noir Releases](https://github.com/NoirWallet/noir-wallet-sdk/releases), [Zingo PC](https://github.com/zingolabs/zingo-pc), [Nozy extension status](https://github.com/LEONINE-DAO/Nozy-wallet/blob/master/browser-extension/README.md), [ZecAuth v1 draft](https://github.com/ZecHub/zechub/blob/main/Hackathon/2026/ZecAuth/PROTOCOL.md), and [ZIP 321](https://zips.z.cash/zip-0321). See [research notes](research/ZCASH_WALLET_INTEROP_2026.md) for scope and limitations.
