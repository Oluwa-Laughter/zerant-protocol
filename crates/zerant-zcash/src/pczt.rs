//! Review-first coordination boundary. No threshold cryptography or secret custody.
use crate::Capabilities;
use crate::payment::{PaymentIntent, PrivacyPolicy};
use serde::Serialize;
use sha2::{Digest, Sha256};
use zerant_core::{Error, MAX_JSON_BYTES, Result, canonicalize, encode_base64url};

/// Sensitive native artifact: no Debug/Serialize implementation or browser transport.
pub struct PcztArtifact(Vec<u8>);
impl PcztArtifact {
    pub fn new(bytes: Vec<u8>) -> Result<Self> {
        if bytes.is_empty() || bytes.len() > MAX_JSON_BYTES {
            return Err(Error::Size);
        }
        Ok(Self(bytes))
    }
    pub fn bytes(&self) -> &[u8] {
        &self.0
    }
    pub fn digest(&self) -> String {
        encode_base64url(&Sha256::digest(&self.0))
    }
}
/// Trusted adapter's review projection. PCZT inspection is creator-claimed metadata,
/// not cryptographic validation; extract must validate the final transaction.
#[derive(Clone)]
pub struct PlanReview {
    pub recipient: String,
    pub amount_zat: u64,
    pub fee_zat: u64,
    pub required_privacy: PrivacyPolicy,
    pub transparent_inputs: bool,
    pub transparent_outputs: bool,
    pub reference_commitment: Option<String>,
}
/// Implementations must bind authenticated external signing approvals to the exact
/// artifact. They must verify extraction against the original proposal before broadcast.
pub trait PcztBackend {
    fn create(&self, intent: &PaymentIntent) -> Result<PcztArtifact>;
    fn inspect(&self, artifact: &PcztArtifact) -> Result<PlanReview>;
    fn prove(&self, artifact: &PcztArtifact) -> Result<PcztArtifact>;
    fn sign(&self, artifact: &PcztArtifact, privacy: PrivacyPolicy) -> Result<PcztArtifact>;
    fn combine(&self, artifacts: &[PcztArtifact]) -> Result<PcztArtifact>;
    /// Returns a validated transaction reference only; no automatic settlement claim.
    fn extract(&self, artifact: &PcztArtifact, intent: &PaymentIntent) -> Result<String>;
}

/// Opaque consent binds the exact inspected artifact, immutable intent, review
/// projection, and fee ceiling. The review is re-derived from the backend before signing.
pub struct ReviewAcknowledgement {
    artifact_digest: String,
    intent_digest: String,
    review_digest: String,
    maximum_fee_zat: u64,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct ReviewBinding<'a> {
    recipient: &'a str,
    amount_zat: u64,
    fee_zat: u64,
    required_privacy: PrivacyPolicy,
    transparent_inputs: bool,
    transparent_outputs: bool,
    reference_commitment: &'a Option<String>,
}

fn review_digest(review: &PlanReview) -> Result<String> {
    let binding = ReviewBinding {
        recipient: &review.recipient,
        amount_zat: review.amount_zat,
        fee_zat: review.fee_zat,
        required_privacy: review.required_privacy,
        transparent_inputs: review.transparent_inputs,
        transparent_outputs: review.transparent_outputs,
        reference_commitment: &review.reference_commitment,
    };
    Ok(encode_base64url(&Sha256::digest(canonicalize(&binding)?)))
}

fn validate_review(
    review: &PlanReview,
    intent: &PaymentIntent,
    maximum_fee_zat: u64,
) -> Result<()> {
    if intent.privacy_policy != PrivacyPolicy::FullPrivacy
        || review.recipient != intent.recipient
        || review.amount_zat != intent.amount_zat
        || review.fee_zat > maximum_fee_zat
        || review.transparent_inputs
        || review.transparent_outputs
        || review.reference_commitment != intent.reference_commitment
        || review.required_privacy != intent.privacy_policy
    {
        return Err(Error::Credential);
    }
    Ok(())
}

/// Derive the user-visible review from the exact artifact through the trusted backend.
/// Callers must display this returned review rather than constructing review data themselves.
pub fn inspect_and_acknowledge(
    backend: &dyn PcztBackend,
    artifact: &PcztArtifact,
    intent: &PaymentIntent,
    maximum_fee_zat: u64,
) -> Result<(PlanReview, ReviewAcknowledgement)> {
    let review = backend.inspect(artifact)?;
    validate_review(&review, intent, maximum_fee_zat)?;
    let acknowledgement = ReviewAcknowledgement {
        artifact_digest: artifact.digest(),
        intent_digest: intent.digest()?,
        review_digest: review_digest(&review)?,
        maximum_fee_zat,
    };
    Ok((review, acknowledgement))
}

