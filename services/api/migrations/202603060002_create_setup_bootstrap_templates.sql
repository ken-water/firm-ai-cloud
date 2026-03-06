CREATE TABLE IF NOT EXISTS setup_bootstrap_templates (
    id BIGSERIAL PRIMARY KEY,
    template_key VARCHAR(64) NOT NULL UNIQUE,
    name VARCHAR(128) NOT NULL,
    category VARCHAR(32) NOT NULL,
    description VARCHAR(512),
    param_schema JSONB NOT NULL DEFAULT '{"fields":[]}'::jsonb,
    apply_plan JSONB NOT NULL DEFAULT '{"actions":[]}'::jsonb,
    rollback_hints JSONB NOT NULL DEFAULT '[]'::jsonb,
    is_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    is_system BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_setup_bootstrap_templates_key_not_blank CHECK (btrim(template_key) <> ''),
    CONSTRAINT chk_setup_bootstrap_templates_name_not_blank CHECK (btrim(name) <> ''),
    CONSTRAINT chk_setup_bootstrap_templates_category_not_blank CHECK (btrim(category) <> ''),
    CONSTRAINT chk_setup_bootstrap_templates_param_schema_object CHECK (jsonb_typeof(param_schema) = 'object'),
    CONSTRAINT chk_setup_bootstrap_templates_apply_plan_object CHECK (jsonb_typeof(apply_plan) = 'object'),
    CONSTRAINT chk_setup_bootstrap_templates_rollback_array CHECK (jsonb_typeof(rollback_hints) = 'array')
);

CREATE INDEX IF NOT EXISTS idx_setup_bootstrap_templates_category
    ON setup_bootstrap_templates (category, is_enabled, template_key);

CREATE TABLE IF NOT EXISTS setup_identity_preferences (
    id SMALLINT PRIMARY KEY DEFAULT 1,
    identity_mode VARCHAR(32) NOT NULL,
    break_glass_users JSONB NOT NULL DEFAULT '[]'::jsonb,
    updated_by VARCHAR(128) NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_setup_identity_preferences_singleton CHECK (id = 1),
    CONSTRAINT chk_setup_identity_preferences_mode CHECK (
        identity_mode IN ('break_glass_only', 'disabled', 'allow_all')
    ),
    CONSTRAINT chk_setup_identity_preferences_users_array CHECK (
        jsonb_typeof(break_glass_users) = 'array'
    ),
    CONSTRAINT chk_setup_identity_preferences_updated_by_not_blank CHECK (btrim(updated_by) <> '')
);

CREATE TABLE IF NOT EXISTS setup_bootstrap_template_runs (
    id BIGSERIAL PRIMARY KEY,
    template_id BIGINT NOT NULL REFERENCES setup_bootstrap_templates (id) ON DELETE CASCADE,
    template_key VARCHAR(64) NOT NULL,
    actor VARCHAR(128) NOT NULL,
    status VARCHAR(16) NOT NULL,
    params JSONB NOT NULL DEFAULT '{}'::jsonb,
    result JSONB NOT NULL DEFAULT '{}'::jsonb,
    error_message VARCHAR(1024),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_setup_bootstrap_template_runs_status CHECK (
        status IN ('applied', 'failed')
    ),
    CONSTRAINT chk_setup_bootstrap_template_runs_params_object CHECK (jsonb_typeof(params) = 'object'),
    CONSTRAINT chk_setup_bootstrap_template_runs_result_object CHECK (jsonb_typeof(result) = 'object'),
    CONSTRAINT chk_setup_bootstrap_template_runs_actor_not_blank CHECK (btrim(actor) <> '')
);

