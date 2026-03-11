CREATE TABLE IF NOT EXISTS ops_runbook_risk_owner_directory (
    id BIGSERIAL PRIMARY KEY,
    owner_key VARCHAR(64) NOT NULL UNIQUE,
    display_name VARCHAR(128) NOT NULL,
    owner_type VARCHAR(32) NOT NULL,
    owner_ref VARCHAR(128) NOT NULL,
    notification_target VARCHAR(512),
    note TEXT,
    is_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    updated_by VARCHAR(128) NOT NULL DEFAULT 'system',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_ops_runbook_risk_owner_directory_owner_type CHECK (
        owner_type IN ('team', 'user', 'group', 'external')
    ),
    CONSTRAINT chk_ops_runbook_risk_owner_directory_owner_key_not_blank CHECK (btrim(owner_key) <> ''),
    CONSTRAINT chk_ops_runbook_risk_owner_directory_display_name_not_blank CHECK (btrim(display_name) <> ''),
    CONSTRAINT chk_ops_runbook_risk_owner_directory_owner_ref_not_blank CHECK (btrim(owner_ref) <> '')
);

CREATE INDEX IF NOT EXISTS idx_ops_runbook_risk_owner_directory_enabled
    ON ops_runbook_risk_owner_directory (is_enabled, display_name, owner_key);

CREATE TABLE IF NOT EXISTS ops_runbook_risk_owner_routing_rules (
    id BIGSERIAL PRIMARY KEY,
    template_key VARCHAR(64) NOT NULL,
    execution_mode VARCHAR(16),
    severity VARCHAR(32),
    owner_key VARCHAR(64) NOT NULL REFERENCES ops_runbook_risk_owner_directory (owner_key) ON DELETE CASCADE,
    priority INTEGER NOT NULL DEFAULT 100,
    note TEXT,
    is_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    updated_by VARCHAR(128) NOT NULL DEFAULT 'system',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_ops_runbook_risk_owner_routing_rules_template_key_not_blank CHECK (btrim(template_key) <> ''),
    CONSTRAINT chk_ops_runbook_risk_owner_routing_rules_execution_mode CHECK (
        execution_mode IS NULL OR execution_mode IN ('simulate', 'live')
    ),
    CONSTRAINT chk_ops_runbook_risk_owner_routing_rules_severity CHECK (
        severity IS NULL OR severity IN ('warning', 'critical')
    ),
    CONSTRAINT chk_ops_runbook_risk_owner_routing_rules_priority CHECK (
        priority >= 1 AND priority <= 1000
    )
);

CREATE INDEX IF NOT EXISTS idx_ops_runbook_risk_owner_routing_rules_match
    ON ops_runbook_risk_owner_routing_rules (
        template_key,
        execution_mode,
        severity,
        priority,
        is_enabled
    );
