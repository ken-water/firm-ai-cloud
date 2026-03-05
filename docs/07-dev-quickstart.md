# CloudOps One Developer Quickstart

Version: v1.2  
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
bash scripts/install.sh --skip-docker-install --mirror cn --dependencies-only
```

`--dependencies-only` keeps API/web ports free for local `cargo run` and `npm run dev` workflows.

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

Discovery scheduler worker env settings (enabled by default):

```bash
export DISCOVERY_SCHEDULER_ENABLED=true
export DISCOVERY_SCHEDULER_POLL_SECONDS=30
```

Workflow execution security settings (default blocks script auto-execution):

```bash
export WORKFLOW_EXECUTION_POLICY_MODE=allowlist
export WORKFLOW_EXECUTION_ALLOWLIST=echo,printf
export WORKFLOW_EXECUTION_SANDBOX_DIR=/tmp/cloudops-workflow-sandbox
```

Notes:

- `disabled`: block all script auto-run steps (default).
- `allowlist`: run only commands matched in `WORKFLOW_EXECUTION_ALLOWLIST`.
- `sandboxed`: same allowlist check, and run with cleared environment under sandbox directory.

Monitoring secret encryption settings:

```bash
# required when using inline secret values (plain/raw)
export MONITORING_SECRET_ENCRYPTION_KEY="$(openssl rand -base64 32)"

# recommended for production: block inline secret_ref and enforce env:KEY mode
export MONITORING_SECRET_INLINE_POLICY=forbid
```

Health check:

```bash
curl http://127.0.0.1:8080/health
curl http://127.0.0.1:8080/api/v1/ping
```

Setup wizard backend APIs:

```bash
# preflight checks: db/rbac/oidc/secrets/workflow policy
curl -H "$AUTH_HEADER" http://127.0.0.1:8080/api/v1/setup/preflight

# integration checklist: api/web/db/redis/opensearch/minio/zabbix + bootstrap seeds
curl -H "$AUTH_HEADER" http://127.0.0.1:8080/api/v1/setup/checklist
```

Notes:

- Response schema is stable: `generated_at`, `category`, `summary`, `checks[]`.
- Every failed check includes `remediation` text for UI next-action guidance.
- Permission required: `ops.setup.read` (viewer/operator/admin default roles have read access).

Unified alert center APIs:

```bash
# list alerts with filters and pagination
curl -H "$AUTH_HEADER" "http://127.0.0.1:8080/api/v1/alerts?status=open&severity=critical&site=dc-a&limit=50&offset=0"

# alert detail with timeline and linked tickets
curl -H "$AUTH_HEADER" http://127.0.0.1:8080/api/v1/alerts/1

# acknowledge/close one alert
curl -X POST http://127.0.0.1:8080/api/v1/alerts/1/ack \
  -H "$AUTH_HEADER" \
  -H 'Content-Type: application/json' \
  -d '{ "note": "accepted by oncall" }'

curl -X POST http://127.0.0.1:8080/api/v1/alerts/1/close \
  -H "$AUTH_HEADER" \
  -H 'Content-Type: application/json' \
  -d '{ "note": "issue mitigated" }'

# bulk acknowledge/close
curl -X POST http://127.0.0.1:8080/api/v1/alerts/bulk/ack \
  -H "$AUTH_HEADER" \
  -H 'Content-Type: application/json' \
  -d '{ "ids": [1, 2, 3], "note": "batch ack from shift lead" }'

curl -X POST http://127.0.0.1:8080/api/v1/alerts/bulk/close \
  -H "$AUTH_HEADER" \
  -H 'Content-Type: application/json' \
  -d '{ "ids": [1, 2], "note": "batch close after recovery" }'

# alert-to-ticket policy templates and custom policy CRUD
curl -H "$AUTH_HEADER" http://127.0.0.1:8080/api/v1/alerts/policies

