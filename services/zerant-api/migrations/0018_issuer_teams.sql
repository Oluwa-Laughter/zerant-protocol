CREATE TABLE IF NOT EXISTS issuer_members (
    issuer_profile_id UUID NOT NULL REFERENCES issuer_profiles(id) ON DELETE CASCADE,
    account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('admin', 'issuer', 'auditor')),
    joined_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (issuer_profile_id, account_id),
    UNIQUE (account_id)
);

CREATE TABLE IF NOT EXISTS issuer_invitations (
    id UUID PRIMARY KEY,
    issuer_profile_id UUID NOT NULL REFERENCES issuer_profiles(id) ON DELETE CASCADE,
    invited_account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    invited_by_account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('admin', 'issuer', 'auditor')),
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'accepted', 'declined', 'cancelled', 'expired')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,
    decided_at TIMESTAMPTZ
);

CREATE UNIQUE INDEX IF NOT EXISTS issuer_invitations_pending_target_idx
    ON issuer_invitations(issuer_profile_id, invited_account_id)
    WHERE status = 'pending';

CREATE INDEX IF NOT EXISTS issuer_invitations_target_idx
    ON issuer_invitations(invited_account_id, status, expires_at);

CREATE INDEX IF NOT EXISTS issuer_members_profile_idx
    ON issuer_members(issuer_profile_id, joined_at);
