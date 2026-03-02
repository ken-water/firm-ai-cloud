CREATE TABLE IF NOT EXISTS assets (
    id BIGSERIAL PRIMARY KEY,
    asset_class VARCHAR(64) NOT NULL,
    name VARCHAR(255) NOT NULL,
    hostname VARCHAR(255),
    ip VARCHAR(64),
    status VARCHAR(32) NOT NULL DEFAULT 'active',
    site VARCHAR(128),
    department VARCHAR(128),
    owner VARCHAR(128),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_assets_class ON assets (asset_class);
CREATE INDEX IF NOT EXISTS idx_assets_status ON assets (status);
CREATE INDEX IF NOT EXISTS idx_assets_name ON assets (name);
