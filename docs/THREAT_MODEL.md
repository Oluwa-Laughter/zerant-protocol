# Threat model

Status: preimplementation M1 model. Assets: private credentials/events/scores, holder signing and vault keys, issuer keys, consent integrity, trust metadata, and one-time verifier acceptance.

Trust boundaries separate issuer, holder browser/vault, verifier session/state, and public metadata. A local demo on one machine does not isolate these actors from a malicious OS. Issuers are trusted only for configured contexts/schemas; verifiers are potentially curious or malicious. Holder-signed assertions cannot replace issuer evidence.

| Threat | Required mitigation | Residual risk / later test |
| --- | --- | --- |
| Forged or modified credential/result | Pinned issuer keys, closed schemas, standard JWS, strict parsing | Honest signature does not imply honest issuer; tamper vectors |
| Malicious request over-discloses | One requirement, exact attestation match, explicit preview/approval, no fallback source upload | User may approve revealing predicates; deny/unknown-field tests |
| Score fabrication or policy substitution | Issuer independently evaluates pinned policy; holder score never proof | Issuer bias/fraud; mismatch/digest tests |
| Replay, concurrent acceptance, restart | Full request binding, authenticated session, durable atomic consumption | State loss invalidates outstanding requests; race/restart tests |
| Domain spoofing or phishing | Authenticated transport origin, exact audience, ASCII domain display, source-window checks | Social engineering remains; port/scheme/window substitution tests |
| Credential theft/transfer | Audience-key possession, encrypted vault, separate source key | Stolen signing key or voluntary sharing defeats possession; wrong-key test |
| Stale/revoked evidence | Signed whole snapshots, 24-hour maximum freshness, sequence persistence, dependency revocation | Up to 24-hour revocation lag; stale/rollback/source-revoke tests |
| Issuer compromise/rotation | Explicit compromise flags, old-key history, fail-closed reissuance | Attack before compromise known; key lifecycle tests |
| Cross-application correlation | Independent audience keys and IDs, minimal result, no global holder ID | Issuer collusion, rare metadata, IP and account correlation remain |
| Vault theft/weak password | Reviewed Web Crypto profile, encrypted manual backup, auto-lock | Offline guessing and backup loss remain; storage inspection/unlock tests |
| XSS, dependency or browser compromise | No credential HTML rendering, CSP review, dependency pinning, secrets outside SSR/logs | Same-origin malicious code can read unlocked data; implementation security review |
| Status lookup tracking or central profiles | Local full snapshots, no individual lookups or portfolio endpoint | Future metadata hosting exposes access patterns; network/log inspection |
| DoS/resource exhaustion | Size limits, bounded event lists, no untrusted URL fetches, request timeout | Future hosting needs rate limits; oversize parser tests |
| Clock manipulation | Trusted local clock assumption, fail closed if unhealthy, no expiry grace | Undetected OS clock manipulation defeats time bounds; simulated-clock tests |

## Limits and response

M1 does not solve sybil resistance, issuer honesty, malicious device control, traffic anonymity, coercion, credential lending or global completeness of evidence. Loss of holder keys requires issuer reissuance after enrollment checks; no universal wallet-derived recovery. Compromised issuer keys require trust metadata update, rejection and reissuance. Revocation/distribution outages fail closed rather than expose extra attributes.

Before implementation, turn mitigations into independent acceptance fixtures, validate maintained crypto library behavior, and document chosen local persistence and authenticated-origin transport. Before hosted or Zcash testnet work, extend this model for deployment, metadata observability, service authentication, retention and wallet-specific authority. No security/privacy claim should exceed tested behavior.
