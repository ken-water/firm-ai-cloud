CREATE TABLE IF NOT EXISTS workflow_playbook_execution_policies (
    id BIGSERIAL PRIMARY KEY,
    policy_key VARCHAR(64) NOT NULL UNIQUE,
    timezone_name VARCHAR(64) NOT NULL DEFAULT 'UTC',
    maintenance_windows JSONB NOT NULL DEFAULT '[]'::jsonb,
    change_freeze_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    override_requires_reason BOOLEAN NOT NULL DEFAULT TRUE,
    updated_by VARCHAR(128) NOT NULL DEFAULT 'system',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_workflow_playbook_execution_policies_key_not_blank CHECK (btrim(policy_key) <> ''),
    CONSTRAINT chk_workflow_playbook_execution_policies_timezone_not_blank CHECK (btrim(timezone_name) <> ''),
    CONSTRAINT chk_workflow_playbook_execution_policies_windows_array CHECK (jsonb_typeof(maintenance_windows) = 'array'),
    CONSTRAINT chk_workflow_playbook_execution_policies_updated_by_not_blank CHECK (btrim(updated_by) <> '')
);

INSERT INTO workflow_playbook_execution_policies (
    policy_key,
    timezone_name,
    maintenance_windows,
    change_freeze_enabled,
    override_requires_reason,
    updated_by
)
VALUES
    (
        'global',
        'UTC',
        jsonb_build_array(
            jsonb_build_object('day_of_week', 1, 'start', '00:00', 'end', '23:59', 'label', 'Mon full-day window'),
            jsonb_build_object('day_of_week', 2, 'start', '00:00', 'end', '23:59', 'label', 'Tue full-day window'),
            jsonb_build_object('day_of_week', 3, 'start', '00:00', 'end', '23:59', 'label', 'Wed full-day window'),
            jsonb_build_object('day_of_week', 4, 'start', '00:00', 'end', '23:59', 'label', 'Thu full-day window'),
            jsonb_build_object('day_of_week', 5, 'start', '00:00', 'end', '23:59', 'label', 'Fri full-day window'),
            jsonb_build_object('day_of_week', 6, 'start', '00:00', 'end', '23:59', 'label', 'Sat full-day window'),
            jsonb_build_object('day_of_week', 7, 'start', '00:00', 'end', '23:59', 'label', 'Sun full-day window')
        ),
        FALSE,
        TRUE,
        'system'
    )
ON CONFLICT (policy_key) DO UPDATE
SET
    timezone_name = EXCLUDED.timezone_name,
    maintenance_windows = EXCLUDED.maintenance_windows,
    override_requires_reason = EXCLUDED.override_requires_reason,
    updated_at = NOW();
