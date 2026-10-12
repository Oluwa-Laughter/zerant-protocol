# Zerant design system

**Status:** current product design principles and frontend conventions. See the [live application](https://zerant.vercel.app), [README](README.md), and [responsive animation audit](docs/ANIMATION_AUDIT.md).

## Product design principles

Zerant should feel like careful privacy infrastructure: **clear, calm, accessible, and explicit about trust boundaries**.

1. **Show the requested fact, not an identity profile.** Issuer, holder, and verifier views should make exactly which claim is requested, who sees it, and when it expires understandable.
2. **Keep payment authority separate.** No credential action implicitly approves a Zcash payment or reveals a wallet history. Wallet handoffs are explicit.
3. **Explain statuses precisely.** Distinguish pending, approved, denied, expired, revoked, submitted, and verified. In particular, **submitted is not settled**.
4. **Favor real product state over storytelling.** No fabricated activity, signatures, issuer endorsements, wallet balances, confirmed transactions, or fake metrics.
5. **Design for recovery.** Always show a usable error or empty state, a path back, and feedback after an authenticated operation.
6. **Accessible by default.** Keyboard support, focus management, headings, labels, and legible controls are part of the product, not decorations.

## Visual foundation

The production frontend owns tokens in [`apps/web/src/app/globals.css`](apps/web/src/app/globals.css). The current primary light-theme colors include:

| Semantic token | Value | Purpose |
| --- | --- | --- |
| `--background` | `#fbfaf7` | Main surface |
| `--card` | `#ffffff` | Cards and panels |
| `--foreground` | `#121416` | Primary text |
| `--muted` | `#5e6469` | Supporting text |
| `--border` | `#deded8` | Dividers and outlines |
| `--accent` | `#176b4d` | Primary actions and focus |
| `--warning` | `#8a5a13` | Cautionary states |
| `--error` | `#a33b32` | Invalid and dangerous states |

CSS variables—not duplicated static colors in documentation—remain the source of truth. Typography uses system fonts (Arial/Helvetica and a monospace stack for IDs). The geometric Zerant mark is in [`apps/web/public/brand/`](apps/web/public/brand/).

Visual hierarchy: a distinct page title, a concise description of what can be done, relevant navigation, then cards with clear primary/secondary actions. Prefer explanatory copy over cryptographic jargon.

## Core interface journeys

### Account / holder

- Register or sign in using passkeys; wallet-based authentication is a separate optional capability.
- Present the Zerant ID as an account identifier, **not** a payment address.
- Show issuer name, claim, context, validity and revocation clearly in credential details.
- A verification request must show verifier, purpose, exact claim and timing before approve/deny actions.
- Denial is accessible without coercion or hidden secondary menus.

### Organizations / issuers

- Guide first-time operators through organization creation, credential types and issuance.
- Owners/admins manage team and configuration; issuers can issue/revoke; auditors have read-only access.
- Creating, issuing, revoking and inviting must reveal pending/success/error states and never silently double-submit.
- Credential history supports readable lifecycle states; **expired is not active**.
- Issuance has a dedicated review step for recipient, claim, schema and expiration.
- Ownership transfer, key compromise and retirement deserve especially clear consequences.

### Verifiers

- Ask only for a specific signed claim, in an explicit context and for a documented purpose.
- Request status and verification result must be understandable without inspecting JSON.
- Show technical verification evidence separately from simplified eligible/not-eligible copy.
- Integration keys and webhook secrets must not be accidentally exposed in public UI or logs.

### Zcash payment handling

- Show address/network/amount validation before QR or external-wallet handoff.
- Do not imply wallet approval, broadcast or settlement just because the request was prepared.
- Shielded payout destination sharing is temporary and limited to the permitted organization.
- Never associate wallet activity with a public Zerant ID in UI copy.

## Responsive and accessible behavior

- Desktop uses a persistent, clearly labeled navigation hierarchy. Narrow viewports use an operable menu.
- Important flows must remain usable at 360–430px widths without horizontal overflow; IDs must wrap safely.
- Keyboard users can access every menu, dialog, disclosure decision, and form.
- Focus indicators must remain visible; menu/dialog closure should restore focus when applicable.
- Respect `prefers-reduced-motion`; no continuous, decorative motion in the primary workflow.
- Operational feedback should use `role="status"` / appropriate live regions; error text must not rely on color alone.
- For issuer/verifier forms, validate inputs and block duplicate requests while awaiting the server.

See [`docs/ANIMATION_AUDIT.md`](docs/ANIMATION_AUDIT.md) for a dated responsive/motion review; it is **not** a guarantee that every later route or device has been audited.

## Design verification

Run frontend lint, typechecks, tests and a production build from the [README](README.md). Manually review the live landing page, mobile navigation, account access, credential issuance, disclosure consent, and wallet handoff after significant changes. The authenticated two-person verification journey should be exercised using separate real accounts, never simulated authorization.

Zerant's privacy claims are limited by the actual [privacy contract](docs/PRIVACY.md) and [threat model](docs/THREAT_MODEL.md)—a calming visual theme does not make user data anonymous.
