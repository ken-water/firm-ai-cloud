UPDATE asset_relations
SET relation_type = 'contains'
WHERE relation_type IN ('hosts', 'host', 'parent_of');

UPDATE asset_relations
SET relation_type = 'depends_on'
WHERE relation_type IN ('dependency', 'requires');

UPDATE asset_relations
SET relation_type = 'runs_service'
WHERE relation_type IN ('serves', 'service_for');

UPDATE asset_relations
SET relation_type = 'owned_by'
WHERE relation_type IN ('owner', 'managed_by');

WITH duplicate_contains AS (
    SELECT
        id,
        ROW_NUMBER() OVER (PARTITION BY dst_asset_id ORDER BY id ASC) AS rn
    FROM asset_relations
    WHERE relation_type = 'contains'
)
DELETE FROM asset_relations ar
USING duplicate_contains d
WHERE ar.id = d.id
  AND d.rn > 1;

CREATE UNIQUE INDEX IF NOT EXISTS uq_asset_relations_contains_single_parent
    ON asset_relations (dst_asset_id)
    WHERE relation_type = 'contains';

CREATE INDEX IF NOT EXISTS idx_asset_relations_contains_src
    ON asset_relations (src_asset_id)
    WHERE relation_type = 'contains';

CREATE INDEX IF NOT EXISTS idx_asset_relations_contains_dst
    ON asset_relations (dst_asset_id)
    WHERE relation_type = 'contains';
