CREATE TABLE IF NOT EXISTS ops_handover_item_updates (
    id BIGSERIAL PRIMARY KEY,
    shift_date DATE NOT NULL,
    item_key VARCHAR(128) NOT NULL,
    source_type VARCHAR(32) NOT NULL,
    source_id BIGINT NOT NULL,
    status VARCHAR(16) NOT NULL DEFAULT 'closed',
    next_owner VARCHAR(128) NOT NULL,
    next_action VARCHAR(1024) NOT NULL,
    note VARCHAR(1024),
    updated_by VARCHAR(128) NOT NULL,
    closed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uq_ops_handover_item_updates_shift_item UNIQUE (shift_date, item_key),
    CONSTRAINT chk_ops_handover_item_updates_status CHECK (status IN ('open', 'closed')),
    CONSTRAINT chk_ops_handover_item_updates_item_key_not_blank CHECK (btrim(item_key) <> ''),
    CONSTRAINT chk_ops_handover_item_updates_source_type_not_blank CHECK (btrim(source_type) <> ''),
    CONSTRAINT chk_ops_handover_item_updates_next_owner_not_blank CHECK (btrim(next_owner) <> ''),
    CONSTRAINT chk_ops_handover_item_updates_next_action_not_blank CHECK (btrim(next_action) <> ''),
    CONSTRAINT chk_ops_handover_item_updates_updated_by_not_blank CHECK (btrim(updated_by) <> ''),
    CONSTRAINT chk_ops_handover_item_updates_closed_consistency CHECK (
        (status = 'open' AND closed_at IS NULL)
        OR (status = 'closed' AND closed_at IS NOT NULL)
    )
);

CREATE INDEX IF NOT EXISTS idx_ops_handover_item_updates_shift_status
    ON ops_handover_item_updates (shift_date, status, updated_at DESC, id DESC);

