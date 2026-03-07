CREATE TABLE IF NOT EXISTS ops_incident_commands (
    id BIGSERIAL PRIMARY KEY,
    alert_id BIGINT NOT NULL UNIQUE REFERENCES unified_alerts (id) ON DELETE CASCADE,
    command_status VARCHAR(32) NOT NULL DEFAULT 'triage',
    command_owner VARCHAR(128) NOT NULL,
    eta_at TIMESTAMPTZ,
    blocker VARCHAR(1024),
    summary VARCHAR(1024),
    updated_by VARCHAR(128) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_ops_incident_commands_status CHECK (
        command_status IN ('triage', 'in_progress', 'blocked', 'mitigated', 'postmortem')
    ),
    CONSTRAINT chk_ops_incident_commands_owner_not_blank CHECK (btrim(command_owner) <> ''),
    CONSTRAINT chk_ops_incident_commands_updated_by_not_blank CHECK (btrim(updated_by) <> '')
);

CREATE INDEX IF NOT EXISTS idx_ops_incident_commands_status_updated
    ON ops_incident_commands (command_status, updated_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_ops_incident_commands_owner_updated
    ON ops_incident_commands (command_owner, updated_at DESC, id DESC);

CREATE TABLE IF NOT EXISTS ops_incident_command_events (
    id BIGSERIAL PRIMARY KEY,
    alert_id BIGINT NOT NULL REFERENCES unified_alerts (id) ON DELETE CASCADE,
    event_type VARCHAR(32) NOT NULL,
    from_status VARCHAR(32),
    to_status VARCHAR(32) NOT NULL,
    command_owner VARCHAR(128) NOT NULL,
    eta_at TIMESTAMPTZ,
    blocker VARCHAR(1024),
    summary VARCHAR(1024),
    note VARCHAR(1024),
    actor VARCHAR(128) NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_ops_incident_command_events_type CHECK (
        event_type IN ('created', 'status_transition', 'command_updated')
    ),
    CONSTRAINT chk_ops_incident_command_events_to_status CHECK (
        to_status IN ('triage', 'in_progress', 'blocked', 'mitigated', 'postmortem')
    ),
    CONSTRAINT chk_ops_incident_command_events_from_status CHECK (
        from_status IS NULL OR from_status IN ('triage', 'in_progress', 'blocked', 'mitigated', 'postmortem')
    ),
    CONSTRAINT chk_ops_incident_command_events_owner_not_blank CHECK (btrim(command_owner) <> ''),
    CONSTRAINT chk_ops_incident_command_events_actor_not_blank CHECK (btrim(actor) <> ''),
    CONSTRAINT chk_ops_incident_command_events_metadata_object CHECK (jsonb_typeof(metadata) = 'object')
);

CREATE INDEX IF NOT EXISTS idx_ops_incident_command_events_alert_created
    ON ops_incident_command_events (alert_id, created_at DESC, id DESC);
