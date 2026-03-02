CREATE TABLE IF NOT EXISTS iam_external_identities (
    id BIGSERIAL PRIMARY KEY,
    auth_source VARCHAR(32) NOT NULL,
    external_subject VARCHAR(255) NOT NULL,
    user_id BIGINT NOT NULL REFERENCES iam_users (id) ON DELETE CASCADE,
    email_snapshot VARCHAR(256),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (auth_source, external_subject)
);

CREATE INDEX IF NOT EXISTS idx_iam_external_identities_user_id
    ON iam_external_identities (user_id);

CREATE TABLE IF NOT EXISTS auth_oidc_login_states (
    state VARCHAR(128) PRIMARY KEY,
    nonce VARCHAR(128) NOT NULL,
    return_to VARCHAR(512),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,
    consumed_at TIMESTAMPTZ,
    consumed_by_user_id BIGINT REFERENCES iam_users (id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_auth_oidc_login_states_expires_at
    ON auth_oidc_login_states (expires_at);

CREATE TABLE IF NOT EXISTS auth_sessions (
    id VARCHAR(128) PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES iam_users (id) ON DELETE CASCADE,
    auth_source VARCHAR(32) NOT NULL,
    issued_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    CONSTRAINT chk_auth_sessions_metadata_object CHECK (jsonb_typeof(metadata) = 'object')
);

CREATE INDEX IF NOT EXISTS idx_auth_sessions_user_id
    ON auth_sessions (user_id);

CREATE INDEX IF NOT EXISTS idx_auth_sessions_expires_at
    ON auth_sessions (expires_at);
