ALTER TABLE credential_schemas
    ADD COLUMN IF NOT EXISTS version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0);

ALTER TABLE credential_schemas
    ADD COLUMN IF NOT EXISTS supersedes_schema_id UUID
    REFERENCES credential_schemas(id) ON DELETE RESTRICT;

ALTER TABLE credential_schemas
    ADD COLUMN IF NOT EXISTS retired_at TIMESTAMPTZ;

UPDATE credential_schemas
SET retired_at = COALESCE(retired_at, updated_at)
WHERE active = FALSE
  AND retired_at IS NULL;

ALTER TABLE credential_schemas
    DROP CONSTRAINT IF EXISTS credential_schemas_issuer_profile_id_slug_key;

ALTER TABLE credential_schemas
    DROP CONSTRAINT IF EXISTS credential_schemas_issuer_profile_id_claim_type_context_key;

CREATE UNIQUE INDEX IF NOT EXISTS credential_schemas_version_idx
    ON credential_schemas(issuer_profile_id, slug, version);

CREATE UNIQUE INDEX IF NOT EXISTS credential_schemas_active_slug_idx
    ON credential_schemas(issuer_profile_id, slug)
    WHERE active = TRUE;

CREATE UNIQUE INDEX IF NOT EXISTS credential_schemas_active_meaning_idx
    ON credential_schemas(issuer_profile_id, claim_type, context)
    WHERE active = TRUE;

CREATE INDEX IF NOT EXISTS credential_schemas_supersedes_idx
    ON credential_schemas(supersedes_schema_id);
