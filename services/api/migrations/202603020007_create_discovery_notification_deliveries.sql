CREATE TABLE IF NOT EXISTS discovery_notification_deliveries (
    id BIGSERIAL PRIMARY KEY,
    event_id BIGINT NOT NULL REFERENCES discovery_events (id) ON DELETE CASCADE,
    subscription_id BIGINT REFERENCES discovery_notification_subscriptions (id) ON DELETE SET NULL,
    channel_id BIGINT REFERENCES discovery_notification_channels (id) ON DELETE SET NULL,
    target VARCHAR(512) NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'queued',
    attempts INTEGER NOT NULL DEFAULT 0,
    response_code INTEGER,
    last_error TEXT,
    delivered_at TIMESTAMPTZ,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_discovery_notification_deliveries_payload_object CHECK (jsonb_typeof(payload) = 'object'),
    CONSTRAINT chk_discovery_notification_deliveries_attempts_nonnegative CHECK (attempts >= 0)
);

CREATE INDEX IF NOT EXISTS idx_discovery_notification_deliveries_event_id
    ON discovery_notification_deliveries (event_id);

CREATE INDEX IF NOT EXISTS idx_discovery_notification_deliveries_status
    ON discovery_notification_deliveries (status);

CREATE INDEX IF NOT EXISTS idx_discovery_notification_deliveries_created_at
    ON discovery_notification_deliveries (created_at DESC);
