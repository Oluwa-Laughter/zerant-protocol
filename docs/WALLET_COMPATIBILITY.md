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

A wallet-independent Zerant account-entry method is the next interoperability layer so holders are not excluded merely because their wallet does not implement an authentication extension.

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
