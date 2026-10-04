CREATE TABLE IF NOT EXISTS account_rate_limits (
    account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    action TEXT NOT NULL CHECK (char_length(action) BETWEEN 1 AND 64),
    window_started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    request_count INTEGER NOT NULL DEFAULT 1 CHECK (request_count > 0),
    PRIMARY KEY (account_id, action)
);

CREATE INDEX IF NOT EXISTS account_rate_limits_window_idx
    ON account_rate_limits(window_started_at);
