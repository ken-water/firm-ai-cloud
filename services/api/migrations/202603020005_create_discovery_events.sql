CREATE TABLE IF NOT EXISTS discovery_events (
    id BIGSERIAL PRIMARY KEY,
    job_id BIGINT REFERENCES discovery_jobs (id) ON DELETE SET NULL,
    asset_id BIGINT REFERENCES assets (id) ON DELETE SET NULL,
    event_type VARCHAR(64) NOT NULL,
    fingerprint VARCHAR(255),
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    happened_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_discovery_events_payload_object CHECK (jsonb_typeof(payload) = 'object')
);

CREATE TABLE IF NOT EXISTS discovery_asset_states (
    id BIGSERIAL PRIMARY KEY,
    job_id BIGINT NOT NULL REFERENCES discovery_jobs (id) ON DELETE CASCADE,
    asset_id BIGINT NOT NULL REFERENCES assets (id) ON DELETE CASCADE,
    profile JSONB NOT NULL DEFAULT '{}'::jsonb,
    last_seen_at TIMESTAMPTZ,
    missed_runs INTEGER NOT NULL DEFAULT 0,
    offboarded_emitted BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_discovery_asset_states_profile_object CHECK (jsonb_typeof(profile) = 'object'),
    CONSTRAINT chk_discovery_asset_states_missed_runs_nonnegative CHECK (missed_runs >= 0),
    UNIQUE (job_id, asset_id)
);

CREATE INDEX IF NOT EXISTS idx_discovery_events_asset_id_happened_at
    ON discovery_events (asset_id, happened_at DESC);

CREATE INDEX IF NOT EXISTS idx_discovery_events_event_type_happened_at
    ON discovery_events (event_type, happened_at DESC);

CREATE INDEX IF NOT EXISTS idx_discovery_events_job_id_happened_at
    ON discovery_events (job_id, happened_at DESC);

CREATE INDEX IF NOT EXISTS idx_discovery_asset_states_job_id
    ON discovery_asset_states (job_id);

CREATE INDEX IF NOT EXISTS idx_discovery_asset_states_last_seen_at
    ON discovery_asset_states (last_seen_at DESC);
