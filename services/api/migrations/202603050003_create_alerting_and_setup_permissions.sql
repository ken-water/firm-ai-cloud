CREATE TABLE IF NOT EXISTS unified_alerts (
    id BIGSERIAL PRIMARY KEY,
    alert_source VARCHAR(64) NOT NULL,
    alert_key VARCHAR(255) NOT NULL,
    dedup_key VARCHAR(255) NOT NULL,
    title VARCHAR(255) NOT NULL,
    severity VARCHAR(32) NOT NULL DEFAULT 'warning',
    status VARCHAR(32) NOT NULL DEFAULT 'open',
    site VARCHAR(128),
    department VARCHAR(128),
    asset_id BIGINT REFERENCES assets (id) ON DELETE SET NULL,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    first_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    acknowledged_by VARCHAR(128),
    acknowledged_at TIMESTAMPTZ,
    closed_by VARCHAR(128),
    closed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uq_unified_alerts_source_key UNIQUE (alert_source, alert_key),
    CONSTRAINT chk_unified_alerts_source_not_blank CHECK (btrim(alert_source) <> ''),
    CONSTRAINT chk_unified_alerts_key_not_blank CHECK (btrim(alert_key) <> ''),
    CONSTRAINT chk_unified_alerts_dedup_not_blank CHECK (btrim(dedup_key) <> ''),
    CONSTRAINT chk_unified_alerts_title_not_blank CHECK (btrim(title) <> ''),
    CONSTRAINT chk_unified_alerts_severity CHECK (severity IN ('critical', 'warning', 'info')),
    CONSTRAINT chk_unified_alerts_status CHECK (status IN ('open', 'acknowledged', 'closed')),
    CONSTRAINT chk_unified_alerts_payload_object CHECK (jsonb_typeof(payload) = 'object')
);

