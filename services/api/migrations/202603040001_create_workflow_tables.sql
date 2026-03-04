CREATE TABLE IF NOT EXISTS workflow_templates (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(128) NOT NULL UNIQUE,
    description TEXT,
    definition_json JSONB NOT NULL,
    is_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_workflow_templates_enabled
    ON workflow_templates (is_enabled);

CREATE TABLE IF NOT EXISTS workflow_requests (
    id BIGSERIAL PRIMARY KEY,
    template_id BIGINT NOT NULL REFERENCES workflow_templates (id),
    title VARCHAR(255) NOT NULL,
    requester VARCHAR(128) NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'pending_approval',
    current_step_index INTEGER NOT NULL DEFAULT 0,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    last_error TEXT,
    approved_by VARCHAR(128),
    approved_at TIMESTAMPTZ,
    executed_by VARCHAR(128),
    executed_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_workflow_requests_status
    ON workflow_requests (status);

CREATE INDEX IF NOT EXISTS idx_workflow_requests_template_id
    ON workflow_requests (template_id);

CREATE INDEX IF NOT EXISTS idx_workflow_requests_created_at
    ON workflow_requests (created_at DESC);

CREATE TABLE IF NOT EXISTS workflow_execution_logs (
    id BIGSERIAL PRIMARY KEY,
    request_id BIGINT NOT NULL REFERENCES workflow_requests (id) ON DELETE CASCADE,
    step_index INTEGER NOT NULL,
    step_id VARCHAR(64) NOT NULL,
    step_name VARCHAR(128) NOT NULL,
    step_kind VARCHAR(32) NOT NULL,
    status VARCHAR(32) NOT NULL,
    executor VARCHAR(128),
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    duration_ms INTEGER,
    exit_code INTEGER,
    output TEXT,
    error TEXT,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_workflow_execution_logs_request_id
    ON workflow_execution_logs (request_id, step_index, id DESC);
