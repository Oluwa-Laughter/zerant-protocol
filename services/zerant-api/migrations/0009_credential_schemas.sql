CREATE TABLE IF NOT EXISTS credential_schemas (
    id UUID PRIMARY KEY,
    issuer_profile_id UUID NOT NULL REFERENCES issuer_profiles(id) ON DELETE CASCADE,
    slug TEXT NOT NULL,
    display_name TEXT NOT NULL CHECK (char_length(display_name) BETWEEN 2 AND 120),
    description TEXT NOT NULL CHECK (char_length(description) BETWEEN 2 AND 512),
    claim_type TEXT NOT NULL,
    context TEXT NOT NULL,
    default_expiry_days INTEGER NOT NULL CHECK (default_expiry_days BETWEEN 1 AND 365),
    active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (issuer_profile_id, slug),
    UNIQUE (issuer_profile_id, claim_type, context)
);

ALTER TABLE issued_credentials
    ADD COLUMN IF NOT EXISTS credential_schema_id UUID
    REFERENCES credential_schemas(id) ON DELETE RESTRICT;

ALTER TABLE verification_requests
    ADD COLUMN IF NOT EXISTS credential_schema_id UUID
    REFERENCES credential_schemas(id) ON DELETE RESTRICT;

CREATE INDEX IF NOT EXISTS credential_schemas_issuer_idx
    ON credential_schemas(issuer_profile_id, active, display_name);
