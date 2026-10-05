ALTER TABLE verification_requests
    ADD COLUMN IF NOT EXISTS proof_issuer_id TEXT,
    ADD COLUMN IF NOT EXISTS proof_issuer_key_id TEXT,
    ADD COLUMN IF NOT EXISTS proof_revocation_jws TEXT;

CREATE INDEX IF NOT EXISTS verification_requests_proof_ready_idx
    ON verification_requests(verifier_profile_id, id)
    WHERE status = 'approved'
      AND proof_issuer_id IS NOT NULL
      AND proof_issuer_key_id IS NOT NULL
      AND proof_revocation_jws IS NOT NULL;