curl -X POST http://127.0.0.1:8080/api/v1/alerts/policies \
  -H "$AUTH_HEADER" \
  -H 'Content-Type: application/json' \
  -d '{
    "policy_key": "critical-dc-a",
    "name": "Critical incidents in dc-a",
    "is_enabled": true,
    "match_source": "monitoring_sync",
    "match_severity": "critical",
    "match_site": "dc-a",
    "dedup_window_seconds": 1800,
    "ticket_priority": "critical",
    "ticket_category": "incident"
  }'
```

Notes:

- Read permission: `alerts.read`; write permission: `alerts.write`.
- Default built-in policy templates are seeded by migration:
  - `critical-infrastructure`
  - `service-degradation`
  - `repeated-failure` (disabled by default)

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

# aggregate asset statistics (status/department/business-service + unbound counters)
curl -H "$AUTH_HEADER" http://127.0.0.1:8080/api/v1/cmdb/assets/stats

# create an asset
curl -X POST http://127.0.0.1:8080/api/v1/cmdb/assets \
  -H "$AUTH_HEADER" \
  -H 'Content-Type: application/json' \
  -d '{
    "asset_class": "server",
    "name": "sample-asset",
    "hostname": "sample.local",
    "ip": "10.0.0.10",
    "status": "idle",
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

Monitoring source and CMDB -> Zabbix auto-provisioning:

```bash
export ZABBIX_LOCAL_PASSWORD='zabbix'

# create one Zabbix monitoring source (for local bundled Zabbix web endpoint)
curl -X POST http://127.0.0.1:8080/api/v1/monitoring/sources \
  -H "$AUTH_HEADER" \
  -H 'Content-Type: application/json' \
  -d '{
    "name": "local-zabbix",
    "source_type": "zabbix",
    "endpoint": "http://127.0.0.1:8082/api_jsonrpc.php",
    "auth_type": "basic",
    "username": "Admin",
    "secret_ref": "env:ZABBIX_LOCAL_PASSWORD",
    "site": "dc-a",
    "department": "platform",
    "is_enabled": true
  }'

# optional connectivity probe
curl -X POST http://127.0.0.1:8080/api/v1/monitoring/sources/1/probe \
  -H "$AUTH_HEADER"

# create/update eligible assets (server/vm/network_device/container/database)
# will enqueue async monitoring sync automatically

# inspect latest binding and sync status for one asset
curl -H "$AUTH_HEADER" http://127.0.0.1:8080/api/v1/cmdb/assets/1/monitoring-binding

# list sync jobs (retry/dead-letter visibility)
curl -H "$AUTH_HEADER" "http://127.0.0.1:8080/api/v1/cmdb/monitoring-sync/jobs?asset_id=1&limit=20"

# manual retry trigger for one asset
curl -X POST http://127.0.0.1:8080/api/v1/cmdb/assets/1/monitoring-sync \
  -H "$AUTH_HEADER" \
  -H 'Content-Type: application/json' \
  -d '{ "reason": "manual retry after source fix" }'

# monitoring overview (layer summary + source summary), supports site/department filter
curl -H "$AUTH_HEADER" "http://127.0.0.1:8080/api/v1/monitoring/overview?site=dc-a&department=platform"

# monitoring layer detail list, supports pagination and same scope filters
curl -H "$AUTH_HEADER" "http://127.0.0.1:8080/api/v1/monitoring/layers/hardware?site=dc-a&department=platform&limit=20&offset=0"

# monitoring metric series for one asset (cpu/load/network/disk from zabbix)
curl -H "$AUTH_HEADER" "http://127.0.0.1:8080/api/v1/monitoring/metrics?asset_id=1&window_minutes=60"
```

Web-console topology entry points:

- Topology workspace page: `http://127.0.0.1:5173/#/topology`
- Legacy CMDB topology section: `http://127.0.0.1:5173/#section-topology`

Topology map API baseline:

