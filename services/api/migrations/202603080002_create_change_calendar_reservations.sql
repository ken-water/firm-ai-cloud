CREATE TABLE IF NOT EXISTS ops_change_calendar_reservations (
    id BIGSERIAL PRIMARY KEY,
    operation_kind VARCHAR(128) NOT NULL,
    risk_level VARCHAR(16) NOT NULL,
    start_at TIMESTAMPTZ NOT NULL,
    end_at TIMESTAMPTZ NOT NULL,
    site VARCHAR(128),
    department VARCHAR(128),
    owner VARCHAR(128) NOT NULL,
    note VARCHAR(1024),
    status VARCHAR(16) NOT NULL DEFAULT 'reserved',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_ops_change_calendar_reservations_operation_kind_not_blank CHECK (btrim(operation_kind) <> ''),
    CONSTRAINT chk_ops_change_calendar_reservations_risk_level CHECK (risk_level IN ('low', 'medium', 'high', 'critical')),
    CONSTRAINT chk_ops_change_calendar_reservations_owner_not_blank CHECK (btrim(owner) <> ''),
    CONSTRAINT chk_ops_change_calendar_reservations_status CHECK (status IN ('reserved', 'cancelled')),
    CONSTRAINT chk_ops_change_calendar_reservations_time_range CHECK (end_at > start_at)
);

CREATE INDEX IF NOT EXISTS idx_ops_change_calendar_reservations_time
    ON ops_change_calendar_reservations (start_at ASC, end_at ASC, id ASC);

CREATE INDEX IF NOT EXISTS idx_ops_change_calendar_reservations_scope
    ON ops_change_calendar_reservations (status, site, department, start_at ASC, id ASC);
