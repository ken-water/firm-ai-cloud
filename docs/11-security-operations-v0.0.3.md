# CloudOps One Security Operations Guide (v0.0.3)

Date: 2026-03-02

This guide is the publish-ready security operations document for `v0.0.3`.

It covers:

- RBAC model and permission matrix
- OIDC setup and user mapping
- audit logging operations and retention guidance
- online/offline deployment security notes
- security-focused release checklist

## 1. RBAC Model and Permission Matrix

### 1.1 Principal Resolution

Protected API routes resolve principal in this order:

1. `x-auth-user` header (dev/local compatibility mode)
2. `Authorization: Bearer <session_token>` from `auth_sessions`

Routes protected by RBAC middleware:

- `/api/v1/cmdb/*`
- `/api/v1/iam/*`
- `/api/v1/audit/*`

### 1.2 Default Roles

| Role Key | Typical Usage | Notes |
| --- | --- | --- |
| `admin` | Platform and security administration | Includes `system.admin` |
| `operator` | Day-to-day platform operations | CMDB/discovery/notification read+write |
| `viewer` | Read-only operational visibility | CMDB/discovery/notification read-only |

### 1.3 Permission Matrix

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

### 1.4 Endpoint Group Mapping

| Route Group | Permission |
| --- | --- |
| `/api/v1/cmdb/assets*` | `cmdb.assets.read` / `cmdb.assets.write` |
| `/api/v1/cmdb/field-definitions*` | `cmdb.field_definitions.read` / `cmdb.field_definitions.write` |
| `/api/v1/cmdb/relations*` | `cmdb.relations.read` / `cmdb.relations.write` |
| `/api/v1/cmdb/discovery*` | `cmdb.discovery.read` / `cmdb.discovery.write` |
| `/api/v1/cmdb/discovery/notification-*` | `cmdb.notifications.read` / `cmdb.notifications.write` |
| `/api/v1/iam/*` | `system.admin` |
| `/api/v1/audit/*` | `system.admin` |

Detailed verification/coverage report:

- `docs/10-rbac-permission-coverage.md`

## 2. OIDC Setup and Operations

### 2.1 Required Environment Variables

| Variable | Required When | Description |
| --- | --- | --- |
| `AUTH_OIDC_ENABLED` | always for OIDC | Enables OIDC endpoints |
| `AUTH_OIDC_AUTHORIZATION_ENDPOINT` | non-dev mode | IdP authorization URL |
| `AUTH_OIDC_TOKEN_ENDPOINT` | non-dev mode | IdP token URL |
| `AUTH_OIDC_USERINFO_ENDPOINT` | non-dev mode | IdP userinfo URL |
| `AUTH_OIDC_CLIENT_ID` | non-dev mode | OAuth client id |
| `AUTH_OIDC_CLIENT_SECRET` | non-dev mode | OAuth client secret |
| `AUTH_OIDC_REDIRECT_URI` | always | API callback URI |
| `AUTH_OIDC_SCOPE` | optional | Default: `openid profile email` |
| `AUTH_OIDC_AUTO_PROVISION` | optional | Auto-create local user when unmapped |
| `AUTH_SESSION_TTL_MINUTES` | optional | Session token TTL |
| `AUTH_OIDC_DEV_MODE_ENABLED` | optional | Enables local dev callback code mode |

### 2.2 Recommended Baseline (Dev/Test)

```bash
export AUTH_OIDC_ENABLED=true
export AUTH_OIDC_DEV_MODE_ENABLED=true
export AUTH_OIDC_REDIRECT_URI='http://127.0.0.1:8080/api/v1/auth/oidc/callback'
export AUTH_OIDC_AUTO_PROVISION=false
```

### 2.3 Login and Mapping Flow

1. Client calls `GET /api/v1/auth/oidc/start`.
2. Callback hits `GET /api/v1/auth/oidc/callback?code=...&state=...`.
3. System maps identity by:
   - existing subject link in `iam_external_identities`, then
   - email match in `iam_users`, then
   - optional auto-provision (`AUTH_OIDC_AUTO_PROVISION=true`).
