DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'passkey_challenges'::regclass
          AND conname = 'passkey_challenges_kind_check'
          AND pg_get_constraintdef(oid) LIKE '%discoverable%'
    ) THEN
        ALTER TABLE passkey_challenges DROP CONSTRAINT IF EXISTS passkey_challenges_kind_check;
        ALTER TABLE passkey_challenges ADD CONSTRAINT passkey_challenges_kind_check
            CHECK (kind IN ('register', 'authenticate', 'attach', 'discoverable'));
    END IF;
END $$;
