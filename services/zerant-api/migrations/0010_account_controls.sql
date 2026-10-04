CREATE OR REPLACE FUNCTION zerant_reject_trust_event_mutation()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE'
       AND current_setting('zerant.allow_account_delete', true) = 'on' THEN
        RETURN OLD;
    END IF;
    RAISE EXCEPTION 'trust_events is append-only';
END;
$$ LANGUAGE plpgsql;
