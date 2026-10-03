# Threat model — M1

## Assets and adversaries

Protect credential plaintext, vault and subject private keys, issuer signing keys, consent decisions, and verifier nonce state. Consider a malicious verifier, forged issuer/verifier metadata, compromised browser or device, replaying network party, curious metadata host, and colluding issuers/verifiers. M1 assumes trusted local device execution and trusted issuer evidence checking; compromise of those assumptions is outside its protection.

| Threat | M1 control | Residual risk |
| --- | --- | --- |
| Forged or altered credential | Allowlisted issuer key, Ed25519 signature, exact scope and schema checks | Trusted issuer may issue false statements; key compromise requires rotation/revocation |
| Request impersonates another domain | Authenticated origin bound to allowlisted verifier key and signed request | Local simulation is not origin security; transport integration must enforce it |
| Stolen response or replay | Short expiry, challenge/nonce and origin binding, holder signature, atomic nonce consumption | Compromised verifier can reuse data it legitimately saw |
| Revoked or stale credential | Signed revocation list with freshness deadline; fail closed | Revocation publication delay; offline use unavailable past deadline |
| Over-disclosure | One claim per credential, exact response preview, explicit per-request consent | A narrow claim can still be identifying |
| Cross-app correlation | Per-origin holder keys and credentials; reject audience mismatch | Issuer, rare claims, timing, IP, and collusion may correlate |
| Vault theft | Client-side encryption at rest and no default central credential store | Unlocked or compromised device can expose plaintext |
| Score manipulation | Versioned deterministic policy and issuer-signed threshold result | Issuer trust and source evidence quality remain decisive |

## Required review gates

Before code: document vault encryption and recovery, key rotation, issuer/verifier trust bootstrap, transport/origin binding, revocation freshness, and logging behavior. Before any public deployment: review dependency and cryptographic library choices, adversarial tests, privacy leakage, and operational incident handling. Future Zcash identity and stronger predicate proofs require their own threat model updates.
