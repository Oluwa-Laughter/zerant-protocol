# Design direction

Build serious privacy infrastructure: calm, precise, modern, and trustworthy. Use readable typography, restrained neutral surfaces, generous spacing, clear borders, and a limited accent for actions and state. No generic crypto neon, glassmorphism, glowing shields or locks, excessive gradients, or generic three-feature-card SaaS layout.

The central interface is a disclosure review, not a marketing dashboard. Show the authenticated verifier origin, purpose, exact claim or threshold, context and policy, selected issuer, expiry, and the full outbound evidence metadata. Distinguish private source credentials, local scores, and issuer-attested results. Explain that the verifier sees the attestation's key, ID, revocation identifier, and timestamps as well as the requested result. Never label a threshold result a ZK proof.

Approval and denial are equally reachable. Default to no disclosure; no preselected approval, hidden optional attributes, bundled consent, or remembered blanket permission. M1 requests contain exactly one result. Changing any request or evidence invalidates approval and returns to review. Unlocking the vault does not approve a request. Denial creates no protocol response and sends no attribute values.

States: locked, request received, invalid request, reviewing, unavailable evidence, approved, generating, delivered, verification accepted/rejected, expired. Do not claim delivery means verification passed. Errors explain recovery without exposing private credential details. Show a local receipt of exactly what was sent; keep it encrypted and deletable.

Target WCAG 2.2 AA: semantic controls, keyboard navigation, visible unobscured focus, meaningful labels, sufficient contrast, accessible status announcements, suitable target sizes, and no color-only status. Respect reduced motion. Motion may explain a transition from consent to response to verification; it must not decorate or delay decisions. Verify with keyboard and assistive technology during implementation.

A future web app uses Next.js, React, TypeScript, Tailwind and shadcn/ui. TanStack Query is appropriate for public asynchronous metadata; private vault contents must not enter server rendering, shared query caches, analytics, or logs.
