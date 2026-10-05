# Threat model

Status: active implementation threat model. Assets: private credentials/events/scores, holder signing and vault keys, issuer keys, consent integrity, trust metadata, and one-time verifier acceptance.

Trust boundaries separate issuer authority, holder account state, verifier request state, the Zerant service, PostgreSQL/KMS custody, and Zcash-native services. The browser is not a secrets boundary. Issuers are trusted only for configured contexts/schemas; verifiers are potentially curious or malicious. Holder-signed assertions cannot replace issuer evidence.

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
| Issuer team privilege escalation | Owner/admin/issuer/auditor RBAC, one issuer membership per account, explicit invitation acceptance, owner-only transfer, server-side authorization | Compromised privileged member can act within its role; role-matrix and ownership-transfer tests |
| Ownership-transfer key loss | Rewrap issuer profile and signing-key ciphertext to the new owner before ownership changes | Service/KEK compromise remains; old-owner decrypt-failure regression test |
| Cross-application correlation | Independent audience keys and IDs, minimal result, no global holder ID | Issuer collusion, rare metadata, IP and account correlation remain |
| Credential-store compromise | Envelope encryption, PostgreSQL access control, KEK separation and server sessions | KEK/runtime compromise remains; encryption and session tests |
| XSS, dependency or browser compromise | No credential HTML rendering, CSP review, dependency pinning, secrets outside SSR/logs | Same-origin malicious code can read unlocked data; implementation security review |
| Status lookup tracking or central profiles | Local full snapshots, no individual lookups or portfolio endpoint | Future metadata hosting exposes access patterns; network/log inspection |
| DoS/resource exhaustion | Size limits, bounded event lists, no untrusted URL fetches, database-backed per-account write quotas, global active-authentication circuit breaker | Per-IP anonymous abuse remains an edge/WAF responsibility; oversize parser and quota tests |
| Verifier API-key theft | One-time secret display, SHA-256 hashed storage, explicit scopes, expiry, immediate revocation, server-only usage and shared verifier quotas | A stolen active key can act within its scopes until revoked or expired; operator secret management remains critical |
| Clock manipulation | Trusted local clock assumption, fail closed if unhealthy, no expiry grace | Undetected OS clock manipulation defeats time bounds; simulated-clock tests |

## Limits and response

M1 does not solve sybil resistance, issuer honesty, malicious device control, traffic anonymity, coercion, credential lending or global completeness of evidence. Loss of holder keys requires issuer reissuance after enrollment checks; no universal wallet-derived recovery. Compromised issuer keys require trust metadata update, rejection and reissuance. Revocation/distribution outages fail closed rather than expose extra attributes.

Before implementation, turn mitigations into independent acceptance fixtures, validate maintained crypto library behavior, and document chosen local persistence and authenticated-origin transport. Before hosted or Zcash testnet work, extend this model for deployment, metadata observability, service authentication, retention and wallet-specific authority. No security/privacy claim should exceed tested behavior.

## Zcash/payment integration threats

- **RPC overreach:** Zcash-specific access stays behind `zerant-zcash`; no seed phrase is accepted and generic credentials do not import wallet-wide balances/history. A compromised wallet/operator or RPC endpoint remains a deployment risk.
- **Payment-claim overreach:** a payment issuer must validate its own expected recipient, amount, network, transaction binding and confirmation/reorg policy before signing a narrow payment claim. A signed claim can still be dishonest if the issuer is dishonest or misconfigured.
- **Capability confusion:** discovering `z_sendmany` means the current Zallet RPC advertises that capability; it does not prove Zerant has safely executed a shielded payment. Live payment behavior must be demonstrated on official regtest before product claims change.

The local Z3 HTTP client disables proxies/redirects, bounds responses and allowlists
read-only methods. Loopback reachability alone does not authenticate a regtest network;
the operator must point it at the isolated official regtest deployment. Payment demo
helpers must prove regtest-only mining works before wallet mutations and use only
synthetic funds. Wallet readiness and confirmation count cannot prove amount/recipient
or settlement finality. Raw wallet responses, account fingerprints and transaction
memos must never be logged or included in generic attestations.


## Compound and payment threats

