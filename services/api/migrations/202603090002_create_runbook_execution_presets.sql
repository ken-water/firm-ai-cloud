CREATE TABLE IF NOT EXISTS ops_runbook_execution_presets (
    id BIGSERIAL PRIMARY KEY,
    template_key VARCHAR(64) NOT NULL,
    template_name VARCHAR(128) NOT NULL,
    name VARCHAR(128) NOT NULL,
    description VARCHAR(512),
    execution_mode VARCHAR(16) NOT NULL DEFAULT 'simulate',
    params JSONB NOT NULL DEFAULT '{}'::jsonb,
    preflight_confirmations JSONB NOT NULL DEFAULT '[]'::jsonb,
    updated_by VARCHAR(128) NOT NULL DEFAULT 'system',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_ops_runbook_execution_presets_template_key_not_blank
        CHECK (btrim(template_key) <> ''),
    CONSTRAINT chk_ops_runbook_execution_presets_template_name_not_blank
        CHECK (btrim(template_name) <> ''),
    CONSTRAINT chk_ops_runbook_execution_presets_name_not_blank
        CHECK (btrim(name) <> ''),
    CONSTRAINT chk_ops_runbook_execution_presets_execution_mode
        CHECK (execution_mode IN ('simulate', 'live')),
    CONSTRAINT chk_ops_runbook_execution_presets_params_object
        CHECK (jsonb_typeof(params) = 'object'),
    CONSTRAINT chk_ops_runbook_execution_presets_preflight_array
        CHECK (jsonb_typeof(preflight_confirmations) = 'array'),
    CONSTRAINT chk_ops_runbook_execution_presets_updated_by_not_blank
        CHECK (btrim(updated_by) <> '')
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_ops_runbook_execution_presets_template_name
    ON ops_runbook_execution_presets (template_key, name);

CREATE INDEX IF NOT EXISTS idx_ops_runbook_execution_presets_template_key
    ON ops_runbook_execution_presets (template_key, updated_at DESC, id DESC);
