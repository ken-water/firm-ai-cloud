CREATE TABLE IF NOT EXISTS asset_field_definitions (
    id BIGSERIAL PRIMARY KEY,
    field_key VARCHAR(64) NOT NULL UNIQUE,
    name VARCHAR(128) NOT NULL,
    field_type VARCHAR(32) NOT NULL,
    max_length INTEGER,
    required BOOLEAN NOT NULL DEFAULT FALSE,
    options JSONB,
    scanner_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    is_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_asset_field_type CHECK (
        field_type IN ('text', 'integer', 'float', 'boolean', 'enum', 'date', 'datetime')
    ),
    CONSTRAINT chk_asset_field_max_length CHECK (
        max_length IS NULL OR max_length > 0
    ),
    CONSTRAINT chk_asset_field_options_array CHECK (
        options IS NULL OR jsonb_typeof(options) = 'array'
    )
);

ALTER TABLE assets
    ADD COLUMN IF NOT EXISTS qr_code VARCHAR(255),
    ADD COLUMN IF NOT EXISTS barcode VARCHAR(255),
    ADD COLUMN IF NOT EXISTS custom_fields JSONB NOT NULL DEFAULT '{}'::jsonb;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'chk_assets_custom_fields_object'
    ) THEN
        ALTER TABLE assets
            ADD CONSTRAINT chk_assets_custom_fields_object
            CHECK (jsonb_typeof(custom_fields) = 'object');
    END IF;
END $$;

CREATE UNIQUE INDEX IF NOT EXISTS idx_assets_qr_code_unique
    ON assets (qr_code)
    WHERE qr_code IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_assets_barcode_unique
    ON assets (barcode)
    WHERE barcode IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_assets_custom_fields_gin
    ON assets
    USING GIN (custom_fields);

CREATE INDEX IF NOT EXISTS idx_asset_field_definitions_key
    ON asset_field_definitions (field_key);
