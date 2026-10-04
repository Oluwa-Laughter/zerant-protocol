ALTER TABLE sessions
    ADD COLUMN IF NOT EXISTS scopes JSONB NOT NULL DEFAULT '["auth"]'::jsonb;

CREATE TABLE IF NOT EXISTS zecauth_identities (
    account_id UUID PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
    verification_key BYTEA NOT NULL UNIQUE CHECK (octet_length(verification_key) = 32),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS zecauth_challenges (
    id UUID PRIMARY KEY,
    nonce_hash BYTEA NOT NULL UNIQUE,
    message TEXT NOT NULL UNIQUE,
    chain TEXT NOT NULL CHECK (chain IN ('zcash:mainnet', 'zcash:testnet')),
    requested_scopes JSONB NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    consumed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS zecauth_challenge_expiry_idx
    ON zecauth_challenges(expires_at)
    WHERE consumed_at IS NULL;
