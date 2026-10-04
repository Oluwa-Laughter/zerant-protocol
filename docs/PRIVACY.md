# Privacy contract and limits

Status: privacy contract and implementation limits. Native credential/disclosure code enforces several data-minimization and binding rules, while vault/browser transport and Zcash settlement privacy remain outside the implemented boundary. Zerant is signed minimal disclosure, not zero knowledge, anonymity or cryptographic unlinkability.

## Pairwise verifier proof keys

Approved proofs now use a protected holder key scoped to the verifier relationship instead of exposing the holder account credential key. Repeated proofs to the same verifier reuse that relationship key, while a different verifier receives a different holder proof key. This reduces direct cross-verifier correlation from a stable cryptographic identifier.

This is not full unlinkability. The Zerant service can associate pairwise keys with the same account, issuers know their own issuance records, and network/timing/application metadata can still correlate activity. No anonymity or zero-knowledge claim follows from pairwise keys alone.


## Required M1 invariants

- Unrequested credentials are never returned. Private source credentials remain in the encrypted vault; a response contains one matching minimal attestation only.
- Denied attributes never appear in verifier responses. Denial emits no response; approving one claim never approves another.
- Exact reputation is not returned for threshold-only requests. Only the policy-bound issuer-attested boolean and required verification metadata appear.
- Wallet/address/balance/transaction history are not part of the generic protocol response.
- Requests and holder signatures are bound to verifier origin/domain, challenge, nonce and expiration, plus the complete request digest.
- Replay to another verifier/domain fails; replay to the same request fails through durable atomic consumption.
- Verifiers receive only approved request-bound results; they do not receive the holder's complete credential portfolio. The current server-first Zerant service can decrypt authorized holder records at runtime, so service compromise is part of the trust model and must be mitigated operationally.

## What is visible

| Party | Visible data |
| --- | --- |
| Originating issuer | Its enrollment/evidence, source subject key, its audience keys and issuance/revocation dependencies |
| Holder | Its decrypted vault while unlocked, local scores and audit trace, requests and outbound metadata |
| Verifier | One result, audience subject public key, issuer/key/schema/context/policy, credential/revocation IDs, validity times, request bindings |
| Future public metadata host | Public keys/schemas/policies/whole revocation snapshots; network access metadata if hosting is introduced |

Issuers know their own issuance records. Verifiers do not receive a holder-wide multi-issuer portfolio. The current Zerant service does maintain holder credential records and can decrypt them for authorized service operations; this is not end-to-end holder-only encryption. Issuer/verifier collusion, service compromise and network metadata can still create correlation risk.

Independent keys and fresh IDs per audience remove obvious global identifiers. They do not prevent correlation by rare claims, timestamps, browser fingerprinting, accounts, IP addresses, timing or collusion. Repeated presentations to the same audience are linkable. Threshold responses disclose predicate outcomes and can enable inference. A policy may define a small reviewed set of supported thresholds, so repeated requests at different supported thresholds can narrow the holder's score range. Deployments should expose only the minimum threshold set their use case needs; there is no arbitrary score-query API. Issuer learning the audience during provisioning is an explicit tradeoff.

## Data handling

Vault encryption protects stored ciphertext against casual storage inspection, not malicious browser code, weak passphrases or device compromise. No server-side rendering of holder secrets. No analytics, plaintext logs, crash payloads, remote backups or public profile indexing. Store only required public metadata and request/replay state outside the vault. Consent receipts and local scoring traces are encrypted, deletable and not shared.

Verifier protocol state retains consumed request IDs until expiry and public revocation sequence watermarks thereafter. Do not persist raw responses by default; retain only request ID, boolean outcome and acceptance time until request expiry. A later business retention requirement needs a documented purpose, deletion policy and holder notice. Issuer retains only its issuance/dependency ledger needed for validity/revocation through credential expiry; enrollment retention beyond that requires a separate decision. Browser/device compromise or deliberate collusion is outside the backend profile invariant, not grounds for advertising stronger guarantees.

No public ledger receives credentials or presentations in M1. Zcash integration will require new decisions about observable identity and transport; Zcash's shielded transaction properties do not automatically apply to Zerant.

## Payment-specific privacy

Payment claims are optional domain assertions, not generic identity. The current Zcash adapter does not export wallet balances, addresses, seed fingerprints, memos or transaction history into Zerant credentials. A minimal paid-invoice attestation may be issued only after the responsible application or issuer validates its expected payment condition.

A boolean paid-invoice claim still reveals that the named business condition was satisfied. It does not make the invoice, browser session, issuer relationship or network traffic anonymous. Zcash shielded-transaction privacy does not automatically make a Zerant presentation unlinkable.


## Compound and payment privacy

A compound request is one explicit all-of consent decision across 2–8 ordered atomic requirements. Every requirement is signed into the same origin/challenge/nonce/expiry binding. Partial satisfaction produces no accepted result, denial produces no response, and private source credentials are never substituted when evidence is missing.

For payment conditions, the verifier receives an issuer-backed `payment.invoice_paid=true` attestation whose context binds the immutable payment-intent digest. The native recipient, amount, transaction ID, memo, account metadata, and wallet history remain outside the compound response. The intent digest is still a correlatable application identifier within the parties that know the underlying intent; it is not an anonymity mechanism.

The browser integration console uses public fixtures and transient state only. Its settlement choices are simulations. Native Rust verification and previously recorded Z3 regtest evidence are labeled separately. No browser code receives wallet secrets, PCZT bytes, FROST shares, or native RPC credentials.

## Server credential vault

`/vault` is server-backed. The browser does not persist credentials, passphrases, wallet secrets, or bearer tokens. It renders server state and submits user actions through same-origin Next.js routes.

The Rust API stores durable credential state in PostgreSQL. Each credential is encrypted with a fresh random AES-256-GCM data key; that data key is wrapped under a versioned server key-encryption key. The database therefore stores credential ciphertext plus a wrapped DEK, not plaintext credentials or the KEK.

Authentication follows the ZecAuth v1 draft model: a purpose-specific RedPallas authentication key signs a short-lived domain/chain/nonce challenge. The auth key is distinct from Zcash spending authority. Session and authentication-attempt tokens are only delivered as HttpOnly, Secure, SameSite=Lax cookies.

Production deployment should place the KEK in a managed KMS/HSM rather than a long-lived raw environment value. Z3/Zallet credentials, FROST shares, Zcash seed phrases and spending keys remain outside the credential store.

Revocation status is visible to the credential holder and issuing organization. A verifier learns only whether an approved proof is valid; Zerant does not expose a holder-wide revocation inventory to generic verifiers.


## Issuer compromise recovery

Routine issuer-key rotation does not change holder identifiers or expose additional holder data. If an issuer reports its current key compromised, credentials signed by that key are invalidated and can no longer satisfy new proof requests. This is a trust-safety response, not an anonymity feature.


## Verifier compromise recovery

Routine verifier-key rotation does not reveal additional holder data and does not change already-issued credentials. A compromise replacement expires pending requests signed by the affected verifier key before a holder can approve them. This prevents a known-compromised request-signing key from continuing to authorize new disclosures.
