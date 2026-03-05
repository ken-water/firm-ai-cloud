CREATE TABLE IF NOT EXISTS workflow_playbooks (
    id BIGSERIAL PRIMARY KEY,
    playbook_key VARCHAR(64) NOT NULL UNIQUE,
    name VARCHAR(128) NOT NULL,
    description VARCHAR(512),
    category VARCHAR(64) NOT NULL,
    risk_level VARCHAR(16) NOT NULL DEFAULT 'low',
    requires_confirmation BOOLEAN NOT NULL DEFAULT FALSE,
    parameter_schema JSONB NOT NULL DEFAULT '{"fields":[]}'::jsonb,
    execution_plan JSONB NOT NULL DEFAULT '{"steps":[]}'::jsonb,
    rbac_hint JSONB NOT NULL DEFAULT '{"read_permission":"workflow.playbooks.read","execute_permission":"workflow.playbooks.write"}'::jsonb,
    is_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    is_system BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_workflow_playbooks_key_not_blank CHECK (btrim(playbook_key) <> ''),
    CONSTRAINT chk_workflow_playbooks_name_not_blank CHECK (btrim(name) <> ''),
    CONSTRAINT chk_workflow_playbooks_category_not_blank CHECK (btrim(category) <> ''),
    CONSTRAINT chk_workflow_playbooks_risk_level CHECK (risk_level IN ('low', 'medium', 'high', 'critical')),
    CONSTRAINT chk_workflow_playbooks_parameter_schema_object CHECK (jsonb_typeof(parameter_schema) = 'object'),
    CONSTRAINT chk_workflow_playbooks_execution_plan_object CHECK (jsonb_typeof(execution_plan) = 'object'),
    CONSTRAINT chk_workflow_playbooks_rbac_hint_object CHECK (jsonb_typeof(rbac_hint) = 'object')
);

CREATE INDEX IF NOT EXISTS idx_workflow_playbooks_category_enabled
    ON workflow_playbooks (category, is_enabled, id DESC);

CREATE INDEX IF NOT EXISTS idx_workflow_playbooks_risk_enabled
    ON workflow_playbooks (risk_level, is_enabled, id DESC);

CREATE TABLE IF NOT EXISTS workflow_playbook_executions (
    id BIGSERIAL PRIMARY KEY,
    playbook_id BIGINT NOT NULL REFERENCES workflow_playbooks (id) ON DELETE CASCADE,
    playbook_key VARCHAR(64) NOT NULL,
    playbook_name VARCHAR(128) NOT NULL,
    category VARCHAR(64) NOT NULL,
    risk_level VARCHAR(16) NOT NULL,
    actor VARCHAR(128) NOT NULL,
    asset_ref VARCHAR(128),
    mode VARCHAR(16) NOT NULL,
    status VARCHAR(32) NOT NULL,
    confirmation_required BOOLEAN NOT NULL DEFAULT FALSE,
    confirmation_token VARCHAR(128),
    confirmation_verified BOOLEAN NOT NULL DEFAULT FALSE,
    confirmed_at TIMESTAMPTZ,
    params_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    planned_steps JSONB NOT NULL DEFAULT '[]'::jsonb,
    result_json JSONB NOT NULL DEFAULT '{}'::jsonb,
    related_ticket_id BIGINT REFERENCES tickets (id) ON DELETE SET NULL,
    related_alert_id BIGINT REFERENCES unified_alerts (id) ON DELETE SET NULL,
    replay_of_execution_id BIGINT REFERENCES workflow_playbook_executions (id) ON DELETE SET NULL,
    expires_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_workflow_playbook_executions_mode CHECK (mode IN ('dry_run', 'execute')),
    CONSTRAINT chk_workflow_playbook_executions_status CHECK (status IN ('planned', 'succeeded', 'failed', 'blocked', 'expired')),
    CONSTRAINT chk_workflow_playbook_executions_risk_level CHECK (risk_level IN ('low', 'medium', 'high', 'critical')),
    CONSTRAINT chk_workflow_playbook_executions_actor_not_blank CHECK (btrim(actor) <> ''),
    CONSTRAINT chk_workflow_playbook_executions_params_object CHECK (jsonb_typeof(params_json) = 'object'),
    CONSTRAINT chk_workflow_playbook_executions_steps_array CHECK (jsonb_typeof(planned_steps) = 'array'),
    CONSTRAINT chk_workflow_playbook_executions_result_object CHECK (jsonb_typeof(result_json) = 'object')
);

