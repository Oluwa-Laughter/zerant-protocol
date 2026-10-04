ALTER TABLE issued_credentials
    ADD COLUMN IF NOT EXISTS revocation_digest TEXT;

CREATE TABLE IF NOT EXISTS issuer_revocation_state (
    issuer_profile_id UUID PRIMARY KEY REFERENCES issuer_profiles(id) ON DELETE CASCADE,
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

INSERT INTO issuer_revocation_state(issuer_profile_id)
SELECT id FROM issuer_profiles
ON CONFLICT (issuer_profile_id) DO NOTHING;

CREATE INDEX IF NOT EXISTS issued_credentials_revoked_idx
    ON issued_credentials(issuer_profile_id, revoked_at)
    WHERE revoked_at IS NOT NULL;