```bash
# global scope
curl -H "$AUTH_HEADER" "http://127.0.0.1:8080/api/v1/topology/maps/global?limit=200&offset=0"

# scoped by path key
curl -H "$AUTH_HEADER" "http://127.0.0.1:8080/api/v1/topology/maps/site:dc-a?limit=200&offset=0"

# scoped by path + query filter
curl -H "$AUTH_HEADER" "http://127.0.0.1:8080/api/v1/topology/maps/department:platform?site=dc-a&limit=100&offset=0"
```

Monitoring overview/layer API conventions:

- Layer enum is fixed to: `hardware`, `network`, `service`, `business`.
- Unknown layer requests return HTTP `400` with explicit validation error.
- Both endpoints include `empty` in payload for frontend empty-state handling.
- `monitoring_health` is normalized from sync status:
  - `healthy` <- `success`
  - `warning` <- `pending`/`running`
  - `critical` <- `failed`/`dead_letter`
  - `unknown` <- missing binding, `skipped`, or unsupported status

SSE stream baseline API:

```bash
# subscribe to SSE stream baseline (operator/viewer/admin must be authenticated)
curl -N -H "$AUTH_HEADER" "http://127.0.0.1:8080/api/v1/streams/sse?site=dc-a&department=platform&severity=all"

# restricted-scope example using explicit auth-scope headers
curl -N \
  -H "$AUTH_HEADER" \
  -H "x-auth-site: dc-a" \
  -H "x-auth-department: platform" \
  "http://127.0.0.1:8080/api/v1/streams/sse?site=dc-a&department=platform&severity=critical"
```

SSE envelope contract:

- Every event is JSON with:
  - `event_type`
  - `scope` (`site`, `department`, `severity`)
  - `timestamp` (RFC3339 UTC)
  - `payload` (event-specific object)
- Event categories include:
  - `stream.connected`
  - `stream.heartbeat`
  - `stream.stale`
  - `stream.recovered`
  - `alert.test`
  - `alert.monitoring_sync`

Reconnect and stale-state semantics:

- Server guidance in `stream.connected.payload`:
  - `reconnect_after_ms` (client reconnect backoff baseline)
  - `heartbeat_interval_seconds`
  - `stale_after_seconds`
- `stream.stale` means no matching alert events arrived within stale window.
- `stream.recovered` means fresh alert events resumed after stale state.

SSE lag metrics endpoint:

```bash
curl -H "$AUTH_HEADER" "http://127.0.0.1:8080/api/v1/streams/metrics?site=dc-a&window_minutes=60&sample_limit=5000"
```

Web-console monitoring source UX baseline:

- Open section: `Monitoring Sources` in the left navigation.
- Operators/admin can:
  - create source entries
  - apply scope filters (`source_type`, `site`, `department`, `enabled`)
  - trigger probe and see immediate status feedback
- Viewer role can inspect table/status only (read-only mode, no write actions).

Troubleshooting notes:

- Preferred mode: use `secret_ref: "env:YOUR_SECRET_ENV"` and export the value before API start.
- If you must use inline `plain:`/raw secret values, set `MONITORING_SECRET_ENCRYPTION_KEY` first so secrets are encrypted at rest.
- If `candidate`/`asset` sync jobs keep retrying, check latest error in:
  - `GET /api/v1/cmdb/monitoring-sync/jobs`
  - `GET /api/v1/audit/logs?action=cmdb.monitoring_sync.provision`
- If default proxy/template mapping does not match your Zabbix setup, set asset custom fields:
  - `monitoring_proxy`
  - `monitoring_host_group`
  - `monitoring_template` or `monitoring_templates`
- Worker burst tuning envs:
  - `MONITORING_SYNC_POLL_SECONDS` (default `3`)
  - `MONITORING_SYNC_BATCH_SIZE` (default `20`, range `1-200`)
  - `MONITORING_SYNC_MAX_PARALLEL` (default `4`, range `1-16`)

