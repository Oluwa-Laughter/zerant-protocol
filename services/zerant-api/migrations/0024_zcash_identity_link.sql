ALTER TABLE zecauth_challenges
    ADD COLUMN IF NOT EXISTS link_account_id UUID REFERENCES accounts(id) ON DELETE CASCADE,
    ADD COLUMN IF NOT EXISTS link_session_id UUID;

ALTER TABLE zecauth_challenges
    DROP CONSTRAINT IF EXISTS zecauth_link_target_session_pair;
ALTER TABLE zecauth_challenges
    ADD CONSTRAINT zecauth_link_target_session_pair
    CHECK ((link_account_id IS NULL) = (link_session_id IS NULL));
