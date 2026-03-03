CREATE TABLE IF NOT EXISTS cmdb_monitoring_bindings (
    id BIGSERIAL PRIMARY KEY,
    asset_id BIGINT NOT NULL UNIQUE REFERENCES assets (id) ON DELETE CASCADE,
    source_system VARCHAR(32) NOT NULL DEFAULT 'zabbix',
    source_id BIGINT REFERENCES monitoring_sources (id) ON DELETE SET NULL,
    external_host_id VARCHAR(64),
    last_sync_status VARCHAR(32) NOT NULL DEFAULT 'pending',
    last_sync_message TEXT,
    last_sync_at TIMESTAMPTZ,
    mapping JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_cmdb_monitoring_bindings_source_system CHECK (
        source_system IN ('zabbix')
    ),
    CONSTRAINT chk_cmdb_monitoring_bindings_status CHECK (
        last_sync_status IN ('pending', 'running', 'success', 'failed', 'dead_letter', 'skipped')
    ),
    CONSTRAINT chk_cmdb_monitoring_bindings_mapping_object CHECK (
        jsonb_typeof(mapping) = 'object'
    )
);

CREATE TABLE IF NOT EXISTS cmdb_monitoring_sync_jobs (
    id BIGSERIAL PRIMARY KEY,
    asset_id BIGINT NOT NULL REFERENCES assets (id) ON DELETE CASCADE,
    trigger_source VARCHAR(32) NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'pending',
    attempt INTEGER NOT NULL DEFAULT 0,
    max_attempts INTEGER NOT NULL DEFAULT 5,
    run_after TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    requested_by VARCHAR(128),
    requested_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    last_error TEXT,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_cmdb_monitoring_sync_jobs_trigger_source CHECK (
        trigger_source IN ('asset_create', 'asset_update', 'manual_retry')
    ),
    CONSTRAINT chk_cmdb_monitoring_sync_jobs_status CHECK (
        status IN ('pending', 'running', 'success', 'failed', 'dead_letter', 'skipped')
    ),
    CONSTRAINT chk_cmdb_monitoring_sync_jobs_attempt_nonnegative CHECK (
        attempt >= 0
    ),
    CONSTRAINT chk_cmdb_monitoring_sync_jobs_max_attempts_positive CHECK (
        max_attempts > 0
    ),
    CONSTRAINT chk_cmdb_monitoring_sync_jobs_payload_object CHECK (
        jsonb_typeof(payload) = 'object'
    )
);

CREATE INDEX IF NOT EXISTS idx_cmdb_monitoring_bindings_status
    ON cmdb_monitoring_bindings (last_sync_status);

CREATE INDEX IF NOT EXISTS idx_cmdb_monitoring_bindings_source_id
    ON cmdb_monitoring_bindings (source_id);

CREATE INDEX IF NOT EXISTS idx_cmdb_monitoring_sync_jobs_asset_id
    ON cmdb_monitoring_sync_jobs (asset_id);

CREATE INDEX IF NOT EXISTS idx_cmdb_monitoring_sync_jobs_status_run_after
    ON cmdb_monitoring_sync_jobs (status, run_after, id);

CREATE INDEX IF NOT EXISTS idx_cmdb_monitoring_sync_jobs_requested_at
    ON cmdb_monitoring_sync_jobs (requested_at DESC);

CREATE UNIQUE INDEX IF NOT EXISTS uq_cmdb_monitoring_sync_jobs_asset_inflight
    ON cmdb_monitoring_sync_jobs (asset_id)
    WHERE status IN ('pending', 'running');
