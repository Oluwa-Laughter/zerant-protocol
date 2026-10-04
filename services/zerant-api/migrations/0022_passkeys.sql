CREATE TABLE IF NOT EXISTS passkey_credentials (
    id UUID PRIMARY KEY,
    account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    credential_id TEXT NOT NULL UNIQUE CHECK (char_length(credential_id) BETWEEN 8 AND 2048),
    passkey JSONB NOT NULL,
    last_used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS passkey_credentials_account_idx
    ON passkey_credentials(account_id, created_at DESC);

CREATE TABLE IF NOT EXISTS passkey_challenges (
    id UUID PRIMARY KEY,
    attempt_hash BYTEA NOT NULL UNIQUE CHECK (octet_length(attempt_hash) = 32),
    account_id UUID NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('register', 'authenticate', 'attach')),
    state JSONB NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    consumed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS passkey_challenges_expiry_idx
    ON passkey_challenges(expires_at)
    WHERE consumed_at IS NULL;
