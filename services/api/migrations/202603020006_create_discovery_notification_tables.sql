CREATE TABLE IF NOT EXISTS discovery_notification_channels (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(128) NOT NULL,
    channel_type VARCHAR(32) NOT NULL,
    target VARCHAR(512) NOT NULL,
    config JSONB NOT NULL DEFAULT '{}'::jsonb,
    is_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_discovery_notification_channels_config_object CHECK (jsonb_typeof(config) = 'object')
);

CREATE TABLE IF NOT EXISTS discovery_notification_templates (
    id BIGSERIAL PRIMARY KEY,
    event_type VARCHAR(64) NOT NULL,
    title_template TEXT NOT NULL,
    body_template TEXT NOT NULL,
    is_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS discovery_notification_subscriptions (
    id BIGSERIAL PRIMARY KEY,
    channel_id BIGINT NOT NULL REFERENCES discovery_notification_channels (id) ON DELETE CASCADE,
    event_type VARCHAR(64) NOT NULL,
    site VARCHAR(128),
    department VARCHAR(128),
    is_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (channel_id, event_type, site, department)
);

CREATE INDEX IF NOT EXISTS idx_discovery_notification_channels_type
    ON discovery_notification_channels (channel_type);

CREATE INDEX IF NOT EXISTS idx_discovery_notification_templates_event_type
    ON discovery_notification_templates (event_type);

CREATE INDEX IF NOT EXISTS idx_discovery_notification_subscriptions_event_type
    ON discovery_notification_subscriptions (event_type);

CREATE INDEX IF NOT EXISTS idx_discovery_notification_subscriptions_channel_id
    ON discovery_notification_subscriptions (channel_id);
