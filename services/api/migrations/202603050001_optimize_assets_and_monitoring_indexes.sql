CREATE INDEX IF NOT EXISTS idx_assets_lower_asset_class_id_desc
    ON assets ((LOWER(asset_class)), id DESC);

CREATE INDEX IF NOT EXISTS idx_cmdb_monitoring_sync_jobs_asset_requested_at_id_desc
    ON cmdb_monitoring_sync_jobs (asset_id, requested_at DESC, id DESC)
    INCLUDE (status, attempt, max_attempts, last_error);