CREATE INDEX IF NOT EXISTS idx_unified_alerts_status_seen
    ON unified_alerts (status, last_seen_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_unified_alerts_severity_seen
    ON unified_alerts (severity, last_seen_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_unified_alerts_scope_seen
    ON unified_alerts (site, department, last_seen_at DESC, id DESC);

CREATE TABLE IF NOT EXISTS unified_alert_timeline (
    id BIGSERIAL PRIMARY KEY,
    alert_id BIGINT NOT NULL REFERENCES unified_alerts (id) ON DELETE CASCADE,
    event_type VARCHAR(32) NOT NULL,
    actor VARCHAR(128) NOT NULL,
    message VARCHAR(1024),
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_unified_alert_timeline_event_type CHECK (
        event_type IN ('observed', 'acknowledged', 'closed', 'reopened', 'ticket_created')
    ),
    CONSTRAINT chk_unified_alert_timeline_actor_not_blank CHECK (btrim(actor) <> ''),
    CONSTRAINT chk_unified_alert_timeline_metadata_object CHECK (jsonb_typeof(metadata) = 'object')
);

CREATE INDEX IF NOT EXISTS idx_unified_alert_timeline_alert_id_created_at
    ON unified_alert_timeline (alert_id, created_at DESC, id DESC);

CREATE TABLE IF NOT EXISTS alert_ticket_policies (
    id BIGSERIAL PRIMARY KEY,
    policy_key VARCHAR(64) NOT NULL UNIQUE,
    name VARCHAR(128) NOT NULL,
    description VARCHAR(512),
    is_system BOOLEAN NOT NULL DEFAULT FALSE,
    is_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    match_source VARCHAR(64),
    match_severity VARCHAR(32),
    match_site VARCHAR(128),
    match_department VARCHAR(128),
    match_status VARCHAR(32),
    dedup_window_seconds INTEGER NOT NULL DEFAULT 1800,
    ticket_priority VARCHAR(16) NOT NULL DEFAULT 'high',
    ticket_category VARCHAR(64) NOT NULL DEFAULT 'incident',
    workflow_template_id BIGINT REFERENCES workflow_templates (id) ON DELETE SET NULL,
    created_by VARCHAR(128) NOT NULL DEFAULT 'system',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_alert_ticket_policies_key_not_blank CHECK (btrim(policy_key) <> ''),
    CONSTRAINT chk_alert_ticket_policies_name_not_blank CHECK (btrim(name) <> ''),
    CONSTRAINT chk_alert_ticket_policies_match_severity CHECK (
        match_severity IS NULL OR match_severity IN ('critical', 'warning', 'info')
    ),
    CONSTRAINT chk_alert_ticket_policies_match_status CHECK (
        match_status IS NULL OR match_status IN ('open', 'acknowledged', 'closed')
    ),
    CONSTRAINT chk_alert_ticket_policies_priority CHECK (
        ticket_priority IN ('low', 'medium', 'high', 'critical')
    ),
    CONSTRAINT chk_alert_ticket_policies_dedup_window CHECK (
        dedup_window_seconds >= 30 AND dedup_window_seconds <= 604800
    ),
    CONSTRAINT chk_alert_ticket_policies_created_by_not_blank CHECK (btrim(created_by) <> '')
);

CREATE INDEX IF NOT EXISTS idx_alert_ticket_policies_enabled
    ON alert_ticket_policies (is_enabled, match_source, match_severity);

CREATE TABLE IF NOT EXISTS alert_policy_actions (
    id BIGSERIAL PRIMARY KEY,
    policy_id BIGINT NOT NULL REFERENCES alert_ticket_policies (id) ON DELETE CASCADE,
    alert_id BIGINT NOT NULL REFERENCES unified_alerts (id) ON DELETE CASCADE,
    action VARCHAR(32) NOT NULL,
    ticket_id BIGINT REFERENCES tickets (id) ON DELETE SET NULL,
    message VARCHAR(1024),
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_alert_policy_actions_action CHECK (
        action IN ('ticket_created', 'suppressed')
    ),
    CONSTRAINT chk_alert_policy_actions_metadata_object CHECK (jsonb_typeof(metadata) = 'object')
);

CREATE INDEX IF NOT EXISTS idx_alert_policy_actions_policy_alert_created
    ON alert_policy_actions (policy_id, alert_id, created_at DESC, id DESC);

INSERT INTO iam_permissions (permission_key, description)
VALUES
    ('ops.setup.read', 'Read setup preflight and integration checklist status'),
    ('alerts.read', 'Read unified alerts and alert timeline'),
    ('alerts.write', 'Acknowledge/close alerts and manage alert automation policies')
ON CONFLICT (permission_key) DO UPDATE
SET
    description = EXCLUDED.description,
    updated_at = NOW();

INSERT INTO iam_role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM iam_roles r
INNER JOIN iam_permissions p ON p.permission_key IN ('ops.setup.read', 'alerts.read')
WHERE r.role_key = 'viewer'
ON CONFLICT DO NOTHING;

INSERT INTO iam_role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM iam_roles r
INNER JOIN iam_permissions p ON p.permission_key IN ('ops.setup.read', 'alerts.read', 'alerts.write')
WHERE r.role_key = 'operator'
ON CONFLICT DO NOTHING;

INSERT INTO iam_role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM iam_roles r
INNER JOIN iam_permissions p ON p.permission_key IN ('ops.setup.read', 'alerts.read', 'alerts.write')
WHERE r.role_key = 'admin'
ON CONFLICT DO NOTHING;

INSERT INTO alert_ticket_policies (
    policy_key,
    name,
    description,
    is_system,
    is_enabled,
    match_source,
    match_severity,
    dedup_window_seconds,
    ticket_priority,
    ticket_category,
    created_by
)
VALUES
    (
        'critical-infrastructure',
        'Critical Infrastructure Incident',
        'Create a critical incident ticket when monitoring sync emits a critical alert.',
        TRUE,
        TRUE,
        'monitoring_sync',
        'critical',
        1800,
        'critical',
        'incident',
        'system'
    ),
    (
        'service-degradation',
        'Service Degradation Incident',
        'Create a high-priority incident ticket when monitoring sync emits a warning alert.',
        TRUE,
        TRUE,
        'monitoring_sync',
        'warning',
        1800,
        'high',
        'incident',
        'system'
    ),
    (
        'repeated-failure',
        'Repeated Failure Follow-up',
        'Template for repeated failure escalation (disabled by default).',
        TRUE,
        FALSE,
        'monitoring_sync',
        'critical',
        3600,
        'high',
        'incident',
        'system'
    )
ON CONFLICT (policy_key) DO UPDATE
SET
    name = EXCLUDED.name,
    description = EXCLUDED.description,
    is_system = EXCLUDED.is_system,
    match_source = EXCLUDED.match_source,
    match_severity = EXCLUDED.match_severity,
    dedup_window_seconds = EXCLUDED.dedup_window_seconds,
    ticket_priority = EXCLUDED.ticket_priority,
    ticket_category = EXCLUDED.ticket_category,
    updated_at = NOW();
