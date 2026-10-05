ALTER TABLE zcash_network_readiness
    ADD COLUMN IF NOT EXISTS high_water_block_height BIGINT
        CHECK (high_water_block_height IS NULL OR high_water_block_height >= 0),
    ADD COLUMN IF NOT EXISTS last_success_at TIMESTAMPTZ;

UPDATE zcash_network_readiness
SET high_water_block_height = block_height,
    last_success_at = checked_at
WHERE available
  AND block_height IS NOT NULL
  AND high_water_block_height IS NULL;
