CREATE TABLE IF NOT EXISTS zcash_payments (
    id UUID PRIMARY KEY,
    account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    request_digest BYTEA NOT NULL CHECK (octet_length(request_digest) = 32),
    recipient TEXT NOT NULL CHECK (char_length(recipient) BETWEEN 1 AND 1024),
    amount_zat BIGINT NOT NULL CHECK (amount_zat BETWEEN 1 AND 9007199254740991),
    network TEXT NOT NULL CHECK (network IN ('zcash:testnet', 'zcash:mainnet')),
    min_confirmations INTEGER NOT NULL CHECK (min_confirmations BETWEEN 1 AND 10000),
    txid TEXT UNIQUE CHECK (txid ~ '^[0-9a-f]{64}$'),
    state TEXT NOT NULL CHECK (state IN ('prepared', 'submitted', 'expired')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,
    submitted_at TIMESTAMPTZ,
    CHECK ((state = 'submitted' AND txid IS NOT NULL AND submitted_at IS NOT NULL)
        OR (state IN ('prepared', 'expired') AND txid IS NULL AND submitted_at IS NULL)),
    CHECK (expires_at > created_at)
);

CREATE INDEX IF NOT EXISTS zcash_payments_account_recent_idx
    ON zcash_payments(account_id, created_at DESC);
