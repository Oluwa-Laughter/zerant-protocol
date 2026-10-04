CREATE TABLE IF NOT EXISTS issuer_signing_keys (
    id UUID PRIMARY KEY,
    issuer_profile_id UUID NOT NULL REFERENCES issuer_profiles(id) ON DELETE CASCADE,
    issuer_key_id TEXT NOT NULL UNIQUE,
    public_jwk JSONB NOT NULL,
    ciphertext BYTEA NOT NULL,
    data_nonce BYTEA NOT NULL CHECK (octet_length(data_nonce) = 12),
    wrapped_dek BYTEA NOT NULL,
    wrap_nonce BYTEA NOT NULL CHECK (octet_length(wrap_nonce) = 12),
    key_version INTEGER NOT NULL,
    valid_from TIMESTAMPTZ NOT NULL,
    retired_at TIMESTAMPTZ,
    compromised_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

INSERT INTO issuer_signing_keys(
    id,
    issuer_profile_id,
    issuer_key_id,
    public_jwk,
    ciphertext,
    data_nonce,
    wrapped_dek,
    wrap_nonce,
    key_version,
    valid_from,
    created_at
)
SELECT
    p.id,
    p.id,
    p.issuer_key_id,
    p.public_jwk,
    p.ciphertext,
    p.data_nonce,
    p.wrapped_dek,
    p.wrap_nonce,
    p.key_version,
    p.created_at,
    p.created_at
FROM issuer_profiles p
ON CONFLICT (issuer_key_id) DO NOTHING;

CREATE UNIQUE INDEX IF NOT EXISTS issuer_signing_keys_active_idx
    ON issuer_signing_keys(issuer_profile_id)
    WHERE retired_at IS NULL AND compromised_at IS NULL;

ALTER TABLE issued_credentials
    ADD COLUMN IF NOT EXISTS issuer_key_id TEXT;

UPDATE issued_credentials c
SET issuer_key_id = p.issuer_key_id
FROM issuer_profiles p
WHERE c.issuer_profile_id = p.id
  AND c.issuer_key_id IS NULL;

ALTER TABLE issued_credentials
    ALTER COLUMN issuer_key_id SET NOT NULL;

CREATE INDEX IF NOT EXISTS issued_credentials_issuer_key_idx
    ON issued_credentials(issuer_profile_id, issuer_key_id, expires_at);
