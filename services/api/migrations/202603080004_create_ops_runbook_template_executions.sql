CREATE TABLE IF NOT EXISTS ops_runbook_template_executions (
    id BIGSERIAL PRIMARY KEY,
    template_key VARCHAR(64) NOT NULL,
    template_name VARCHAR(128) NOT NULL,
    status VARCHAR(16) NOT NULL,
    actor VARCHAR(128) NOT NULL,
    params JSONB NOT NULL DEFAULT '{}'::jsonb,
    preflight JSONB NOT NULL DEFAULT '{}'::jsonb,
    timeline JSONB NOT NULL DEFAULT '[]'::jsonb,
    evidence JSONB NOT NULL DEFAULT '{}'::jsonb,
    remediation_hints JSONB NOT NULL DEFAULT '[]'::jsonb,
    note VARCHAR(1024),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_ops_runbook_template_executions_template_key_not_blank CHECK (btrim(template_key) <> ''),
    CONSTRAINT chk_ops_runbook_template_executions_template_name_not_blank CHECK (btrim(template_name) <> ''),
    CONSTRAINT chk_ops_runbook_template_executions_status CHECK (status IN ('succeeded', 'failed')),
    CONSTRAINT chk_ops_runbook_template_executions_actor_not_blank CHECK (btrim(actor) <> ''),
    CONSTRAINT chk_ops_runbook_template_executions_params_object CHECK (jsonb_typeof(params) = 'object'),
    CONSTRAINT chk_ops_runbook_template_executions_preflight_object CHECK (jsonb_typeof(preflight) = 'object'),
    CONSTRAINT chk_ops_runbook_template_executions_timeline_array CHECK (jsonb_typeof(timeline) = 'array'),
    CONSTRAINT chk_ops_runbook_template_executions_evidence_object CHECK (jsonb_typeof(evidence) = 'object'),
    CONSTRAINT chk_ops_runbook_template_executions_remediation_hints_array CHECK (jsonb_typeof(remediation_hints) = 'array')
);

CREATE INDEX IF NOT EXISTS idx_ops_runbook_template_executions_template_created
    ON ops_runbook_template_executions (template_key, created_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_ops_runbook_template_executions_status_created
    ON ops_runbook_template_executions (status, created_at DESC, id DESC);
