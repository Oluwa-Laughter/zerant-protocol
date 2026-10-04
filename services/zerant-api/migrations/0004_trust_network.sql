ALTER TABLE accounts
    ADD COLUMN IF NOT EXISTS public_handle TEXT UNIQUE;

CREATE TABLE IF NOT EXISTS account_credential_keys (
    account_id UUID PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
    public_jwk JSONB NOT NULL,
    ciphertext BYTEA NOT NULL,
    data_nonce BYTEA NOT NULL CHECK (octet_length(data_nonce) = 12),
    wrapped_dek BYTEA NOT NULL,
    wrap_nonce BYTEA NOT NULL CHECK (octet_length(wrap_nonce) = 12),
    key_version INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS issuer_profiles (
    id UUID PRIMARY KEY,
    account_id UUID NOT NULL UNIQUE REFERENCES accounts(id) ON DELETE CASCADE,
    display_name TEXT NOT NULL CHECK (char_length(display_name) BETWEEN 2 AND 120),
    issuer_id TEXT NOT NULL UNIQUE,
    issuer_key_id TEXT NOT NULL UNIQUE,
    public_jwk JSONB NOT NULL,
    ciphertext BYTEA NOT NULL,
    data_nonce BYTEA NOT NULL CHECK (octet_length(data_nonce) = 12),
    wrapped_dek BYTEA NOT NULL,
    wrap_nonce BYTEA NOT NULL CHECK (octet_length(wrap_nonce) = 12),
    key_version INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS issued_credentials (
    id UUID PRIMARY KEY,
    issuer_profile_id UUID NOT NULL REFERENCES issuer_profiles(id) ON DELETE CASCADE,
    subject_account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    credential_id TEXT NOT NULL UNIQUE,
    claim_type TEXT NOT NULL,
    context TEXT NOT NULL,
    issued_at TIMESTAMPTZ NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS issued_credentials_issuer_idx
    ON issued_credentials(issuer_profile_id, created_at DESC);

CREATE INDEX IF NOT EXISTS issued_credentials_subject_idx
    ON issued_credentials(subject_account_id, created_at DESC);
