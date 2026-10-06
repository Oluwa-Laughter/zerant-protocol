CREATE OR REPLACE FUNCTION zerant_reject_retired_verifier_insert()
RETURNS TRIGGER AS $$
DECLARE
    verifier_retired_at TIMESTAMPTZ;
BEGIN
    SELECT retired_at
      INTO verifier_retired_at
      FROM verifier_profiles
     WHERE id = NEW.verifier_profile_id
     FOR SHARE;

    IF NOT FOUND THEN
        RAISE EXCEPTION 'verifier profile not found';
    END IF;

    IF verifier_retired_at IS NOT NULL THEN
        RAISE EXCEPTION 'verifier is retired';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
