CREATE TABLE IF NOT EXISTS zcash_payout_destinations (
    id UUID PRIMARY KEY,
    issuer_profile_id UUID NOT NULL REFERENCES issuer_profiles(id) ON DELETE CASCADE,
    subject_account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    network TEXT NOT NULL CHECK (network IN ('zcash:testnet', 'zcash:mainnet')),
    transparent_only BOOLEAN NOT NULL DEFAULT FALSE,
    ciphertext BYTEA,
    data_nonce BYTEA CHECK (data_nonce IS NULL OR octet_length(data_nonce) = 12),
    wrapped_dek BYTEA,
    wrap_nonce BYTEA CHECK (wrap_nonce IS NULL OR octet_length(wrap_nonce) = 12),
    key_version INTEGER,
    state TEXT NOT NULL CHECK (state IN ('active', 'withdrawn', 'expired')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,
    withdrawn_at TIMESTAMPTZ,
    CHECK (expires_at > created_at),
    CHECK (
        (state = 'active'
            AND ciphertext IS NOT NULL
            AND data_nonce IS NOT NULL
            AND wrapped_dek IS NOT NULL
            AND wrap_nonce IS NOT NULL
            AND key_version IS NOT NULL
            AND withdrawn_at IS NULL)
        OR
        (state = 'withdrawn'
            AND ciphertext IS NULL
            AND data_nonce IS NULL
            AND wrapped_dek IS NULL
            AND wrap_nonce IS NULL
            AND key_version IS NULL
            AND withdrawn_at IS NOT NULL)
        OR
        (state = 'expired'
            AND ciphertext IS NULL
            AND data_nonce IS NULL
            AND wrapped_dek IS NULL
            AND wrap_nonce IS NULL
            AND key_version IS NULL)
    )
);

CREATE INDEX IF NOT EXISTS zcash_payout_destinations_subject_recent_idx
    ON zcash_payout_destinations(subject_account_id, created_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS zcash_payout_destinations_issuer_active_idx
    ON zcash_payout_destinations(issuer_profile_id, created_at DESC, id DESC)
    WHERE state = 'active';

ALTER TABLE trust_events
    DROP CONSTRAINT IF EXISTS trust_events_event_type_check;

ALTER TABLE trust_events
    ADD CONSTRAINT trust_events_event_type_check CHECK (
        event_type IN (
            'credential_issued',
            'credential_revoked',
            'verification_requested',
            'verification_approved',
            'verification_denied',
            'verification_expired',
            'verifier_api_key_created',
            'verifier_api_key_revoked',
            'verifier_webhook_created',
            'verifier_webhook_disabled',
            'verification_policy_created',
            'verification_policy_versioned',
            'verification_policy_retired',
            'zcash_payment_prepared',
            'zcash_payment_submitted',
            'zcash_invoice_created',
            'zcash_invoice_cancelled',
            'zcash_payout_shared',
            'zcash_payout_withdrawn'
        )
    );

ALTER TABLE issuer_events
    DROP CONSTRAINT IF EXISTS issuer_events_event_type_check;

ALTER TABLE issuer_events
    ADD CONSTRAINT issuer_events_event_type_check CHECK (
        event_type IN (
            'team_invited',
            'team_joined',
            'team_declined',
            'team_member_removed',
            'ownership_transferred',
            'issuer_key_rotated',
            'issuer_key_compromised',
            'credential_schema_created',
            'credential_schema_versioned',
            'credential_schema_retired',
            'credential_issued',
            'credential_revoked',
            'issuer_retired',
            'payout_destination_received',
            'payout_destination_withdrawn'
        )
    );
