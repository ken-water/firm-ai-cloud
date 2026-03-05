CREATE INDEX IF NOT EXISTS idx_assets_class_id_desc
    ON assets (asset_class, id DESC);

CREATE INDEX IF NOT EXISTS idx_assets_status_id_desc
    ON assets (status, id DESC);