CREATE INDEX IF NOT EXISTS idx_setup_bootstrap_template_runs_template
    ON setup_bootstrap_template_runs (template_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_setup_bootstrap_template_runs_actor
    ON setup_bootstrap_template_runs (actor, created_at DESC);

INSERT INTO iam_permissions (permission_key, description)
VALUES
    ('ops.setup.write', 'Apply setup bootstrap templates and persisted onboarding defaults')
ON CONFLICT (permission_key) DO UPDATE
SET
    description = EXCLUDED.description,
    updated_at = NOW();

INSERT INTO iam_role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM iam_roles r
INNER JOIN iam_permissions p ON p.permission_key = 'ops.setup.write'
WHERE r.role_key IN ('operator', 'admin')
ON CONFLICT DO NOTHING;

INSERT INTO setup_bootstrap_templates (
    template_key,
    name,
    category,
    description,
    param_schema,
    apply_plan,
    rollback_hints,
    is_enabled,
    is_system
)
VALUES
    (
        'identity-safe-baseline',
        'Identity Safe Baseline',
        'identity',
        'Persist preferred local fallback mode and break-glass account list for onboarding governance.',
        $$
        {
          "fields": [
            {
              "key": "identity_mode",
              "label": "Identity Mode",
              "type": "enum",
              "required": true,
              "options": ["break_glass_only", "disabled", "allow_all"],
              "default": "break_glass_only"
            },
            {
              "key": "break_glass_users",
              "label": "Break-Glass Users",
              "type": "string",
              "required": false,
              "max_length": 1024,
              "placeholder": "admin,ops.emergency"
            }
          ]
        }
        $$::jsonb,
        $$
        {
          "actions": [
            "Persist setup identity preference profile",
            "Record break-glass user allowlist recommendation"
          ]
        }
        $$::jsonb,
        $$
        [
          "If rollback is needed, re-apply with identity_mode=allow_all for temporary local access.",
          "After rollback, verify AUTH_LOCAL_FALLBACK_MODE and AUTH_LOCAL_BREAK_GLASS_USERS env values."
        ]
        $$::jsonb,
        TRUE,
        TRUE
    ),
    (
        'monitoring-zabbix-bootstrap',
        'Monitoring Source Bootstrap (Zabbix)',
        'monitoring',
        'Create or update one baseline Zabbix monitoring source without manual JSON editing.',
        $$
        {
          "fields": [
            {
              "key": "name",
              "label": "Source Name",
              "type": "string",
              "required": true,
              "max_length": 128,
              "placeholder": "zabbix-core"
            },
            {
              "key": "endpoint",
              "label": "API Endpoint",
              "type": "string",
              "required": true,
              "max_length": 512,
              "placeholder": "http://127.0.0.1:8082/api_jsonrpc.php"
            },
            {
              "key": "auth_type",
              "label": "Auth Type",
              "type": "enum",
              "required": false,
              "options": ["token", "basic"],
              "default": "token"
            },
            {
              "key": "username",
              "label": "Username (for basic auth)",
              "type": "string",
              "required": false,
              "max_length": 128,
              "placeholder": "Admin"
            },
            {
              "key": "secret_ref",
              "label": "Secret Ref",
              "type": "string",
              "required": true,
              "max_length": 255,
              "placeholder": "env:ZABBIX_API_TOKEN"
            },
            {
              "key": "site",
              "label": "Site",
              "type": "string",
              "required": false,
              "max_length": 64,
              "placeholder": "dc-a"
            },
            {
              "key": "department",
              "label": "Department",
              "type": "string",
              "required": false,
              "max_length": 64,
              "placeholder": "platform"
            }
          ]
        }
        $$::jsonb,
        $$
        {
          "actions": [
            "Create or update monitoring source",
            "Mark monitoring source as enabled"
          ]
        }
        $$::jsonb,
        $$
        [
          "Rollback by disabling or deleting the created monitoring source from Monitoring Sources page.",
          "If probe fails after apply, verify endpoint reachability and env secret reference."
        ]
        $$::jsonb,
        TRUE,
        TRUE
    ),
    (
        'notification-oncall-bootstrap',
        'Notification On-Call Bootstrap',
        'notification',
        'Create or update one channel/template/subscription tuple for on-call notification baseline.',
        $$
        {
          "fields": [
            {
              "key": "channel_name",
              "label": "Channel Name",
              "type": "string",
              "required": true,
              "max_length": 128,
              "placeholder": "oncall-webhook"
            },
            {
              "key": "channel_type",
              "label": "Channel Type",
              "type": "enum",
              "required": false,
              "options": ["webhook", "email"],
              "default": "webhook"
            },
            {
              "key": "target",
              "label": "Target",
              "type": "string",
              "required": true,
              "max_length": 512,
              "placeholder": "https://ops.example.local/hooks/cloudops"
            },
            {
              "key": "event_type",
              "label": "Event Type",
              "type": "string",
              "required": true,
              "max_length": 64,
              "placeholder": "asset.offboarded_suspected"
            },
            {
              "key": "title_template",
              "label": "Title Template",
              "type": "string",
              "required": false,
              "max_length": 512,
              "placeholder": "Discovery Event: {{event_type}}"
            },
            {
              "key": "body_template",
              "label": "Body Template",
              "type": "string",
              "required": false,
              "max_length": 512,
              "placeholder": "{{payload}}"
            },
            {
              "key": "site",
              "label": "Site",
              "type": "string",
              "required": false,
              "max_length": 128,
              "placeholder": "dc-a"
            },
            {
              "key": "department",
              "label": "Department",
              "type": "string",
              "required": false,
              "max_length": 128,
              "placeholder": "platform"
            }
          ]
        }
        $$::jsonb,
        $$
        {
          "actions": [
            "Create or update notification channel",
            "Create or update notification template",
            "Create or update notification subscription"
          ]
        }
        $$::jsonb,
        $$
        [
          "Rollback by disabling the subscription or channel in Notifications section.",
          "If duplicate notifications are observed, keep one subscription per channel/event/scope."
        ]
        $$::jsonb,
        TRUE,
        TRUE
    )
ON CONFLICT (template_key) DO UPDATE
SET
    name = EXCLUDED.name,
    category = EXCLUDED.category,
    description = EXCLUDED.description,
    param_schema = EXCLUDED.param_schema,
    apply_plan = EXCLUDED.apply_plan,
    rollback_hints = EXCLUDED.rollback_hints,
    is_enabled = EXCLUDED.is_enabled,
    is_system = EXCLUDED.is_system,
    updated_at = NOW();
