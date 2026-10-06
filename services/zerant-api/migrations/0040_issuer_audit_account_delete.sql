CREATE OR REPLACE FUNCTION zerant_reject_issuer_event_mutation()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'UPDATE'
       AND current_setting('zerant.allow_account_delete', true) = 'on'
       AND OLD.actor_account_id IS NOT NULL
       AND NEW.actor_account_id IS NULL
       AND NEW.id = OLD.id
       AND NEW.issuer_profile_id = OLD.issuer_profile_id
       AND NEW.actor_zerant_id = OLD.actor_zerant_id
       AND NEW.event_type = OLD.event_type
       AND NEW.object_id = OLD.object_id
       AND NEW.label = OLD.label
       AND NEW.context IS NOT DISTINCT FROM OLD.context
       AND NEW.counterparty IS NOT DISTINCT FROM OLD.counterparty
       AND NEW.created_at = OLD.created_at THEN
        RETURN NEW;
    END IF;

    RAISE EXCEPTION 'issuer_events is append-only';
END;
$$ LANGUAGE plpgsql;
