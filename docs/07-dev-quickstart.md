# CloudOps One Developer Quickstart

Version: v1.1  
Date: 2026-03-03

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

Bundled Zabbix stack defaults after install:

- Web UI: `http://127.0.0.1:8082`
- Login: `Admin / zabbix`
- Server trapper: `10051`
- Proxy listener for agents: `10061`
- Auto-provisioned proxy/host: `cloudops-proxy` + `cloudops-local-agent`

For remote devices, install `zabbix-agent` or `zabbix-agent2` and set:

- `Server=<cloudops-host-ip>:10061`
- `ServerActive=<cloudops-host-ip>:10061`
- `Hostname=<your-device-unique-name>`

If you need to rerun Zabbix bootstrap:

```bash
bash scripts/bootstrap-zabbix.sh --env-file deploy/.env
```

## 3. Run Backend API

```bash
cargo run -p api
```

RBAC is enabled by default (`AUTH_RBAC_ENABLED=true`).
For local development, a bootstrap admin user `admin` is created automatically by migration.
Protected APIs support two auth modes:

- Legacy dev header mode (for local bootstrap and scripts)
- OIDC session bearer token mode (recommended baseline for SSO flow)

For legacy header mode:

```bash
AUTH_HEADER='x-auth-user: admin'
```

For bearer mode (after OIDC callback):

```bash
BEARER_HEADER='Authorization: Bearer <access_token>'
```

All `/api/v1/cmdb/*`, `/api/v1/iam/*`, and `/api/v1/audit/*` routes require an authenticated principal.

Minimal OIDC env settings for local dev mode:

```bash
export AUTH_OIDC_ENABLED=true
export AUTH_OIDC_DEV_MODE_ENABLED=true
export AUTH_OIDC_REDIRECT_URI='http://127.0.0.1:8080/api/v1/auth/oidc/callback'
export AUTH_OIDC_AUTO_PROVISION=false
```

Health check:

```bash
curl http://127.0.0.1:8080/health
curl http://127.0.0.1:8080/api/v1/ping
```

CMDB field definition APIs:

```bash
# list custom field definitions
curl -H "$AUTH_HEADER" http://127.0.0.1:8080/api/v1/cmdb/field-definitions

# create a custom field definition
curl -X POST http://127.0.0.1:8080/api/v1/cmdb/field-definitions \
  -H "$AUTH_HEADER" \
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

IAM APIs (admin only):

```bash
# list users
curl -H "$AUTH_HEADER" http://127.0.0.1:8080/api/v1/iam/users

# create a local user
curl -X POST http://127.0.0.1:8080/api/v1/iam/users \
  -H "$AUTH_HEADER" \
  -H 'Content-Type: application/json' \
  -d '{
    "username": "operator01",
    "display_name": "Ops Operator 01",
    "email": "operator01@example.local",
    "auth_source": "local",
    "is_enabled": true
  }'

# list roles
curl -H "$AUTH_HEADER" http://127.0.0.1:8080/api/v1/iam/roles

# bind role 2 to user 2
curl -X POST -H "$AUTH_HEADER" http://127.0.0.1:8080/api/v1/iam/users/2/roles/2

# unbind role
curl -X DELETE -H "$AUTH_HEADER" http://127.0.0.1:8080/api/v1/iam/users/2/roles/2
```

Audit APIs (admin read-only):

```bash
# query latest audit logs
curl -H "$AUTH_HEADER" "http://127.0.0.1:8080/api/v1/audit/logs?limit=20"

# filter by actor and time range (RFC3339)
curl -H "$AUTH_HEADER" "http://127.0.0.1:8080/api/v1/audit/logs?actor=admin&time_from=2026-03-02T00:00:00Z&time_to=2026-03-02T23:59:59Z"
```

Notes:

- `audit_logs` is append-only (UPDATE/DELETE are blocked by DB trigger).
- Permission-denied actions and CMDB/IAM write actions are persisted with actor/action/target/result/timestamp.

CMDB asset APIs:

```bash
# list assets
curl -H "$AUTH_HEADER" http://127.0.0.1:8080/api/v1/cmdb/assets

# create an asset
curl -X POST http://127.0.0.1:8080/api/v1/cmdb/assets \
  -H "$AUTH_HEADER" \
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
curl -H "$AUTH_HEADER" http://127.0.0.1:8080/api/v1/cmdb/assets/by-code/QR-100001?mode=auto
```

CMDB relation APIs:

```bash
# create a relation: source asset depends on target asset
curl -X POST http://127.0.0.1:8080/api/v1/cmdb/relations \
  -H "$AUTH_HEADER" \
  -H 'Content-Type: application/json' \
  -d '{
    "src_asset_id": 1,
    "dst_asset_id": 2,
    "relation_type": "depends_on",
    "source": "manual"
  }'

# list all relations that involve asset 1
curl -H "$AUTH_HEADER" "http://127.0.0.1:8080/api/v1/cmdb/relations?asset_id=1"

# get one-hop relation graph for asset 1
curl -H "$AUTH_HEADER" "http://127.0.0.1:8080/api/v1/cmdb/assets/1/graph"

# delete relation
curl -X DELETE http://127.0.0.1:8080/api/v1/cmdb/relations/1 \
  -H "$AUTH_HEADER"
```

CMDB discovery APIs:

```bash
# create a zabbix host discovery job (MVP supports mock_hosts in scope for local testing)
curl -X POST http://127.0.0.1:8080/api/v1/cmdb/discovery/jobs \
  -H "$AUTH_HEADER" \
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
curl -X POST http://127.0.0.1:8080/api/v1/cmdb/discovery/jobs/1/run \
  -H "$AUTH_HEADER"

