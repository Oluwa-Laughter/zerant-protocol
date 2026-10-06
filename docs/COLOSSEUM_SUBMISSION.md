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

Zerant helps applications verify one fact without collecting a person’s full identity, credential history, wallet address, balance, or transaction history. Issuers create signed credentials, holders keep them private under a Zerant ID, and verifiers request one purpose-bound claim. Zcash payments remain a separate wallet-approved capability, with ZIP-321 review, Noir Wallet support, invoices, and bounded payment tracking.

## Problem

Most digital trust systems over-collect information.

A product that only needs to know whether someone satisfies one condition can end up receiving a full identity profile, unrelated credentials, wallet addresses, transaction history, or other evidence that is unnecessary for the decision.

This creates three problems:

1. users disclose more than the decision requires;
2. applications inherit sensitive data they did not need to store;
3. wallet identity, reputation, and payment activity become unnecessarily linked.

## Solution

Zerant separates three capabilities:

- **Identity/account access** — a Zerant account can be accessed with a passkey; a Zcash wallet is optional.
- **Credential disclosure** — a verifier asks for one narrow claim with a purpose, audience, nonce, and expiry; the holder reviews and approves or denies it.
- **Payment authority** — if ZEC must move, the wallet separately authorizes that exact transaction.

A verifier receives the approved result, not the holder’s entire private credential portfolio.

A wallet connection never becomes automatic proof consent, and proof consent never becomes spending authority.

## User flow

1. An issuer creates a credential type and issues a signed credential to a holder’s Zerant ID.
2. The holder sees the credential privately in the Vault.
3. A verifier creates a short-lived request for one claim.
4. The holder previews the exact disclosure and approves or denies it.
5. Zerant re-verifies the request, issuer evidence, revocation state, expiry, audience, and replay binding before returning a verifier-specific result.
6. If a payment is required, Zerant prepares or validates the Zcash payment request.
7. The user connects a compatible wallet or opens the reviewed ZIP-321 request in another wallet.
8. The wallet approves the transaction. Zerant records a returned transaction ID as submitted and keeps settlement claims separate from wallet submission.

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

- passkey account access;
- optional Zcash wallet authentication;
- opaque authenticated sessions;
- remote session revocation;
- encrypted credential storage;
- bounded account activity history.

### Zcash integration

- Zcash testnet configuration;
- Noir Wallet injected-provider support;
- interactive zcash_requestAccounts connection;
- silent zcash_getAccounts restoration;
- derived-key wallet message signing for the supported identity path;
- direct shielded payment when the connected wallet advertises that capability;
- ZIP-321 request validation and portable wallet handoff;
- Zcash address inspection;
- prepared/submitted payment tracking;
- bounded transaction observation;
- shareable Zcash invoices;
- Z3/Zallet capability and regtest integration boundaries.

## Architecture

The product is split into explicit trust boundaries:

- **Next.js web app** — product UI, consent interaction, wallet selection, and transient connection state.
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

**Zcash:** Noir Wallet SDK, ZIP-321, Z3/Zallet integration boundaries, lightwalletd-compatible network readiness.

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

5. **The system is implemented as real product infrastructure.**
   It includes issuer governance, revocation, passkeys, verifier APIs, signed webhooks, invoices, payment tracking, deployment, and tests—not only a mock UI.

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

### 1:25–2:25 — Zcash payment boundary

Open the Zcash workspace.

Prepare a testnet payment.

Show the exact recipient/amount and privacy review.

Connect Testnet Noir Wallet.

Approve the wallet request.

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

Final manual screenshots to add during the real demo session are listed in docs/DEMO.md.

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
- [ ] Testnet Noir Wallet is unlocked and approved for the production origin.
- [ ] Payment demo uses testnet only.
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
