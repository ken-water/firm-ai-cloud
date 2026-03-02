CREATE TABLE IF NOT EXISTS audit_logs (
    id BIGSERIAL PRIMARY KEY,
    actor VARCHAR(128) NOT NULL,
    action VARCHAR(128) NOT NULL,
    target_type VARCHAR(64) NOT NULL,
    target_id VARCHAR(128),
    result VARCHAR(32) NOT NULL,
    message TEXT,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_audit_logs_actor_not_blank CHECK (btrim(actor) <> ''),
    CONSTRAINT chk_audit_logs_action_not_blank CHECK (btrim(action) <> ''),
    CONSTRAINT chk_audit_logs_target_type_not_blank CHECK (btrim(target_type) <> ''),
    CONSTRAINT chk_audit_logs_result_not_blank CHECK (btrim(result) <> ''),
    CONSTRAINT chk_audit_logs_metadata_object CHECK (jsonb_typeof(metadata) = 'object')
);

CREATE INDEX IF NOT EXISTS idx_audit_logs_created_at
    ON audit_logs (created_at DESC);

CREATE INDEX IF NOT EXISTS idx_audit_logs_actor_created_at
    ON audit_logs (actor, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_audit_logs_action_created_at
    ON audit_logs (action, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_audit_logs_target_type_created_at
    ON audit_logs (target_type, created_at DESC);

CREATE OR REPLACE FUNCTION audit_logs_block_mutation() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'audit_logs is append-only';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_audit_logs_block_update ON audit_logs;
CREATE TRIGGER trg_audit_logs_block_update
BEFORE UPDATE ON audit_logs
FOR EACH ROW
EXECUTE FUNCTION audit_logs_block_mutation();

DROP TRIGGER IF EXISTS trg_audit_logs_block_delete ON audit_logs;
CREATE TRIGGER trg_audit_logs_block_delete
BEFORE DELETE ON audit_logs
FOR EACH ROW
EXECUTE FUNCTION audit_logs_block_mutation();
