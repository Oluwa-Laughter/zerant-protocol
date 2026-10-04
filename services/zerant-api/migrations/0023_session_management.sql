ALTER TABLE sessions
    ADD COLUMN IF NOT EXISTS auth_method TEXT NOT NULL DEFAULT 'legacy'
    CHECK (auth_method IN ('legacy', 'passkey', 'zcash'));

ALTER TABLE sessions
    ADD COLUMN IF NOT EXISTS last_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW();

CREATE INDEX IF NOT EXISTS sessions_account_active_idx
    ON sessions(account_id, expires_at DESC, created_at DESC);
