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

CMDB relation APIs:

```bash
# create a relation: source asset depends on target asset
curl -X POST http://127.0.0.1:8080/api/v1/cmdb/relations \
  -H 'Content-Type: application/json' \
  -d '{
    "src_asset_id": 1,
    "dst_asset_id": 2,
    "relation_type": "depends_on",
    "source": "manual"
  }'

# list all relations that involve asset 1
curl "http://127.0.0.1:8080/api/v1/cmdb/relations?asset_id=1"

# get one-hop relation graph for asset 1
curl "http://127.0.0.1:8080/api/v1/cmdb/assets/1/graph"

# delete relation
curl -X DELETE http://127.0.0.1:8080/api/v1/cmdb/relations/1
```

CMDB discovery APIs:

```bash
# create a zabbix host discovery job (MVP supports mock_hosts in scope for local testing)
curl -X POST http://127.0.0.1:8080/api/v1/cmdb/discovery/jobs \
  -H 'Content-Type: application/json' \
  -d '{
    "name": "zabbix-host-discovery",
    "source_type": "zabbix_hosts",
    "scope": {
      "mock_hosts": [
        { "name": "srv-a", "hostname": "srv-a.local", "ip": "10.0.1.11", "asset_class": "server" },
        { "name": "sw-a", "hostname": "sw-a.local", "ip": "10.0.1.21", "asset_class": "network_device" }
      ]
    }
  }'

# trigger a discovery run
curl -X POST http://127.0.0.1:8080/api/v1/cmdb/discovery/jobs/1/run

# list pending discovery candidates
curl "http://127.0.0.1:8080/api/v1/cmdb/discovery/candidates?review_status=pending"

# approve a candidate (create or merge asset automatically)
curl -X POST http://127.0.0.1:8080/api/v1/cmdb/discovery/candidates/1/approve \
  -H 'Content-Type: application/json' \
  -d '{ "reviewed_by": "ops-admin" }'

# reject a candidate
curl -X POST http://127.0.0.1:8080/api/v1/cmdb/discovery/candidates/2/reject \
  -H 'Content-Type: application/json' \
  -d '{ "reviewed_by": "ops-admin" }'

# query discovery events by asset and time range (RFC3339)
curl "http://127.0.0.1:8080/api/v1/cmdb/discovery/events?asset_id=1&time_from=2026-03-02T00:00:00Z&time_to=2026-03-02T23:59:59Z"
```

Container CMDB discovery (k8s seed example):

```bash
curl -X POST http://127.0.0.1:8080/api/v1/cmdb/discovery/jobs \
  -H 'Content-Type: application/json' \
  -d '{
    "name": "k8s-container-discovery",
    "source_type": "k8s_seed",
    "scope": {
      "seed_containers": [
        {
          "cluster": "prod-cluster-a",
          "namespace": "payments",
          "pod": "payment-api-7f8d9c",
          "container": "payment-api",
          "image": "registry.local/payment-api:v1.2.0",
          "node": "k8s-node-01",
          "pod_ip": "10.42.0.18"
        }
      ]
    }
  }'
```

CMDB discovery notification APIs:

```bash
# create a notification channel
curl -X POST http://127.0.0.1:8080/api/v1/cmdb/discovery/notification-channels \
  -H 'Content-Type: application/json' \
  -d '{
    "name": "platform-webhook",
    "channel_type": "webhook",
    "target": "https://example.local/cmdb-events",
    "config": {
      "headers": {
        "Authorization": "Bearer replace-me"
      }
    }
  }'

# create a template
curl -X POST http://127.0.0.1:8080/api/v1/cmdb/discovery/notification-templates \
  -H 'Content-Type: application/json' \
  -d '{
    "event_type": "asset.offboarded_suspected",
    "title_template": "Asset offboarded suspected",
    "body_template": "Asset {{asset_id}} has been missing for {{missed_runs}} runs."
  }'

# subscribe event to channel
curl -X POST http://127.0.0.1:8080/api/v1/cmdb/discovery/notification-subscriptions \
  -H 'Content-Type: application/json' \
  -d '{
    "channel_id": 1,
    "event_type": "asset.offboarded_suspected",
    "site": "dc-a",
    "department": "platform"
  }'

# list delivery logs
curl "http://127.0.0.1:8080/api/v1/cmdb/discovery/notification-deliveries?status=delivered"
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

## 7. Troubleshooting

### 7.1 Discovery job run fails

- Check `source_type` and `scope` format first.
- For `zabbix_hosts`, confirm:
  - endpoint URL is reachable
  - token/auth is valid
  - response includes `result` array
- Query recent events:
  - `curl "http://127.0.0.1:8080/api/v1/cmdb/discovery/events?limit=20"`

### 7.2 Candidate approve fails

- Common reason: candidate already reviewed (not `pending`).
- Verify candidate state:
  - `curl "http://127.0.0.1:8080/api/v1/cmdb/discovery/candidates?limit=50"`
- If merge/create conflicts happen, retry after checking duplicated assets (`hostname + ip`).

### 7.3 Notification not delivered

- Verify channel/template/subscription all enabled.
- Check delivery logs:
  - `curl "http://127.0.0.1:8080/api/v1/cmdb/discovery/notification-deliveries?limit=50"`
- For webhook channels, inspect `status`, `attempts`, `response_code`, and `last_error`.