CMDB binding and lifecycle APIs:

```bash
# upsert multi-bindings for one asset
curl -X PUT http://127.0.0.1:8080/api/v1/cmdb/assets/1/bindings \
  -H "$AUTH_HEADER" \
  -H 'Content-Type: application/json' \
  -d '{
    "departments": ["platform", "dba"],
    "business_services": ["orders", "product-catalog"],
    "owners": [
      { "owner_type": "team", "owner_ref": "dba-team" },
      { "owner_type": "team", "owner_ref": "biz-ops" },
      { "owner_type": "user", "owner_ref": "alice" }
    ]
  }'

# inspect current bindings and operational readiness
curl -H "$AUTH_HEADER" http://127.0.0.1:8080/api/v1/cmdb/assets/1/bindings

# transition lifecycle (operational is blocked until required bindings are complete)
curl -X POST http://127.0.0.1:8080/api/v1/cmdb/assets/1/lifecycle \
  -H "$AUTH_HEADER" \
  -H 'Content-Type: application/json' \
  -d '{ "status": "operational" }'

# clear all bindings (unbind) and keep asset in non-operational states
curl -X PUT http://127.0.0.1:8080/api/v1/cmdb/assets/1/bindings \
  -H "$AUTH_HEADER" \
  -H 'Content-Type: application/json' \
  -d '{
    "departments": [],
    "business_services": [],
    "owners": []
  }'

# inspect monitoring sync result shown in web-console readiness panel
curl -H "$AUTH_HEADER" http://127.0.0.1:8080/api/v1/cmdb/assets/1/monitoring-binding
```

CMDB relation APIs:

```bash
# create a dependency relation: source asset depends on target asset
curl -X POST http://127.0.0.1:8080/api/v1/cmdb/relations \
  -H "$AUTH_HEADER" \
  -H 'Content-Type: application/json' \
  -d '{
    "src_asset_id": 1,
    "dst_asset_id": 2,
    "relation_type": "depends_on",
    "source": "manual"
  }'

# create hierarchy relation: physical host contains VM
curl -X POST http://127.0.0.1:8080/api/v1/cmdb/relations \
  -H "$AUTH_HEADER" \
  -H 'Content-Type: application/json' \
  -d '{
    "src_asset_id": 10,
    "dst_asset_id": 11,
    "relation_type": "contains",
    "source": "manual"
  }'

# create business ownership/service mapping
curl -X POST http://127.0.0.1:8080/api/v1/cmdb/relations \
  -H "$AUTH_HEADER" \
  -H 'Content-Type: application/json' \
  -d '{
    "src_asset_id": 11,
    "dst_asset_id": 20,
    "relation_type": "runs_service",
    "source": "manual"
  }'
curl -X POST http://127.0.0.1:8080/api/v1/cmdb/relations \
  -H "$AUTH_HEADER" \
  -H 'Content-Type: application/json' \
  -d '{
    "src_asset_id": 11,
    "dst_asset_id": 30,
    "relation_type": "owned_by",
    "source": "manual"
  }'

# list all relations that involve asset 1
curl -H "$AUTH_HEADER" "http://127.0.0.1:8080/api/v1/cmdb/relations?asset_id=1"

# get one-hop relation graph for asset 1
curl -H "$AUTH_HEADER" "http://127.0.0.1:8080/api/v1/cmdb/assets/1/graph"

# get incident impact graph (direction + depth + relation type filter)
curl -H "$AUTH_HEADER" "http://127.0.0.1:8080/api/v1/cmdb/assets/1/impact?direction=both&depth=4&relation_types=contains,depends_on,runs_service,owned_by"

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
    },
    "schedule": "every:1m"
  }'

# trigger a manual discovery run (scheduler can also run automatically by schedule)
curl -X POST http://127.0.0.1:8080/api/v1/cmdb/discovery/jobs/1/run \
  -H "$AUTH_HEADER"

# list pending discovery candidates
curl -H "$AUTH_HEADER" "http://127.0.0.1:8080/api/v1/cmdb/discovery/candidates?review_status=pending"

# approve a candidate with explicit create strategy
curl -X POST http://127.0.0.1:8080/api/v1/cmdb/discovery/candidates/1/approve \
  -H "$AUTH_HEADER" \
  -H 'Content-Type: application/json' \
  -d '{ "reviewed_by": "ops-admin", "strategy": "create", "reason": "new onboarding" }'

# approve a candidate with explicit merge strategy
curl -X POST http://127.0.0.1:8080/api/v1/cmdb/discovery/candidates/2/approve \
  -H "$AUTH_HEADER" \
  -H 'Content-Type: application/json' \
  -d '{ "reviewed_by": "ops-admin", "strategy": "merge", "target_asset_id": 1, "reason": "same host already exists" }'

# reject a candidate
curl -X POST http://127.0.0.1:8080/api/v1/cmdb/discovery/candidates/2/reject \
  -H "$AUTH_HEADER" \
  -H 'Content-Type: application/json' \
  -d '{ "reviewed_by": "ops-admin", "reason": "out of scope" }'

# query discovery events by asset and time range (RFC3339)
curl -H "$AUTH_HEADER" "http://127.0.0.1:8080/api/v1/cmdb/discovery/events?asset_id=1&time_from=2026-03-02T00:00:00Z&time_to=2026-03-02T23:59:59Z"
```

