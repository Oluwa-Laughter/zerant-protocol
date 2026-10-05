# Zcash wallet compatibility

Zerant is wallet-agnostic. Wallet choice must not change credential, issuer, verifier or consent semantics.

## Compatibility layers

### 1. Portable wallet handoff

These paths do not depend on a browser-extension vendor:

- **Authentication:** Zerant can create a short-lived `zecauth:` request for wallet apps that implement the ZecAuth handoff draft.
- **Payments:** Zerant uses canonical ZIP-321 `zcash:` payment-request URIs. A wallet that understands ZIP-321 can receive the exact validated request without Zerant translating it into a vendor-specific transaction format.
- **Addresses:** Zerant validates Zcash addresses through the maintained Zcash address libraries rather than a wallet-specific parser.

Portable handoff is the preferred interoperability layer because the wallet remains responsible for keys, transaction construction and user approval.

### 2. Injected wallet adapters

Browser wallets may expose richer capabilities. Zerant models those through `InjectedZcashWalletAdapter` rather than importing wallet behavior into the product core.

An adapter advertises capabilities independently:

- connection restoration;
- private identity-message signing;
- direct shielded payment submission.

A wallet can support any subset. Zerant must not assume that connection implies signing, or that signing implies payment submission.

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

## Adding another injected wallet

Implement `InjectedZcashWalletAdapter` in `apps/web/src/lib/zcash-wallet.ts` or a dedicated adapter module, advertise only the capabilities actually supported, add the adapter factory to `getInjectedZcashWallets()`, and add conformance tests.

No issuer, credential, verifier, disclosure or Zcash-native Rust code should need to change merely to add a wallet adapter.

## Current connection router

The product has one **Connect Zcash wallet** entry point. Its chooser lists detected installed providers, an optional WalletConnect-compatible session, ZecAuth wallet-app sign-in, and portable ZIP-321 payment handoff. "Supported Zcash wallet" means support for a specific capability, not membership in a brand allowlist. There is no universal Zcash dApp connector today. New injected providers require an explicit safe detector and adapter; Zerant does not scan arbitrary browser globals.

WalletConnect requires `NEXT_PUBLIC_ZCASH_WALLETCONNECT_PROJECT_ID` and a wallet implementing the Zcash `bip122` namespace on mainnet. Before pairing it is offered only for connection, with no payment or identity capability. The session asks for `zcash_getAddress`; `zcash_transfer` is optional. Direct transparent payment appears only after an approved session advertises that method and a transparent account, and disappears on disconnect. Current reference wallets do not expose Zerant identity signing or shielded payment through WalletConnect. A remote session therefore does not sign in to Zerant. A wallet with only ZIP-321 support can still receive the exact payment request, but it cannot maintain a live dApp session through that URI.

ZecAuth is an authentication-specific handoff, not the definition of wallet support. A browser wallet may authenticate only if it provides the reviewed derived-signing method. Neither wallet connection nor a payment address creates an account identity. Direct payment is limited to exact simple requests; all richer requests use canonical ZIP-321 handoff. Wallet submission remains separate from settlement verification.

## Evidence-based wallet matrix (2026-10-05)

`Yes` means the cited provider or protocol documents the capability and Zerant has a corresponding implementation or bounded test. `Unknown` means wallet-side support was not verified. A portable format being available in Zerant does not establish that a named wallet handles it.

| Wallet / route | Injected identity signing | ZecAuth auth handoff | WalletConnect Zcash transport | ZIP-321 payment URI | Direct shielded payment | Restore connection |
| --- | --- | --- | --- | --- | --- | --- |
| Noir injected provider | Yes, derived mode | Unknown | Separate optional reference transport | Unknown | Yes, `sendTransaction` shielded funding | Yes, `getAccounts` |
| Noir WalletConnect reference | No | Unknown | Yes, mainnet `bip122` with limited methods | Unknown | No | Yes, approved session |
| NozyWallet extension | Unknown; contract not verified for Zerant | Unknown | Unknown | Unknown | Unknown | Unknown |
| ZecAuth-compatible wallet (unbranded) | N/A | Yes, if wallet implements draft | N/A | Unknown | Unknown | N/A |
| ZIP-321-compatible wallet (unbranded) | N/A | N/A | N/A | Yes, if wallet implements ZIP 321 | N/A | N/A |
| Zodl/Zashi, YWallet, Unstoppable | Unknown | Unknown | Unknown | Unknown | Unknown | Unknown |

Sources: [Noir adapter/provider behavior](https://github.com/NoirWallet/zcash-wallet-adapter/blob/main/README.md), [Nozy extension status](https://github.com/LEONINE-DAO/Nozy-wallet/blob/master/browser-extension/README.md), [ZecAuth v1 draft](https://github.com/ZecHub/zechub/blob/main/Hackathon/2026/ZecAuth/PROTOCOL.md), and [ZIP 321](https://zips.z.cash/zip-0321). See [research notes](research/ZCASH_WALLET_INTEROP_2026.md) for scope and limitations.
