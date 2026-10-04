CREATE TABLE IF NOT EXISTS verifier_api_keys (
    id UUID PRIMARY KEY,
    verifier_profile_id UUID NOT NULL REFERENCES verifier_profiles(id) ON DELETE CASCADE,
    name TEXT NOT NULL CHECK (char_length(name) BETWEEN 2 AND 80),
    key_prefix TEXT NOT NULL UNIQUE CHECK (char_length(key_prefix) BETWEEN 8 AND 32),
    token_hash BYTEA NOT NULL UNIQUE CHECK (octet_length(token_hash) = 32),
    scopes JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    CHECK (expires_at IS NULL OR expires_at > created_at)
);

CREATE INDEX IF NOT EXISTS verifier_api_keys_active_idx
    ON verifier_api_keys(verifier_profile_id, created_at DESC)
    WHERE revoked_at IS NULL;

CREATE INDEX IF NOT EXISTS verifier_api_keys_expiry_idx
    ON verifier_api_keys(expires_at)
    WHERE revoked_at IS NULL AND expires_at IS NOT NULL;

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
            'verifier_api_key_revoked'
        )
    );

CREATE OR REPLACE FUNCTION zerant_audit_verifier_api_key()
RETURNS TRIGGER AS $$
DECLARE
    verifier_account UUID;
BEGIN
    SELECT v.account_id
      INTO verifier_account
      FROM verifier_profiles v
     WHERE v.id = NEW.verifier_profile_id;

    IF TG_OP = 'INSERT' THEN
        INSERT INTO trust_events(account_id, event_type, object_id, label)
        VALUES (
            verifier_account,
            'verifier_api_key_created',
            NEW.id::text,
            NEW.name
        );
    ELSIF OLD.revoked_at IS NULL AND NEW.revoked_at IS NOT NULL THEN
        INSERT INTO trust_events(account_id, event_type, object_id, label)
        VALUES (
            verifier_account,
            'verifier_api_key_revoked',
            NEW.id::text,
            NEW.name
        );
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS verifier_api_keys_audit_insert ON verifier_api_keys;
CREATE TRIGGER verifier_api_keys_audit_insert
AFTER INSERT ON verifier_api_keys
FOR EACH ROW EXECUTE FUNCTION zerant_audit_verifier_api_key();

DROP TRIGGER IF EXISTS verifier_api_keys_audit_revoke ON verifier_api_keys;
CREATE TRIGGER verifier_api_keys_audit_revoke
AFTER UPDATE OF revoked_at ON verifier_api_keys
FOR EACH ROW EXECUTE FUNCTION zerant_audit_verifier_api_key();