4. System issues bearer session token in `auth_sessions`.
5. RBAC still applies based on local role bindings in `iam_user_roles`.

### 2.4 Troubleshooting

- `invalid oidc state`: login state is missing, reused, or expired.
- `OIDC user is not mapped; ask admin to map by email or subject`: no subject/email match and auto-provision is disabled.
- `OIDC user is mapped but has no role binding`: user exists but has no role assignment.
- `bearer token is invalid or expired`: token revoked or TTL elapsed; user must re-authenticate.

## 3. Audit Logging Operations

### 3.1 What Is Audited

Current implementation captures:

- RBAC allow/deny checks for protected routes
- IAM writes and role binding changes
- OIDC callback success/failure and session logout actions
- CMDB/discovery write-path activity

### 3.2 Querying Audit Logs

API:

- `GET /api/v1/audit/logs`

Example:

```bash
curl -H 'x-auth-user: admin' \
  "http://127.0.0.1:8080/api/v1/audit/logs?limit=50&actor=admin"
```

### 3.3 Retention Recommendations

For production rollout:

1. Keep hot audit logs in PostgreSQL for 30 to 90 days.
2. Export long-term audit history to object storage (MinIO/S3) on a fixed schedule.
3. Index high-value audit fields in OpenSearch for faster security investigations.
4. Define explicit retention/rotation policy by environment (dev/staging/prod).
5. Restrict audit-read APIs to `admin` role and monitor access to audit endpoints.

### 3.4 Workflow Automation Execution Policy

Workflow script-step execution is protected by explicit runtime policy:

| Variable | Default | Description |
| --- | --- | --- |
| `WORKFLOW_EXECUTION_POLICY_MODE` | `disabled` | `disabled`, `allowlist`, or `sandboxed` |
| `WORKFLOW_EXECUTION_ALLOWLIST` | empty | Comma-separated command allowlist for script steps |
| `WORKFLOW_EXECUTION_SANDBOX_DIR` | `/tmp/cloudops-workflow-sandbox` | Working directory used in `sandboxed` mode |

Policy behaviors:

- `disabled`: all script auto-run steps are blocked.
- `allowlist`: only allowlisted commands can run.
- `sandboxed`: same allowlist check, with cleared environment (`PATH=/usr/bin:/bin`) and sandbox working directory.

Operational notes:

1. Use command-form scripts for automation (for example: `echo "ok"` or JSON array form `["/usr/bin/echo","ok"]`).
2. Avoid arbitrary shell fragments and pipelines in production templates.
3. Monitor `workflow_execution_logs.metadata.execution_policy` fields:
   - `mode`
   - `decision`
   - `command`
   - `command_hash_sha256`
   - `allowlist_match`

## 4. Deployment Security Notes

### 4.1 Online/Connected Environments

- Use TLS for API endpoints and IdP callbacks.
- Rotate `AUTH_OIDC_CLIENT_SECRET` and database credentials regularly.
- Trigger CI pipeline security checks manually when required (`.github/workflows/ci.yml`, `workflow_dispatch`).
- Validate release notes include security-impact changes.

### 4.2 Offline/Air-Gapped Environments

- Build offline bundle on a trusted connected machine:
  - `bash scripts/build-offline-bundle.sh --mirror cn`
- Verify bundle integrity (`SHA256SUMS`) before transfer.
- Keep OIDC integration optional; local header mode can bootstrap admin access in isolated test labs.
- Mirror internal package/image sources and manage credential rotation locally.

## 5. Security Release Checklist

For releases that include auth/RBAC/audit changes, verify:

- [ ] Affected endpoints and permission impacts are documented.
- [ ] Migration and rollback considerations are included.
- [ ] OIDC config changes are listed with secure defaults.
- [ ] Audit behavior changes and retention impact are documented.
- [ ] Online and offline deployment security notes are updated if behavior changed.
- [ ] Validation includes auth/RBAC scripts and CI status.
- [ ] `CHANGELOG.md` and release notes use consistent security terminology in English.
