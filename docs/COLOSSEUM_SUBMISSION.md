# Colosseum Submission Sheet — Zerant

Use this file as the source of truth when completing the final hackathon project form. Keep every claim aligned with the deployed product.

## Project identity

**Name:** Zerant

**Tagline:** Prove trust. Preserve privacy.

**Live product:** https://zerant.vercel.app

**Repository:** https://github.com/Oluwa-Laughter/zerant-protocol

**Demo video:** Add the final public video URL after recording and verify it in a private/incognito browser before submission.

## One-sentence description

Zerant is a privacy-preserving trust layer for Zcash applications that lets issuers create trusted credentials, holders approve narrowly scoped proofs, and users authorize Zcash payments separately without turning wallet history into identity.

## Short description

Zerant helps applications verify one fact without collecting a person’s full identity, credential history, wallet address, balance, or transaction history. Issuers create signed credentials, holders keep them private under a Zerant ID, and verifiers request one purpose-bound claim. Holders can also share a temporary encrypted payout destination with one organization without turning the address into identity. Zcash payments stay a separate authority boundary: Zerant validates and reviews the exact ZIP-321 request, then hands it to an external wallet for approval.

## Problem

Most digital trust systems over-collect information.

A product that only needs to know whether someone satisfies one condition can end up receiving a full identity profile, unrelated credentials, wallet addresses, transaction history, or other evidence that is unnecessary for the decision.

This creates three problems:

1. users disclose more than the decision requires;
2. applications inherit sensitive data they did not need to store;
3. wallet identity, reputation, and payment activity become unnecessarily linked.

## Solution

Zerant separates three capabilities:

- **Identity/account access** — a Zerant account is accessed with a passkey; a payment wallet is not part of sign-in.
- **Credential disclosure** — a verifier asks for one narrow claim with a purpose, audience, nonce, and expiry; the holder reviews and approves or denies it.
- **Payment authority** — if ZEC must move, the wallet separately authorizes that exact transaction.

A verifier receives the approved result, not the holder’s entire private credential portfolio.

Payment handoff never becomes automatic proof consent, and proof consent never becomes spending authority.

## User flow

1. An issuer creates a credential type and issues a signed credential to a holder’s Zerant ID.
2. The holder sees the credential privately in the Vault.
3. A verifier creates a short-lived request for one claim.
4. The holder previews the exact disclosure and approves or denies it.
5. Zerant re-verifies the request, issuer evidence, revocation state, expiry, audience, and replay binding before returning a verifier-specific result.
6. If a payment is required, Zerant prepares or validates the Zcash payment request.
7. Zerant exposes the reviewed ZIP-321 request as a link, QR code, and exact simple-payment details where safe.
8. The user opens that request in an external compatible wallet, which separately approves any transaction. A transaction ID may be recorded as submitted; settlement claims remain separate.

## What is implemented

### Credentials and trust

- signed issuer credentials;
- private holder Vault;
- issuer organizations and role-based administration;
- credential definitions and lifecycle management;
- issuer key rotation;
- credential revocation;
- public issuer discovery;
- append-only issuer audit history.

### Verification and consent

- verifier applications;
- short-lived domain/purpose/audience-bound requests;
- holder preview before approval;
- explicit approval or denial;
- verifier-specific results;
- replay protection;
- API keys for verifier integrations;
- signed webhook delivery and retry handling.

### Account security

- discoverable passkey account access with Zerant-ID/passkey fallback;
- opaque authenticated sessions;
- remote session revocation;
- encrypted credential storage;
- bounded account activity history.

### Zcash integration

- Zcash testnet configuration;
- ZIP-321 request validation, canonical review, QR/link handoff, and exact simple-payment fallback;
- external-wallet payment approval with no persistent browser-wallet connection state;
- Zcash address inspection;
- prepared/submitted payment tracking;
- bounded transaction observation;
- shareable Zcash invoices;
- temporary encrypted, organization-scoped private payout destinations;
- direct server-side payout preparation into the existing ZIP-321/payment tracker without putting the private address in a URL;
- shielded-only payout-address validation, active-trust gating, seven-day expiry, withdrawal, trust-revocation expiry, and retention scrubbing;
- Z3/Zallet capability and regtest integration boundaries.

