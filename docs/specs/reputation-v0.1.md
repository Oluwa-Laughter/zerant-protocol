# Reputation v0.1

Status: implemented generic native policy parser/evaluator; vault and issuer dependency ledger remain application responsibilities. No universal score, AI model, hidden weighting or cross-context ranking.

## Policy contract

Immutable policy JSON has exactly `policy_id`, `version`, `context_id`, `source_schema_id`, `source_schema_version`, `issuer_id`, `window_seconds`, `category_rules`, `score_cap`, `supported_thresholds`, `operator`. The policy digest is SHA-256 of its JCS bytes. Verifier/holder/issuer pin the same bytes; a digest alone is not authorization. Policies are public local metadata; never fetch URLs from requests.

One example fixture: `policy_id: zerant:oss:contribution`, `version: 0.1`, `context_id: oss-community`, source schema `zerant:source:oss-contribution` version `0.1`, and `issuer_id: zerant:issuer:local-demo`. `window_seconds` is 7,776,000 (90 days), `score_cap` is 100, `supported_thresholds` is `[40]`, `operator` is `gte`. `category_rules` is exactly:

| Category | Integer weight | Maximum counted events |
| --- | --- | --- |
| `merged_contribution` | 10 | 6 |
| `review` | 5 | 4 |
| `mentorship` | 20 | 1 |

Represent each category rule as `{ "weight": <integer>, "max_events": <integer> }` under its category key. No negative events, normalization, floats, probabilistic features or inferred behavior. This fixture is a demo eligibility rule, not a validated measure of merit.

## Deterministic evaluation

At explicit integer `as_of`, select only this issuer's signature-valid, nonexpired, nonrevoked source credentials in the policy context/schema, bound to the same private source subject. Require each credential `issued_at <= as_of < expires_at`; revocation snapshot must be valid at evaluation. Missing/stale status or conflicting evidence returns unavailable, not zero.

Combine events; deduplicate by `(issuer_id, credential_id)`. Identical duplicate events count once. A repeated ID with different category/time makes evaluation unavailable. Count events satisfying `as_of - window_seconds < claim.occurred_at <= as_of`. Reject malformed source events or unknown categories. For each category multiply its weight by the lesser of event count and category cap. Sum and cap at 100. Threshold result is `score >= 40`.

Example: three merged contributions and two reviews yield 40, so the threshold result is true. Empty valid evidence yields 0/false; missing required status yields unavailable. Boundary event at exactly `as_of - window_seconds` is excluded. Policy version, event validity, rounding (none), caps and tie behavior are fixed, so equal input plus equal time yields equal output.

The holder keeps an encrypted local audit trace with policy digest, evaluation time, selected private source IDs, deduplication/exclusion reasons, counts and score. No trace is sent to verifiers. The issuer independently evaluates its own evidence using this policy, binds a pairwise audience key, and signs only a boolean attestation. Local holder score is not independently verifiable evidence. If holder and issuer disagree, stop and inspect local evidence; never silently replace a policy or sign an unchecked holder result.

## Freshness and limitations

Attestation `as_of` equals issuance. Its expiry is bounded by 24 hours, source expiry, and the next time a counted event leaves the window. Revoking a source propagates to dependent attestations through the issuer's revocation ledger, subject to the snapshot lag. Verifier checks current attestation validity and pinned policy; it does not reconstruct score inputs. It trusts the issuer's evaluation and limited freshness, not a hidden-input proof.

M1 supports one issuer and the fixed threshold per policy. No arbitrary threshold probing, exact-score responses or multi-issuer aggregation. Holder can omit evidence, so an assertion does not establish completeness of a global history. Do not use this policy for negative-history predicates. Issuer fraud, sybil enrollment, event quality and issuer bias remain trust/governance questions outside M1 automation. Future policies require immutable versions, documented rationale and evaluation; never silently repurpose this score in another context.

## Generic implementation

`zerant-policy` applies this schema to all contexts. Public examples live in
`fixtures/scenarios.json`; no use-case branch exists in the runtime. `evaluate` accepts
payloads already verified by its caller for issuer signatures, authorization and fresh
revocation at the explicit evaluation time. Local `Evaluation` and
`AuditSummary` are intentionally not serialized as verifier responses. Rules are bounded
to 64 categories, 64 unique thresholds and 4096 evidence records. Zero weights
and zero supported thresholds are allowed; safe integer bounds and wide intermediate arithmetic
prevent overflow. Exact-score wire disclosure is not implemented.
