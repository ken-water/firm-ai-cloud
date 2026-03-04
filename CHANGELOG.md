# Changelog

All notable changes to this project are documented in this file.

The format follows Keep a Changelog principles and uses Semantic Versioning.

## [Unreleased]

### Added

- Discovery scheduler baseline:
  - migration: `202603040002_add_discovery_job_next_run_at.sql`
  - worker: `services/api/src/discovery_scheduler_worker.rs`
  - scheduler env controls: `DISCOVERY_SCHEDULER_ENABLED`, `DISCOVERY_SCHEDULER_POLL_SECONDS`
- Ticket domain baseline:
  - migrations: `202603040003_create_ticket_tables.sql`, `202603040004_add_ticket_permissions.sql`
  - APIs and route wiring in `services/api/src/tickets.rs`
  - web-console ticket dashboard and detail surfaces
- Monitoring source secret hardening:
  - migration: `202603040005_encrypt_monitoring_source_secrets.sql`
  - encrypted-at-rest secret support and masked API outputs
  - shared secret resolution module: `services/api/src/secrets.rs`
- Runtime language switch baseline:
  - supported locales: `en-US`, `zh-CN`
  - new locale key coverage checker: `apps/web-console/scripts/check-i18n-coverage.mjs`

### Changed

- Workflow script execution now enforces explicit runtime policy:
  - `disabled` (default), `allowlist`, `sandboxed`
  - allowlist and sandbox options are configurable by env
- Web console now supports runtime language switching with persisted preference storage.

### Fixed

- Unauthorized ticket list route handling now aligns with RBAC mapping.

## [0.0.7] - 2026-03-04

### Added

- Page-level web-console navigation with active route highlighting and legacy hash compatibility:
  - `#/overview`, `#/cmdb`, `#/monitoring`, `#/workflow`, `#/admin`
- Workflow automation backend baseline:
  - migration: `202603040001_create_workflow_tables.sql`
  - APIs in `services/api/src/workflow.rs`
  - template/request/approval/execution-log lifecycle support
- Workflow cockpit and report-center visual baseline:
  - KPI cards and distribution/trend panels
  - CSV export and week/month ranking comparison
- CMDB and monitoring chart baselines:
  - `GET /api/v1/cmdb/assets/stats`
  - `GET /api/v1/monitoring/metrics`
- Demo operations toolkit:
  - `scripts/demo-seed-data.sh`
  - `scripts/demo-health-check.sh`
  - `scripts/demo-cleanup-data.sh`
  - `docs/16-demo-runbook.md`

### Changed

- RBAC alias mapping now covers nested and relative workflow/monitoring routes.
- CI policy remains manual-trigger only (`workflow_dispatch`).

### Fixed

- Corrected workflow nested route permission mapping coverage.

## [0.0.6] - 2026-03-03

### Added

- CMDB lifecycle and multi-binding governance:
  - migration: `202603030002_create_asset_bindings_and_lifecycle.sql`
  - APIs: `GET/PUT /api/v1/cmdb/assets/{id}/bindings`, `POST /api/v1/cmdb/assets/{id}/lifecycle`
- Discovery candidate decision hardening:
  - migration: `202603030003_harden_discovery_candidate_review.sql`
  - candidate review metadata and stricter pending-candidate identity guards
- Async CMDB-to-monitoring provisioning baseline:
  - migration: `202603030004_create_cmdb_monitoring_sync.sql`
  - queue table: `cmdb_monitoring_sync_jobs`
  - binding table: `cmdb_monitoring_bindings`
  - APIs: `GET /api/v1/cmdb/assets/{id}/monitoring-binding`, `POST /api/v1/cmdb/assets/{id}/monitoring-sync`, `GET /api/v1/cmdb/monitoring-sync/jobs`
- Infrastructure hierarchy + impact traversal baseline:
  - migration: `202603030005_standardize_relation_types_and_hierarchy_indexes.sql`
  - impact API: `GET /api/v1/cmdb/assets/{id}/impact`

### Changed

- Candidate approval now supports explicit strategy (`auto|create|merge`) and optional merge target.
- Discovery dedup now refreshes existing pending candidates for matching identity signals.
- Relation API canonicalizes aliases to standard relation types (`contains`, `depends_on`, `runs_service`, `owned_by`).
- Web console includes readiness/binding operations for lifecycle transition gating.

### Fixed

- Prevented duplicate pending candidates across repeated discovery runs.
- Prevented invalid hierarchy cycles and conflicting multi-parent `contains` structures.

## [0.0.5] - 2026-03-03

### Added

- Monitoring source registry baseline:
  - migration: `202603030001_create_monitoring_sources.sql`
  - APIs: `GET /api/v1/monitoring/sources`, `POST /api/v1/monitoring/sources`, `POST /api/v1/monitoring/sources/{id}/probe`
  - new RBAC permission keys: `monitoring.sources.read`, `monitoring.sources.write`
- v0.0.5 issue-first monitoring plan document: `docs/14-v0.0.5-monitoring-bootstrap-plan.md`.
- Bundled Zabbix deployment stack in one-click install:
  - `zabbix-db`, `zabbix-server`, `zabbix-web`, `zabbix-proxy`, `zabbix-agent-local`
  - configurable server/proxy ports for external agent access
- Offline bundle now includes Zabbix images in `cloudops-images.tar`.
- Zabbix bootstrap automation script:
  - `scripts/bootstrap-zabbix.sh`
  - auto-registers proxy `cloudops-proxy` and local host `cloudops-local-agent` after install/upgrade
  - waits for local agent availability through proxy path
