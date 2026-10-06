CREATE OR REPLACE FUNCTION zerant_reject_retired_issuer_insert()
RETURNS TRIGGER AS $$
DECLARE
    issuer_retired_at TIMESTAMPTZ;
BEGIN
    SELECT retired_at
      INTO issuer_retired_at
      FROM issuer_profiles
     WHERE id = NEW.issuer_profile_id
     FOR SHARE;

    IF NOT FOUND THEN
        RAISE EXCEPTION 'issuer profile not found';
    END IF;

    IF issuer_retired_at IS NOT NULL THEN
        RAISE EXCEPTION 'issuer is retired';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS issuer_signing_keys_retired_issuer_guard ON issuer_signing_keys;
CREATE TRIGGER issuer_signing_keys_retired_issuer_guard
BEFORE INSERT ON issuer_signing_keys
FOR EACH ROW EXECUTE FUNCTION zerant_reject_retired_issuer_insert();
