CREATE TABLE IF NOT EXISTS trust_events (
    id BIGSERIAL PRIMARY KEY,
    account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    event_type TEXT NOT NULL CHECK (
        event_type IN (
            'credential_issued',
            'credential_revoked',
            'verification_requested',
            'verification_approved',
            'verification_denied',
            'verification_expired'
        )
    ),
    object_id TEXT NOT NULL,
    label TEXT NOT NULL,
    context TEXT,
    counterparty TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS trust_events_account_page_idx
    ON trust_events(account_id, created_at DESC, id DESC);

CREATE OR REPLACE FUNCTION zerant_reject_trust_event_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'trust_events is append-only';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trust_events_append_only ON trust_events;
CREATE TRIGGER trust_events_append_only
BEFORE UPDATE OR DELETE ON trust_events
FOR EACH ROW EXECUTE FUNCTION zerant_reject_trust_event_mutation();

CREATE OR REPLACE FUNCTION zerant_audit_issued_credential()
RETURNS TRIGGER AS $$
DECLARE
    issuer_account UUID;
    issuer_name TEXT;
    holder_handle TEXT;
BEGIN
    SELECT p.account_id, p.display_name
      INTO issuer_account, issuer_name
      FROM issuer_profiles p
     WHERE p.id = NEW.issuer_profile_id;

    SELECT a.public_handle
      INTO holder_handle
      FROM accounts a
     WHERE a.id = NEW.subject_account_id;

    INSERT INTO trust_events(account_id, event_type, object_id, label, context, counterparty)
    VALUES
        (issuer_account, 'credential_issued', NEW.credential_id, NEW.claim_type, NEW.context, holder_handle),
        (NEW.subject_account_id, 'credential_issued', NEW.credential_id, NEW.claim_type, NEW.context, issuer_name);

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS issued_credentials_audit_insert ON issued_credentials;
CREATE TRIGGER issued_credentials_audit_insert
AFTER INSERT ON issued_credentials
FOR EACH ROW EXECUTE FUNCTION zerant_audit_issued_credential();

CREATE OR REPLACE FUNCTION zerant_audit_revoked_credential()
RETURNS TRIGGER AS $$
DECLARE
    issuer_account UUID;
    issuer_name TEXT;
    holder_handle TEXT;
BEGIN
    IF OLD.revoked_at IS NULL AND NEW.revoked_at IS NOT NULL THEN
        SELECT p.account_id, p.display_name
          INTO issuer_account, issuer_name
          FROM issuer_profiles p
         WHERE p.id = NEW.issuer_profile_id;

        SELECT a.public_handle
          INTO holder_handle
          FROM accounts a
         WHERE a.id = NEW.subject_account_id;

        INSERT INTO trust_events(account_id, event_type, object_id, label, context, counterparty)
        VALUES
            (issuer_account, 'credential_revoked', NEW.credential_id, NEW.claim_type, NEW.context, holder_handle),
            (NEW.subject_account_id, 'credential_revoked', NEW.credential_id, NEW.claim_type, NEW.context, issuer_name);
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS issued_credentials_audit_revoke ON issued_credentials;
CREATE TRIGGER issued_credentials_audit_revoke
AFTER UPDATE OF revoked_at ON issued_credentials
FOR EACH ROW EXECUTE FUNCTION zerant_audit_revoked_credential();

CREATE OR REPLACE FUNCTION zerant_audit_verification_request()
RETURNS TRIGGER AS $$
DECLARE
    verifier_account UUID;
    verifier_name TEXT;
    holder_handle TEXT;
BEGIN
    SELECT v.account_id, v.display_name
      INTO verifier_account, verifier_name
      FROM verifier_profiles v
     WHERE v.id = NEW.verifier_profile_id;

    SELECT a.public_handle
      INTO holder_handle
      FROM accounts a
     WHERE a.id = NEW.subject_account_id;

    INSERT INTO trust_events(account_id, event_type, object_id, label, context, counterparty)
    VALUES
        (verifier_account, 'verification_requested', NEW.request_id, NEW.claim_type, NEW.context, holder_handle),
        (NEW.subject_account_id, 'verification_requested', NEW.request_id, NEW.claim_type, NEW.context, verifier_name);

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS verification_requests_audit_insert ON verification_requests;
CREATE TRIGGER verification_requests_audit_insert
AFTER INSERT ON verification_requests
FOR EACH ROW EXECUTE FUNCTION zerant_audit_verification_request();

CREATE OR REPLACE FUNCTION zerant_audit_verification_decision()
RETURNS TRIGGER AS $$
DECLARE
    verifier_account UUID;
    verifier_name TEXT;
    holder_handle TEXT;
    audit_type TEXT;
BEGIN
    IF OLD.status = NEW.status THEN
        RETURN NEW;
    END IF;

    audit_type := CASE NEW.status
        WHEN 'approved' THEN 'verification_approved'
        WHEN 'denied' THEN 'verification_denied'
        WHEN 'expired' THEN 'verification_expired'
        ELSE NULL
    END;

    IF audit_type IS NULL THEN
        RETURN NEW;
    END IF;

    SELECT v.account_id, v.display_name
      INTO verifier_account, verifier_name
      FROM verifier_profiles v
     WHERE v.id = NEW.verifier_profile_id;

    SELECT a.public_handle
      INTO holder_handle
      FROM accounts a
     WHERE a.id = NEW.subject_account_id;

    INSERT INTO trust_events(account_id, event_type, object_id, label, context, counterparty)
    VALUES
        (verifier_account, audit_type, NEW.request_id, NEW.claim_type, NEW.context, holder_handle),
        (NEW.subject_account_id, audit_type, NEW.request_id, NEW.claim_type, NEW.context, verifier_name);

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS verification_requests_audit_status ON verification_requests;
CREATE TRIGGER verification_requests_audit_status
AFTER UPDATE OF status ON verification_requests
FOR EACH ROW EXECUTE FUNCTION zerant_audit_verification_decision();