pub fn sign_reviewed(
    backend: &dyn PcztBackend,
    artifact: &PcztArtifact,
    intent: &PaymentIntent,
    acknowledgement: &ReviewAcknowledgement,
    now: u64,
    origin: &str,
    loopback: &[String],
) -> Result<PcztArtifact> {
    intent.validate(now, origin, loopback)?;
    if intent.network != crate::payment::ZcashNetwork::Regtest
        || !intent.privacy_policy.automatic_spend_allowed()
    {
        return Err(Error::Trust);
    }
    if acknowledgement.artifact_digest != artifact.digest()
        || acknowledgement.intent_digest != intent.digest()?
    {
        return Err(Error::Trust);
    }

    let current_review = backend.inspect(artifact)?;
    validate_review(&current_review, intent, acknowledgement.maximum_fee_zat)?;
    if acknowledgement.review_digest != review_digest(&current_review)? {
        return Err(Error::Trust);
    }

    backend.sign(artifact, intent.privacy_policy)
}

#[derive(Debug, PartialEq, Eq)]
pub enum SpendPath {
    PcztReviewAvailable,
    LegacyExplicitReviewRequired,
    Unavailable,
}
/// Availability is not readiness or permission. Concrete HTTP sends remain disabled.
pub fn preferred_path(capabilities: &Capabilities) -> SpendPath {
    if capabilities.pczt_complete {
        SpendPath::PcztReviewAvailable
    } else if capabilities.sendmany_advertised || capabilities.sendfromaccount_advertised {
        SpendPath::LegacyExplicitReviewRequired
    } else {
        SpendPath::Unavailable
    }
}

/// External FROST coordinator owns participant secrets and cryptographic verification.
/// Zerant accepts only validated approval records from that trusted boundary.
pub trait SharedControlCoordinator {
    fn verify_approval(&self, participant: &str, plan_digest: &str, approval: &[u8]) -> Result<()>;
}
pub struct SharedControl {
    plan_digest: String,
    participants: Vec<String>,
    approved: Vec<String>,
    threshold: usize,
}
impl SharedControl {
    pub fn new(
        artifact: &PcztArtifact,
        participants: Vec<String>,
        threshold: usize,
    ) -> Result<Self> {
        if participants.is_empty()
            || participants.len() > 32
            || threshold == 0
            || threshold > participants.len()
            || participants.iter().any(|p| p.is_empty() || p.len() > 256)
            || participants.windows(2).any(|p| p[0] >= p[1])
        {
            return Err(Error::Trust);
        }
        Ok(Self {
            plan_digest: artifact.digest(),
            participants,
            approved: vec![],
            threshold,
        })
    }
    pub fn approve(
        &mut self,
        coordinator: &dyn SharedControlCoordinator,
        participant: &str,
        approval: &[u8],
    ) -> Result<()> {
        if approval.is_empty()
            || approval.len() > MAX_JSON_BYTES
            || !self.participants.iter().any(|p| p == participant)
            || self.approved.iter().any(|p| p == participant)
        {
            return Err(Error::Trust);
        }
        coordinator.verify_approval(participant, &self.plan_digest, approval)?;
        self.approved.push(participant.into());
        Ok(())
    }
    pub fn ready(&self) -> bool {
        self.approved.len() >= self.threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CapabilityState,
        payment::{PAYMENT_INTENT_SCHEMA, ZcashNetwork},
    };
    use std::collections::BTreeMap;
    use zerant_core::encode_base64url;

    fn intent() -> PaymentIntent {
        PaymentIntent {
            schema: PAYMENT_INTENT_SCHEMA.into(),
            intent_id: encode_base64url(&[1; 16]),
            network: ZcashNetwork::Regtest,
            requester_origin: "https://pay.example".into(),
            recipient: "uregtest-recipient".into(),
            amount_zat: 75_000,
            min_confirmations: 3,
            privacy_policy: PrivacyPolicy::FullPrivacy,
            issued_at: 100,
            expires_at: 500,
            reference_commitment: None,
        }
    }

    struct ReviewBackend {
        review: PlanReview,
    }
    impl PcztBackend for ReviewBackend {
        fn create(&self, _: &PaymentIntent) -> Result<PcztArtifact> {
            PcztArtifact::new(vec![1, 2, 3])
        }
        fn inspect(&self, _: &PcztArtifact) -> Result<PlanReview> {
            Ok(self.review.clone())
        }
        fn prove(&self, artifact: &PcztArtifact) -> Result<PcztArtifact> {
            PcztArtifact::new(artifact.bytes().to_vec())
        }
        fn sign(&self, artifact: &PcztArtifact, _: PrivacyPolicy) -> Result<PcztArtifact> {
            PcztArtifact::new(artifact.bytes().to_vec())
        }
        fn combine(&self, _: &[PcztArtifact]) -> Result<PcztArtifact> {
            Err(Error::Trust)
        }
        fn extract(&self, _: &PcztArtifact, _: &PaymentIntent) -> Result<String> {
            Err(Error::Trust)
        }
    }

