# Zerant Demo Runbook

This runbook is for the hackathon submission, screenshots, and recorded demo. It is intentionally short and uses only flows that are implemented in the current product.

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
- Testnet Noir Wallet unlocked and available;
- a testnet recipient/payment request that you are comfortable demonstrating.

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

### 1:35–2:30 — Zcash payment boundary

Open `/zcash`.

Prepare a Zcash payment:

1. enter recipient;
2. enter amount;
3. review the canonical request;
4. show the privacy warning/state;
5. choose Testnet Noir Wallet;
6. approve the connection in Noir;
7. approve the payment in Noir.

Explain:

> Sign-in is optional, but a payment still needs wallet authorization. Zerant prepares and validates the request; the wallet keeps the spending keys and approves the transaction.

After the wallet returns a transaction ID, show Zerant's saved payment card.

Call out the state as **submitted/pending verification**, not settled.

### 2:30–2:50 — Shareable invoice

Create or open one invoice.

Show the public `/pay/[id]` page.

Explain that the public page exposes the payment request, not the account's credential history.

### 2:50–3:00 — Close

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

### 07 — Noir Wallet approval

Capture the Noir approval window while Zerant is requesting the connection or payment. Do not expose seed phrases, wallet history, or unrelated account data.

Filename:

`docs/assets/screenshots/07-noir-approval.png`

### 08 — Submitted payment

Capture the saved payment card showing the wallet-returned transaction ID and Zerant's submitted/pending network state.

Filename:

`docs/assets/screenshots/08-payment-submitted.png`

### 09 — Public invoice

If an invoice is part of the recorded demo, capture its shareable public page.

Filename:

`docs/assets/screenshots/09-public-invoice.png`

## Screenshot quality rules

- Use the production URL, not localhost.
- Keep browser zoom at 100%.
- Use one consistent viewport.
- Do not include console/devtools.
- Avoid showing unrelated browser tabs.
- Hide passwords, API keys, environment variables, seed phrases, or account recovery information.
- Prefer product states with short realistic data so the interface is readable.
- Do not crop away status labels that explain whether something is prepared, submitted, or verified.

## Demo failure fallback

If direct Noir payment approval fails during recording:

1. keep the validated payment visible;
2. show that Noir is detected;
3. use the portable reviewed ZIP-321 payment link;
4. explain that Zerant supports portable wallet handoff independently of account sign-in.

Do not claim a transaction succeeded if the wallet did not return a valid transaction ID.

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
