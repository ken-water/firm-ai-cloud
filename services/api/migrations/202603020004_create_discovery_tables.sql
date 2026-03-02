CREATE TABLE IF NOT EXISTS discovery_jobs (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(128) NOT NULL,
    source_type VARCHAR(32) NOT NULL,
    scope JSONB NOT NULL DEFAULT '{}'::jsonb,
    schedule VARCHAR(128),
    status VARCHAR(32) NOT NULL DEFAULT 'idle',
    is_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    last_run_at TIMESTAMPTZ,
    last_run_status VARCHAR(32),
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_discovery_jobs_scope_object CHECK (jsonb_typeof(scope) = 'object')
);

CREATE TABLE IF NOT EXISTS discovery_candidates (
    id BIGSERIAL PRIMARY KEY,
    job_id BIGINT REFERENCES discovery_jobs (id) ON DELETE SET NULL,
    fingerprint VARCHAR(255) NOT NULL,
    payload JSONB NOT NULL,
    review_status VARCHAR(32) NOT NULL DEFAULT 'pending',
    discovered_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    reviewed_by VARCHAR(128),
    reviewed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_discovery_candidates_payload_object CHECK (jsonb_typeof(payload) = 'object')
);

CREATE INDEX IF NOT EXISTS idx_discovery_jobs_source_type
    ON discovery_jobs (source_type);

CREATE INDEX IF NOT EXISTS idx_discovery_jobs_status
    ON discovery_jobs (status);

CREATE INDEX IF NOT EXISTS idx_discovery_candidates_review_status
    ON discovery_candidates (review_status);

CREATE INDEX IF NOT EXISTS idx_discovery_candidates_discovered_at
    ON discovery_candidates (discovered_at DESC);

CREATE INDEX IF NOT EXISTS idx_discovery_candidates_fingerprint
    ON discovery_candidates (fingerprint);