CREATE INDEX IF NOT EXISTS idx_workflow_playbook_exec_lookup
    ON workflow_playbook_executions (playbook_key, mode, created_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_workflow_playbook_exec_actor
    ON workflow_playbook_executions (actor, created_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_workflow_playbook_exec_asset
    ON workflow_playbook_executions (asset_ref, created_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_workflow_playbook_exec_status
    ON workflow_playbook_executions (status, created_at DESC, id DESC);

INSERT INTO iam_permissions (permission_key, description)
VALUES
    ('workflow.playbooks.read', 'Read playbook catalog and execution logs'),
    ('workflow.playbooks.write', 'Execute playbooks and run dry-run/confirmation flows')
ON CONFLICT (permission_key) DO UPDATE
SET
    description = EXCLUDED.description,
    updated_at = NOW();

INSERT INTO iam_role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM iam_roles r
INNER JOIN iam_permissions p ON p.permission_key = 'workflow.playbooks.read'
WHERE r.role_key = 'viewer'
ON CONFLICT DO NOTHING;

INSERT INTO iam_role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM iam_roles r
INNER JOIN iam_permissions p ON p.permission_key IN ('workflow.playbooks.read', 'workflow.playbooks.write')
WHERE r.role_key = 'operator'
ON CONFLICT DO NOTHING;

INSERT INTO iam_role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM iam_roles r
INNER JOIN iam_permissions p ON p.permission_key IN ('workflow.playbooks.read', 'workflow.playbooks.write')
WHERE r.role_key = 'admin'
ON CONFLICT DO NOTHING;

INSERT INTO workflow_playbooks (
    playbook_key,
    name,
    description,
    category,
    risk_level,
    requires_confirmation,
    parameter_schema,
    execution_plan,
    rbac_hint,
    is_enabled,
    is_system
)
VALUES
    (
        'restart-service-safe',
        'Safe Service Restart',
        'Graceful restart with health checks and rollback hint.',
        'service',
        'medium',
        TRUE,
        $$
        {
          "fields": [
            {"key":"asset_ref","label":"Asset / Host","type":"string","required":true,"max_length":128,"placeholder":"asset-101"},
            {"key":"service_name","label":"Service Name","type":"string","required":true,"max_length":64,"placeholder":"nginx"},
            {"key":"grace_seconds","label":"Grace Seconds","type":"integer","required":false,"default":30,"min":0,"max":600}
          ]
        }
        $$::jsonb,
        $$
        {
          "steps": [
            "Validate target ownership and maintenance window",
            "Drain traffic and wait for grace period",
            "Restart service process",
            "Run post-restart health checks",
            "If health check fails, trigger rollback guidance"
          ]
        }
        $$::jsonb,
        '{"read_permission":"workflow.playbooks.read","execute_permission":"workflow.playbooks.write"}'::jsonb,
        TRUE,
        TRUE
    ),
    (
        'drain-node-maintenance',
        'Node Drain for Maintenance',
        'Prepare host for maintenance with workload drain checklist.',
        'infrastructure',
        'high',
        TRUE,
        $$
        {
          "fields": [
            {"key":"asset_ref","label":"Asset / Host","type":"string","required":true,"max_length":128},
            {"key":"maintenance_ticket","label":"Maintenance Ticket","type":"string","required":true,"max_length":64},
            {"key":"max_wait_seconds","label":"Max Wait Seconds","type":"integer","required":false,"default":900,"min":60,"max":7200}
          ]
        }
        $$::jsonb,
        $$
        {
          "steps": [
            "Verify maintenance ticket and owner approval",
            "Cordone/drain workloads from target node",
            "Verify no critical workloads remain",
            "Mark node as maintenance in CMDB",
            "Output rollback command if drain times out"
          ]
        }
        $$::jsonb,
        '{"read_permission":"workflow.playbooks.read","execute_permission":"workflow.playbooks.write"}'::jsonb,
        TRUE,
        TRUE
    ),
    (
        'refresh-monitoring-binding',
        'Refresh Monitoring Binding',
        'Reconcile monitoring source mapping and probe connectivity.',
        'monitoring',
        'low',
        FALSE,
        $$
        {
          "fields": [
            {"key":"asset_ref","label":"Asset / Host","type":"string","required":true,"max_length":128},
            {"key":"source_type","label":"Monitoring Source","type":"enum","required":false,"default":"zabbix","options":["zabbix"]},
            {"key":"force_probe","label":"Force Probe","type":"boolean","required":false,"default":true}
          ]
        }
        $$::jsonb,
        $$
        {
          "steps": [
            "Read current monitoring binding",
            "Trigger monitoring sync job",
            "Run source probe and collect latest status",
            "Report updated mapping and sync result"
          ]
        }
        $$::jsonb,
        '{"read_permission":"workflow.playbooks.read","execute_permission":"workflow.playbooks.write"}'::jsonb,
        TRUE,
        TRUE
    ),
    (
        'rotate-app-secret',
        'Rotate Application Secret',
        'Rotate high-risk credential with staged validation.',
        'security',
        'critical',
        TRUE,
        $$
        {
          "fields": [
            {"key":"asset_ref","label":"Asset / Host","type":"string","required":true,"max_length":128},
            {"key":"secret_ref","label":"Secret Reference","type":"string","required":true,"max_length":128},
            {"key":"ticket_no","label":"Change Ticket","type":"string","required":true,"max_length":64},
            {"key":"notify_channel","label":"Notify Channel","type":"string","required":false,"max_length":128}
          ]
        }
        $$::jsonb,
        $$
        {
          "steps": [
            "Validate change window and approver identity",
            "Generate rotated secret version",
            "Rollout secret to target service",
            "Run smoke checks and owner confirmation",
            "Archive previous secret version and audit result"
          ]
        }
        $$::jsonb,
        '{"read_permission":"workflow.playbooks.read","execute_permission":"workflow.playbooks.write"}'::jsonb,
        TRUE,
        TRUE
    ),
    (
        'collect-topology-diagnostics',
        'Collect Topology Diagnostics Snapshot',
        'Gather link-level diagnostics context for escalation.',
        'diagnostics',
        'medium',
        FALSE,
        $$
        {
          "fields": [
            {"key":"edge_id","label":"Topology Edge ID","type":"integer","required":true,"min":1,"max":2147483647},
            {"key":"window_minutes","label":"Window Minutes","type":"integer","required":false,"default":120,"min":15,"max":1440},
            {"key":"include_changes","label":"Include Recent Changes","type":"boolean","required":false,"default":true}
          ]
        }
        $$::jsonb,
        $$
        {
          "steps": [
            "Resolve source/target assets from topology edge",
            "Collect trend and active alerts around edge assets",
            "Collect recent changes and impacted owners/services",
            "Generate triage snapshot with quick links"
          ]
        }
        $$::jsonb,
        '{"read_permission":"workflow.playbooks.read","execute_permission":"workflow.playbooks.write"}'::jsonb,
        TRUE,
        TRUE
    )
ON CONFLICT (playbook_key) DO UPDATE
SET
    name = EXCLUDED.name,
    description = EXCLUDED.description,
    category = EXCLUDED.category,
    risk_level = EXCLUDED.risk_level,
    requires_confirmation = EXCLUDED.requires_confirmation,
    parameter_schema = EXCLUDED.parameter_schema,
    execution_plan = EXCLUDED.execution_plan,
    rbac_hint = EXCLUDED.rbac_hint,
    is_enabled = EXCLUDED.is_enabled,
    is_system = EXCLUDED.is_system,
    updated_at = NOW();