Ticket APIs:

```bash
# create one ticket with linked assets + alert ref and optional workflow trigger
curl -X POST http://127.0.0.1:8080/api/v1/tickets \
  -H "$AUTH_HEADER" \
  -H 'Content-Type: application/json' \
  -d '{
    "title": "Investigate db node latency spike",
    "description": "Observed high p95 latency on order-db.",
    "priority": "high",
    "category": "incident",
    "asset_ids": [1, 2],
    "alert_refs": [
      {
        "source": "zabbix",
        "alert_key": "problemid:123456",
        "alert_title": "Order DB CPU high",
        "severity": "warning"
      }
    ],
    "workflow_template_id": 1,
    "trigger_workflow": true
  }'

# list tickets with filters
curl -H "$AUTH_HEADER" "http://127.0.0.1:8080/api/v1/tickets?status=open&priority=high&limit=50"

# get one ticket detail
curl -H "$AUTH_HEADER" "http://127.0.0.1:8080/api/v1/tickets/1"

# update ticket lifecycle status
curl -X PATCH http://127.0.0.1:8080/api/v1/tickets/1/status \
  -H "$AUTH_HEADER" \
  -H 'Content-Type: application/json' \
  -d '{
    "status": "in_progress",
    "note": "accepted by platform-oncall"
  }'
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
- Authenticated shell supports runtime language switch (`English` / `简体中文`) from top-right toolbar.
- Language preference is persisted in browser local storage and restored on next visit.
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

Frontend i18n key coverage check:

```bash
cd apps/web-console
npm run check:i18n
```

## 9. Demo Toolkit

Prepare demo data pack:

```bash
bash scripts/demo-seed-data.sh
```

Validate latest demo dataset:

```bash
bash scripts/demo-health-check.sh
```

Cleanup by tag (dry-run first):

```bash
bash scripts/demo-cleanup-data.sh --tag demo-20260303 --dry-run
bash scripts/demo-cleanup-data.sh --tag demo-20260303
```

For full demo flow and screen-recording guidance, see `docs/16-demo-runbook.md`.

## 10. Troubleshooting

### 10.1 Discovery job run fails

- Check `source_type` and `scope` format first.
- For `zabbix_hosts`, confirm:
  - endpoint URL is reachable
  - token/auth is valid
  - response includes `result` array
- Query recent events:
  - `curl "http://127.0.0.1:8080/api/v1/cmdb/discovery/events?limit=20"`

### 10.2 Candidate approve fails

- Common reason: candidate already reviewed (not `pending`).
- Verify candidate state:
  - `curl "http://127.0.0.1:8080/api/v1/cmdb/discovery/candidates?limit=50"`
- If merge/create conflicts happen, retry after checking duplicated assets (`hostname + ip`).

### 10.3 Notification not delivered

- Verify channel/template/subscription all enabled.
- Check delivery logs:
  - `curl "http://127.0.0.1:8080/api/v1/cmdb/discovery/notification-deliveries?limit=50"`
- For webhook channels, inspect `status`, `attempts`, `response_code`, and `last_error`.

## 11. Integration Smoke Test

Run end-to-end CMDB loop smoke test (relations + discovery + candidate review + notification dispatch):

```bash
bash scripts/test-cmdb-loop.sh
```

Optional API base override:

```bash
API_BASE_URL=http://127.0.0.1:8080 bash scripts/test-cmdb-loop.sh
```

## 12. Performance and Reliability Baseline

Run API benchmark baseline:

```bash
bash scripts/benchmark-api-load.sh
```

Run scale profile API benchmark (`scale-1k` / `scale-5k`):

```bash
bash scripts/benchmark-api-load.sh --profile scale-1k
```

Run SSE burst stability smoke:

```bash
bash scripts/benchmark-sse-burst-smoke.sh
```

Run scale profile SSE burst benchmark:

```bash
bash scripts/benchmark-sse-burst-smoke.sh --profile scale-5k
```

Run one-command profile orchestration (API + SSE + gate):

```bash
bash scripts/benchmark-scale-profiles.sh --profile scale-1k
```

Run threshold gate against benchmark artifacts:

```bash
bash scripts/benchmark-threshold-gate.sh \
  --profile scale-1k \
  --api-summary .run/benchmarks/profile-scale-1k-<run-id>/api/summary.csv \
  --sse-summary .run/benchmarks/profile-scale-1k-<run-id>/sse/summary.json
