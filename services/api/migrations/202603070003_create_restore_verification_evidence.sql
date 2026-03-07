CREATE TABLE IF NOT EXISTS ops_backup_restore_evidence (
    id BIGSERIAL PRIMARY KEY,
    run_id BIGINT NOT NULL REFERENCES ops_backup_policy_runs (id) ON DELETE CASCADE,
    policy_id BIGINT NOT NULL REFERENCES ops_backup_policies (id) ON DELETE CASCADE,
    run_type VARCHAR(16) NOT NULL,
    run_status VARCHAR(16) NOT NULL,
    ticket_ref VARCHAR(128),
    artifact_url VARCHAR(1024) NOT NULL,
    note VARCHAR(1024),
    verifier VARCHAR(128) NOT NULL,
    closure_status VARCHAR(16) NOT NULL DEFAULT 'open',
    closed_at TIMESTAMPTZ,
    closed_by VARCHAR(128),
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_ops_backup_restore_evidence_run_type CHECK (run_type IN ('backup', 'drill')),
    CONSTRAINT chk_ops_backup_restore_evidence_run_status CHECK (run_status IN ('succeeded', 'failed')),
    CONSTRAINT chk_ops_backup_restore_evidence_ticket_ref_len CHECK (ticket_ref IS NULL OR length(ticket_ref) <= 128),
    CONSTRAINT chk_ops_backup_restore_evidence_artifact_url_not_blank CHECK (btrim(artifact_url) <> ''),
    CONSTRAINT chk_ops_backup_restore_evidence_verifier_not_blank CHECK (btrim(verifier) <> ''),
    CONSTRAINT chk_ops_backup_restore_evidence_closure_status CHECK (closure_status IN ('open', 'closed')),
    CONSTRAINT chk_ops_backup_restore_evidence_closed_consistency CHECK (
        (closure_status = 'open' AND closed_at IS NULL)
        OR (closure_status = 'closed' AND closed_at IS NOT NULL)
    ),
    CONSTRAINT chk_ops_backup_restore_evidence_metadata_object CHECK (jsonb_typeof(metadata) = 'object')
);

CREATE INDEX IF NOT EXISTS idx_ops_backup_restore_evidence_run_created
    ON ops_backup_restore_evidence (run_id, created_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_ops_backup_restore_evidence_policy_created
    ON ops_backup_restore_evidence (policy_id, created_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_ops_backup_restore_evidence_status
    ON ops_backup_restore_evidence (run_status, closure_status, created_at DESC, id DESC);
