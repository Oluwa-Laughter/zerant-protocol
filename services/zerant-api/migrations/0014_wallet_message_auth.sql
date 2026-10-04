CREATE TABLE IF NOT EXISTS wallet_message_identities (
    account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    chain TEXT NOT NULL CHECK (chain IN ('zcash:mainnet', 'zcash:testnet')),
    public_key BYTEA NOT NULL CHECK (octet_length(public_key) = 33),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (account_id, chain),
    UNIQUE (chain, public_key)
);

CREATE INDEX IF NOT EXISTS wallet_message_identity_key_idx
    ON wallet_message_identities(chain, public_key);
