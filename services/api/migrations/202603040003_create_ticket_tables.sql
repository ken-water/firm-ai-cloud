CREATE TABLE IF NOT EXISTS tickets (
    id BIGSERIAL PRIMARY KEY,
    ticket_no VARCHAR(32) NOT NULL UNIQUE,
    title VARCHAR(255) NOT NULL,
    description TEXT,
    status VARCHAR(32) NOT NULL DEFAULT 'open',
    priority VARCHAR(16) NOT NULL DEFAULT 'medium',
    category VARCHAR(64) NOT NULL DEFAULT 'general',
    requester VARCHAR(128) NOT NULL,
    assignee VARCHAR(128),
    workflow_template_id BIGINT REFERENCES workflow_templates (id) ON DELETE SET NULL,
    workflow_request_id BIGINT REFERENCES workflow_requests (id) ON DELETE SET NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    last_status_note TEXT,
    closed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_tickets_status CHECK (status IN ('open', 'in_progress', 'resolved', 'closed', 'cancelled')),
    CONSTRAINT chk_tickets_priority CHECK (priority IN ('low', 'medium', 'high', 'critical')),
    CONSTRAINT chk_tickets_title_not_blank CHECK (btrim(title) <> ''),
    CONSTRAINT chk_tickets_requester_not_blank CHECK (btrim(requester) <> ''),
    CONSTRAINT chk_tickets_metadata_object CHECK (jsonb_typeof(metadata) = 'object')
);

CREATE INDEX IF NOT EXISTS idx_tickets_status
    ON tickets (status);

CREATE INDEX IF NOT EXISTS idx_tickets_priority
    ON tickets (priority);

CREATE INDEX IF NOT EXISTS idx_tickets_requester
    ON tickets (requester);

CREATE INDEX IF NOT EXISTS idx_tickets_assignee
    ON tickets (assignee);

CREATE INDEX IF NOT EXISTS idx_tickets_created_at
    ON tickets (created_at DESC);

CREATE TABLE IF NOT EXISTS ticket_asset_links (
    ticket_id BIGINT NOT NULL REFERENCES tickets (id) ON DELETE CASCADE,
    asset_id BIGINT NOT NULL REFERENCES assets (id) ON DELETE CASCADE,
    linked_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (ticket_id, asset_id)
);

CREATE INDEX IF NOT EXISTS idx_ticket_asset_links_asset_id
    ON ticket_asset_links (asset_id);

CREATE TABLE IF NOT EXISTS ticket_alert_links (
    ticket_id BIGINT NOT NULL REFERENCES tickets (id) ON DELETE CASCADE,
    alert_source VARCHAR(64) NOT NULL,
    alert_key VARCHAR(255) NOT NULL,
    alert_title VARCHAR(255),
    severity VARCHAR(32),
    linked_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (ticket_id, alert_source, alert_key),
    CONSTRAINT chk_ticket_alert_links_source_not_blank CHECK (btrim(alert_source) <> ''),
    CONSTRAINT chk_ticket_alert_links_key_not_blank CHECK (btrim(alert_key) <> '')
);

CREATE INDEX IF NOT EXISTS idx_ticket_alert_links_lookup
    ON ticket_alert_links (alert_source, alert_key);
