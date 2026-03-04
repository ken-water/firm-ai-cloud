ALTER TABLE discovery_jobs
    ADD COLUMN IF NOT EXISTS next_run_at TIMESTAMPTZ;

UPDATE discovery_jobs
SET next_run_at = NOW()
WHERE next_run_at IS NULL
  AND is_enabled = TRUE
  AND schedule IS NOT NULL
  AND BTRIM(schedule) <> '';

CREATE INDEX IF NOT EXISTS idx_discovery_jobs_next_run_at
    ON discovery_jobs (next_run_at);