## Architecture

The product is split into explicit trust boundaries:

- **Next.js web app** — product UI, consent interaction, payment review, and external-wallet handoff.
- **Rust/Axum service** — authorization, durable workflow state, encrypted credential access, replay protection, issuer/verifier logic.
- **Protocol crates** — canonicalization, credential signatures, policy evaluation, disclosure binding, and Zcash request validation.
- **PostgreSQL** — encrypted credentials and durable application state.
- **Zcash wallet** — spending keys and wallet-side transaction approval.
- **Verifier** — receives only the result approved for its request.
- **Zcash network services** — bounded readiness and named-transaction observation.

See docs/assets/screenshots/03-architecture.png and docs/ARCHITECTURE.md.

## Privacy and security boundaries

Zerant intentionally does not claim more privacy than it implements.

Current guarantees and boundaries:

- wallet address, balance, and transaction history are not generic Zerant identity fields;
- a payout address can be shared temporarily with one organization without becoming a credential or public profile field;
- denial emits no credential result;
- verifier requests are purpose-, audience-, expiry-, and replay-bound;
- proofs/results are verifier-specific;
- wallet payment approval is separate from proof consent;
- a wallet-returned transaction ID is submission metadata, not automatic proof of exact shielded settlement;
- credential records are encrypted at rest behind a versioned key-provider boundary.

Current limitations:

- the deployed credential service is not holder-only end-to-end encrypted;
- Zerant does not claim anonymity or full unlinkability;
- stronger unlinkable credential schemes remain future work;
- exact shielded recipient/amount settlement needs an authorized observation path;
- PCZT/FROST are capability boundaries, not custom cryptography implemented by Zerant;
- mainnet direct spending is not the hackathon target.

## Why Zcash

Zcash is a natural settlement layer for a product whose goal is to minimize unnecessary disclosure.

Zerant does not treat using Zcash as automatically making the rest of the application private. Instead, it keeps the privacy boundary explicit:

- credentials and consent are handled by Zerant;
- payment authority stays in the wallet;
- Zcash testnet handles the payment;
- the verifier receives only the approved trust result.

This prevents the common mistake of turning a payment wallet into a universal application identity.

## Technical stack

**Frontend:** Next.js 16, React 19, TypeScript, Tailwind CSS.

**Backend:** Rust, Axum, PostgreSQL/Neon.

**Zcash:** ZIP-321, Zcash address parsing, Z3/Zallet integration boundaries, and lightwalletd-compatible network readiness.

**Security:** WebAuthn/passkeys, opaque sessions, encrypted credential storage, strict canonical parsing, origin/audience/nonce/expiry binding, replay protection, SSRF-safe webhooks.

**Deployment:** Vercel Services with a private web-to-Rust service binding and Neon PostgreSQL.

## What to emphasize to judges

1. **The wallet is not the identity.**
   Zerant deliberately separates account access, credential disclosure, and spending authority.

2. **Consent is concrete.**
   The holder reviews the exact requested claim before approval.

3. **The verifier receives less data.**
   The design is based on narrow, purpose-bound results instead of portfolio disclosure.

4. **Zcash integration is honest about settlement.**
   A wallet-returned txid is not overstated as exact shielded settlement proof.

5. **Private payouts preserve the identity boundary.**
   A holder can give one organization a temporary shielded receive address without turning the address into their Zerant identity or a public credential field.

6. **The system is implemented as real product infrastructure.**
   It includes issuer governance, revocation, passkeys, verifier APIs, signed webhooks, invoices, private payout routing, payment tracking, deployment, and tests—not only a mock UI.

## Recommended 3-minute demo

### 0:00–0:20 — Problem

Show the landing page.

Explain that applications often receive more identity and wallet information than a decision requires.

### 0:20–0:45 — Private credential

Sign in with a passkey and show one real credential in the Vault.

Point out that no Zcash wallet is required to view or use the credential.

### 0:45–1:25 — Narrow verification

Create or open a verifier request.

Show the purpose and requested claim.

