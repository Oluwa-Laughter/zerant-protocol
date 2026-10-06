CREATE TABLE IF NOT EXISTS zcash_invoices (
    id UUID PRIMARY KEY,
    account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    request_digest BYTEA NOT NULL CHECK (octet_length(request_digest) = 32),
    recipient TEXT NOT NULL CHECK (char_length(recipient) BETWEEN 1 AND 1024),
    amount_zat BIGINT NOT NULL CHECK (amount_zat BETWEEN 1 AND 9007199254740991),
    network TEXT NOT NULL CHECK (network IN ('zcash:testnet', 'zcash:mainnet')),
    state TEXT NOT NULL CHECK (state IN ('open', 'cancelled', 'expired')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,
    cancelled_at TIMESTAMPTZ,
    CHECK (expires_at > created_at),
    CHECK ((state = 'cancelled' AND cancelled_at IS NOT NULL)
        OR (state IN ('open', 'expired') AND cancelled_at IS NULL))
);

CREATE INDEX IF NOT EXISTS zcash_invoices_account_recent_idx
    ON zcash_invoices(account_id, created_at DESC, id DESC);

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
            'zcash_invoice_cancelled'
        )
    );

CREATE OR REPLACE FUNCTION zerant_audit_zcash_invoice_insert()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO trust_events(account_id, event_type, object_id, label, context)
    VALUES (NEW.account_id, 'zcash_invoice_created', NEW.id::text, 'Zcash invoice', NEW.network);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS zcash_invoices_audit_insert ON zcash_invoices;
CREATE TRIGGER zcash_invoices_audit_insert
AFTER INSERT ON zcash_invoices
FOR EACH ROW EXECUTE FUNCTION zerant_audit_zcash_invoice_insert();

CREATE OR REPLACE FUNCTION zerant_audit_zcash_invoice_cancel()
RETURNS TRIGGER AS $$
BEGIN
    IF OLD.state = 'open' AND NEW.state = 'cancelled' THEN
        INSERT INTO trust_events(account_id, event_type, object_id, label, context)
        VALUES (NEW.account_id, 'zcash_invoice_cancelled', NEW.id::text, 'Zcash invoice', NEW.network);
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS zcash_invoices_audit_cancel ON zcash_invoices;
CREATE TRIGGER zcash_invoices_audit_cancel
AFTER UPDATE OF state ON zcash_invoices
FOR EACH ROW EXECUTE FUNCTION zerant_audit_zcash_invoice_cancel();
