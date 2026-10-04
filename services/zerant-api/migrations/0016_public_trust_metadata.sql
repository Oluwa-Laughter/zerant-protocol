CREATE TABLE IF NOT EXISTS issuer_revocation_publications (
    issuer_profile_id UUID PRIMARY KEY REFERENCES issuer_profiles(id) ON DELETE CASCADE,
    version BIGINT NOT NULL CHECK (version > 0),
    issuer_key_id TEXT NOT NULL,
    snapshot_jws TEXT NOT NULL,
    issued_at TIMESTAMPTZ NOT NULL,
    next_update TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS issuer_revocation_publications_expiry_idx
    ON issuer_revocation_publications(next_update);