Switch to the holder request view, preview the disclosure, and approve it.

Return to the verifier and show only the resulting claim.

### 1:25–1:55 — Private payout routing

Open `/zcash/payouts` as the holder and share a shielded-capable testnet destination with the issuer organization for a short purpose. Then open `/issuer/payouts` as the organization and show the private payout inbox.

Explain that this destination is encrypted organization-scoped payment routing data, not a credential, public profile field, or Zerant identity attribute. It expires after seven days or can be withdrawn sooner.

### 1:55–2:25 — Zcash payment boundary

Enter the amount directly on the organization payout record and choose **Prepare payout**. Zerant decrypts the private destination server-side, creates the canonical ZIP-321 request, saves it in the normal prepared-payment tracker, and then lets the operator review it in `/zcash/payments`. Show the ZIP-321 link/QR or exact simple-payment handoff. Zerant does not create a persistent browser-wallet session; an external wallet keeps the spending keys and separately authorizes any transaction.

If a real testnet payment is submitted, show the saved transaction ID and label it as submitted/pending verification—not automatically settled.

### 2:25–2:45 — Invoice

Show a shareable invoice or public payment request.

Explain that the public page exposes the intended payment request, not the holder’s credential portfolio.

### 2:45–3:00 — Close

Zerant separates trust, consent, and payment authority. Applications learn the fact they asked for while unrelated identity and wallet activity stay outside the request.

## Existing submission visuals

- docs/assets/screenshots/01-landing.png
- docs/assets/screenshots/02-zcash.png
- docs/assets/screenshots/03-architecture.png
- docs/assets/screenshots/04-flow.png
- docs/assets/screenshots/05-vault.png
- docs/assets/screenshots/06-holder-consent.png
- docs/assets/screenshots/07-verifier-result.png
- docs/assets/screenshots/08-private-payout.png
- docs/assets/screenshots/09-payout-inbox.png
- docs/assets/screenshots/10-payment-review.png
- docs/assets/screenshots/11-mobile-menu.png

These frames use real product states from the production workflow. The optional public-invoice frame is listed in docs/DEMO.md.

## Demo video title

**Zerant — Private Trust and Zcash Payments Without the Data Dragnet**

## Demo video description

Zerant is a privacy-preserving trust layer for Zcash applications. This demo shows private credential issuance, holder-controlled disclosure, verifier-specific results, and a separate Zcash payment flow where the user’s wallet retains spending authority.

Live: https://zerant.vercel.app

Code: https://github.com/Oluwa-Laughter/zerant-protocol

## Final submission checklist

- [ ] Latest production deployment is green.
- [ ] https://zerant.vercel.app loads in an incognito/private browser.
- [ ] Repository main branch contains the final implementation.
- [ ] README images render correctly on GitHub.
- [ ] Holder demo account contains one safe demo credential.
- [ ] Verifier demo request can be completed end-to-end.
- [ ] Passkey sign-in works on the production origin and survives refresh.
- [ ] Payment demo uses testnet only.
- [ ] No normal demo screen shows a wallet-connect or "Wallet not connected" state.
- [ ] Mobile navigation is usable at 390px with no horizontal overflow.
- [ ] Demo copy says submitted rather than settled unless exact settlement has actually been verified.
- [ ] Final manual screenshots contain no seed phrase, API key, environment variable, wallet history, or private credential signature.
- [ ] Demo video is publicly viewable without requiring the judge to log in.
- [ ] Colosseum team submission profiles are complete.
- [ ] Project details are complete.
- [ ] Final survey is complete.
- [ ] Final submission is reviewed before the deadline.

## Final smoke checks

Before final submission run:

- pnpm --filter @zerant/web lint
- pnpm --filter @zerant/web typecheck
- pnpm --filter @zerant/web test
- pnpm --filter @zerant/web build
- git diff --check
- python3 scripts/secret-check.py

Production checks:

- GET https://zerant.vercel.app/
- GET https://zerant.vercel.app/api/zerant/health

Expected health response:

{"status":"ok","storage":"postgres","authentication":"zecauth-passkey","zcash_boundary":"z3-zallet"}
