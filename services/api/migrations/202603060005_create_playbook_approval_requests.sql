CREATE TABLE IF NOT EXISTS workflow_playbook_approval_requests (
    id BIGSERIAL PRIMARY KEY,
    dry_run_execution_id BIGINT NOT NULL REFERENCES workflow_playbook_executions (id) ON DELETE CASCADE,
    playbook_id BIGINT NOT NULL REFERENCES workflow_playbooks (id) ON DELETE CASCADE,
    playbook_key VARCHAR(64) NOT NULL,
    requester VARCHAR(128) NOT NULL,
    request_note VARCHAR(1024),
    status VARCHAR(16) NOT NULL DEFAULT 'pending',
    approver VARCHAR(128),
    approver_note VARCHAR(1024),
    approval_token VARCHAR(128),
    approved_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ NOT NULL,
    used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_workflow_playbook_approval_requests_status CHECK (
        status IN ('pending', 'approved', 'rejected', 'expired', 'used')
    ),
    CONSTRAINT chk_workflow_playbook_approval_requests_playbook_key_not_blank CHECK (btrim(playbook_key) <> ''),
    CONSTRAINT chk_workflow_playbook_approval_requests_requester_not_blank CHECK (btrim(requester) <> ''),
    CONSTRAINT chk_workflow_playbook_approval_requests_approver_token_required_when_approved CHECK (
        (status = 'approved' AND approver IS NOT NULL AND approval_token IS NOT NULL)
        OR status <> 'approved'
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_workflow_playbook_approval_active_by_dry_run
    ON workflow_playbook_approval_requests (dry_run_execution_id)
    WHERE status IN ('pending', 'approved');

CREATE INDEX IF NOT EXISTS idx_workflow_playbook_approval_status_created
    ON workflow_playbook_approval_requests (status, created_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_workflow_playbook_approval_requester
    ON workflow_playbook_approval_requests (requester, created_at DESC, id DESC);
