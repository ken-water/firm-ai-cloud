ALTER TABLE ops_runbook_template_executions
    ADD COLUMN IF NOT EXISTS replay_source_execution_id BIGINT;

CREATE INDEX IF NOT EXISTS idx_ops_runbook_template_executions_replay_source
    ON ops_runbook_template_executions (replay_source_execution_id, created_at DESC, id DESC);
