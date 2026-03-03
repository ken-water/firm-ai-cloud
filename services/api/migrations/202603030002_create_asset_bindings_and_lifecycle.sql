ALTER TABLE assets
    ALTER COLUMN status SET DEFAULT 'idle';

CREATE TABLE IF NOT EXISTS asset_department_bindings (
    id BIGSERIAL PRIMARY KEY,
    asset_id BIGINT NOT NULL REFERENCES assets (id) ON DELETE CASCADE,
    department VARCHAR(128) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_asset_department_bindings_department_not_blank CHECK (btrim(department) <> ''),
    UNIQUE (asset_id, department)
);

CREATE TABLE IF NOT EXISTS asset_business_service_bindings (
    id BIGSERIAL PRIMARY KEY,
    asset_id BIGINT NOT NULL REFERENCES assets (id) ON DELETE CASCADE,
    business_service VARCHAR(128) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_asset_business_service_bindings_not_blank CHECK (btrim(business_service) <> ''),
    UNIQUE (asset_id, business_service)
);

CREATE TABLE IF NOT EXISTS asset_owner_bindings (
    id BIGSERIAL PRIMARY KEY,
    asset_id BIGINT NOT NULL REFERENCES assets (id) ON DELETE CASCADE,
    owner_type VARCHAR(32) NOT NULL,
    owner_ref VARCHAR(128) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_asset_owner_bindings_owner_type CHECK (
        owner_type IN ('team', 'user', 'group', 'external')
    ),
    CONSTRAINT chk_asset_owner_bindings_owner_ref_not_blank CHECK (btrim(owner_ref) <> ''),
    UNIQUE (asset_id, owner_type, owner_ref)
);

CREATE INDEX IF NOT EXISTS idx_asset_department_bindings_asset_id
    ON asset_department_bindings (asset_id);

CREATE INDEX IF NOT EXISTS idx_asset_business_service_bindings_asset_id
    ON asset_business_service_bindings (asset_id);

CREATE INDEX IF NOT EXISTS idx_asset_owner_bindings_asset_id
    ON asset_owner_bindings (asset_id);
