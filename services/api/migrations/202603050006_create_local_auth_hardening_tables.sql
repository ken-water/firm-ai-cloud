CREATE TABLE IF NOT EXISTS auth_local_credentials (
    user_id BIGINT PRIMARY KEY REFERENCES iam_users (id) ON DELETE CASCADE,
    password_salt VARCHAR(64) NOT NULL,
    password_hash VARCHAR(128) NOT NULL,
    mfa_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    totp_secret VARCHAR(128),
    totp_enrolled_at TIMESTAMPTZ,
    failed_attempts INTEGER NOT NULL DEFAULT 0,
    last_failed_at TIMESTAMPTZ,
    locked_until TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_auth_local_credentials_password_salt_not_blank CHECK (btrim(password_salt) <> ''),
    CONSTRAINT chk_auth_local_credentials_password_hash_not_blank CHECK (btrim(password_hash) <> '')
);

CREATE INDEX IF NOT EXISTS idx_auth_local_credentials_locked_until
    ON auth_local_credentials (locked_until);

ALTER TABLE auth_sessions
    ADD COLUMN IF NOT EXISTS last_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW();

CREATE INDEX IF NOT EXISTS idx_auth_sessions_auth_source_expires
    ON auth_sessions (auth_source, expires_at);
