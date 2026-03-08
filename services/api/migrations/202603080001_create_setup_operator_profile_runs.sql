CREATE TABLE IF NOT EXISTS setup_operator_profile_runs (
    id BIGSERIAL PRIMARY KEY,
    profile_key VARCHAR(64) NOT NULL,
    profile_name VARCHAR(128) NOT NULL,
    actor VARCHAR(128) NOT NULL,
    status VARCHAR(16) NOT NULL DEFAULT 'applied',
    note VARCHAR(1024),
    previous_state JSONB NOT NULL DEFAULT '{}'::jsonb,
    applied_state JSONB NOT NULL DEFAULT '{}'::jsonb,
    reverted_by VARCHAR(128),
    reverted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_setup_operator_profile_runs_key_not_blank CHECK (btrim(profile_key) <> ''),
    CONSTRAINT chk_setup_operator_profile_runs_name_not_blank CHECK (btrim(profile_name) <> ''),
    CONSTRAINT chk_setup_operator_profile_runs_actor_not_blank CHECK (btrim(actor) <> ''),
    CONSTRAINT chk_setup_operator_profile_runs_status CHECK (status IN ('applied', 'reverted', 'failed')),
    CONSTRAINT chk_setup_operator_profile_runs_previous_state_object CHECK (jsonb_typeof(previous_state) = 'object'),
    CONSTRAINT chk_setup_operator_profile_runs_applied_state_object CHECK (jsonb_typeof(applied_state) = 'object')
);

CREATE INDEX IF NOT EXISTS idx_setup_operator_profile_runs_created
    ON setup_operator_profile_runs (created_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_setup_operator_profile_runs_profile
    ON setup_operator_profile_runs (profile_key, created_at DESC, id DESC);
