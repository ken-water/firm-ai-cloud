CREATE TABLE IF NOT EXISTS ops_runbook_risk_alert_notification_deliveries (
    id BIGSERIAL PRIMARY KEY,
    source_key TEXT NOT NULL,
    template_key TEXT NOT NULL,
    execution_mode TEXT,
    window_days INTEGER NOT NULL,
    ticket_id BIGINT NOT NULL REFERENCES tickets (id) ON DELETE CASCADE,
    ticket_no TEXT NOT NULL,
    event_type TEXT NOT NULL,
    dispatch_status TEXT NOT NULL,
    subscription_id BIGINT REFERENCES discovery_notification_subscriptions (id) ON DELETE SET NULL,
    channel_id BIGINT REFERENCES discovery_notification_channels (id) ON DELETE SET NULL,
    channel_type TEXT,
    target TEXT NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0,
    response_code INTEGER,
    last_error TEXT,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    delivered_at TIMESTAMPTZ,
    created_by TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_ops_runbook_risk_alert_notification_deliveries_window_days_positive
        CHECK (window_days >= 1 AND window_days <= 90),
    CONSTRAINT chk_ops_runbook_risk_alert_notification_deliveries_dispatch_status
        CHECK (dispatch_status IN ('queued', 'delivered', 'failed', 'skipped')),
    CONSTRAINT chk_ops_runbook_risk_alert_notification_deliveries_attempts_nonnegative
        CHECK (attempts >= 0),
    CONSTRAINT chk_ops_runbook_risk_alert_notification_deliveries_payload_object
        CHECK (jsonb_typeof(payload) = 'object')
);

CREATE INDEX IF NOT EXISTS idx_ops_runbook_risk_alert_notification_deliveries_source_key
    ON ops_runbook_risk_alert_notification_deliveries (source_key, created_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_ops_runbook_risk_alert_notification_deliveries_template_scope
    ON ops_runbook_risk_alert_notification_deliveries (template_key, execution_mode, window_days);

CREATE INDEX IF NOT EXISTS idx_ops_runbook_risk_alert_notification_deliveries_ticket_id
    ON ops_runbook_risk_alert_notification_deliveries (ticket_id);

CREATE INDEX IF NOT EXISTS idx_ops_runbook_risk_alert_notification_deliveries_status
    ON ops_runbook_risk_alert_notification_deliveries (dispatch_status);
