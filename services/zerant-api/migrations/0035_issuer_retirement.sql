ALTER TABLE issuer_profiles
    ADD COLUMN IF NOT EXISTS retired_at TIMESTAMPTZ;

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
            'issuer_retired'
        )
    );

CREATE INDEX IF NOT EXISTS issuer_profiles_active_directory_idx
    ON issuer_profiles(created_at DESC, id DESC)
    WHERE retired_at IS NULL;

CREATE OR REPLACE FUNCTION zerant_reject_retired_issuer_insert()
RETURNS TRIGGER AS $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM issuer_profiles
        WHERE id = NEW.issuer_profile_id
          AND retired_at IS NOT NULL
    ) THEN
        RAISE EXCEPTION 'issuer is retired';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS credential_schemas_retired_issuer_guard ON credential_schemas;
CREATE TRIGGER credential_schemas_retired_issuer_guard
BEFORE INSERT ON credential_schemas
FOR EACH ROW EXECUTE FUNCTION zerant_reject_retired_issuer_insert();

DROP TRIGGER IF EXISTS issuer_invitations_retired_issuer_guard ON issuer_invitations;
CREATE TRIGGER issuer_invitations_retired_issuer_guard
BEFORE INSERT ON issuer_invitations
FOR EACH ROW EXECUTE FUNCTION zerant_reject_retired_issuer_insert();

DROP TRIGGER IF EXISTS issued_credentials_retired_issuer_guard ON issued_credentials;
CREATE TRIGGER issued_credentials_retired_issuer_guard
BEFORE INSERT ON issued_credentials
FOR EACH ROW EXECUTE FUNCTION zerant_reject_retired_issuer_insert();
