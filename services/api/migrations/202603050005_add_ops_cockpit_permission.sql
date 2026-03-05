INSERT INTO iam_permissions (permission_key, description)
VALUES
    ('ops.cockpit.read', 'Read daily operations cockpit prioritized queue')
ON CONFLICT (permission_key) DO UPDATE
SET
    description = EXCLUDED.description,
    updated_at = NOW();

INSERT INTO iam_role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM iam_roles r
INNER JOIN iam_permissions p ON p.permission_key = 'ops.cockpit.read'
WHERE r.role_key IN ('viewer', 'operator', 'admin')
ON CONFLICT DO NOTHING;
