ALTER TABLE zcash_payments
    ADD COLUMN IF NOT EXISTS network_state TEXT,
    ADD COLUMN IF NOT EXISTS observed_height BIGINT,
    ADD COLUMN IF NOT EXISTS confirmations BIGINT,
    ADD COLUMN IF NOT EXISTS observed_at TIMESTAMPTZ;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'zcash_payments_network_observation_check'
    ) THEN
        ALTER TABLE zcash_payments
            ADD CONSTRAINT zcash_payments_network_observation_check CHECK (
                (network_state IS NULL
                    AND observed_height IS NULL
                    AND confirmations IS NULL
                    AND observed_at IS NULL)
                OR
                (state = 'submitted'
                    AND network_state IN ('mempool', 'forked')
                    AND observed_height IS NULL
                    AND confirmations IS NOT NULL
                    AND confirmations = 0
                    AND observed_at IS NOT NULL)
                OR
                (network_state = 'mined'
                    AND observed_height IS NOT NULL
                    AND observed_height BETWEEN 1 AND 9007199254740991
                    AND confirmations IS NOT NULL
                    AND confirmations BETWEEN 1 AND 9007199254740991
                    AND observed_at IS NOT NULL)
            );
    END IF;
END $$;
