---
name: zerant-security
description: Threat modeling, privacy invariants, abuse cases, and security review for Zerant.
---
# Zerant Security
Use for security review, privacy review, tests, and any feature touching credentials, keys, identity, or disclosure.

Read docs/THREAT_MODEL.md and docs/PRIVACY.md.

Always test negative guarantees:
- unrequested credentials are absent;
- denied attributes are absent;
- threshold-only response has no exact score;
- wallet/address/balance/history are absent;
- wrong origin/domain fails;
- wrong challenge/nonce fails;
- expired request fails;
- replay fails;
- revoked credential fails;
- unknown/stale issuer metadata fails closed.

Never claim anonymity, unlinkability, ZK, or Zcash privacy unless the implemented mechanism actually provides it.
