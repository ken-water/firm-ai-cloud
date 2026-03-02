# RBAC Permission Coverage (v0.0.3)

Date: 2026-03-02

This document is the permission coverage report/checklist for issue `V030-005`.

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
| `workflow.requests.read` | Allow | Allow | Allow |
| `workflow.requests.write` | Allow | Allow | Deny |
| `workflow.approvals.read` | Allow | Allow | Allow |
| `workflow.approvals.write` | Allow | Allow | Deny |

Source of truth:

- Migration seed policy: `services/api/migrations/202603020008_create_rbac_tables.sql`

## 2. Endpoint Group Mapping

RBAC route mapping source:

- `services/api/src/auth.rs` (`required_permission`)

### 2.1 CMDB/Discovery/Notification

| Route Group | Permission |
| --- | --- |
| `/api/v1/cmdb/assets*` | `cmdb.assets.read` / `cmdb.assets.write` |
| `/api/v1/cmdb/field-definitions*` | `cmdb.field_definitions.read` / `cmdb.field_definitions.write` |
| `/api/v1/cmdb/relations*` | `cmdb.relations.read` / `cmdb.relations.write` |
| `/api/v1/cmdb/discovery*` | `cmdb.discovery.read` / `cmdb.discovery.write` |
| `/api/v1/cmdb/discovery/notification-{channels,templates,subscriptions}*` | `cmdb.notifications.read` / `cmdb.notifications.write` |

### 2.2 Administration

| Route Group | Permission |
| --- | --- |
| `/api/v1/iam/users*` | `system.admin` |
| `/api/v1/iam/roles*` | `system.admin` |
| `/api/v1/audit/logs*` | `system.admin` |

### 2.3 Workflow (reserved mapping)

Workflow APIs are not implemented yet, but RBAC mapping is already reserved:

| Route Group | Permission |
| --- | --- |
| `/api/v1/workflow/requests*` | `workflow.requests.read` / `workflow.requests.write` |
| `/api/v1/workflow/approvals*` | `workflow.approvals.read` / `workflow.approvals.write` |

## 3. Deny-By-Default Strategy

- Protected routers (`/api/v1/cmdb`, `/api/v1/iam`, `/api/v1/audit`) are wrapped by RBAC middleware.
- If no permission mapping exists, middleware returns `403` with English message:
  - `"no RBAC permission mapping found for route ..."`
- If principal lacks mapped permission, middleware returns `403`:
  - `"permission denied: ..."`
- Prefix lookalike routes (for example `/api/v1/cmdb/assetsx`) are not matched and therefore denied by mapping.

## 4. Validation Checklist

- [x] Default role policy matrix documented.
- [x] Existing protected endpoint groups mapped to permissions.
- [x] Reserved workflow mappings documented.
- [x] Deny-by-default behavior defined and tested.
- [x] Forbidden responses are consistent English messages.

## 5. Automated Validation

- Unit coverage in `services/api/src/auth.rs`:
  - permission matrix test for existing protected endpoints
  - lookalike-prefix deny test
  - audit/iam/workflow mapping tests
- Integration matrix script:
  - `scripts/test-rbac-policy.sh`
  - verifies no-header deny, operator/viewer allow/deny matrix, and English forbidden response text.
- OIDC + bearer regression script:
  - `scripts/test-oidc-dev.sh`
  - verifies invalid bearer deny, role-scoped access, and token revocation behavior.

## 6. CI Verification

GitHub Actions workflow:

- `.github/workflows/ci.yml`

Default CI includes:

- Rust format check, compile check, and API unit tests.
- Web console build verification.
- Auth/RBAC integration suite against a live API + PostgreSQL service:
  - `bash scripts/test-rbac-policy.sh`
  - `bash scripts/test-oidc-dev.sh`

Run the auth suite locally (API must be running on `127.0.0.1:8080`):

```bash
bash scripts/test-rbac-policy.sh
bash scripts/test-oidc-dev.sh
```
