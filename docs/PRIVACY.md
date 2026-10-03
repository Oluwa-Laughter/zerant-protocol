# Privacy model and limits

## M1 invariants

- Unrequested credentials are never returned.
- Denied attributes never appear in verifier responses, including error payloads and telemetry.
- A threshold-only request receives only an issuer-signed boolean threshold claim; it never receives the exact reputation score or underlying events.
- Wallet, address, balance, and transaction history are not fields in a generic protocol response.
- Requests bind verifier origin, challenge, nonce, and expiration; a response for another verifier/origin fails validation.
- No default backend can reconstruct a holder's complete credential profile. Holder credentials remain in an encrypted client-side vault.

## Necessary disclosures

An approval reveals one signed atomic claim, issuer/key reference, credential and revocation identifiers, origin-specific holder public key, and response timing. A boolean threshold can still reveal sensitive eligibility. A verifier can retain what it receives. The issuer knows its issuance and may know the verifier origin; a public revocation service may observe lookups. Origin-specific keys and credentials reduce trivial cross-verifier matching, but metadata, distinctive claims, network identifiers, issuer collusion, and repeated interactions can correlate a holder. No anonymity or unlinkability guarantee is made.

The holder's local score is not transmitted. Issuer-signed threshold predicates require the issuer to evaluate evidence it has authority to inspect. M1 does not hide that evidence from the issuer or prove the predicate to the verifier without issuer trust. More private proofs require a separately specified future mechanism.

## Data handling decisions before code

Specify vault encryption, key derivation, backup/recovery, lock timeout, origin authentication, metadata cache freshness, and analytics defaults before implementation. The local demo should use no analytics and keep secrets out of logs. Reject requests that cannot be displayed and validated exactly. Consent applies to one request and one response; it is never blanket authorization.
