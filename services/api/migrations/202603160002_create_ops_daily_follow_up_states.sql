CREATE TABLE IF NOT EXISTS ops_daily_follow_up_states (
    id BIGSERIAL PRIMARY KEY,
    task_key TEXT NOT NULL UNIQUE,
    item_type TEXT NOT NULL,
    follow_up_state TEXT NOT NULL,
    note TEXT NULL,
    defer_until TIMESTAMPTZ NULL,
    actor TEXT NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_ops_daily_follow_up_states_item_type
    ON ops_daily_follow_up_states (item_type);

CREATE INDEX IF NOT EXISTS idx_ops_daily_follow_up_states_state
    ON ops_daily_follow_up_states (follow_up_state);

CREATE INDEX IF NOT EXISTS idx_ops_daily_follow_up_states_defer_until
    ON ops_daily_follow_up_states (defer_until);