| Threat | Mitigation | Residual risk |
| --- | --- | --- |
| Partial requirement bypass | v0.3 verifies every ordered requirement before one atomic replay consumption | A trusted issuer can still make a dishonest assertion |
| Payment amount/recipient substitution | immutable intent digest plus exact native recipient and integer-zatoshi checks | Merchant/application must authenticate how the intent reached the payer |
| Payment reuse across invoices | unique transaction constraint and one-time intent credit | Cross-system reuse outside the shared ledger requires issuer coordination |
| RPC capability confusion | each wallet method is discovered independently; capability is never spend authorization | Beta Zallet methods can change between builds |
| Stale light-client failover | expected-network check, shared readiness cache, ordered trusted endpoints and a persistent block-height high-water mark reject candidates more than 20 blocks behind the last confirmed height; outage UI may show last-good state for at most 15 minutes with network actions disabled | A deep legitimate reorg beyond the bound fails closed and requires operator review; degraded state is informational and readiness is not consensus validation |
| RPC success mistaken for settlement | send results require explicit broadcast semantics; settlement still requires later named-transaction confirmation | Reorgs remain possible after finite confirmations |
| Transparent/privacy downgrade | automatic spend planning permits only `FullPrivacy`; transparent matching outputs fail settlement verification | External wallet tooling can still be misconfigured |
| Stale receipt replay | receipt observations have a freshness bound and are revalidated before credit | Clock compromise remains outside protocol guarantees |
| PCZT plan substitution | review acknowledgement binds exact plan digest and intent digest | PCZT inspection output must come from trusted wallet tooling |
| Shared-control secret leakage | Zerant stores no FROST/private-share material | External coordinator/tooling becomes its own trust boundary |

No generic Zerant verifier is entitled to whole-wallet RPC output. Reorg detection, downstream attestation revocation, invoice retention/deletion, and operational monitoring remain responsibilities of the payment issuer/application.

| Proof-package overexposure | dedicated `proofs:read` scope, verifier ownership check, encrypted stored response, approval-only export, and cryptographic revalidation before export | the approved proof intentionally reveals the requested verifier-specific claim and must be handled as sensitive application data |

## Server credential-vault threats

| Threat | Mitigation | Residual risk |
| --- | --- | --- |
| Database theft | per-record AES-256-GCM DEK; DEK separately wrapped by server KEK | ciphertext length/timing metadata remains observable |
| Record substitution | account ID, credential ID and key version are authenticated as AAD | application/database compromise can still delete or roll back rows |
| Session theft | 256-bit random token, DB stores only token hash, HttpOnly secure cookie, active-session review, remote revocation and fresh-session protection for remote sign-out | compromised device/browser can still act until its session is revoked or expires |
| ZecAuth replay | short-lived server challenge, exact message match, one-time consumption and one-time browser redemption | ZecAuth v1 capability grants remain server-authoritative |
| Zcash sign-in link theft or account fragmentation | dedicated challenge purpose, server-bound target and initiating session, recent active session recheck, unique identity ownership, no silent replacement, one-time message consumption | Compromise of a recent session, its link-attempt cookie, and an unlinked authentication key can add a sign-in method; the public wallet callback only records bounded pending verification and cannot finalize, while the original live session and one-time attempt are rechecked during completion |
| Obsolete or compromised Zcash authentication key | recent session required for removal; account row lock and full access-method count prevent last-method lockout; identity deletion and all Zcash-session revocation share a transaction | Sessions are not key-specific, so removal signs out every Zcash-authenticated session; an attacker holding a still-valid recent session may act before revocation. Rotation requires explicit remove and link. Wallet spending keys are unaffected. |
| KEK compromise | per-record wrapped DEKs and key versioning | managed KMS/HSM integration is still required for production |
| Incomplete vault KEK rotation | operator-only bounded transactions re-encrypt every encrypted record family; zero-reference inventory gates old-key removal | stale service instances can write old-version rows after a count, and live service or KEK compromise can expose plaintext during migration |
| Zcash operator compromise | browser never receives RPC credentials; API returns bounded projections only | compromised Z3/Zallet can lie about its own state |


## Issuer governance audit

Sensitive issuer actions are recorded atomically in `issuer_events`. PostgreSQL rejects UPDATE and DELETE operations on this table through an append-only trigger. The history records the acting Zerant ID, action, affected object, related counterparty and timestamp. Database-superuser compromise remains a residual risk and requires infrastructure-level logging and backup controls.

Browser wallet capability confusion is mitigated by explicit per-adapter capabilities and session-method checks. A WalletConnect pairing is not authentication; an address is never submitted as a Zerant identity. Rich ZIP-321 requests cannot be simplified for direct transfer, and a transparent-only connector cannot execute a shielded request. WalletConnect relay metadata, a compromised injected provider, URI-handler selection, and transparent-chain observability remain external trust/privacy risks. User approval in a wallet does not establish settlement.
