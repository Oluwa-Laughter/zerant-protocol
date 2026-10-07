# Zerant Demo Runbook

This runbook is for the hackathon submission, screenshots, and recorded demo. It is intentionally short and uses only flows that are implemented in the current product.

The current [public walkthrough](assets/demo-public-walkthrough.mp4) is a 50-second recording of the live landing page and signed-out product routes. It does not show an authenticated credential, holder consent, Noir approval, or payment submission. Record those with real accounts and an onboarded Testnet Noir wallet before presenting a complete product demo.

## Demo goal

Show one coherent story:

**A person receives a private credential, approves a narrow verification request, then uses Zcash separately for a payment without turning the wallet into their identity.**

## Before recording

Use the deployed production build and prepare:

- one holder account with a passkey;
- one issuer organization;
- one active credential type;
- one credential issued to the holder;
- one verifier application;
- one shielded-capable Zcash testnet receive address for the holder;
- a testnet payment request that you are comfortable demonstrating;
- Testnet Noir Wallet only if you want to show the optional final wallet-approval step.

Do not show private keys, seed phrases, environment variables, database credentials, raw credential signatures, or wallet history.

## Recommended 3-minute video

### 0:00–0:20 — Problem and product

Open the landing page.

Say:

> Zerant is a privacy-preserving trust layer for Zcash applications. Instead of giving every app a full identity or wallet history, an issuer can attest to a fact, the holder keeps that evidence private, and a verifier asks only for the exact result it needs.

### 0:20–0:45 — Holder Vault

Sign in with the passkey and open `/vault`.

Show:

- the Zerant ID;
- one credential card;
- credential status;
- that the wallet is not required to view the credential.

Explain that the holder's wallet address is not their Zerant identity.

### 0:45–1:15 — Verification request

Open the verifier flow and create a narrow request.

Show:

- verifier name/domain;
- requested claim;
- purpose;
- expiry.

Then open `/requests` as the holder.

Show the preview and approve it.

Emphasize that the holder sees what is requested before anything is disclosed.

### 1:15–1:35 — Verifier result

Return to the verifier.

Show the verified result only.

Do not expose the full source credential.

Explain that denial would produce no credential response.

### 1:35–2:10 — Private payout destination

Open `/zcash/payouts` as the holder.

Show:

1. the selected organization;
2. a shielded-capable testnet receive address;
3. the short payout purpose;
4. the seven-day expiry and withdrawal language.

Share it, then open `/issuer/payouts` as an authorized organization operator and show the private payout inbox.

Explain:

> This address is payment routing data, not the holder's Zerant identity. It is encrypted, shared only with this organization for the stated purpose, and disappears from the active record after withdrawal or expiry.

### 2:10–2:40 — Zcash payment boundary

From the organization payout inbox, copy the active destination and open `/zcash/payments`. Prepare and review the exact payment amount.

Show the ZIP-321 link/QR or exact manual handoff. A live browser wallet connection is **not required** to prepare or hand off the payment. If Testnet Noir is stable during recording, you may also show its separate approval step.

Explain:

> Zerant handles trust, consent, and private payout routing. The wallet still keeps the spending keys and separately authorizes any transaction.

If a wallet returns a transaction ID, show Zerant's saved payment card and call the state **submitted/pending verification**, not settled.

### 2:40–2:55 — Shareable invoice

Create or open one invoice.

Show the public `/pay/[id]` page.

Explain that the public page exposes the payment request, not the account's credential history.

### 2:55–3:00 — Close

End with:

> Zerant separates trust, consent, and payment authority. Applications receive the fact they asked for; the holder keeps unrelated identity and wallet activity private.

## Screenshot set

The repository already contains four clean submission visuals captured from the current production build or rendered directly from the documented architecture:

- `docs/assets/screenshots/01-landing.png` — production landing page;
- `docs/assets/screenshots/02-zcash.png` — public Zcash workspace and four-step wallet/payment boundary;
- `docs/assets/screenshots/03-architecture.png` — system architecture and trust boundaries;
- `docs/assets/screenshots/04-flow.png` — end-to-end credential, consent, verifier, and optional payment flow.

For the final demo package, add the following authenticated/manual frames from the real browser session. These should not be fabricated from seeded data.

### 05 — Vault

Capture one real credential card and the Zerant ID after passkey sign-in.

Filename:

`docs/assets/screenshots/05-vault.png`

### 06 — Holder consent

Capture the verification request preview before approval.

Filename:

`docs/assets/screenshots/06-holder-consent.png`

### 07 — Private payout sharing

Capture the holder's `/zcash/payouts` review showing organization, purpose, privacy boundary, and expiry. Do not expose a real address outside the safe demo account.

Filename:

`docs/assets/screenshots/07-private-payout.png`

### 08 — Organization payout inbox

Capture `/issuer/payouts` showing the holder relationship, purpose, and private organization-only handling.

Filename:

`docs/assets/screenshots/08-payout-inbox.png`

### 09 — Payment review

Capture the exact reviewed Zcash payment request/ZIP-321 handoff. If you also record a successful wallet submission, capture the submitted txid as an additional frame.

Filename:

`docs/assets/screenshots/09-payment-review.png`

### 10 — Public invoice

If an invoice is part of the recorded demo, capture its shareable public page.

Filename:

`docs/assets/screenshots/10-public-invoice.png`

## Screenshot quality rules

- Use the production URL, not localhost.
- Keep browser zoom at 100%.
- Use one consistent viewport.
- Do not include console/devtools.
- Avoid showing unrelated browser tabs.
- Hide passwords, API keys, environment variables, seed phrases, or account recovery information.
- Prefer product states with short realistic data so the interface is readable.
- Do not crop away status labels that explain whether something is prepared, submitted, or verified.

## Demo payment fallback

The core demo does not depend on a live browser-wallet connection. If direct Noir approval is unavailable during recording:

1. keep the validated payment visible;
2. show the exact reviewed ZIP-321 link/QR or manual destination and amount;
3. explain that a compatible testnet wallet can complete the payment independently of Zerant sign-in;
4. stop before claiming submission unless a wallet actually returns a valid transaction ID.

Do not fabricate a txid or call a prepared request paid/settled.

## Submission checklist

Before final Colosseum submission:

- production deployment is green;
- README accurately describes the current implementation;
- architecture diagram renders on GitHub;
- demo screenshots are committed;
- demo video link works in an incognito/private browser;
- project description matches the product, not an older prototype;
- repository URL points to the final branch;
- team profiles are complete;
- final survey is complete;
- no secrets are committed;
- no demo copy claims settlement, zero knowledge, anonymity, or production mainnet support beyond the implemented mechanism.

## Useful verification commands

```bash
pnpm --filter @zerant/web typecheck
pnpm --filter @zerant/web test
pnpm --filter @zerant/web build
git status --short
```
