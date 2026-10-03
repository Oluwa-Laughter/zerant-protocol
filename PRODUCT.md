# Zerant Protocol

Zerant is a proposed privacy-preserving credential and contextual reputation protocol for the Zcash ecosystem. A holder should prove eligibility, credentials, or trust while exposing only what the requesting application needs.

Issuers sign assertions they can substantiate. Holders keep credentials privately and approve disclosure. Verifiers request narrowly defined results and validate evidence against their own trust policy. A signature authenticates an issuer's assertion; it does not establish that the issuer is honest.

## Initial market wedge

Start with developer ecosystems and OSS communities: contribution eligibility, grant applications, hackathon participation, and accelerator admissions. A grants verifier might ask whether an applicant meets a community contribution threshold without receiving an exact score or contribution history. Reputation belongs to a named context and policy; it must never become a universal social score or an exchangeable ranking of people.

Holders remain free. Eventual revenue comes from B2B verifier/issuer infrastructure, integration support, and public metadata services. Revenue must not depend on selling holder profiles or tracking disclosure across applications. Pricing, product-market fit, and hosted services are not M1 commitments.

## Foundation and success

M1 is a local demonstration: issuance, encrypted holder storage, deterministic contextual scoring, domain-bound consent, minimal signed response, and fail-closed verification. It is successful when the negative privacy and security cases in the milestone specification pass, not when a wallet connects.

M1 has no Zcash testnet integration or ZK predicates. Stronger predicates and Zcash testnet identity are later work with separate security decisions. The protocol must remain usable without Zcash. See [scope](docs/scope/milestone-01.md) and [privacy limits](docs/PRIVACY.md).
