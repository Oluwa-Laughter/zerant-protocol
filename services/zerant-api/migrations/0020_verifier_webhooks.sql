CREATE TABLE IF NOT EXISTS verifier_webhooks (
    id UUID PRIMARY KEY,
    verifier_profile_id UUID NOT NULL REFERENCES verifier_profiles(id) ON DELETE CASCADE,
    name TEXT NOT NULL CHECK (char_length(name) BETWEEN 2 AND 80),
    url TEXT NOT NULL CHECK (char_length(url) BETWEEN 12 AND 2048),
    ciphertext BYTEA NOT NULL,
    data_nonce BYTEA NOT NULL CHECK (octet_length(data_nonce) = 12),
    wrapped_dek BYTEA NOT NULL,
    wrap_nonce BYTEA NOT NULL CHECK (octet_length(wrap_nonce) = 12),
    key_version INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    disabled_at TIMESTAMPTZ,
    last_delivery_at TIMESTAMPTZ
);

CREATE UNIQUE INDEX IF NOT EXISTS verifier_webhooks_active_url_idx
    ON verifier_webhooks(verifier_profile_id, url)
    WHERE disabled_at IS NULL;

CREATE INDEX IF NOT EXISTS verifier_webhooks_profile_idx
    ON verifier_webhooks(verifier_profile_id, created_at DESC);

CREATE TABLE IF NOT EXISTS webhook_deliveries (
    id UUID PRIMARY KEY,
    webhook_id UUID NOT NULL REFERENCES verifier_webhooks(id) ON DELETE CASCADE,
    verification_request_id UUID NOT NULL REFERENCES verification_requests(id) ON DELETE CASCADE,
    event_type TEXT NOT NULL CHECK (
        event_type IN ('verification.approved', 'verification.denied')
    ),
    payload JSONB NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (
        status IN ('pending', 'delivering', 'delivered', 'dead')
    ),
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_attempt_at TIMESTAMPTZ,
    delivered_at TIMESTAMPTZ,
    last_status_code INTEGER,
    last_error TEXT CHECK (last_error IS NULL OR char_length(last_error) <= 160),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(webhook_id, verification_request_id, event_type)
);

CREATE INDEX IF NOT EXISTS webhook_deliveries_due_idx
    ON webhook_deliveries(next_attempt_at, created_at)
    WHERE status = 'pending';

CREATE INDEX IF NOT EXISTS webhook_deliveries_webhook_idx
    ON webhook_deliveries(webhook_id, created_at DESC);

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
            'verifier_webhook_disabled'
        )
    );

CREATE OR REPLACE FUNCTION zerant_audit_verifier_webhook()
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
            'verifier_webhook_created',
            NEW.id::text,
            NEW.name
        );
    ELSIF OLD.disabled_at IS NULL AND NEW.disabled_at IS NOT NULL THEN
        INSERT INTO trust_events(account_id, event_type, object_id, label)
        VALUES (
            verifier_account,
            'verifier_webhook_disabled',
            NEW.id::text,
            NEW.name
        );
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS verifier_webhooks_audit_insert ON verifier_webhooks;
CREATE TRIGGER verifier_webhooks_audit_insert
AFTER INSERT ON verifier_webhooks
FOR EACH ROW EXECUTE FUNCTION zerant_audit_verifier_webhook();

DROP TRIGGER IF EXISTS verifier_webhooks_audit_disable ON verifier_webhooks;
CREATE TRIGGER verifier_webhooks_audit_disable
AFTER UPDATE OF disabled_at ON verifier_webhooks
FOR EACH ROW EXECUTE FUNCTION zerant_audit_verifier_webhook();
