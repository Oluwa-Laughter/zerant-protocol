# Protocol overview

Version 0.1 is a proposed M1 application protocol, not a claim of W3C VC compatibility or an implemented system. MUST/MUST NOT are normative requirements. Exact field contracts live in the three specs; privacy and threat documents define their limits.

## Local lifecycle

Issuer signs source credentials after authenticating its subject. Holder validates and encrypts them. Holder runs a public, pinned contextual policy locally. The issuer independently evaluates only its own evidence and provisions separate domain-bound attestations, with holder authorization for the audience key. These contain a single claim value or supported threshold boolean. No source IDs, global subject key, exact threshold score or evidence history are copied into attestations.

A verifier supplies a request through an origin-authenticated local browser flow. Holder matches an existing attestation, reviews result and necessary metadata, and explicitly approves. Holder signs the entire response under the attestation's audience key. Verifier checks its persisted request, trusted issuer and policy, credential and holder signatures, validity and revocation, then consumes the request atomically. Rejection never constitutes verified eligibility.

## Two disclosure levels

**M1 signed selective/minimal disclosure:** select one separately signed atomic attestation. Ordinary Ed25519 signatures establish issuer authenticity and holder key possession. An issuer-attested boolean hides the exact score from the verifier, but requires trusting issuer evaluation. Source credential redaction, unsigned holder booleans and hidden-input proofs are unsupported.

**Future stronger cryptographic predicates:** evaluate maintained selective-disclosure and ZK systems against issuer/holder/verifier trust and correlation requirements. A new version, security review, interoperable vectors and explicit implementation evidence are required before claiming any ZK or unlinkability property. No custom circuits in M1.

## Trust and transport

Pinned local metadata maps issuer ID + key ID to public keys, allowed schemas, contexts and policies, validity intervals and compromise status. Requests cannot nominate a new trust root. Authenticating transport origin is separate from signing credentials: production requires HTTPS and authenticated verifier sessions; the local demo permits explicitly configured loopback origins. A JSON string saying `verifier_origin` is not proof of origin. Generic pasted files or QR requests without authenticated origin provenance are unsupported in M1.

All roles use strict parsers, bounded payloads, explicit version negotiation and fail-closed errors. Unrecognized fields, versions, algorithms or operators are rejected. No remote key URLs from untrusted payloads are fetched. Verifiers retain only required result and replay/request state, never request additional credentials on a failed proof.

## Shared encoding rules

Use UTF-8 JSON canonicalized with [RFC 8785 JCS](https://www.rfc-editor.org/rfc/rfc8785). Reject duplicate keys, noncanonical payload bytes, floats, unsafe integers and invalid Unicode. Times are integer UTC Unix seconds. IDs are independent 128-bit CSPRNG values encoded base64url without padding; challenges/nonces are independent 256-bit values. Protocol origin strings follow the disclosure spec. Context/schema/policy identifiers are immutable local registry strings, not network fetch instructions.

Sign with standard compact JWS using Ed25519 and a maintained JOSE library; follow [RFC 8037](https://www.rfc-editor.org/rfc/rfc8037) and its algorithm update [RFC 9864](https://www.rfc-editor.org/rfc/rfc9864). The v0.1 profile pins `alg: EdDSA` (no algorithm negotiation), `kid`, and a message-specific `typ`, all protected. Reject unprotected headers, `none`, other algorithms, `crit`, detached payloads, and extra headers. JWS signs the standard encoded protected-header and payload signing input, not an invented concatenation. No private JWK `d` appears on the wire.
