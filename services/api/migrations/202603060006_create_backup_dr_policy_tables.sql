CREATE TABLE IF NOT EXISTS ops_backup_policies (
    id BIGSERIAL PRIMARY KEY,
    policy_key VARCHAR(64) NOT NULL UNIQUE,
    name VARCHAR(128) NOT NULL,
    frequency VARCHAR(16) NOT NULL,
    schedule_time_utc VARCHAR(5) NOT NULL,
    schedule_weekday SMALLINT,
    retention_days INTEGER NOT NULL,
    destination_type VARCHAR(32) NOT NULL,
    destination_uri VARCHAR(512) NOT NULL,
    destination_validated BOOLEAN NOT NULL DEFAULT FALSE,
    drill_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    drill_frequency VARCHAR(16) NOT NULL DEFAULT 'weekly',
    drill_weekday SMALLINT,
    drill_time_utc VARCHAR(5) NOT NULL DEFAULT '02:00',
    last_backup_status VARCHAR(16) NOT NULL DEFAULT 'never',
    last_backup_at TIMESTAMPTZ,
    last_backup_error VARCHAR(1024),
    last_drill_status VARCHAR(16) NOT NULL DEFAULT 'never',
    last_drill_at TIMESTAMPTZ,
    last_drill_error VARCHAR(1024),
    next_backup_at TIMESTAMPTZ,
    next_drill_at TIMESTAMPTZ,
    updated_by VARCHAR(128) NOT NULL DEFAULT 'system',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_ops_backup_policies_key_not_blank CHECK (btrim(policy_key) <> ''),
    CONSTRAINT chk_ops_backup_policies_name_not_blank CHECK (btrim(name) <> ''),
    CONSTRAINT chk_ops_backup_policies_frequency CHECK (frequency IN ('daily', 'weekly')),
    CONSTRAINT chk_ops_backup_policies_schedule_time CHECK (schedule_time_utc ~ '^[0-2][0-9]:[0-5][0-9]$'),
    CONSTRAINT chk_ops_backup_policies_schedule_weekday CHECK (schedule_weekday IS NULL OR schedule_weekday BETWEEN 1 AND 7),
    CONSTRAINT chk_ops_backup_policies_retention_days CHECK (retention_days BETWEEN 1 AND 3650),
    CONSTRAINT chk_ops_backup_policies_destination_type CHECK (destination_type IN ('s3', 'nfs', 'local')),
    CONSTRAINT chk_ops_backup_policies_destination_uri_not_blank CHECK (btrim(destination_uri) <> ''),
    CONSTRAINT chk_ops_backup_policies_drill_frequency CHECK (drill_frequency IN ('weekly', 'monthly', 'quarterly')),
    CONSTRAINT chk_ops_backup_policies_drill_weekday CHECK (drill_weekday IS NULL OR drill_weekday BETWEEN 1 AND 7),
    CONSTRAINT chk_ops_backup_policies_drill_time CHECK (drill_time_utc ~ '^[0-2][0-9]:[0-5][0-9]$'),
    CONSTRAINT chk_ops_backup_policies_last_backup_status CHECK (last_backup_status IN ('never', 'succeeded', 'failed')),
    CONSTRAINT chk_ops_backup_policies_last_drill_status CHECK (last_drill_status IN ('never', 'succeeded', 'failed')),
    CONSTRAINT chk_ops_backup_policies_updated_by_not_blank CHECK (btrim(updated_by) <> '')
);

CREATE INDEX IF NOT EXISTS idx_ops_backup_policies_next_backup
    ON ops_backup_policies (next_backup_at, id);

CREATE INDEX IF NOT EXISTS idx_ops_backup_policies_next_drill
    ON ops_backup_policies (next_drill_at, id)
    WHERE drill_enabled = TRUE;

CREATE TABLE IF NOT EXISTS ops_backup_policy_runs (
    id BIGSERIAL PRIMARY KEY,
    policy_id BIGINT NOT NULL REFERENCES ops_backup_policies (id) ON DELETE CASCADE,
    run_type VARCHAR(16) NOT NULL,
    status VARCHAR(16) NOT NULL,
    triggered_by VARCHAR(128) NOT NULL,
    triggered_by_scheduler BOOLEAN NOT NULL DEFAULT FALSE,
    note VARCHAR(1024),
    remediation_hint VARCHAR(1024),
    error_message VARCHAR(1024),
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    finished_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_ops_backup_policy_runs_type CHECK (run_type IN ('backup', 'drill')),
    CONSTRAINT chk_ops_backup_policy_runs_status CHECK (status IN ('succeeded', 'failed')),
    CONSTRAINT chk_ops_backup_policy_runs_triggered_by_not_blank CHECK (btrim(triggered_by) <> ''),
    CONSTRAINT chk_ops_backup_policy_runs_metadata_object CHECK (jsonb_typeof(metadata) = 'object')
);

CREATE INDEX IF NOT EXISTS idx_ops_backup_policy_runs_policy_created
    ON ops_backup_policy_runs (policy_id, created_at DESC, id DESC);

INSERT INTO ops_backup_policies (
    policy_key,
    name,
    frequency,
    schedule_time_utc,
    schedule_weekday,
    retention_days,
    destination_type,
    destination_uri,
    destination_validated,
    drill_enabled,
    drill_frequency,
    drill_weekday,
    drill_time_utc,
    next_backup_at,
    next_drill_at,
    updated_by
)
VALUES (
    'default-continuity',
    'Default Continuity Policy',
    'daily',
    '01:30',
    NULL,
    14,
    'local',
    'file:///var/lib/cloudops/backups',
    TRUE,
    TRUE,
    'weekly',
    3,
    '02:30',
    NOW() + INTERVAL '1 day',
    NOW() + INTERVAL '7 day',
    'system'
)
ON CONFLICT (policy_key) DO UPDATE
SET
    name = EXCLUDED.name,
    frequency = EXCLUDED.frequency,
    schedule_time_utc = EXCLUDED.schedule_time_utc,
    retention_days = EXCLUDED.retention_days,
    destination_type = EXCLUDED.destination_type,
    destination_uri = EXCLUDED.destination_uri,
    destination_validated = EXCLUDED.destination_validated,
    drill_enabled = EXCLUDED.drill_enabled,
    drill_frequency = EXCLUDED.drill_frequency,
    drill_time_utc = EXCLUDED.drill_time_utc,
    updated_at = NOW();
