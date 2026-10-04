CREATE TABLE IF NOT EXISTS verification_policies (
    id UUID PRIMARY KEY,
    verifier_profile_id UUID NOT NULL REFERENCES verifier_profiles(id) ON DELETE CASCADE,
    slug TEXT NOT NULL CHECK (char_length(slug) BETWEEN 2 AND 80),
    display_name TEXT NOT NULL CHECK (char_length(display_name) BETWEEN 2 AND 120),
    description TEXT NOT NULL CHECK (char_length(description) BETWEEN 2 AND 1024),
    purpose TEXT NOT NULL CHECK (char_length(purpose) BETWEEN 2 AND 1024),
    credential_schema_id UUID NOT NULL REFERENCES credential_schemas(id) ON DELETE RESTRICT,
    request_ttl_seconds INTEGER NOT NULL DEFAULT 300
        CHECK (request_ttl_seconds BETWEEN 60 AND 900),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    active BOOLEAN NOT NULL DEFAULT TRUE,
    supersedes_policy_id UUID REFERENCES verification_policies(id) ON DELETE RESTRICT,
    retired_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK ((active = TRUE AND retired_at IS NULL) OR (active = FALSE AND retired_at IS NOT NULL))
);

CREATE UNIQUE INDEX IF NOT EXISTS verification_policies_version_idx
    ON verification_policies(verifier_profile_id, slug, version);

CREATE UNIQUE INDEX IF NOT EXISTS verification_policies_active_slug_idx
    ON verification_policies(verifier_profile_id, slug)
    WHERE active = TRUE;

CREATE INDEX IF NOT EXISTS verification_policies_schema_idx
    ON verification_policies(credential_schema_id, active);

CREATE INDEX IF NOT EXISTS verification_policies_supersedes_idx
    ON verification_policies(supersedes_policy_id);

ALTER TABLE verification_requests
    ADD COLUMN IF NOT EXISTS verification_policy_id UUID
    REFERENCES verification_policies(id) ON DELETE RESTRICT;

CREATE INDEX IF NOT EXISTS verification_requests_policy_idx
    ON verification_requests(verification_policy_id, created_at DESC);

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
            'verification_policy_retired'
        )
    );

CREATE OR REPLACE FUNCTION zerant_audit_verification_policy()
RETURNS TRIGGER AS $$
DECLARE
    verifier_account UUID;
    event_name TEXT;
BEGIN
    SELECT v.account_id
      INTO verifier_account
      FROM verifier_profiles v
     WHERE v.id = NEW.verifier_profile_id;

    IF TG_OP = 'INSERT' THEN
        IF NEW.supersedes_policy_id IS NULL THEN
            event_name := 'verification_policy_created';
        ELSE
            event_name := 'verification_policy_versioned';
        END IF;

        INSERT INTO trust_events(account_id, event_type, object_id, label)
        VALUES (
            verifier_account,
            event_name,
            NEW.id::text,
            NEW.display_name || ' v' || NEW.version::text
        );
    ELSIF OLD.active = TRUE AND NEW.active = FALSE THEN
        INSERT INTO trust_events(account_id, event_type, object_id, label)
        VALUES (
            verifier_account,
            'verification_policy_retired',
            NEW.id::text,
            NEW.display_name || ' v' || NEW.version::text
        );
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS verification_policies_audit_insert ON verification_policies;
CREATE TRIGGER verification_policies_audit_insert
AFTER INSERT ON verification_policies
FOR EACH ROW EXECUTE FUNCTION zerant_audit_verification_policy();

DROP TRIGGER IF EXISTS verification_policies_audit_retire ON verification_policies;
CREATE TRIGGER verification_policies_audit_retire
AFTER UPDATE OF active ON verification_policies
FOR EACH ROW EXECUTE FUNCTION zerant_audit_verification_policy();
