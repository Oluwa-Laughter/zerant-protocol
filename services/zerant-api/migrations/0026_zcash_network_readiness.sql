CREATE TABLE IF NOT EXISTS zcash_network_readiness (
    network TEXT PRIMARY KEY CHECK (network IN ('mainnet', 'testnet')),
    endpoint_fingerprint TEXT NOT NULL CHECK (char_length(endpoint_fingerprint) = 64),
    available BOOLEAN NOT NULL,
    synced BOOLEAN,
    block_height BIGINT CHECK (block_height IS NULL OR block_height >= 0),
    estimated_height BIGINT CHECK (estimated_height IS NULL OR estimated_height >= 0),
    lag BIGINT CHECK (lag IS NULL OR lag >= 0),
    checked_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    failure_count INTEGER NOT NULL DEFAULT 0 CHECK (failure_count >= 0),
    last_error_at TIMESTAMPTZ,
    CHECK (
        (available AND synced IS NOT NULL AND block_height IS NOT NULL
            AND estimated_height IS NOT NULL AND lag IS NOT NULL)
        OR
        (NOT available AND synced IS NULL AND block_height IS NULL
            AND estimated_height IS NULL AND lag IS NULL)
    )
);
