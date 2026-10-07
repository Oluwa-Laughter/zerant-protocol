-- Keep organization payout routing unambiguous: one live destination per
-- holder/issuer trust relationship. If pre-existing duplicates exist, retain the
-- newest active record and cryptographically discard the older active payloads.
WITH ranked_active AS (
    SELECT
        id,
        ROW_NUMBER() OVER (
            PARTITION BY issuer_profile_id, subject_account_id
            ORDER BY created_at DESC, id DESC
        ) AS position
    FROM zcash_payout_destinations
    WHERE state = 'active'
)
UPDATE zcash_payout_destinations AS payout
SET
    state = 'expired',
    ciphertext = NULL,
    data_nonce = NULL,
    wrapped_dek = NULL,
    wrap_nonce = NULL,
    key_version = NULL
FROM ranked_active
WHERE payout.id = ranked_active.id
  AND ranked_active.position > 1;

CREATE UNIQUE INDEX IF NOT EXISTS zcash_payout_destinations_one_active_relationship_idx
    ON zcash_payout_destinations(issuer_profile_id, subject_account_id)
    WHERE state = 'active';