```

Generate trend delta against a previous baseline run:

```bash
bash scripts/benchmark-trend-delta.sh \
  --profile scale-1k \
  --current-api-summary .run/benchmarks/profile-scale-1k-<current-run-id>/api/summary.csv \
  --baseline-api-summary .run/benchmarks/profile-scale-1k-<baseline-run-id>/api/summary.csv \
  --current-sse-summary .run/benchmarks/profile-scale-1k-<current-run-id>/sse/summary.json \
  --baseline-sse-summary .run/benchmarks/profile-scale-1k-<baseline-run-id>/sse/summary.json
```

Run quarterly DR drill automation:

```bash
bash scripts/dr-drill.sh --env-file deploy/.env --output-dir .run/dr-drill/<run-id> --yes
```

Baseline report and KPI comparison:

- `docs/18-performance-reliability-baseline-v0.0.8.md`

## 13. v0.1.1 Operator Simplicity APIs

Playbook catalog:

```bash
curl -H 'x-auth-user: admin' "http://127.0.0.1:8080/api/v1/workflow/playbooks?limit=20"
```

Playbook dry-run (high-risk confirmation token returned in response):

```bash
curl -X POST -H 'x-auth-user: admin' -H 'Content-Type: application/json' \
  -d '{"asset_ref":"asset-101","params":{"asset_ref":"asset-101","service_name":"nginx","grace_seconds":30}}' \
  "http://127.0.0.1:8080/api/v1/workflow/playbooks/restart-service-safe/dry-run"
```

Daily operations cockpit queue:

```bash
curl -H 'x-auth-user: admin' "http://127.0.0.1:8080/api/v1/ops/cockpit/queue?limit=20"
```

Topology edge diagnostics context:

```bash
curl -H 'x-auth-user: admin' \
  "http://127.0.0.1:8080/api/v1/topology/diagnostics/edges/1?window_minutes=120"
```
