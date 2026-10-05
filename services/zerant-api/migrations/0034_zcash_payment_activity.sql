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
            'zcash_payment_submitted'
        )
    );

CREATE OR REPLACE FUNCTION zerant_audit_zcash_payment_insert()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO trust_events(account_id, event_type, object_id, label, context)
    VALUES (
        NEW.account_id,
        'zcash_payment_prepared',
        NEW.id::text,
        'Zcash payment',
        NEW.network
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS zcash_payments_audit_insert ON zcash_payments;
CREATE TRIGGER zcash_payments_audit_insert
AFTER INSERT ON zcash_payments
FOR EACH ROW EXECUTE FUNCTION zerant_audit_zcash_payment_insert();

CREATE OR REPLACE FUNCTION zerant_audit_zcash_payment_submit()
RETURNS TRIGGER AS $$
BEGIN
    IF OLD.state = 'prepared' AND NEW.state = 'submitted' THEN
        INSERT INTO trust_events(account_id, event_type, object_id, label, context)
        VALUES (
            NEW.account_id,
            'zcash_payment_submitted',
            NEW.id::text,
            'Zcash payment',
            NEW.network
        );
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS zcash_payments_audit_submit ON zcash_payments;
CREATE TRIGGER zcash_payments_audit_submit
AFTER UPDATE OF state ON zcash_payments
FOR EACH ROW EXECUTE FUNCTION zerant_audit_zcash_payment_submit();
