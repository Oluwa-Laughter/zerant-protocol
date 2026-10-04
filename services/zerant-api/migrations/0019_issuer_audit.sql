CREATE TABLE IF NOT EXISTS issuer_events (
    id BIGSERIAL PRIMARY KEY,
    issuer_profile_id UUID NOT NULL REFERENCES issuer_profiles(id) ON DELETE CASCADE,
    actor_account_id UUID REFERENCES accounts(id) ON DELETE SET NULL,
    actor_zerant_id TEXT NOT NULL,
    event_type TEXT NOT NULL CHECK (
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
            'credential_revoked'
        )
    ),
    object_id TEXT NOT NULL,
    label TEXT NOT NULL,
    context TEXT,
    counterparty TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS issuer_events_profile_page_idx
    ON issuer_events(issuer_profile_id, created_at DESC, id DESC);

CREATE OR REPLACE FUNCTION zerant_reject_issuer_event_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'issuer_events is append-only';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS issuer_events_append_only ON issuer_events;
CREATE TRIGGER issuer_events_append_only
BEFORE UPDATE OR DELETE ON issuer_events
FOR EACH ROW EXECUTE FUNCTION zerant_reject_issuer_event_mutation();