    fn valid_review() -> PlanReview {
        PlanReview {
            recipient: intent().recipient,
            amount_zat: 75_000,
            fee_zat: 1_000,
            required_privacy: PrivacyPolicy::FullPrivacy,
            transparent_inputs: false,
            transparent_outputs: false,
            reference_commitment: None,
        }
    }

    #[test]
    fn review_acknowledgement_is_derived_from_exact_backend_inspection() {
        let artifact = PcztArtifact::new(vec![1, 2, 3]).unwrap();
        let backend = ReviewBackend {
            review: valid_review(),
        };
        let (_review, ack) =
            inspect_and_acknowledge(&backend, &artifact, &intent(), 1_000).unwrap();
        sign_reviewed(
            &backend,
            &artifact,
            &intent(),
            &ack,
            100,
            "https://pay.example",
            &[],
        )
        .unwrap();

        let mut wrong = valid_review();
        wrong.transparent_outputs = true;
        let bad_backend = ReviewBackend { review: wrong };
        assert!(inspect_and_acknowledge(&bad_backend, &artifact, &intent(), 1_000).is_err());
    }

    struct Approval;
    impl SharedControlCoordinator for Approval {
        fn verify_approval(
            &self,
            participant: &str,
            _plan_digest: &str,
            approval: &[u8],
        ) -> Result<()> {
            if participant.is_empty() || approval != b"approved" {
                return Err(Error::Signature);
            }
            Ok(())
        }
    }

    #[test]
    fn shared_control_requires_unique_authenticated_threshold_approvals() {
        let artifact = PcztArtifact::new(vec![7, 8, 9]).unwrap();
        let mut control = SharedControl::new(
            &artifact,
            vec!["alice".into(), "bob".into(), "carol".into()],
            2,
        )
        .unwrap();
        control.approve(&Approval, "alice", b"approved").unwrap();
        assert!(!control.ready());
        assert!(control.approve(&Approval, "alice", b"approved").is_err());
        control.approve(&Approval, "bob", b"approved").unwrap();
        assert!(control.ready());
    }

    #[test]
    fn preferred_path_never_infers_missing_pczt() {
        let mut states = BTreeMap::new();
        states.insert("z_viewtransaction".into(), CapabilityState::Advertised);
        states.insert("z_sendmany".into(), CapabilityState::Advertised);
        let capabilities = Capabilities {
            states,
            methods: vec!["z_viewtransaction".into(), "z_sendmany".into()],
            sendmany_advertised: true,
            pczt_complete: false,
            sendfromaccount_advertised: false,
            receipt_verification: true,
        };
        assert_eq!(
            preferred_path(&capabilities),
            SpendPath::LegacyExplicitReviewRequired
        );
    }
}

#[cfg(test)]
mod security_tests {
    use super::*;
    use crate::payment::{PAYMENT_INTENT_SCHEMA, ZcashNetwork};
    use std::cell::Cell;

    struct StableBackend {
        review: PlanReview,
    }
    impl PcztBackend for StableBackend {
        fn create(&self, _: &PaymentIntent) -> Result<PcztArtifact> {
            PcztArtifact::new(vec![1])
        }
        fn inspect(&self, _: &PcztArtifact) -> Result<PlanReview> {
            Ok(self.review.clone())
        }
        fn prove(&self, _: &PcztArtifact) -> Result<PcztArtifact> {
            Err(Error::Trust)
        }
        fn sign(&self, _: &PcztArtifact, _: PrivacyPolicy) -> Result<PcztArtifact> {
            PcztArtifact::new(vec![2])
        }
        fn combine(&self, _: &[PcztArtifact]) -> Result<PcztArtifact> {
            Err(Error::Trust)
        }
        fn extract(&self, _: &PcztArtifact, _: &PaymentIntent) -> Result<String> {
            Err(Error::Trust)
        }
    }

