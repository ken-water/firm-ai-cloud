INSERT INTO iam_permissions (permission_key, description)
VALUES
    ('tickets.read', 'Read ticket records and links'),
    ('tickets.write', 'Create tickets and update ticket status')
ON CONFLICT (permission_key) DO UPDATE
SET
    description = EXCLUDED.description,
    updated_at = NOW();

INSERT INTO iam_role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM iam_roles r
INNER JOIN iam_permissions p ON p.permission_key = 'tickets.read'
WHERE r.role_key = 'viewer'
ON CONFLICT DO NOTHING;

INSERT INTO iam_role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM iam_roles r
INNER JOIN iam_permissions p ON p.permission_key IN ('tickets.read', 'tickets.write')
WHERE r.role_key = 'operator'
ON CONFLICT DO NOTHING;
