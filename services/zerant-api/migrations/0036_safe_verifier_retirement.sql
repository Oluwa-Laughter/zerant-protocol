ALTER TABLE verifier_profiles
    ADD COLUMN IF NOT EXISTS retired_at TIMESTAMPTZ;

ALTER TABLE verifier_profiles
    DROP CONSTRAINT IF EXISTS verifier_profiles_account_id_fkey;

ALTER TABLE verifier_profiles
    ALTER COLUMN account_id DROP NOT NULL;

ALTER TABLE verifier_profiles
    ADD CONSTRAINT verifier_profiles_account_id_fkey
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS verifier_profiles_active_account_idx
    ON verifier_profiles(account_id)
    WHERE retired_at IS NULL AND account_id IS NOT NULL;

CREATE OR REPLACE FUNCTION zerant_reject_retired_verifier_insert()
RETURNS TRIGGER AS $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM verifier_profiles
        WHERE id = NEW.verifier_profile_id
          AND retired_at IS NOT NULL
    ) THEN
        RAISE EXCEPTION 'verifier is retired';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS verification_requests_retired_verifier_guard ON verification_requests;
CREATE TRIGGER verification_requests_retired_verifier_guard
BEFORE INSERT ON verification_requests
FOR EACH ROW EXECUTE FUNCTION zerant_reject_retired_verifier_insert();

DROP TRIGGER IF EXISTS verifier_api_keys_retired_verifier_guard ON verifier_api_keys;
CREATE TRIGGER verifier_api_keys_retired_verifier_guard
BEFORE INSERT ON verifier_api_keys
FOR EACH ROW EXECUTE FUNCTION zerant_reject_retired_verifier_insert();

DROP TRIGGER IF EXISTS verifier_webhooks_retired_verifier_guard ON verifier_webhooks;
CREATE TRIGGER verifier_webhooks_retired_verifier_guard
BEFORE INSERT ON verifier_webhooks
FOR EACH ROW EXECUTE FUNCTION zerant_reject_retired_verifier_insert();

DROP TRIGGER IF EXISTS verification_policies_retired_verifier_guard ON verification_policies;
CREATE TRIGGER verification_policies_retired_verifier_guard
BEFORE INSERT ON verification_policies
FOR EACH ROW EXECUTE FUNCTION zerant_reject_retired_verifier_insert();
