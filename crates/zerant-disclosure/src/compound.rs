//! Versioned all-of atomic disclosure. v0.2 APIs remain unchanged.
use super::*;
pub const SCHEMA: &str = "zerant.disclosure.request.v0.3";
const REQUEST_TYPE: &str = "zerant-request-v0.3";
const RESPONSE_TYPE: &str = "zerant-response-v0.3";

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompoundRequest {
    pub schema: String,
    /// Common request metadata; its atomic requirement is the first requirement.
    pub primary: Request,
    /// Ordered additional atomic requirements. All are mandatory (all-of).
    pub additional: Vec<Request>,
    /// Exact ordinary values in requirement order; threshold entries must be null.
    pub expected_values: Vec<Option<zerant_credential::ClaimValue>>,
}
impl CompoundRequest {
    pub fn validate(&self, context: &Context<'_>) -> Result<()> {
        if self.schema != SCHEMA || self.additional.is_empty() || self.additional.len() > 7 {
            return Err(Error::Credential);
        }
        self.primary
            .validate(context.now, context.origin, context.allowed_loopback)?;
        if self.expected_values.len() != self.additional.len() + 1 {
            return Err(Error::Credential);
        }
        for (requirement, expected) in self.requirements().zip(&self.expected_values) {
            if (requirement.predicate.is_some() && expected.is_some())
                || (requirement.predicate.is_none() && expected.is_none())
            {
                return Err(Error::Credential);
            }
            if requirement.claim_type == "payment.invoice_paid" {
                if expected != &Some(zerant_credential::ClaimValue::Boolean(true)) {
                    return Err(Error::Credential);
                }
                let digest = requirement
                    .context
                    .as_deref()
                    .and_then(|c| c.strip_prefix("payment-intent:"))
                    .ok_or(Error::Credential)?;
                zerant_core::decode_fixed::<32>(digest)?;
            }
        }
        let mut ids = vec![self.primary.request_id.clone()];
        for requirement in &self.additional {
            requirement.validate(context.now, context.origin, context.allowed_loopback)?;
            if requirement.verifier_id != self.primary.verifier_id
                || requirement.verifier_key_id != self.primary.verifier_key_id
                || requirement.verifier_origin != self.primary.verifier_origin
                || requirement.purpose != self.primary.purpose
                || requirement.challenge != self.primary.challenge
                || requirement.nonce != self.primary.nonce
                || requirement.issued_at != self.primary.issued_at
                || requirement.expires_at != self.primary.expires_at
                || ids.contains(&requirement.request_id)
            {
                return Err(Error::Credential);
            }
            ids.push(requirement.request_id.clone());
        }
        Ok(())
    }
    fn requirements(&self) -> impl Iterator<Item = &Request> {
        std::iter::once(&self.primary).chain(&self.additional)
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CompoundResponse {
    schema: String,
    request_id: String,
    request_digest: String,
    verifier_origin: String,
    challenge: String,
    nonce: String,
    attestations: Vec<String>,
    responded_at: u64,
}

pub fn sign(request: &CompoundRequest, context: &Context<'_>, key: &Jwk) -> Result<String> {
    request.validate(context)?;
    let signed = sign_jws(request, REQUEST_TYPE, &request.primary.verifier_key_id, key)?;
    verify(&signed, context)?;
    Ok(signed)
}
pub fn verify(token: &str, context: &Context<'_>) -> Result<CompoundRequest> {
    context
        .pin
        .validate(context.now, context.origin, context.allowed_loopback)?;
    let request: CompoundRequest =
        verify_jws(token, REQUEST_TYPE, &context.pin.key_id, &context.pin.key)?;
    request.validate(context)?;
    if request.primary.verifier_id != context.pin.verifier_id
        || request.primary.verifier_key_id != context.pin.key_id
        || request.primary.issued_at < context.pin.valid_from
        || request.primary.issued_at >= context.pin.valid_until
    {
        return Err(Error::Trust);
    }
    Ok(request)
}
fn verify_evidence(
    request: &CompoundRequest,
    context: &Context<'_>,
    evidence: &[Evidence<'_>],
) -> Result<Vec<CredentialPayload>> {
    if evidence.len() != request.additional.len() + 1 {
        return Err(Error::Credential);
    }
    let mut payloads: Vec<CredentialPayload> = Vec::new();
    for ((requirement, item), expected) in request
        .requirements()
        .zip(evidence)
        .zip(&request.expected_values)
    {
        let payload = verify_credential(
            item.attestation_jws,
            item.revocation_jws,
            context.trust,
            item.issuer_id,
            context.origin,
            context.allowed_loopback,
            context.now,
            item.minimum_revocation_version,
        )?
        .payload;
        requirement.matches_attestation(&payload)?;
        if let zerant_credential::Claim::Ordinary(claim) = &payload.claim
            && Some(&claim.value) != expected.as_ref()
        {
            return Err(Error::Credential);
        }
        if payloads.iter().any(|p| {
            p.subject_key != payload.subject_key || p.credential_id == payload.credential_id
        }) {
            return Err(Error::Credential);
        }
        payloads.push(payload);
    }
    Ok(payloads)
}
pub fn respond(
    token: &str,
    context: &Context<'_>,
    decision: Decision,
    evidence: &[Evidence<'_>],
    holder_key: &Jwk,
) -> Result<Option<String>> {
    let request = verify(token, context)?;
    if decision == Decision::Deny {
        return Ok(None);
    }
    let payloads = verify_evidence(&request, context, evidence)?;
    let response = CompoundResponse {
        schema: "zerant.disclosure.response.v0.3".into(),
        request_id: request.primary.request_id,
        request_digest: request_digest(token),
        verifier_origin: request.primary.verifier_origin,
        challenge: request.primary.challenge,
        nonce: request.primary.nonce,
        attestations: evidence.iter().map(|e| e.attestation_jws.into()).collect(),
        responded_at: context.now,
    };
    let token = sign_jws(
        &response,
        RESPONSE_TYPE,
        &payloads[0].subject_key.x,
        holder_key,
    )?;
    let _: CompoundResponse = verify_jws(
        &token,
        RESPONSE_TYPE,
        &payloads[0].subject_key.x,
        &payloads[0].subject_key,
    )?;
    Ok(Some(token))
}
pub fn register(token: &str, context: &Context<'_>, store: &dyn ReplayStore) -> Result<()> {
    let request = verify(token, context)?.primary;
    store.register(ReplayEntry {
        request_id: request.request_id,
        digest: request_digest(token),
        origin: request.verifier_origin,
        expires_at: request.expires_at,
        status: ReplayStatus::Pending,
    })
}
pub fn accept(
    token: &str,
    response: &str,
    context: &Context<'_>,
    evidence: &[Evidence<'_>],
    store: &dyn ReplayStore,
) -> Result<Vec<CredentialPayload>> {
    let request = verify(token, context)?;
    let digest = request_digest(token);
    let row = store
        .get(&request.primary.request_id)?
        .ok_or(Error::Trust)?;
    if row.status != ReplayStatus::Pending
        || row.digest != digest
        || row.origin != context.origin
        || row.expires_at != request.primary.expires_at
    {
        return Err(Error::Trust);
    }
    let payloads = verify_evidence(&request, context, evidence)?;
    let response: CompoundResponse = verify_jws(
        response,
        RESPONSE_TYPE,
        &payloads[0].subject_key.x,
        &payloads[0].subject_key,
    )?;
    if response.schema != "zerant.disclosure.response.v0.3"
        || response.request_id != request.primary.request_id
        || response.request_digest != digest
        || response.verifier_origin != context.origin
        || response.challenge != request.primary.challenge
        || response.nonce != request.primary.nonce
        || response.responded_at < request.primary.issued_at
        || response.responded_at > context.now
        || response.attestations
            != evidence
                .iter()
                .map(|e| e.attestation_jws.to_string())
                .collect::<Vec<_>>()
    {
        return Err(Error::Credential);
    }
    for payload in &payloads {
        payload.validate(
            response.responded_at,
            context.origin,
            context.allowed_loopback,
        )?;
    }
    store.consume_if_pending(
        &request.primary.request_id,
        &digest,
        context.origin,
        context.now,
    )?;
    Ok(payloads)
}
