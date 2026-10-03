# Contextual reputation v0.1

Status: proposed deterministic M1 policy. Reputation is scoped to a named context and policy version; there is no universal social score.

## Source and policy

Each source event is an issuer-signed atomic credential with a registry-controlled event claim type, stable event ID as its credential ID, context, event timestamp (`issued_at`), and bounded integer value. Issuer, signature, expiry, audience, and revocation must validate before use. A policy specifies `policy_id`, `version`, `context`, allowed event types and issuer IDs, integer weight per type, maximum count per type, a UTC evaluation cutoff, and threshold values. Policies are immutable once versioned and auditable as local configuration. Reject unrecognized inputs; count each credential ID once.

For each allowed type, sort eligible events by `issued_at` then credential ID, take the first `max_count` events, and sum `weight * value` using checked integers. The score is that sum within the named context and policy version. No cross-context aggregation or inferred traits. Negative weights/values may be allowed only if the specific policy declares bounds; default M1 example uses nonnegative integers. The policy must record why each event type matters, its cap, and score range. No opaque AI scoring.

## M1 threshold attestations

The holder can calculate this score locally from vault events for explanation and audit. A verifier cannot validate a hidden calculation from an ordinary signature. For a threshold-only request, a trusted issuer evaluates evidence available to it under the same policy and signs an origin-scoped `reputation.threshold` credential for `score >= threshold`. The verifier checks that signed boolean and exact policy context/version/threshold, not the holder's local numeric score. If issuer and holder inputs differ, the holder UI must show a mismatch and must not imply the attestation proves its current local score. Threshold credentials need expiry and revocation like any other credential.

The issuer might already have the underlying events; otherwise it must obtain them with separate holder consent. Such issuance discloses evidence to that issuer and is not private computation. Multiple-issuer aggregation requires an explicitly trusted evaluator or future cryptographic mechanism and is outside M1. Stronger zero-knowledge predicates may be specified later under a new version after security review.
