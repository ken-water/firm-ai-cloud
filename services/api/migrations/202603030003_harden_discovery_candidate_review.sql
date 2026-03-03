ALTER TABLE discovery_candidates
    ADD COLUMN IF NOT EXISTS review_strategy VARCHAR(32),
    ADD COLUMN IF NOT EXISTS review_reason TEXT,
    ADD COLUMN IF NOT EXISTS review_asset_id BIGINT REFERENCES assets (id) ON DELETE SET NULL;

UPDATE discovery_candidates
SET review_strategy = CASE
    WHEN review_status = 'created' THEN 'approve:create'
    WHEN review_status = 'merged' THEN 'approve:merge'
    WHEN review_status = 'rejected' THEN 'reject'
    ELSE review_strategy
END
WHERE review_strategy IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS uq_discovery_candidates_pending_fingerprint
    ON discovery_candidates (fingerprint)
    WHERE review_status = 'pending';

CREATE INDEX IF NOT EXISTS idx_discovery_candidates_pending_hostname
    ON discovery_candidates ((LOWER(NULLIF(payload ->> 'hostname', ''))))
    WHERE review_status = 'pending' AND NULLIF(payload ->> 'hostname', '') IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_discovery_candidates_pending_ip
    ON discovery_candidates ((NULLIF(payload ->> 'ip', '')))
    WHERE review_status = 'pending' AND NULLIF(payload ->> 'ip', '') IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_discovery_candidates_review_asset_id
    ON discovery_candidates (review_asset_id);
