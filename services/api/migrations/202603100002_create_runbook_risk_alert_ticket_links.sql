CREATE TABLE IF NOT EXISTS ops_runbook_risk_alert_ticket_links (
    id BIGSERIAL PRIMARY KEY,
    template_key VARCHAR(64) NOT NULL,
    execution_mode VARCHAR(16),
    window_days INTEGER NOT NULL DEFAULT 14,
    source_key VARCHAR(160) NOT NULL UNIQUE,
    status VARCHAR(16) NOT NULL DEFAULT 'open',
    ticket_id BIGINT NOT NULL REFERENCES tickets (id) ON DELETE CASCADE,
    note VARCHAR(1024),
    created_by VARCHAR(128) NOT NULL DEFAULT 'system',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_ops_runbook_risk_alert_ticket_links_template_key_not_blank
        CHECK (btrim(template_key) <> ''),
    CONSTRAINT chk_ops_runbook_risk_alert_ticket_links_execution_mode
        CHECK (execution_mode IS NULL OR execution_mode IN ('simulate', 'live')),
    CONSTRAINT chk_ops_runbook_risk_alert_ticket_links_window_days_range
        CHECK (window_days BETWEEN 1 AND 90),
    CONSTRAINT chk_ops_runbook_risk_alert_ticket_links_source_key_not_blank
        CHECK (btrim(source_key) <> ''),
    CONSTRAINT chk_ops_runbook_risk_alert_ticket_links_status
        CHECK (status IN ('open', 'in_progress', 'resolved', 'closed', 'cancelled')),
    CONSTRAINT chk_ops_runbook_risk_alert_ticket_links_created_by_not_blank
        CHECK (btrim(created_by) <> '')
);

CREATE INDEX IF NOT EXISTS idx_ops_runbook_risk_alert_ticket_links_scope
    ON ops_runbook_risk_alert_ticket_links (
        template_key,
        execution_mode,
        window_days,
        status,
        updated_at DESC,
        id DESC
    );

CREATE INDEX IF NOT EXISTS idx_ops_runbook_risk_alert_ticket_links_ticket_id
    ON ops_runbook_risk_alert_ticket_links (ticket_id, updated_at DESC, id DESC);
