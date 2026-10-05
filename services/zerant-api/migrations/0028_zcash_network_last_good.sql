ALTER TABLE zcash_network_readiness
    ADD COLUMN IF NOT EXISTS last_success_block_height BIGINT
        CHECK (last_success_block_height IS NULL OR last_success_block_height >= 0),
    ADD COLUMN IF NOT EXISTS last_success_estimated_height BIGINT
        CHECK (last_success_estimated_height IS NULL OR last_success_estimated_height >= 0),
    ADD COLUMN IF NOT EXISTS last_success_lag BIGINT
        CHECK (last_success_lag IS NULL OR last_success_lag >= 0),
    ADD COLUMN IF NOT EXISTS last_success_synced BOOLEAN;

UPDATE zcash_network_readiness
SET last_success_block_height = block_height,
    last_success_estimated_height = estimated_height,
    last_success_lag = lag,
    last_success_synced = synced
WHERE available
  AND last_success_at IS NOT NULL
  AND last_success_block_height IS NULL;
