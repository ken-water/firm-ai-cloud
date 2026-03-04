ALTER TABLE monitoring_sources
    ADD COLUMN IF NOT EXISTS secret_ciphertext TEXT;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'chk_monitoring_sources_secret_ciphertext_not_blank'
    ) THEN
        ALTER TABLE monitoring_sources
            ADD CONSTRAINT chk_monitoring_sources_secret_ciphertext_not_blank
            CHECK (secret_ciphertext IS NULL OR btrim(secret_ciphertext) <> '');
    END IF;
END
$$;

COMMENT ON COLUMN monitoring_sources.secret_ciphertext IS
    'Encrypted monitoring secret envelope (enc:v1:<nonce>:<ciphertext>)';
