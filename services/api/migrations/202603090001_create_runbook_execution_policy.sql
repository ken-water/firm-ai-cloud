CREATE TABLE IF NOT EXISTS ops_runbook_execution_policies (
    policy_key VARCHAR(32) PRIMARY KEY,
    mode VARCHAR(32) NOT NULL DEFAULT 'simulate_only',
    live_templates JSONB NOT NULL DEFAULT '[]'::jsonb,
    max_live_step_timeout_seconds INTEGER NOT NULL DEFAULT 10,
    allow_simulate_failure BOOLEAN NOT NULL DEFAULT TRUE,
    note VARCHAR(1024),
    updated_by VARCHAR(128) NOT NULL DEFAULT 'system',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_ops_runbook_execution_policies_key_not_blank CHECK (btrim(policy_key) <> ''),
    CONSTRAINT chk_ops_runbook_execution_policies_mode CHECK (mode IN ('simulate_only', 'hybrid_live')),
    CONSTRAINT chk_ops_runbook_execution_policies_live_templates_array CHECK (jsonb_typeof(live_templates) = 'array'),
    CONSTRAINT chk_ops_runbook_execution_policies_timeout CHECK (max_live_step_timeout_seconds BETWEEN 1 AND 120),
    CONSTRAINT chk_ops_runbook_execution_policies_updated_by_not_blank CHECK (btrim(updated_by) <> '')
);

INSERT INTO ops_runbook_execution_policies (
    policy_key,
    mode,
    live_templates,
    max_live_step_timeout_seconds,
    allow_simulate_failure,
    note,
    updated_by
)
VALUES (
    'global',
    'simulate_only',
    '[]'::jsonb,
    10,
    TRUE,
    'default seeded policy',
    'system'
)
ON CONFLICT (policy_key) DO NOTHING;

ALTER TABLE ops_runbook_template_executions
    ADD COLUMN IF NOT EXISTS execution_mode VARCHAR(16) NOT NULL DEFAULT 'simulate';

ALTER TABLE ops_runbook_template_executions
    ADD COLUMN IF NOT EXISTS runtime_summary JSONB NOT NULL DEFAULT '{}'::jsonb;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'chk_ops_runbook_template_executions_execution_mode'
    ) THEN
        ALTER TABLE ops_runbook_template_executions
            ADD CONSTRAINT chk_ops_runbook_template_executions_execution_mode
            CHECK (execution_mode IN ('simulate', 'live'));
    END IF;

    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'chk_ops_runbook_template_executions_runtime_summary_object'
    ) THEN
        ALTER TABLE ops_runbook_template_executions
            ADD CONSTRAINT chk_ops_runbook_template_executions_runtime_summary_object
            CHECK (jsonb_typeof(runtime_summary) = 'object');
    END IF;
END
$$;

CREATE INDEX IF NOT EXISTS idx_ops_runbook_template_executions_mode_created
    ON ops_runbook_template_executions (execution_mode, created_at DESC, id DESC);
