# CloudOps One Developer Quickstart

Version: v1.0  
Date: 2026-03-02

## 1. Repository Layout

- `services/api`: Rust Axum API service
- `libs/common`: shared Rust models/config
- `apps/web-console`: React + Vite frontend shell
- `deploy`: local dependency stack definitions
- `scripts`: install, upgrade, offline packaging, and dev helpers

## 2. Start Dependencies

If Docker is already installed:

```bash
bash scripts/install.sh --skip-docker-install --mirror cn
```

## 3. Run Backend API

```bash
cargo run -p api
```

Health check:

```bash
curl http://127.0.0.1:8080/health
curl http://127.0.0.1:8080/api/v1/ping
```

CMDB field definition APIs:

```bash
# list custom field definitions
curl http://127.0.0.1:8080/api/v1/cmdb/field-definitions

# create a custom field definition
curl -X POST http://127.0.0.1:8080/api/v1/cmdb/field-definitions \
  -H 'Content-Type: application/json' \
  -d '{
    "field_key": "serial_no",
    "name": "Serial Number",
    "field_type": "text",
    "max_length": 64,
    "required": true,
    "scanner_enabled": true
  }'
```

CMDB asset APIs:

```bash
# list assets
curl http://127.0.0.1:8080/api/v1/cmdb/assets

# create an asset
curl -X POST http://127.0.0.1:8080/api/v1/cmdb/assets \
  -H 'Content-Type: application/json' \
  -d '{
    "asset_class": "server",
    "name": "sample-asset",
    "hostname": "sample.local",
    "ip": "10.0.0.10",
    "status": "active",
    "site": "dc-a",
    "department": "platform",
    "owner": "ops",
    "qr_code": "QR-100001",
    "barcode": "BC-100001",
    "custom_fields": {
      "serial_no": "SN-2026-001"
    }
  }'

# scan lookup by qr or barcode
curl http://127.0.0.1:8080/api/v1/cmdb/assets/by-code/QR-100001?mode=auto
```

## 4. Run Frontend

```bash
cd apps/web-console
npm install
npm run dev
```

Optional frontend API base override:

```bash
VITE_API_BASE_URL=http://127.0.0.1:8080 npm run dev
```

## 5. One-Command Dev Entry

```bash
bash scripts/dev-up.sh
```

This command:

1. Ensures dependency stack is up.
2. Starts `services/api`.

## 6. Stop Dependencies

```bash
bash scripts/dev-down.sh
```
