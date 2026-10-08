# Zerant Demo Runbook

This runbook is for the hackathon submission, screenshots, and recorded demo. It is intentionally short and uses only flows that are implemented in the current product.

The [full 2:57 narrated walkthrough](assets/demo/zerant-demo-full.mp4) and [36-second silent preview](assets/demo/zerant-demo-preview.mp4) use real captured states from the authenticated Vault, holder consent, verified result, private payout, organization inbox, and prepared Zcash payment review. The longer edit has synthesized narration and subtitles. Camera and cursor movements are illustrative, not a continuous live transaction recording or proof of payment settlement. The interactive recording plan is documented below; the core story does not require a browser-wallet connection.

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
- an external compatible testnet wallet only if you intentionally choose to demonstrate a real submission. It is not required for the core demo.

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

From the organization payout inbox, enter the payment amount and choose **Prepare payout**. Zerant creates the normal tracked Zcash payment from the private destination and opens `/zcash/payments` for review.

Show the complete ZIP-321 link/QR and, for a simple request, the exact manual recipient and amount. Zerant does not establish a persistent browser-wallet connection: the reviewed request leaves Zerant and the wallet separately authorizes any transaction.

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

The following authenticated frames have been captured from real browser sessions and are in the repository. Recheck them after significant visual changes; never fabricate credential, approval, or payment states.

### 05 — Vault

Capture one real credential card and the Zerant ID after passkey sign-in.

Filename:

`docs/assets/screenshots/05-vault.png`

### 06 — Holder consent

Capture the verification request preview before approval.

Filename:

`docs/assets/screenshots/06-holder-consent.png`

### 07 — Verifier result

Capture the verifier-side bounded result after Zerant re-verifies the approved proof. The frame should show the single approved claim and the privacy note explaining what was not disclosed.

Filename:

`docs/assets/screenshots/07-verifier-result.png`

### 08 — Private payout sharing

Capture the holder's `/zcash/payouts` review showing organization, purpose, privacy boundary, and expiry. Do not expose a real address outside the safe demo account.

Filename:

`docs/assets/screenshots/08-private-payout.png`

### 09 — Organization payout inbox

Capture `/issuer/payouts` showing the holder relationship, purpose, and private organization-only handling.

Filename:

`docs/assets/screenshots/09-payout-inbox.png`

### 10 — Payment review

Capture the exact reviewed Zcash payment request/ZIP-321 handoff. If you also record a successful wallet submission, capture the submitted txid as an additional frame.

Filename:

`docs/assets/screenshots/10-payment-review.png`

### 11 — Mobile workspace navigation

Capture the 390px workspace drawer with the grouped workspace, organization, account, and landing-page navigation. Prefer an authenticated production session for the final frame.

Filename:

`docs/assets/screenshots/11-mobile-menu.png`

### 12 — Public invoice (optional)

If an invoice is part of the recorded demo, capture its shareable public page.

Filename:

`docs/assets/screenshots/12-public-invoice.png`

## Screenshot quality rules

- Use the production URL, not localhost.
- Keep browser zoom at 100%.
- Use one consistent viewport.
- Do not include console/devtools.
- Avoid showing unrelated browser tabs.
- Hide passwords, API keys, environment variables, seed phrases, or account recovery information.
- Prefer product states with short realistic data so the interface is readable.
- Do not crop away status labels that explain whether something is prepared, submitted, or verified.

## Payment handoff rule

The core demo stops at a reviewed external-wallet handoff unless you deliberately complete a real testnet submission:

1. keep the server-validated payment visible;
2. show the complete ZIP-321 link/QR or, for a simple request, the exact destination and amount;
3. explain that a compatible testnet wallet completes the payment independently of Zerant sign-in;
4. stop before claiming submission unless you actually have a valid transaction ID from an external wallet.

Do not fabricate a txid or call a prepared request paid or settled.

## Responsive and animation QA

The production landing page was profiled after load at 1440×1100 and 390×844 at the top, middle, and footer positions. Across all six measured states:

- there was no horizontal page overflow;
- there were zero running CSS animations after their short entrance transitions completed;
- there were zero offscreen-running animations;
- the product source contains no requestAnimationFrame, WebGL, or persistent interval animation loop;
- reduced-motion CSS disables the remaining short entrance and hover motion where appropriate.

The result is intentionally lightweight: Zerant keeps brief transform/opacity interactions for menu, proof, result, and payment-state transitions without adding a continuous animation engine.

## Production passkey QA

A production WebAuthn smoke test was run against `https://zerant.vercel.app` with an isolated Chrome virtual authenticator, not a user credential. It verified:

- account registration creates an authenticated Zerant session with RP ID `zerant.vercel.app`;
- the test authenticator produced a valid non-resident credential, demonstrating why discoverable sign-in cannot be the only recovery path;
- when discoverable sign-in cannot locate that credential, Zerant automatically reveals the Zerant-ID + passkey fallback;
- the Zerant-ID fallback successfully signs the same account back in;
- the authenticated session remains valid after a full Vault reload.

No wallet connection or wallet identity participates in this account flow.

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
