# Zerant Protocol agent instructions

Use the current session and this repository only. Do not retrieve previous sessions or inspect sibling projects unless the user explicitly requests it.

Before coding, read PRODUCT.md, DESIGN.md, docs/scope/milestone-01.md, docs/ARCHITECTURE.md, docs/PRIVACY.md, docs/THREAT_MODEL.md, and the relevant protocol specs. The repository contains the M1A web product shell and the M1B Rust core/credential implementation. Do not describe planned M1C/M2 behavior as implemented or introduce new dependencies without an explicit milestone need.

Document load-bearing decisions before implementation: trust boundaries, cryptographic profiles, subject binding, consent, storage, key lifecycle, revocation, replay handling, and data retention. Keep specs and implementation aligned; record changes and their privacy implications in docs/ARCHITECTURE.md before changing the contract.

Keep changes scoped. Avoid unrelated cleanup. Keep Zcash dependencies in zerant-zcash; generic credentials and reputation must not depend on wallet state. Do not invent cryptography or silently substitute a protocol. Use maintained libraries and official documentation, verify current compatibility, and create interoperable test vectors when implementation begins.

Never invent privacy guarantees. M1 is signed minimal disclosure using separate atomic attestations, not zero knowledge. Pairwise keys reduce obvious correlation but do not establish anonymity or unlinkability. Never describe planned behavior as implemented. Do not send whole credentials to a verifier to make verification easier.

Future implementation must test denied/unrequested disclosure, threshold leakage, domain substitution, tampering, expiry, revocation, key compromise, and concurrent/restarted replay handling. Inspect git status before editing; preserve user changes. Do not commit, push, publish, or deploy unless requested.