    struct ChangingBackend {
        inspections: Cell<u8>,
    }
    impl PcztBackend for ChangingBackend {
        fn create(&self, _: &PaymentIntent) -> Result<PcztArtifact> {
            PcztArtifact::new(vec![1])
        }
        fn inspect(&self, _: &PcztArtifact) -> Result<PlanReview> {
            let count = self.inspections.get();
            self.inspections.set(count.saturating_add(1));
            Ok(PlanReview {
                recipient: "synthetic".into(),
                amount_zat: if count == 0 { 10 } else { 11 },
                fee_zat: 1,
                required_privacy: PrivacyPolicy::FullPrivacy,
                transparent_inputs: false,
                transparent_outputs: false,
                reference_commitment: None,
            })
        }
        fn prove(&self, _: &PcztArtifact) -> Result<PcztArtifact> {
            Err(Error::Trust)
        }
        fn sign(&self, _: &PcztArtifact, _: PrivacyPolicy) -> Result<PcztArtifact> {
            PcztArtifact::new(vec![2])
        }
        fn combine(&self, _: &[PcztArtifact]) -> Result<PcztArtifact> {
            Err(Error::Trust)
        }
        fn extract(&self, _: &PcztArtifact, _: &PaymentIntent) -> Result<String> {
            Err(Error::Trust)
        }
    }

    fn security_intent() -> PaymentIntent {
        PaymentIntent {
            schema: PAYMENT_INTENT_SCHEMA.into(),
            intent_id: encode_base64url(&[1; 16]),
            network: ZcashNetwork::Regtest,
            requester_origin: "https://merchant.example".into(),
            recipient: "synthetic".into(),
            amount_zat: 10,
            min_confirmations: 3,
            privacy_policy: PrivacyPolicy::FullPrivacy,
            issued_at: 100,
            expires_at: 200,
            reference_commitment: None,
        }
    }

    fn security_review() -> PlanReview {
        PlanReview {
            recipient: "synthetic".into(),
            amount_zat: 10,
            fee_zat: 1,
            required_privacy: PrivacyPolicy::FullPrivacy,
            transparent_inputs: false,
            transparent_outputs: false,
            reference_commitment: None,
        }
    }

    #[test]
    fn review_changes_invalidate_acknowledgement_and_privacy_downgrades_fail() {
        let intent = security_intent();
        let artifact = PcztArtifact::new(vec![1]).unwrap();
        let backend = StableBackend {
            review: security_review(),
        };
        let (_review, ack) = inspect_and_acknowledge(&backend, &artifact, &intent, 1).unwrap();

        sign_reviewed(
            &backend,
            &artifact,
            &intent,
            &ack,
            100,
            &intent.requester_origin,
            &[],
        )
        .unwrap();

        assert!(
            sign_reviewed(
                &backend,
                &PcztArtifact::new(vec![2]).unwrap(),
                &intent,
                &ack,
                100,
                &intent.requester_origin,
                &[]
            )
            .is_err()
        );
        assert!(
            sign_reviewed(
                &backend,
                &artifact,
                &intent,
                &ack,
                200,
                &intent.requester_origin,
                &[]
            )
            .is_err()
        );

        let mut downgraded = security_review();
        downgraded.required_privacy = PrivacyPolicy::NoPrivacy;
        let downgraded_backend = StableBackend { review: downgraded };
        assert!(inspect_and_acknowledge(&downgraded_backend, &artifact, &intent, 1).is_err());
    }

    #[test]
    fn inspection_drift_between_consent_and_signing_fails_closed() {
        let intent = security_intent();
        let artifact = PcztArtifact::new(vec![1]).unwrap();
        let backend = ChangingBackend {
            inspections: Cell::new(0),
        };
        let (_review, ack) = inspect_and_acknowledge(&backend, &artifact, &intent, 1).unwrap();
        assert!(
            sign_reviewed(
                &backend,
                &artifact,
                &intent,
                &ack,
                100,
                &intent.requester_origin,
                &[],
            )
            .is_err()
        );
    }

    struct Coordinator;
    impl SharedControlCoordinator for Coordinator {
        fn verify_approval(&self, _: &str, _: &str, approval: &[u8]) -> Result<()> {
            if approval == [1] {
                Ok(())
            } else {
                Err(Error::Signature)
            }
        }
    }
    #[test]
    fn shared_control_requires_distinct_authenticated_participants() {
        let artifact = PcztArtifact::new(vec![1]).unwrap();
        let mut state = SharedControl::new(&artifact, vec!["a".into(), "b".into()], 2).unwrap();
        assert!(!state.ready());
        assert!(state.approve(&Coordinator, "a", &[2]).is_err());
        state.approve(&Coordinator, "a", &[1]).unwrap();
        assert!(!state.ready());
        assert!(state.approve(&Coordinator, "a", &[1]).is_err());
        assert!(state.approve(&Coordinator, "unknown", &[1]).is_err());
        state.approve(&Coordinator, "b", &[1]).unwrap();
        assert!(state.ready());
    }
}
