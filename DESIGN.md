# Zerant design system

Zerant should feel like serious privacy infrastructure: calm, precise, modern, and trustworthy. The UI is deliberately quieter than typical crypto products.

## Visual principles
- Use restrained surfaces, clear hierarchy, thin borders, and generous spacing.
- The consent review is the visual center of the product.
- Avoid generic crypto neon, glassmorphism, glowing shields/locks, excessive gradients, and decorative blockchain imagery.
- No fake metrics, testimonials, logos, or claims.
- Motion explains state changes only and respects `prefers-reduced-motion`.

## Tokens and typography

The concrete M1A token table and typography below are authoritative for the implemented shell. Light and dark themes share the same semantic roles; dark mode follows the system preference.

## Brand mark
The provisional Zerant mark combines:
1. a geometric Z path,
2. bounded corners suggesting explicit trust boundaries,
3. an open aperture through the bounded path.

It intentionally avoids shields, locks, eyes, and chain-link clichés. The mark must work in monochrome. Assets live in `apps/web/public/brand/`.

## Accessibility
Target WCAG 2.2 AA. Maintain visible focus, keyboard navigation, semantic status announcements, adequate contrast, and 44px interactive targets where practical. Never rely on color alone.

## Product surfaces

The landing page and authenticated workspace should identify Zerant as a product
for the Zcash ecosystem and label the current network as testnet. Navigation
should name the action or content users will find: credentials, verification
requests, Zcash payments, issuer tools, verifier tools, and account settings.
Explain that Zcash moves funds through a separate wallet while Zerant handles
credential issuance and consent. Do not present credentials as on-chain assets
or a submitted transaction as confirmed settlement. Show capabilities from
actual product state, without seeded balances, requests, or credentials.

M1A:
- Landing page
- Holder preview
- Issuer preview
- Verifier preview
- Disclosure consent panel

Future protocol-backed states must visually distinguish:
locked, request received, invalid request, reviewing, unavailable evidence, approved, generating, delivered, accepted, rejected, expired.

## M1A implementation tokens

Colors use CSS variables in `apps/web/src/app/globals.css`. System preference selects the theme; both themes expose the same semantic states. Accent is a restrained botanical green, never a privacy guarantee or a verification status by itself.

| Token | Light | Dark |
| --- | --- | --- |
| background | #f7f7f2 | #121d1a |
| card | #ffffff | #1a2924 |
| foreground | #182b28 | #e9eee7 |
| muted (text) | #566560 | #b1bfb5 |
| border | #d4dcd6 | #3b4e43 |
| accent / success | #236951 | #9dcbb0 |
| accent foreground | #ffffff | #152b20 |
| warning | #805a1b | #e7c18a |
| error | #a43832 | #f1a59c |
| soft surface | #edf2eb | #22352b |

Typography: Arial/Helvetica/system sans for reading and headings; SFMono-Regular/Consolas/monospace for identifiers and small labels. No remote font fetching. Body 16px with 1.6 line height; supporting text 13–15px; labels 10–11px. Hero scales 50–94px on desktop, 43–72px on mobile. Section headings scale 32–52px. Headings use restrained negative tracking and medium weight; paragraphs remain within 65 characters where practical.

Authenticated workspace typography uses a denser reading hierarchy: page headings
36–60px on desktop and 32–43px on mobile, supporting text at least 14px, and
form labels at least 14px. Small uppercase navigation labels remain 11–12px.
This keeps account, issuer, verifier, consent, and payment instructions legible
without turning operational pages into marketing heroes.

Layout: maximum 1280px outer container; horizontal gutters 48/30/20px; section spacing 84px desktop, 50px mobile. Surfaces use 4–10px corner radii, fine borders and limited elevation. Controls target at least 44px height and use a visible 3px focus outline. Tabs implement arrow/Home/End navigation. CSS transitions are limited to button opacity; reduced motion disables transitions and smooth scrolling.

The provisional geometric mark combines a Z traversal with open corner boundaries: an aperture rather than a shield or lock. SVG mark, path-based wordmark lockup and app icon share the geometry. Final logo review is pending. M1A status indicators always include text, and origin labels explicitly say they are illustrative and unauthenticated. UI approval does not produce or transmit evidence.

## Protocol-backed consent requirements

For future M1B/M1C, show the authenticated verifier origin, purpose, exact claim or threshold, context/policy, selected issuer, expiry and full outbound evidence. Distinguish source credentials, local scores and issuer-attested results. Keys, IDs, revocation handles and timestamps are visible to the verifier; threshold outcomes are not ZK proofs.

Approval and denial must be equally reachable. Default to no disclosure; no bundled consent, remembered blanket permission or preselected approval. Changing request/evidence invalidates approval. Unlocking does not approve. Denial emits no response. Future receipts and scoring traces must be encrypted and deletable; delivery does not establish acceptance. Private vault state must not enter SSR, shared caches or logs.
