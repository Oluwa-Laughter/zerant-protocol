---
name: zerant-frontend
description: Next.js, React, Tailwind, shadcn/ui, TanStack Query, accessibility, and motion standards for Zerant.
---
# Zerant Frontend
Use for UI, design system, client state, data fetching, responsiveness, and animation.

Read DESIGN.md first.

Rules:
- Next.js + React + TypeScript.
- Tailwind + shadcn/ui; use TanStack Query for server state, not local UI state.
- Validate data boundaries and keep query keys deterministic.
- Consent UI is a security surface: show exact requester, claim, purpose, expiry, and exact disclosure.
- Default to no disclosure.
- Never leak denied claims through UI telemetry or network requests.
- Meet WCAG 2.2 AA; visible focus, keyboard access, reduced motion.
- Motion explains verification/disclosure state; it does not decorate.
- Avoid crypto neon, glassmorphism, generic gradient SaaS layouts, and random lock/shield imagery.
