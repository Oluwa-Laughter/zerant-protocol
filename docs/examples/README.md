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

## Composable trust and payment templates

All-of v0.3 combines exact ordinary claims, pinned thresholds and invoice-bound
paid booleans. Each requirement still consumes one atomic attestation, never a source
credential. No vertical-specific branch exists in the native verifier.

| Template | Compose explicit requirements | Disclosed | Kept private |
| --- | --- | --- | --- |
| Individual | membership.active=true, optional payment.invoice_paid=true | membership, opaque invoice digest, verification metadata | other memberships, identity records, wallet history |
| Freelancer | service.completed=completed + invoice-paid | one engagement result and invoice digest | clients, rates, source events, txid |
| Vendor | qualification threshold + invoice-paid | exact policy-bound true predicate and invoice digest | exact score, financial portfolio |
| Community | membership + contribution threshold + disbursement receipt | context facts, grant invoice digest | unrelated communities and contributions |
| Open source | contribution threshold + grant-paid | contribution eligibility and grant receipt boolean | exact score, full contribution history |
| Marketplace | fulfillment status + settlement | one fulfillment value and invoice digest | other orders, counterparties, wallet history |
| Organization | role.active=approver + approval.granted=true + treasury receipt | role and approval facts, intent digest | team directory and signing shares |
| Services / subscriptions | entitlement.active=true + renewal-paid | entitlement and renewal invoice digest | usage history and other subscriptions |
| Developer / agent API | service.authorized=true + usage-invoice-paid | machine-verifiable exact claims and digest | credentials for unrelated services, secrets |

For every template pin the issuer claim/context authorization and the verifier origin;
choose immutable policy bytes for thresholds. Payment issuers validate native exact
receipts before signing and bound expiry/revocation to their invoice policy. FROST
shared-control examples use adapter interfaces only; no live signing is advertised.
Require explicit per-request consent/authority for software agents too. A direct
native receipt exposes the selected txid/intent to its local issuer and should not be
forwarded to a generic verifier. None of these flows imply anonymity or unlinkability.
