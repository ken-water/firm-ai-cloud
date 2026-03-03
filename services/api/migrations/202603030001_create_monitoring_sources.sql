CREATE TABLE IF NOT EXISTS monitoring_sources (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(128) NOT NULL UNIQUE,
    source_type VARCHAR(32) NOT NULL,
    endpoint VARCHAR(512) NOT NULL,
    proxy_endpoint VARCHAR(512),
    auth_type VARCHAR(32) NOT NULL DEFAULT 'token',
    username VARCHAR(128),
    secret_ref VARCHAR(255) NOT NULL,
    site VARCHAR(64),
    department VARCHAR(64),
    is_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    last_probe_at TIMESTAMPTZ,
    last_probe_status VARCHAR(16),
    last_probe_message VARCHAR(512),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_monitoring_sources_name_not_blank CHECK (btrim(name) <> ''),
    CONSTRAINT chk_monitoring_sources_source_type CHECK (source_type IN ('zabbix')),
    CONSTRAINT chk_monitoring_sources_auth_type CHECK (auth_type IN ('token', 'basic')),
    CONSTRAINT chk_monitoring_sources_endpoint_not_blank CHECK (btrim(endpoint) <> ''),
    CONSTRAINT chk_monitoring_sources_secret_ref_not_blank CHECK (btrim(secret_ref) <> ''),
    CONSTRAINT chk_monitoring_sources_probe_status CHECK (
        last_probe_status IS NULL OR last_probe_status IN ('reachable', 'unreachable')
    )
);

CREATE INDEX IF NOT EXISTS idx_monitoring_sources_source_type
    ON monitoring_sources (source_type);

CREATE INDEX IF NOT EXISTS idx_monitoring_sources_site
    ON monitoring_sources (site);

CREATE INDEX IF NOT EXISTS idx_monitoring_sources_department
    ON monitoring_sources (department);

CREATE INDEX IF NOT EXISTS idx_monitoring_sources_is_enabled
    ON monitoring_sources (is_enabled);

INSERT INTO iam_permissions (permission_key, description)
VALUES
    ('monitoring.sources.read', 'Read monitoring source configuration and status'),
    ('monitoring.sources.write', 'Create and probe monitoring source configuration')
ON CONFLICT (permission_key) DO UPDATE
SET
    description = EXCLUDED.description,
    updated_at = NOW();

INSERT INTO iam_role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM iam_roles r
INNER JOIN iam_permissions p ON p.permission_key = 'monitoring.sources.read'
WHERE r.role_key = 'viewer'
ON CONFLICT DO NOTHING;

INSERT INTO iam_role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM iam_roles r
INNER JOIN iam_permissions p ON p.permission_key IN (
    'monitoring.sources.read',
    'monitoring.sources.write'
)
WHERE r.role_key = 'operator'
ON CONFLICT DO NOTHING;
