CREATE OR REPLACE FUNCTION zerant_guard_retired_issuer_invitation()
RETURNS TRIGGER AS $$
DECLARE
    issuer_retired_at TIMESTAMPTZ;
    issuer_owner UUID;
BEGIN
    SELECT retired_at, account_id
      INTO issuer_retired_at, issuer_owner
      FROM issuer_profiles
     WHERE id = NEW.issuer_profile_id
     FOR SHARE;

    IF NOT FOUND THEN
        RAISE EXCEPTION 'issuer profile not found';
    END IF;

    IF issuer_retired_at IS NOT NULL
       AND (NEW.role <> 'admin' OR NEW.invited_by_account_id <> issuer_owner) THEN
        RAISE EXCEPTION 'retired issuer accepts owner-invited admin successors only';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS issuer_invitations_retired_issuer_guard ON issuer_invitations;
CREATE TRIGGER issuer_invitations_retired_issuer_guard
BEFORE INSERT ON issuer_invitations
FOR EACH ROW EXECUTE FUNCTION zerant_guard_retired_issuer_invitation();
