CREATE TABLE IF NOT EXISTS auth_local_recovery_codes (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES iam_users (id) ON DELETE CASCADE,
    code_hash VARCHAR(128) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    consumed_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    UNIQUE (user_id, code_hash),
    CONSTRAINT chk_auth_local_recovery_codes_hash_not_blank CHECK (btrim(code_hash) <> '')
);

CREATE INDEX IF NOT EXISTS idx_auth_local_recovery_codes_user_id
    ON auth_local_recovery_codes (user_id);

CREATE INDEX IF NOT EXISTS idx_auth_local_recovery_codes_user_active
    ON auth_local_recovery_codes (user_id, consumed_at, revoked_at);
