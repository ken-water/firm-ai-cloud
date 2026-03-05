# RBAC Permission Coverage (v0.1.1)

Date: 2026-03-05

This document records the current permission matrix, route-to-permission mapping, and validation evidence for the RBAC baseline.

## 1. Default Role Policy Matrix

| Permission Key | Admin | Operator | Viewer |
| --- | --- | --- | --- |
| `system.admin` | Allow | Deny | Deny |
| `cmdb.assets.read` | Allow | Allow | Allow |
| `cmdb.assets.write` | Allow | Allow | Deny |
| `cmdb.field_definitions.read` | Allow | Allow | Allow |
| `cmdb.field_definitions.write` | Allow | Allow | Deny |
| `cmdb.relations.read` | Allow | Allow | Allow |
| `cmdb.relations.write` | Allow | Allow | Deny |
| `cmdb.discovery.read` | Allow | Allow | Allow |
| `cmdb.discovery.write` | Allow | Allow | Deny |
| `cmdb.notifications.read` | Allow | Allow | Allow |
| `cmdb.notifications.write` | Allow | Allow | Deny |
| `monitoring.sources.read` | Allow | Allow | Allow |
| `monitoring.sources.write` | Allow | Allow | Deny |
| `ops.setup.read` | Allow | Allow | Allow |
| `ops.cockpit.read` | Allow | Allow | Allow |
| `alerts.read` | Allow | Allow | Allow |
| `alerts.write` | Allow | Allow | Deny |
| `workflow.requests.read` | Allow | Allow | Allow |
| `workflow.requests.write` | Allow | Allow | Deny |
| `workflow.approvals.read` | Allow | Allow | Allow |
| `workflow.approvals.write` | Allow | Allow | Deny |
| `workflow.playbooks.read` | Allow | Allow | Allow |
| `workflow.playbooks.write` | Allow | Allow | Deny |
| `tickets.read` | Allow | Allow | Allow |
| `tickets.write` | Allow | Allow | Deny |

Source of truth:

- Base RBAC seed: `services/api/migrations/202603020008_create_rbac_tables.sql`
- Ticket permissions: `services/api/migrations/202603040004_add_ticket_permissions.sql`
- Setup/alert permissions: `services/api/migrations/202603050003_create_alerting_and_setup_permissions.sql`
- Playbook permissions: `services/api/migrations/202603050004_create_playbook_catalog_and_execution_logs.sql`
- Daily cockpit permission: `services/api/migrations/202603050005_add_ops_cockpit_permission.sql`

## 2. Endpoint Group Mapping

RBAC route mapping source:

- `services/api/src/auth.rs` (`required_permission`)

### 2.1 CMDB / Discovery / Notification

| Route Group | Permission |
| --- | --- |
| `/api/v1/cmdb/assets*` | `cmdb.assets.read` / `cmdb.assets.write` |
| `/api/v1/cmdb/field-definitions*` | `cmdb.field_definitions.read` / `cmdb.field_definitions.write` |
| `/api/v1/cmdb/relations*` | `cmdb.relations.read` / `cmdb.relations.write` |
| `/api/v1/topology/maps*` | `cmdb.relations.read` |
| `/api/v1/cmdb/discovery*` | `cmdb.discovery.read` / `cmdb.discovery.write` |
| `/api/v1/cmdb/discovery/notification-{channels,templates,subscriptions}*` | `cmdb.notifications.read` / `cmdb.notifications.write` |

### 2.2 Monitoring / Streams

| Route Group | Permission |
| --- | --- |
| `/api/v1/monitoring/sources*` | `monitoring.sources.read` / `monitoring.sources.write` |
| `/api/v1/monitoring/overview` | `monitoring.sources.read` |
| `/api/v1/monitoring/layers*` | `monitoring.sources.read` |
| `/api/v1/monitoring/metrics` | `monitoring.sources.read` |
| `/api/v1/streams/sse` | `monitoring.sources.read` |
| `/api/v1/streams/metrics` | `monitoring.sources.read` |

### 2.3 Setup and Alert Center

| Route Group | Permission |
| --- | --- |
| `/api/v1/setup/preflight` | `ops.setup.read` |
| `/api/v1/setup/checklist` | `ops.setup.read` |
| `/api/v1/ops/cockpit/queue` | `ops.cockpit.read` |
| `/api/v1/alerts*` | `alerts.read` / `alerts.write` |

### 2.4 Workflow and Tickets

| Route Group | Permission |
| --- | --- |
| `/api/v1/workflow/requests*` | `workflow.requests.read` / `workflow.requests.write` |
| `/api/v1/workflow/approvals*` | `workflow.approvals.read` / `workflow.approvals.write` |
| `/api/v1/workflow/playbooks*` | `workflow.playbooks.read` / `workflow.playbooks.write` |
| `/api/v1/tickets*` | `tickets.read` / `tickets.write` |

### 2.5 Administration

| Route Group | Permission |
| --- | --- |
| `/api/v1/iam/users*` | `system.admin` |
| `/api/v1/iam/roles*` | `system.admin` |
| `/api/v1/audit/logs*` | `system.admin` |

## 3. Deny-by-Default Strategy

- Protected routers are wrapped by RBAC middleware.
- If no permission mapping exists, middleware returns `403` with:
  - `"no RBAC permission mapping found for route ..."`
- If the principal lacks the mapped permission, middleware returns `403` with:
  - `"permission denied: ..."`
- Prefix lookalike routes (for example `/api/v1/cmdb/assetsx`) are not matched and are denied.

## 4. Validation Checklist

- [x] Default role policy matrix documented.
- [x] Existing protected endpoint groups mapped to permissions.
- [x] Setup and alert-center routes included in mapping.
- [x] Deny-by-default behavior defined and tested.
- [x] Forbidden responses are consistent English messages.

## 5. Automated Validation

- Unit coverage in `services/api/src/auth.rs`:
  - permission matrix test for protected endpoints
  - setup/alerts mapping tests
  - lookalike-prefix deny tests
- Integration matrix script:
  - `scripts/test-rbac-policy.sh`
  - verifies no-header deny, operator/viewer allow-deny matrix (including setup and alerts), and English forbidden response text.
- OIDC + bearer regression script:
  - `scripts/test-oidc-dev.sh`

## 6. CI Verification

GitHub Actions workflow:

- `.github/workflows/ci.yml`

Manual CI (`workflow_dispatch`) includes:

- Rust format + compile + API unit tests
- Web console `check:ui`
- RBAC integration suite (`scripts/test-rbac-policy.sh`)
- OIDC bearer flow suite (`scripts/test-oidc-dev.sh`)

Run auth suites locally (API should be reachable at `127.0.0.1:8080`):

```bash
bash scripts/test-rbac-policy.sh
bash scripts/test-oidc-dev.sh
```
