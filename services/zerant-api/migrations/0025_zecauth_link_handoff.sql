ALTER TABLE zecauth_challenges
    ADD COLUMN IF NOT EXISTS link_attempt_hash BYTEA,
    ADD COLUMN IF NOT EXISTS pending_link_key BYTEA,
    ADD COLUMN IF NOT EXISTS pending_link_at TIMESTAMPTZ;

ALTER TABLE zecauth_challenges
    DROP CONSTRAINT IF EXISTS zecauth_link_pending_pair;
ALTER TABLE zecauth_challenges
    ADD CONSTRAINT zecauth_link_pending_pair
    CHECK ((pending_link_key IS NULL) = (pending_link_at IS NULL)
       AND (pending_link_key IS NULL OR octet_length(pending_link_key) = 32)
       AND (link_attempt_hash IS NULL OR octet_length(link_attempt_hash) = 32)
       AND (pending_link_key IS NULL OR link_attempt_hash IS NOT NULL)
       AND (link_account_id IS NOT NULL OR (link_attempt_hash IS NULL AND pending_link_key IS NULL)));

CREATE UNIQUE INDEX IF NOT EXISTS zecauth_link_attempt_idx
    ON zecauth_challenges(link_attempt_hash)
    WHERE link_attempt_hash IS NOT NULL;
