CREATE TABLE IF NOT EXISTS iam_users (
    id BIGSERIAL PRIMARY KEY,
    username VARCHAR(128) NOT NULL UNIQUE,
    display_name VARCHAR(128),
    email VARCHAR(256),
    auth_source VARCHAR(32) NOT NULL DEFAULT 'local',
    is_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_iam_users_username_not_blank CHECK (btrim(username) <> '')
);

CREATE TABLE IF NOT EXISTS iam_roles (
    id BIGSERIAL PRIMARY KEY,
    role_key VARCHAR(64) NOT NULL UNIQUE,
    name VARCHAR(128) NOT NULL,
    is_system BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_iam_roles_role_key_not_blank CHECK (btrim(role_key) <> '')
);

CREATE TABLE IF NOT EXISTS iam_permissions (
    id BIGSERIAL PRIMARY KEY,
    permission_key VARCHAR(128) NOT NULL UNIQUE,
    description VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_iam_permissions_permission_key_not_blank CHECK (btrim(permission_key) <> '')
);

CREATE TABLE IF NOT EXISTS iam_role_permissions (
    role_id BIGINT NOT NULL REFERENCES iam_roles (id) ON DELETE CASCADE,
    permission_id BIGINT NOT NULL REFERENCES iam_permissions (id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (role_id, permission_id)
);

CREATE TABLE IF NOT EXISTS iam_user_roles (
    user_id BIGINT NOT NULL REFERENCES iam_users (id) ON DELETE CASCADE,
    role_id BIGINT NOT NULL REFERENCES iam_roles (id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, role_id)
);

CREATE INDEX IF NOT EXISTS idx_iam_user_roles_user_id
    ON iam_user_roles (user_id);

CREATE INDEX IF NOT EXISTS idx_iam_user_roles_role_id
    ON iam_user_roles (role_id);

CREATE INDEX IF NOT EXISTS idx_iam_role_permissions_role_id
    ON iam_role_permissions (role_id);

CREATE INDEX IF NOT EXISTS idx_iam_role_permissions_permission_id
    ON iam_role_permissions (permission_id);

INSERT INTO iam_roles (role_key, name, is_system)
VALUES
    ('admin', 'Administrator', TRUE),
    ('operator', 'Operator', TRUE),
    ('viewer', 'Viewer', TRUE)
ON CONFLICT (role_key) DO UPDATE
SET
    name = EXCLUDED.name,
    is_system = EXCLUDED.is_system,
    updated_at = NOW();

INSERT INTO iam_permissions (permission_key, description)
VALUES
    ('system.admin', 'Global administrator permission'),
    ('cmdb.assets.read', 'Read CMDB assets'),
    ('cmdb.assets.write', 'Create/update CMDB assets'),
    ('cmdb.field_definitions.read', 'Read CMDB custom field definitions'),
    ('cmdb.field_definitions.write', 'Create/update CMDB custom field definitions'),
    ('cmdb.relations.read', 'Read CMDB asset relations'),
    ('cmdb.relations.write', 'Create/update CMDB asset relations'),
    ('cmdb.discovery.read', 'Read discovery jobs, candidates, events, and delivery logs'),
    ('cmdb.discovery.write', 'Run and review discovery operations'),
    ('cmdb.notifications.read', 'Read notification channels/templates/subscriptions'),
    ('cmdb.notifications.write', 'Create notification channels/templates/subscriptions'),
    ('workflow.requests.read', 'Read workflow requests'),
    ('workflow.requests.write', 'Create/update workflow requests'),
    ('workflow.approvals.read', 'Read workflow approvals'),
    ('workflow.approvals.write', 'Approve/reject workflow requests')
ON CONFLICT (permission_key) DO UPDATE
SET
    description = EXCLUDED.description,
    updated_at = NOW();

INSERT INTO iam_role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM iam_roles r
INNER JOIN iam_permissions p ON p.permission_key = 'system.admin'
WHERE r.role_key = 'admin'
ON CONFLICT DO NOTHING;

INSERT INTO iam_role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM iam_roles r
INNER JOIN iam_permissions p ON p.permission_key IN (
    'cmdb.assets.read',
    'cmdb.field_definitions.read',
    'cmdb.relations.read',
    'cmdb.discovery.read',
    'cmdb.notifications.read',
    'workflow.requests.read',
    'workflow.approvals.read'
)
WHERE r.role_key = 'viewer'
ON CONFLICT DO NOTHING;

INSERT INTO iam_role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM iam_roles r
INNER JOIN iam_permissions p ON p.permission_key IN (
    'cmdb.assets.read',
    'cmdb.assets.write',
    'cmdb.field_definitions.read',
    'cmdb.field_definitions.write',
    'cmdb.relations.read',
    'cmdb.relations.write',
    'cmdb.discovery.read',
    'cmdb.discovery.write',
    'cmdb.notifications.read',
    'cmdb.notifications.write',
    'workflow.requests.read',
    'workflow.requests.write',
    'workflow.approvals.read',
    'workflow.approvals.write'
)
WHERE r.role_key = 'operator'
ON CONFLICT DO NOTHING;

INSERT INTO iam_users (username, display_name, auth_source, is_enabled)
VALUES ('admin', 'Bootstrap Administrator', 'bootstrap', TRUE)
ON CONFLICT (username) DO UPDATE
SET
    display_name = EXCLUDED.display_name,
    is_enabled = TRUE,
    updated_at = NOW();

INSERT INTO iam_user_roles (user_id, role_id)
SELECT u.id, r.id
FROM iam_users u
INNER JOIN iam_roles r ON r.role_key = 'admin'
WHERE u.username = 'admin'
ON CONFLICT DO NOTHING;
