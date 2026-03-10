CREATE TABLE IF NOT EXISTS ops_runbook_analytics_policies (
    policy_key VARCHAR(32) PRIMARY KEY,
    failure_rate_threshold_percent INTEGER NOT NULL DEFAULT 20,
    minimum_sample_size INTEGER NOT NULL DEFAULT 5,
    note VARCHAR(1024),
    updated_by VARCHAR(128) NOT NULL DEFAULT 'system',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_ops_runbook_analytics_policies_key_not_blank CHECK (btrim(policy_key) <> ''),
    CONSTRAINT chk_ops_runbook_analytics_policies_threshold_range
        CHECK (failure_rate_threshold_percent BETWEEN 1 AND 100),
    CONSTRAINT chk_ops_runbook_analytics_policies_sample_range
        CHECK (minimum_sample_size BETWEEN 1 AND 500),
    CONSTRAINT chk_ops_runbook_analytics_policies_updated_by_not_blank CHECK (btrim(updated_by) <> '')
);

INSERT INTO ops_runbook_analytics_policies (
    policy_key,
    failure_rate_threshold_percent,
    minimum_sample_size,
    note,
    updated_by
)
VALUES (
    'global',
    20,
    5,
    'default seeded analytics policy',
    'system'
)
ON CONFLICT (policy_key) DO NOTHING;
