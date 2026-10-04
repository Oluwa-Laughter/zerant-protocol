//! Generic bounded contextual policy evaluation. Local scores are never verifier proof.
#![forbid(unsafe_code)]
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use zerant_core::{
    Error, MAX_SAFE_INTEGER, Result, canonicalize, encode_base64url, parse_canonical,
    validate_timestamp,
};
use zerant_credential::{Claim, CredentialKind, CredentialPayload, HOLDER_LOCAL_AUDIENCE};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CategoryRule {
    pub weight: u64,
    pub max_events: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub policy_id: String,
    pub version: String,
    pub context_id: String,
    pub source_schema_id: String,
    pub source_schema_version: String,
    pub issuer_id: String,
    pub window_seconds: u64,
    pub category_rules: BTreeMap<String, CategoryRule>,
    pub score_cap: u64,
    pub supported_thresholds: Vec<u64>,
    pub operator: String,
}
impl Policy {
    /// Parses canonical JSON and requires a caller-pinned digest. Examples may be
    /// pretty printed: canonicalize their JSON value before parsing.
    pub fn parse(bytes: &[u8], pinned_digest: &str) -> Result<Self> {
        let policy: Self = parse_canonical(bytes)?;
        policy.validate()?;
        if policy.digest()? != pinned_digest {
            return Err(Error::Trust);
        }
        Ok(policy)
    }
    pub fn digest(&self) -> Result<String> {
        self.validate()?;
        Ok(encode_base64url(&Sha256::digest(canonicalize(self)?)))
    }
    pub fn validate(&self) -> Result<()> {
        let invalid = |s: &str| s.trim().is_empty() || s.len() > 256;
        if [
            &self.policy_id,
            &self.version,
            &self.context_id,
            &self.source_schema_id,
            &self.source_schema_version,
            &self.issuer_id,
        ]
        .iter()
        .any(|s| invalid(s))
            || self.operator != "gte"
            || self.window_seconds == 0
            || self.window_seconds > MAX_SAFE_INTEGER
            || self.score_cap == 0
            || self.score_cap > MAX_SAFE_INTEGER
            || self.category_rules.is_empty()
            || self.category_rules.len() > 64
            || self.supported_thresholds.is_empty()
            || self.supported_thresholds.len() > 64
            || self
                .supported_thresholds
                .iter()
                .any(|t| *t > self.score_cap)
            || self
                .supported_thresholds
                .iter()
                .collect::<BTreeSet<_>>()
                .len()
                != self.supported_thresholds.len()
            || self.category_rules.iter().any(|(k, r)| {
                invalid(k)
                    || r.weight > MAX_SAFE_INTEGER
                    || r.max_events == 0
                    || r.max_events > MAX_SAFE_INTEGER
            })
        {
            return Err(Error::Credential);
        }
        Ok(())
    }
}
/// Local-only summaries deliberately cannot be serialized as disclosure messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditSummary {
    pub supplied: u64,
    pub unique: u64,
    pub counted_by_category: BTreeMap<String, u64>,
    pub excluded_by_reason: BTreeMap<String, u64>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evaluation {
    pub score: u64,
    pub thresholds: BTreeMap<u64, bool>,
    pub audit: AuditSummary,
    /// Issuers must also bound dependent attestation expiry by source expiry.
    pub next_change_at: Option<u64>,
}
/// Evaluate ONLY payloads already signature/trust/revocation verified by caller at
/// as_of. This API cannot establish authenticity or completeness of supplied data.
/// Invalid/mismatched evidence fails unavailable; valid old events are excluded.
pub fn evaluate(
    policy: &Policy,
    verified_sources: &[CredentialPayload],
    as_of: u64,
) -> Result<Evaluation> {
    policy.validate()?;
    validate_timestamp(as_of)?;
    if verified_sources.len() > 4096 {
        return Err(Error::Size);
    }
    let mut unique = BTreeMap::<(&str, &str), Vec<u8>>::new();
    let mut subject = None;
    let mut counts = BTreeMap::<String, u64>::new();
    let mut excluded = BTreeMap::<String, u64>::new();
    let mut next = None;
    for payload in verified_sources {
        payload.validate(as_of, HOLDER_LOCAL_AUDIENCE, &[])?;
        let Claim::Source(claim) = &payload.claim else {
            return Err(Error::Credential);
        };
        if payload.kind != CredentialKind::Source
            || payload.issuer_id != policy.issuer_id
            || claim.context != policy.context_id
            || claim.claim_type != policy.source_schema_id
            || claim.source_schema_version != policy.source_schema_version
            || !policy.category_rules.contains_key(&claim.value)
            || claim.occurred_at > as_of
        {
            return Err(Error::Credential);
        }
        if subject.is_some_and(|s| s != &payload.subject_key) {
            return Err(Error::Credential);
        }
        subject = Some(&payload.subject_key);
        let bytes = canonicalize(payload)?;
        let key = (payload.issuer_id.as_str(), payload.credential_id.as_str());
        if let Some(previous) = unique.get(&key) {
            if previous != &bytes {
                return Err(Error::Credential);
            }
            *excluded.entry("identical_duplicate".into()).or_default() += 1;
            continue;
        }
        unique.insert(key, bytes);
        if as_of
            .checked_sub(policy.window_seconds)
            .is_some_and(|start| claim.occurred_at <= start)
        {
            *excluded.entry("outside_window".into()).or_default() += 1;
            continue;
        }
        *counts.entry(claim.value.clone()).or_default() += 1;
        // u128 avoids overflow even for safe timestamps near the JCS maximum.
        // Source expiry is safe and bounds the result back into u64.
        let change = (u128::from(claim.occurred_at) + u128::from(policy.window_seconds))
            .min(u128::from(payload.expires_at)) as u64;
        next = Some(next.map_or(change, |n: u64| n.min(change)));
    }
    let mut score = 0u128;
    let mut counted = BTreeMap::new();
    for (category, rule) in &policy.category_rules {
        let count = counts.get(category).copied().unwrap_or(0);
        let capped = count.min(rule.max_events);
        counted.insert(category.clone(), capped);
        *excluded.entry("category_cap".into()).or_default() += count - capped;
        score = (score + u128::from(rule.weight) * u128::from(capped))
            .min(u128::from(policy.score_cap));
    }
    let score = score as u64;
    Ok(Evaluation {
        score,
        thresholds: policy
            .supported_thresholds
            .iter()
            .map(|t| (*t, score >= *t))
            .collect(),
        audit: AuditSummary {
            supplied: verified_sources.len() as u64,
            unique: unique.len() as u64,
            counted_by_category: counted,
            excluded_by_reason: excluded,
        },
        next_change_at: next,
    })
}
#[cfg(test)]
mod tests;
