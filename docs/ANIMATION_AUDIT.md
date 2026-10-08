# Animation and Responsive Verification

Validation date: 2026-10-08  
Site: https://zerant.vercel.app  
Reference workflow: MengTo `optimize-web-animations` (measure live behavior, reduce unnecessary offscreen work, respect reduced motion).

## Runtime motion audit

A Chrome DevTools Protocol script sampled the production landing page after hydration at the top, middle, and footer, using desktop 1440 × 1100 and mobile 390 × 844 viewports.

| View | Animated CSS instances | Running | Running offscreen | Horizontal overflow |
| --- | ---: | ---: | ---: | --- |
| Desktop top | 0 | 0 | 0 | No |
| Desktop middle | 0 | 0 | 0 | No |
| Desktop footer | 0 | 0 | 0 | No |
| Mobile top | 0 | 0 | 0 | No |
| Mobile middle | 0 | 0 | 0 | No |
| Mobile footer | 0 | 0 | 0 | No |

The source audit did not find persistent requestAnimationFrame or WebGL animation loops in the production component code. Zerant retains brief UI transition animations (including drawer entry) rather than continuous ambient animation. CSS includes a `prefers-reduced-motion: reduce` override that disables the drawer animation.

These measurements are **steady-state samples**, not frame-rate measurements or a comparative pre-redesign baseline. They do not establish a Core Web Vitals score or prove zero short-lived animation work during interactions.

## Responsive navigation interaction check

Browser automation checked the visible hamburger, open state, scroll lock, Escape dismissal, focus restoration, and overflow.

| Width | Tested route | Navigation | Opens | Escape closes | Focus returns | No overflow |
| --- | --- | --- | --- | --- | --- | --- |
| 360px | /app | Hamburger | Yes | Yes | Yes | Yes |
| 390px | /app | Hamburger | Yes | Yes | Yes | Yes |
| 430px | / | Hamburger | Yes | Yes | Yes | Yes |
| 768px | /app | Hamburger | Yes | Yes | Yes | Yes |
| 1024px | /app | Desktop | Not applicable | Not applicable | Not applicable | Yes |

Automated tests cover these mechanics, but a manual accessibility review with VoiceOver, TalkBack, or another screen reader would add further confidence. The authenticated issuer/verifier forms should be manually spot-checked on an actual mobile device before wider release.

## Design and performance guardrails

- Keep motion purposeful, brief, and based primarily on transforms or opacity.
- Respect `prefers-reduced-motion` for all nonessential movement.
- Never retain timers, animation frames, or observers after unmount.
- Do not animate hidden sections continuously; use an observer to pause any future ambient effects.
- Test normal, empty, loading, and error states at narrow widths.
- Re-run mobile and animation CDP checks after any major design-system changes.

Scripts used for the measurements were temporary local QA utilities and are not part of the deployed application.
