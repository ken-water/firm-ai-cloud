CREATE TABLE IF NOT EXISTS asset_relations (
    id BIGSERIAL PRIMARY KEY,
    src_asset_id BIGINT NOT NULL REFERENCES assets (id) ON DELETE CASCADE,
    dst_asset_id BIGINT NOT NULL REFERENCES assets (id) ON DELETE CASCADE,
    relation_type VARCHAR(64) NOT NULL,
    source VARCHAR(32) NOT NULL DEFAULT 'manual',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (src_asset_id, dst_asset_id, relation_type)
);

CREATE INDEX IF NOT EXISTS idx_asset_relations_src_asset_id
    ON asset_relations (src_asset_id);

CREATE INDEX IF NOT EXISTS idx_asset_relations_dst_asset_id
    ON asset_relations (dst_asset_id);

CREATE INDEX IF NOT EXISTS idx_asset_relations_relation_type
    ON asset_relations (relation_type);
