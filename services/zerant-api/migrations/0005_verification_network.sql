ALTER TABLE issued_credentials
    ADD COLUMN IF NOT EXISTS vault_record_id UUID UNIQUE
    REFERENCES credential_envelopes(id) ON DELETE SET NULL;

CREATE TABLE IF NOT EXISTS verifier_profiles (
    id UUID PRIMARY KEY,
    account_id UUID NOT NULL UNIQUE REFERENCES accounts(id) ON DELETE CASCADE,
    display_name TEXT NOT NULL CHECK (char_length(display_name) BETWEEN 2 AND 120),
    origin TEXT NOT NULL UNIQUE,
    verifier_id TEXT NOT NULL UNIQUE,
    verifier_key_id TEXT NOT NULL UNIQUE,
    public_jwk JSONB NOT NULL,
    ciphertext BYTEA NOT NULL,
    data_nonce BYTEA NOT NULL CHECK (octet_length(data_nonce) = 12),
    wrapped_dek BYTEA NOT NULL,
    wrap_nonce BYTEA NOT NULL CHECK (octet_length(wrap_nonce) = 12),
    key_version INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS verification_requests (
    id UUID PRIMARY KEY,
    verifier_profile_id UUID NOT NULL REFERENCES verifier_profiles(id) ON DELETE CASCADE,
    subject_account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    request_id TEXT NOT NULL UNIQUE,
    request_jws TEXT NOT NULL,
    purpose TEXT NOT NULL CHECK (char_length(purpose) BETWEEN 2 AND 1024),
    claim_type TEXT NOT NULL,
    context TEXT NOT NULL,
    accepted_issuer_ids JSONB NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'approved', 'denied', 'expired')),
    response_ciphertext BYTEA,
    response_data_nonce BYTEA CHECK (response_data_nonce IS NULL OR octet_length(response_data_nonce) = 12),
    response_wrapped_dek BYTEA,
    response_wrap_nonce BYTEA CHECK (response_wrap_nonce IS NULL OR octet_length(response_wrap_nonce) = 12),
    response_key_version INTEGER,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,
    decided_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS verification_requests_subject_idx
    ON verification_requests(subject_account_id, status, expires_at);

CREATE INDEX IF NOT EXISTS verification_requests_verifier_idx
    ON verification_requests(verifier_profile_id, created_at DESC);
