CREATE TABLE IF NOT EXISTS holder_pairwise_keys (
    id UUID PRIMARY KEY,
    holder_account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    verifier_profile_id UUID NOT NULL REFERENCES verifier_profiles(id) ON DELETE CASCADE,
    public_jwk JSONB NOT NULL,
    ciphertext BYTEA NOT NULL,
    data_nonce BYTEA NOT NULL CHECK (octet_length(data_nonce) = 12),
    wrapped_dek BYTEA NOT NULL,
    wrap_nonce BYTEA NOT NULL CHECK (octet_length(wrap_nonce) = 12),
    key_version INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (holder_account_id, verifier_profile_id)
);

CREATE INDEX IF NOT EXISTS holder_pairwise_keys_holder_idx
    ON holder_pairwise_keys(holder_account_id, created_at DESC);