# list pending discovery candidates
curl -H "$AUTH_HEADER" "http://127.0.0.1:8080/api/v1/cmdb/discovery/candidates?review_status=pending"

# approve a candidate (create or merge asset automatically)
curl -X POST http://127.0.0.1:8080/api/v1/cmdb/discovery/candidates/1/approve \
  -H "$AUTH_HEADER" \
  -H 'Content-Type: application/json' \
  -d '{ "reviewed_by": "ops-admin" }'

# reject a candidate
curl -X POST http://127.0.0.1:8080/api/v1/cmdb/discovery/candidates/2/reject \
  -H "$AUTH_HEADER" \
  -H 'Content-Type: application/json' \
  -d '{ "reviewed_by": "ops-admin" }'

# query discovery events by asset and time range (RFC3339)
curl -H "$AUTH_HEADER" "http://127.0.0.1:8080/api/v1/cmdb/discovery/events?asset_id=1&time_from=2026-03-02T00:00:00Z&time_to=2026-03-02T23:59:59Z"
```

Container CMDB discovery (k8s seed example):

```bash
curl -X POST http://127.0.0.1:8080/api/v1/cmdb/discovery/jobs \
  -H "$AUTH_HEADER" \
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
  -H "$AUTH_HEADER" \
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
  -H "$AUTH_HEADER" \
  -H 'Content-Type: application/json' \
  -d '{
    "event_type": "asset.offboarded_suspected",
    "title_template": "Asset offboarded suspected",
    "body_template": "Asset {{asset_id}} has been missing for {{missed_runs}} runs."
  }'

# subscribe event to channel
curl -X POST http://127.0.0.1:8080/api/v1/cmdb/discovery/notification-subscriptions \
  -H "$AUTH_HEADER" \
  -H 'Content-Type: application/json' \
  -d '{
    "channel_id": 1,
    "event_type": "asset.offboarded_suspected",
    "site": "dc-a",
    "department": "platform"
  }'

# list delivery logs
curl -H "$AUTH_HEADER" "http://127.0.0.1:8080/api/v1/cmdb/discovery/notification-deliveries?status=delivered"
```

OIDC auth APIs:

```bash
# start OIDC login and get authorization URL + state token
curl "http://127.0.0.1:8080/api/v1/auth/oidc/start?return_to=%2Fconsole"

# complete callback in dev mode format:
# code=dev::<sub>::<email>::<name>&state=<state-from-start>
curl "http://127.0.0.1:8080/api/v1/auth/oidc/callback?code=dev::oidc-sub-1::oidc.user@example.local::OIDC%20User&state=<state>"

# query current identity with bearer token
curl -H "$BEARER_HEADER" http://127.0.0.1:8080/api/v1/auth/me

# revoke current bearer session
curl -X POST -H "$BEARER_HEADER" http://127.0.0.1:8080/api/v1/auth/logout
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

Optional frontend auth user override for RBAC-protected APIs:

```bash
VITE_AUTH_USER=admin npm run dev
```

Optional frontend bearer token bootstrap:

```bash
VITE_AUTH_TOKEN=<access_token_from_oidc_callback> npm run dev
```

Notes:

- Web console now has a built-in sign-in session panel.
- Header mode (`x-auth-user`) is for local/dev usage.
- Bearer mode is preferred for OIDC session testing.

## 5. One-Command Dev Entry

```bash
bash scripts/dev-up.sh
```

This command:

1. Ensures dependency stack is up.
2. Starts `services/api`.

## 6. LAN Access (Web + API)

Start API + web console for LAN users:

```bash
bash scripts/dev-lan-up.sh
```

Stop LAN-mode services:

```bash
bash scripts/dev-lan-down.sh
```

Notes:

- API listens on `0.0.0.0:8080`.
- Web console listens on `0.0.0.0:5173`.
- Web console auto-targets API via current host by default (or `VITE_API_BASE_URL` if set).

## 7. Stop Dependencies

```bash
bash scripts/dev-down.sh
```

## 8. Security Checks

RBAC matrix integration check (requires API running):

```bash
bash scripts/test-rbac-policy.sh
```

OIDC dev-flow smoke check (requires API running with `AUTH_OIDC_ENABLED=true` and `AUTH_OIDC_DEV_MODE_ENABLED=true`):

```bash
bash scripts/test-oidc-dev.sh
```

## 9. Troubleshooting

### 8.1 Discovery job run fails

- Check `source_type` and `scope` format first.
- For `zabbix_hosts`, confirm:
  - endpoint URL is reachable
  - token/auth is valid
  - response includes `result` array
- Query recent events:
  - `curl "http://127.0.0.1:8080/api/v1/cmdb/discovery/events?limit=20"`

### 8.2 Candidate approve fails

- Common reason: candidate already reviewed (not `pending`).
- Verify candidate state:
  - `curl "http://127.0.0.1:8080/api/v1/cmdb/discovery/candidates?limit=50"`
- If merge/create conflicts happen, retry after checking duplicated assets (`hostname + ip`).

### 8.3 Notification not delivered

- Verify channel/template/subscription all enabled.
- Check delivery logs:
  - `curl "http://127.0.0.1:8080/api/v1/cmdb/discovery/notification-deliveries?limit=50"`
- For webhook channels, inspect `status`, `attempts`, `response_code`, and `last_error`.

## 10. Integration Smoke Test

Run end-to-end CMDB loop smoke test (relations + discovery + candidate review + notification dispatch):

```bash
bash scripts/test-cmdb-loop.sh
```

Optional API base override:

```bash
API_BASE_URL=http://127.0.0.1:8080 bash scripts/test-cmdb-loop.sh
```
