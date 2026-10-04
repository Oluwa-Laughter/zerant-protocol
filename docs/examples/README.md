# Zerant use-case examples

Zerant uses one protocol across different applications. These examples are public policy templates, not separate product forks and not universal reputation scores.

| Example | What a verifier can ask for | What should remain private |
| --- | --- | --- |
| Freelancer | completed service / supported threshold | unrelated clients, rates, wallet history |
| Business / vendor | vendor qualification | unrelated customers, full financial history |
| Community | active membership / contribution threshold | unrelated communities and source events |
| Grant | eligibility under one named policy | exact local score and full portfolio |
| Open source | contribution threshold | complete contribution history |
| Marketplace | fulfillment/service threshold | unrelated buyers/sellers and transactions |
| Organization | active role/membership | unrelated teams and credentials |
| Payment receipt | paid-invoice attestation | txid, address, balance, memo and wallet history |

Each JSON file under policies/ uses the same zerant-policy schema. Production deployments should create immutable, reviewed policies for their own context and pin the JCS SHA-256 digest. A policy digest is not itself authorization; issuer/verifier trust still applies.

The payment example is especially narrow: Zerant does not treat a wallet or transaction history as identity. An authorized issuer may attest that one business condition such as payment.invoice_paid is true after it independently validates the expected recipient, amount, network, transaction binding, confirmation/reorg policy and invoice context.
