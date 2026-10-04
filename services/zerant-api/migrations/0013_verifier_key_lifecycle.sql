CREATE TABLE IF NOT EXISTS verifier_signing_keys (
    id UUID PRIMARY KEY,
    verifier_profile_id UUID NOT NULL REFERENCES verifier_profiles(id) ON DELETE CASCADE,
    verifier_key_id TEXT NOT NULL UNIQUE,
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

INSERT INTO verifier_signing_keys(
    id,
    verifier_profile_id,
    verifier_key_id,
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
    v.id,
    v.id,
    v.verifier_key_id,
    v.public_jwk,
    v.ciphertext,
    v.data_nonce,
    v.wrapped_dek,
    v.wrap_nonce,
    v.key_version,
    v.created_at,
    v.created_at
FROM verifier_profiles v
ON CONFLICT (verifier_key_id) DO NOTHING;

CREATE UNIQUE INDEX IF NOT EXISTS verifier_signing_keys_active_idx
    ON verifier_signing_keys(verifier_profile_id)
    WHERE retired_at IS NULL AND compromised_at IS NULL;

ALTER TABLE verification_requests
    ADD COLUMN IF NOT EXISTS verifier_key_id TEXT;

UPDATE verification_requests r
SET verifier_key_id = v.verifier_key_id
FROM verifier_profiles v
WHERE r.verifier_profile_id = v.id
  AND r.verifier_key_id IS NULL;

ALTER TABLE verification_requests
    ALTER COLUMN verifier_key_id SET NOT NULL;

CREATE INDEX IF NOT EXISTS verification_requests_verifier_key_idx
    ON verification_requests(verifier_profile_id, verifier_key_id, expires_at);
