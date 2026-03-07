CREATE TABLE IF NOT EXISTS ticket_escalation_policies (
    id BIGSERIAL PRIMARY KEY,
    policy_key VARCHAR(64) NOT NULL UNIQUE,
    name VARCHAR(128) NOT NULL,
    is_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    near_critical_minutes INTEGER NOT NULL DEFAULT 30,
    breach_critical_minutes INTEGER NOT NULL DEFAULT 60,
    near_high_minutes INTEGER NOT NULL DEFAULT 60,
    breach_high_minutes INTEGER NOT NULL DEFAULT 120,
    near_medium_minutes INTEGER NOT NULL DEFAULT 120,
    breach_medium_minutes INTEGER NOT NULL DEFAULT 240,
    near_low_minutes INTEGER NOT NULL DEFAULT 240,
    breach_low_minutes INTEGER NOT NULL DEFAULT 480,
    escalate_to_assignee VARCHAR(128) NOT NULL DEFAULT 'ops-escalation',
    updated_by VARCHAR(128) NOT NULL DEFAULT 'system',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_ticket_escalation_policy_key_not_blank CHECK (btrim(policy_key) <> ''),
    CONSTRAINT chk_ticket_escalation_policy_name_not_blank CHECK (btrim(name) <> ''),
    CONSTRAINT chk_ticket_escalation_policy_owner_not_blank CHECK (btrim(escalate_to_assignee) <> ''),
    CONSTRAINT chk_ticket_escalation_policy_critical_window CHECK (
        near_critical_minutes > 0 AND breach_critical_minutes > near_critical_minutes
    ),
    CONSTRAINT chk_ticket_escalation_policy_high_window CHECK (
        near_high_minutes > 0 AND breach_high_minutes > near_high_minutes
    ),
    CONSTRAINT chk_ticket_escalation_policy_medium_window CHECK (
        near_medium_minutes > 0 AND breach_medium_minutes > near_medium_minutes
    ),
    CONSTRAINT chk_ticket_escalation_policy_low_window CHECK (
        near_low_minutes > 0 AND breach_low_minutes > near_low_minutes
    )
);

CREATE INDEX IF NOT EXISTS idx_ticket_escalation_policies_enabled_updated
    ON ticket_escalation_policies (is_enabled, updated_at DESC, id DESC);

CREATE TABLE IF NOT EXISTS ticket_escalation_actions (
    id BIGSERIAL PRIMARY KEY,
    ticket_id BIGINT NOT NULL REFERENCES tickets (id) ON DELETE CASCADE,
    policy_id BIGINT NOT NULL REFERENCES ticket_escalation_policies (id) ON DELETE CASCADE,
    action_kind VARCHAR(32) NOT NULL,
    state_before VARCHAR(32) NOT NULL,
    state_after VARCHAR(32) NOT NULL,
    from_assignee VARCHAR(128),
    to_assignee VARCHAR(128),
    actor VARCHAR(128) NOT NULL,
    reason VARCHAR(1024),
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_ticket_escalation_actions_kind CHECK (action_kind IN ('escalated', 'run_skipped')),
    CONSTRAINT chk_ticket_escalation_actions_state_before CHECK (
        state_before IN ('normal', 'near_breach', 'breached')
    ),
    CONSTRAINT chk_ticket_escalation_actions_state_after CHECK (
        state_after IN ('normal', 'near_breach', 'breached')
    ),
    CONSTRAINT chk_ticket_escalation_actions_actor_not_blank CHECK (btrim(actor) <> ''),
    CONSTRAINT chk_ticket_escalation_actions_metadata_object CHECK (jsonb_typeof(metadata) = 'object')
);

CREATE INDEX IF NOT EXISTS idx_ticket_escalation_actions_ticket_created
    ON ticket_escalation_actions (ticket_id, created_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_ticket_escalation_actions_policy_created
    ON ticket_escalation_actions (policy_id, created_at DESC, id DESC);

INSERT INTO ticket_escalation_policies (
    policy_key,
    name,
    is_enabled,
    near_critical_minutes,
    breach_critical_minutes,
    near_high_minutes,
    breach_high_minutes,
    near_medium_minutes,
    breach_medium_minutes,
    near_low_minutes,
    breach_low_minutes,
    escalate_to_assignee,
    updated_by
)
VALUES (
    'default-ticket-sla',
    'Default Ticket SLA Policy',
    TRUE,
    30,
    60,
    60,
    120,
    120,
    240,
    240,
    480,
    'ops-escalation',
    'system'
)
ON CONFLICT (policy_key) DO UPDATE
SET
    name = EXCLUDED.name,
    is_enabled = EXCLUDED.is_enabled,
    near_critical_minutes = EXCLUDED.near_critical_minutes,
    breach_critical_minutes = EXCLUDED.breach_critical_minutes,
    near_high_minutes = EXCLUDED.near_high_minutes,
    breach_high_minutes = EXCLUDED.breach_high_minutes,
    near_medium_minutes = EXCLUDED.near_medium_minutes,
    breach_medium_minutes = EXCLUDED.breach_medium_minutes,
    near_low_minutes = EXCLUDED.near_low_minutes,
    breach_low_minutes = EXCLUDED.breach_low_minutes,
    escalate_to_assignee = EXCLUDED.escalate_to_assignee,
    updated_at = NOW();
