---
name: zerant-architect
description: System architecture and load-bearing technical decisions for Zerant Protocol.
---

# Zerant Architecture

Use for stack, data ownership, service boundaries, protocol boundaries, and integration decisions.

Read docs/ARCHITECTURE.md, docs/PROTOCOL.md, docs/PRIVACY.md, and docs/THREAT_MODEL.md.

Rules:
- Document load-bearing trust and data-lifecycle decisions before implementation.
- Keep generic credential and policy logic independent of Zcash adapters.
- Respect the current Next.js, private Rust/Axum API, and PostgreSQL service boundaries.
- Credential records currently use server-side encryption; do not misrepresent them as holder-only or end-to-end encrypted.
- Keep wallet spending authorization external and distinct from Zerant sessions.
- Avoid new services or dependencies without a concrete product or security need.
- Write specs with acceptance criteria, negative tests, and residual risks.