- CMDB lifecycle and binding baseline:
  - migration: `202603030002_create_asset_bindings_and_lifecycle.sql`
  - APIs: `GET/PUT /api/v1/cmdb/assets/{id}/bindings`, `POST /api/v1/cmdb/assets/{id}/lifecycle`
  - multi-binding support for departments, business services, and owners (team/user/group/external)
  - operational transition gate requires complete bindings

### Changed

- RBAC route mapping and auth tests now cover monitoring source endpoints.
- RBAC integration script now validates monitoring source read/write permissions for operator and viewer roles.
- RBAC coverage documentation now includes monitoring permission matrix and endpoint mapping.
- Install/upgrade health checks now wait for bundled Zabbix services.
- Install/upgrade flows now invoke Zabbix bootstrap automatically after health checks.
- Installation guides now document default Zabbix access and remote agent onboarding parameters.
- CMDB asset create default status changed from `active` to `idle` and direct `operational` creation is blocked.

### Fixed

- Zabbix server first-start database bootstrap on MySQL:
  - enabled `--log-bin-trust-function-creators=1` for `zabbix-db`
  - prevents schema import failure caused by trigger creation privileges when binary logging is enabled

## [0.0.4] - 2026-03-03

### Added

- LAN helper scripts for local network access:
  - `scripts/dev-lan-up.sh`
  - `scripts/dev-lan-down.sh`
- Web-console layout primitives:
  - `AuthGate`
  - `AppShell`
  - `SectionCard`
- CMDB asset usability controls:
  - search input
  - status/class/site filters
  - sort options (updated time, name, id)
  - filter-empty guidance and quick reset actions
- Discovery and notification operation UX feedback:
  - summary indicators
  - loading-state messages
  - post-action confirmation banners
  - status chip display for operation status
- Frontend quality guardrail tooling:
  - `apps/web-console/scripts/check-guardrails.mjs`
  - `npm run check:guardrails`
  - `npm run check:ui`
- UI verification and guardrail documentation:
  - `docs/12-v0.0.4-ui-iteration-plan.md`
  - `docs/13-web-ui-quality-guardrails.md`

### Changed

- CI trigger policy remains manual-only (`workflow_dispatch`) and web checks now run `npm run check:ui` in CI.
- Web-console default API base now follows current host (`<current-host>:8080`) when `VITE_API_BASE_URL` is not provided, improving LAN accessibility.
- Web-console UI now uses a consistent shell-and-card visual baseline for CMDB/discovery/notification operations.
- Discovery and notification pages now provide clearer read-only guidance for non-writer roles.

### Fixed

- Reduced accidental relation management errors by adding:
  - self-relation prevention validation
  - delete confirmation before destructive relation removal
- Improved operational clarity for role-restricted users by surfacing in-context permission hints instead of ambiguous action absence.

## [0.0.3] - 2026-03-02

### Added

- OIDC baseline authentication APIs:
  - `GET /api/v1/auth/oidc/start`
  - `GET /api/v1/auth/oidc/callback`
  - `GET /api/v1/auth/me`
  - `POST /api/v1/auth/logout`
- OIDC identity/session schema:
  - `iam_external_identities`
  - `auth_oidc_login_states`
  - `auth_sessions`
- Developer OIDC smoke test script: `scripts/test-oidc-dev.sh`.
- GitHub Actions CI workflow: `.github/workflows/ci.yml`.
- Web console sign-in session panel with header-mode and bearer-token mode.
- Security operations guide for v0.0.3: `docs/11-security-operations-v0.0.3.md`.

### Changed

- RBAC principal resolution now supports either `x-auth-user` header or `Authorization: Bearer <session_token>`.
- Developer quickstart now documents OIDC env settings, bearer-token flow, and OIDC smoke validation.
- OIDC dev smoke script now verifies invalid-token deny and session revocation behavior.
- RBAC coverage doc now includes CI verification and auth suite runbook.
- Web console now applies role-aware action visibility for CMDB/discovery/notification write operations.
- Web console now detects bearer session expiry and returns users to sign-in.
- Release governance now includes a mandatory security-change release checklist.

### Fixed

- Normalized protected API examples in quickstart to consistently include auth headers.

## [0.0.2] - 2026-03-02

### Added

- Dynamic CMDB custom field definitions with type/length/enum validation.
- Asset QR/barcode support and scan lookup endpoint.
- CMDB relationship schema, CRUD APIs, and one-hop graph API.
- Discovery jobs/candidates APIs with runnable discovery execution.
- Multi-source discovery adapters: `zabbix_hosts`, `snmp_seed`, `k8s_seed`.
- Candidate review APIs (`approve`/`reject`) with auto asset create/merge path.
- Discovery event model and query API (`asset.new_detected`, `asset.profile_changed`, `asset.offboarded_suspected`).
- Notification channels/templates/subscriptions APIs.
- Notification dispatch with webhook retry and delivery logs.
- Web console relation panel and discovery review workflows.
- Web console notification rule/subscription management workflows.
- Integration smoke script: `scripts/test-cmdb-loop.sh`.
- Expanded developer quickstart and troubleshooting docs.

### Changed

- Discovery run endpoint now performs actual collection, dedup, candidate enqueue, and event generation.
- Event emission now triggers notification dispatch flow.

### Fixed

- Better validation and conflict messages for relation and code uniqueness paths.

## [0.0.1] - 2026-03-02

### Added

- Initial open-source scaffold for CloudOps One.
- Rust API baseline (`/health`, `/api/v1/ping`).
- React web-console scaffold with English i18n setup.
- One-click dependency stack bootstrap via Docker Compose.
- Offline packaging/install scripts for disconnected environments.
- Initial product and technical documentation set.

### Changed

- None

### Fixed

- None
