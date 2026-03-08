CREATE TABLE IF NOT EXISTS ops_restore_evidence_compliance_policies (
    policy_key VARCHAR(64) PRIMARY KEY,
    mode VARCHAR(16) NOT NULL DEFAULT 'advisory',
    sla_hours INTEGER NOT NULL DEFAULT 24,
    require_failed_runs BOOLEAN NOT NULL DEFAULT TRUE,
    require_drill_runs BOOLEAN NOT NULL DEFAULT TRUE,
    updated_by VARCHAR(128) NOT NULL DEFAULT 'system',
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_ops_restore_evidence_compliance_policy_key_not_blank CHECK (btrim(policy_key) <> ''),
    CONSTRAINT chk_ops_restore_evidence_compliance_mode CHECK (mode IN ('advisory', 'enforced')),
    CONSTRAINT chk_ops_restore_evidence_compliance_sla_hours CHECK (sla_hours BETWEEN 1 AND 720),
    CONSTRAINT chk_ops_restore_evidence_compliance_selector CHECK (require_failed_runs OR require_drill_runs),
    CONSTRAINT chk_ops_restore_evidence_compliance_updated_by_not_blank CHECK (btrim(updated_by) <> ''),
    CONSTRAINT chk_ops_restore_evidence_compliance_metadata_object CHECK (jsonb_typeof(metadata) = 'object')
);

INSERT INTO ops_restore_evidence_compliance_policies (
    policy_key,
    mode,
    sla_hours,
    require_failed_runs,
    require_drill_runs,
    updated_by,
    metadata
)
VALUES (
    'global',
    'advisory',
    24,
    TRUE,
    TRUE,
    'system',
    jsonb_build_object('initialized_by', 'migration', 'policy_kind', 'restore_evidence_sla')
)
ON CONFLICT (policy_key) DO NOTHING;
