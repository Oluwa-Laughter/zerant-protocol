---
name: zerant-architect
description: System architecture and load-bearing technical decisions for Zerant Protocol.
---
# Zerant Architecture
Use for stack, data ownership, service boundaries, protocol boundaries, and integration decisions.

Read docs/ARCHITECTURE.md, docs/PROTOCOL.md, docs/PRIVACY.md, and docs/THREAT_MODEL.md.

Rules:
- Document load-bearing decisions before implementation.
- Keep generic credential/reputation logic independent of Zcash adapters.
- Prefer a simple monorepo and explicit trust boundaries.
- Holder credentials remain client-side by default.
- Do not introduce services/databases until a real requirement exists.
- Write specs with acceptance criteria and failure cases.
- State assumptions and residual risks.
