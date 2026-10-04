# Privacy contract and limits

Status: privacy contract and implementation limits. Native credential/disclosure code enforces several data-minimization and binding rules, while vault/browser transport and Zcash settlement privacy remain outside the implemented boundary. Zerant is signed minimal disclosure, not zero knowledge, anonymity or cryptographic unlinkability.

## Required M1 invariants

- Unrequested credentials are never returned. Private source credentials remain in the encrypted vault; a response contains one matching minimal attestation only.
- Denied attributes never appear in verifier responses. Denial emits no response; approving one claim never approves another.
- Exact reputation is not returned for threshold-only requests. Only the policy-bound issuer-attested boolean and required verification metadata appear.
- Wallet/address/balance/transaction history are not part of the generic protocol response.
- Requests and holder signatures are bound to verifier origin/domain, challenge, nonce and expiration, plus the complete request digest.
- Replay to another verifier/domain fails; replay to the same request fails through durable atomic consumption.
- Backend protocol data cannot reconstruct a holder's complete credential profile. No vault upload, central holder index, cross-issuer aggregation endpoint or source-credential telemetry exists by default.

## What is visible

| Party | Visible data |
| --- | --- |
| Originating issuer | Its enrollment/evidence, source subject key, its audience keys and issuance/revocation dependencies |
| Holder | Its decrypted vault while unlocked, local scores and audit trace, requests and outbound metadata |
| Verifier | One result, audience subject public key, issuer/key/schema/context/policy, credential/revocation IDs, validity times, request bindings |
| Future public metadata host | Public keys/schemas/policies/whole revocation snapshots; network access metadata if hosting is introduced |

Issuers know their own issuance records; no component is allowed to collect a holder-wide multi-issuer portfolio. A one-issuer demo naturally gives that issuer knowledge of all evidence it issued, not a guarantee that an issuer cannot know its own subjects. Issuer/verifier collusion can correlate audience keys via issuance records. Metadata hosts could correlate IP/timing; a future deployment needs a separate transport privacy review.

Independent keys and fresh IDs per audience remove obvious global identifiers. They do not prevent correlation by rare claims, timestamps, browser fingerprinting, accounts, IP addresses, timing or collusion. Repeated presentations to the same audience are linkable. Threshold responses disclose predicate outcomes and can enable inference. A policy may define a small reviewed set of supported thresholds, so repeated requests at different supported thresholds can narrow the holder's score range. Deployments should expose only the minimum threshold set their use case needs; there is no arbitrary score-query API. Issuer learning the audience during provisioning is an explicit tradeoff.

## Data handling

Vault encryption protects stored ciphertext against casual storage inspection, not malicious browser code, weak passphrases or device compromise. No server-side rendering of holder secrets. No analytics, plaintext logs, crash payloads, remote backups or public profile indexing. Store only required public metadata and request/replay state outside the vault. Consent receipts and local scoring traces are encrypted, deletable and not shared.

Verifier protocol state retains consumed request IDs until expiry and public revocation sequence watermarks thereafter. Do not persist raw responses by default; retain only request ID, boolean outcome and acceptance time until request expiry. A later business retention requirement needs a documented purpose, deletion policy and holder notice. Issuer retains only its issuance/dependency ledger needed for validity/revocation through credential expiry; enrollment retention beyond that requires a separate decision. Browser/device compromise or deliberate collusion is outside the backend profile invariant, not grounds for advertising stronger guarantees.

No public ledger receives credentials or presentations in M1. Zcash integration will require new decisions about observable identity and transport; Zcash's shielded transaction properties do not automatically apply to Zerant.

## Payment-specific privacy

Payment claims are optional domain assertions, not generic identity. The current Zcash adapter does not export wallet balances, addresses, seed fingerprints, memos or transaction history into Zerant credentials. A minimal paid-invoice attestation may be issued only after the responsible application or issuer validates its expected payment condition.

A boolean paid-invoice claim still reveals that the named business condition was satisfied. It does not make the invoice, browser session, issuer relationship or network traffic anonymous. Zcash shielded-transaction privacy does not automatically make a Zerant presentation unlinkable.
