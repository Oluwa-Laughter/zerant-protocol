ALTER TABLE zecauth_challenges
    ADD COLUMN IF NOT EXISTS attempt_hash BYTEA UNIQUE,
    ADD COLUMN IF NOT EXISTS authenticated_account_id UUID REFERENCES accounts(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS authenticated_scopes JSONB;

CREATE INDEX IF NOT EXISTS zecauth_attempt_idx
    ON zecauth_challenges(attempt_hash)
    WHERE attempt_hash IS NOT NULL;
