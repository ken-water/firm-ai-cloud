CREATE TABLE IF NOT EXISTS ops_checklist_templates (
    id BIGSERIAL PRIMARY KEY,
    template_key VARCHAR(64) NOT NULL UNIQUE,
    title VARCHAR(128) NOT NULL,
    description VARCHAR(512),
    frequency VARCHAR(16) NOT NULL,
    due_weekday SMALLINT,
    guidance VARCHAR(1024),
    sort_order INTEGER NOT NULL DEFAULT 100,
    is_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_ops_checklist_templates_key_not_blank CHECK (btrim(template_key) <> ''),
    CONSTRAINT chk_ops_checklist_templates_title_not_blank CHECK (btrim(title) <> ''),
    CONSTRAINT chk_ops_checklist_templates_frequency CHECK (frequency IN ('daily', 'weekly')),
    CONSTRAINT chk_ops_checklist_templates_due_weekday CHECK (
        (
            frequency = 'daily'
            AND due_weekday IS NULL
        )
        OR (
            frequency = 'weekly'
            AND due_weekday BETWEEN 1 AND 7
        )
    )
);

CREATE INDEX IF NOT EXISTS idx_ops_checklist_templates_enabled
    ON ops_checklist_templates (is_enabled, sort_order, template_key);

CREATE TABLE IF NOT EXISTS ops_checklist_entries (
    id BIGSERIAL PRIMARY KEY,
    template_id BIGINT NOT NULL REFERENCES ops_checklist_templates (id) ON DELETE CASCADE,
    check_date DATE NOT NULL,
    operator VARCHAR(128) NOT NULL,
    site VARCHAR(128) NOT NULL DEFAULT '',
    department VARCHAR(128) NOT NULL DEFAULT '',
    status VARCHAR(16) NOT NULL,
    exception_note VARCHAR(1024),
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uq_ops_checklist_entries_scope UNIQUE (template_id, check_date, operator, site, department),
    CONSTRAINT chk_ops_checklist_entries_status CHECK (status IN ('pending', 'completed', 'skipped')),
    CONSTRAINT chk_ops_checklist_entries_operator_not_blank CHECK (btrim(operator) <> ''),
    CONSTRAINT chk_ops_checklist_entries_completed_at CHECK (
        (status = 'completed' AND completed_at IS NOT NULL)
        OR (status <> 'completed')
    )
);

CREATE INDEX IF NOT EXISTS idx_ops_checklist_entries_lookup
    ON ops_checklist_entries (check_date, operator, site, department, template_id);

INSERT INTO iam_permissions (permission_key, description)
VALUES
    ('ops.cockpit.write', 'Update cockpit checklist completion and exception records')
ON CONFLICT (permission_key) DO UPDATE
SET
    description = EXCLUDED.description,
    updated_at = NOW();

INSERT INTO iam_role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM iam_roles r
INNER JOIN iam_permissions p ON p.permission_key = 'ops.cockpit.write'
WHERE r.role_key IN ('operator', 'admin')
ON CONFLICT DO NOTHING;

INSERT INTO ops_checklist_templates (
    template_key,
    title,
    description,
    frequency,
    due_weekday,
    guidance,
    sort_order,
    is_enabled
)
VALUES
    (
        'daily-alert-queue-review',
        'Daily Alert Queue Review',
        'Review open and acknowledged alerts, prioritize critical items, and assign owner for each high-risk queue item.',
        'daily',
        NULL,
        'Use remediation dry-run first for high-risk alerts and keep ticket linkage current before shift handoff.',
        10,
        TRUE
    ),
    (
        'daily-monitoring-sync-backlog',
        'Daily Monitoring Sync Backlog Sweep',
        'Check failed or stale monitoring sync jobs and clear backlog before end of day.',
        'daily',
        NULL,
        'Retry dead-letter jobs only after validating endpoint/secret reachability and record exception note when deferred.',
        20,
        TRUE
    ),
    (
        'weekly-permission-break-glass-review',
        'Weekly Break-Glass Permission Review',
        'Validate break-glass users and emergency access mapping for current on-call roster.',
        'weekly',
        1,
        'Review AUTH_LOCAL_BREAK_GLASS_USERS and recent auth audit logs; file ticket for unexpected privilege drift.',
        110,
        TRUE
    ),
    (
        'weekly-capacity-risk-review',
        'Weekly Capacity Risk Review',
        'Review benchmark trend and outstanding capacity risks for next sprint planning.',
        'weekly',
        5,
        'Use latest benchmark gate/trend artifacts and capture deferred risks for release planning.',
        120,
        TRUE
    )
ON CONFLICT (template_key) DO UPDATE
SET
    title = EXCLUDED.title,
    description = EXCLUDED.description,
    frequency = EXCLUDED.frequency,
    due_weekday = EXCLUDED.due_weekday,
    guidance = EXCLUDED.guidance,
    sort_order = EXCLUDED.sort_order,
    is_enabled = EXCLUDED.is_enabled,
    updated_at = NOW();
