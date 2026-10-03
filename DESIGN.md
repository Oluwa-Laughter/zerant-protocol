# Zerant design system

Zerant should feel like serious privacy infrastructure: calm, precise, modern, and trustworthy. The UI is deliberately quieter than typical crypto products.

## Visual principles
- Use restrained surfaces, clear hierarchy, thin borders, and generous spacing.
- The consent review is the visual center of the product.
- Avoid generic crypto neon, glassmorphism, glowing shields/locks, excessive gradients, and decorative blockchain imagery.
- No fake metrics, testimonials, logos, or claims.
- Motion explains state changes only and respects `prefers-reduced-motion`.

## Tokens

| Token | Light | Dark | Role |
| --- | --- | --- | --- |
| background | `#F5F4EF` | `#111310` | page |
| card | `#FFFFFF` | `#1B1E19` | panels |
| foreground | `#171916` | `#F3F3ED` | primary text |
| muted | `#EBE9E1` | `#262922` | subtle fill |
| muted-foreground | `#666960` | `#A6AAA0` | secondary text |
| border | `#D8D6CC` | `#34382F` | separators |
| accent | `#B9A83A` | `#C8B852` | proof/verification accent |
| success | `#2F7251` | `#63B587` | accepted state |
| warning | `#8C671C` | `#D7AD57` | caution |
| error | `#9A4138` | `#E07E73` | invalid/denied |

## Typography
Use a high-quality system sans stack for reliability: Inter when available, then system UI. Monospace is reserved for identifiers, hashes, origins, protocol versions, and key material.

Headlines are compact, weight 600–650, with slightly negative tracking. Body copy prioritizes readability over density.

## Brand mark
The provisional Zerant mark combines:
1. a geometric Z path,
2. bounded corners suggesting explicit trust boundaries,
3. a single accent aperture/verification point.

It intentionally avoids shields, locks, eyes, and chain-link clichés. The mark must work in monochrome. Assets live in `apps/web/public/brand/`.

## Accessibility
Target WCAG 2.2 AA. Maintain visible focus, keyboard navigation, semantic status announcements, adequate contrast, and 44px interactive targets where practical. Never rely on color alone.

## Product surfaces
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

Layout: maximum 1280px outer container; horizontal gutters 48/30/20px; section spacing 84px desktop, 50px mobile. Surfaces use 4–10px corner radii, fine borders and limited elevation. Controls target at least 44px height and use a visible 3px focus outline. Tabs implement arrow/Home/End navigation. CSS transitions are limited to button opacity; reduced motion disables transitions and smooth scrolling.

The provisional geometric mark combines a Z traversal with open corner boundaries: an aperture rather than a shield or lock. SVG mark, path-based wordmark lockup and app icon share the geometry. Final logo review is pending. M1A status indicators always include text, and origin labels explicitly say they are illustrative and unauthenticated. UI approval does not produce or transmit evidence.
