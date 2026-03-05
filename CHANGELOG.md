# Changelog

All notable changes to this project are documented in this file.

The format follows Keep a Changelog principles and uses Semantic Versioning.

## [Unreleased]

### Added

- Scale benchmark profile orchestration script: `scripts/benchmark-scale-profiles.sh`.
- Benchmark trend delta reporting script: `scripts/benchmark-trend-delta.sh`.

### Changed

- Benchmark scripts now support profile presets (`smoke`, `scale-1k`, `scale-5k`) with profile metadata artifacts:
  - `scripts/benchmark-api-load.sh`
  - `scripts/benchmark-sse-burst-smoke.sh`
- Benchmark threshold policy and gate are now profile-aware (`smoke`, `scale-1k`, `scale-5k`) with strict profile existence validation.
- Profile orchestration script can optionally generate trend delta artifacts when baseline summaries are provided.
- Developer quickstart benchmark section now includes profile-aware commands and one-command profile run flow.

### Fixed

- None yet.

## [0.1.1] - 2026-03-05

### Added

- Playbook catalog and safe execution baseline:
  - migration: `202603050004_create_playbook_catalog_and_execution_logs.sql`
  - built-in catalog entries with risk metadata and parameter schemas
  - APIs:
    - `GET /api/v1/workflow/playbooks`
    - `GET /api/v1/workflow/playbooks/{key}`
    - `POST /api/v1/workflow/playbooks/{key}/dry-run`
    - `POST /api/v1/workflow/playbooks/{key}/execute`
    - `GET /api/v1/workflow/playbooks/executions`
    - `GET /api/v1/workflow/playbooks/executions/{id}`
    - `POST /api/v1/workflow/playbooks/executions/{id}/replay`
- Daily operations cockpit aggregation:
  - migration: `202603050005_add_ops_cockpit_permission.sql`
  - API: `GET /api/v1/ops/cockpit/queue`
  - deterministic queue ordering with rationale and action metadata
- Topology diagnostics context API:
  - `GET /api/v1/topology/diagnostics/edges/{edge_id}`
  - trend, alert, change, impacted-hint, checklist, and quick-action payloads
- Web console v0.1.1 experience:
  - Playbook library section (`#/workflow`)
  - Daily operations cockpit section (`#/overview`, `#/monitoring`)
  - Topology diagnostics panel (`#/topology`)
- Validation and rollout docs:
  - `scripts/qa-v0.1.1.sh`
  - `docs/21-v0.1.1-validation-suite.md`
  - `docs/22-v0.1.1-operator-runbook.md`

### Changed

- RBAC matrix now includes `workflow.playbooks.read`, `workflow.playbooks.write`, and `ops.cockpit.read`.
- RBAC integration script now validates playbook and cockpit allow/deny paths for operator and viewer users.
- Developer quickstart now includes v0.1.1 operator simplicity API smoke examples.
- Topology workspace UI now includes diagnostics context loading, checklist, and quick actions from selected edges.

### Fixed

- Cockpit ranking order is now deterministic across refreshes by score/time/key sorting.
- Topology diagnostics query validation now rejects out-of-range window values early.

## [0.1.0] - 2026-03-05

### Added

- v0.1.x planning document: `docs/20-v0.1x-operator-simplicity-plan.md`.
- v0.1.x issue baseline: GitHub issues `#72` to `#86` (onboarding, alert loop, one-click operations, enterprise hardening, scale gate, staged releases).
- Setup readiness APIs for first-run operators:
  - `GET /api/v1/setup/preflight`
  - `GET /api/v1/setup/checklist`
  - stable response schema with `summary` and per-check `remediation`
- Unified alert center backend baseline:
  - migration: `202603050003_create_alerting_and_setup_permissions.sql`
  - APIs: list/detail, single and bulk acknowledge/close, policy CRUD
  - timeline persistence and linked-ticket query path
- Alert-to-ticket automation baseline:
  - policy table + action history table
  - built-in templates: `critical-infrastructure`, `service-degradation`, `repeated-failure`
  - worker-integrated policy evaluation and ticket linkage
- New web-console routes:
  - `#/setup` first-run onboarding wizard
  - `#/alerts` alert center with filters, bulk actions, and detail drill panel
- New RBAC permission keys:
  - `ops.setup.read`
  - `alerts.read`
  - `alerts.write`

### Changed

- Monitoring sync worker now opens/updates unified alerts on failures and auto-closes them on recovery.
- Route-level RBAC mapping now includes setup and alert-center endpoints with role tests.
- Developer quickstart and RBAC coverage docs now include setup and alert-center API usage examples.

### Fixed

- Removed dead-code status branch in alert lifecycle handler to keep API test build warning-free.

## [0.0.9] - 2026-03-05

### Added

- v0.0.9 planning document: `docs/19-v0.0.9-topology-reliability-plan.md`.
- v0.0.9 issue baseline: GitHub issues `#64` to `#71` (topology, stream metrics, perf, CI gate, DR, release closure).
- Topology map API baseline:
  - `GET /api/v1/topology/maps/{scope}` with `site`/`department` scope filters
  - windowing via `limit`/`offset`
  - deterministic node/edge payload with `empty` marker and health normalization
- Stream lag metrics API:
  - `GET /api/v1/streams/metrics`
  - lag summary (`samples`, `p50`, `p95`, `p99`, `max`) and lag buckets
- Web-console topology workspace route:
  - `#/topology` navigation entry
  - graph drill canvas with node/edge detail drawer
- Benchmark threshold gate tooling:
  - policy file: `scripts/benchmark-threshold-policy.json`
  - gate script: `scripts/benchmark-threshold-gate.sh`
  - machine-readable (`gate-summary.json`) and markdown (`gate-summary.md`) outputs
- DR drill automation:
  - `scripts/dr-drill.sh`
  - report artifacts: `report.json`, `report.md`, `logs/*.log`
  - runbook updates in `docs/17-disaster-recovery-runbook.md`

### Changed

- Monitoring-sync worker burst path now supports configurable batch claim + parallel processing:
  - `MONITORING_SYNC_BATCH_SIZE` (default `20`)
  - `MONITORING_SYNC_MAX_PARALLEL` (default `4`)
  - queue depth and throughput telemetry logs for each worker batch
- Manual CI now supports optional benchmark gate via `workflow_dispatch` input (`run_bench`):
  - runs sequential+concurrency API benchmarks and SSE burst benchmark
  - executes threshold gate script and uploads benchmark artifacts
- Developer quickstart now documents topology map API, stream lag metrics, threshold gate usage, and DR drill command.

### Fixed

- CI `auth-rbac-integration` startup stability:
  - build API binary before startup and run `target/debug/api` directly
  - increase readiness wait budget and fail fast if API process exits early

## [0.0.8] - 2026-03-05

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
- Read-path performance index baseline:
  - migrations: `202603050001_optimize_assets_and_monitoring_indexes.sql`, `202603050002_add_cmdb_asset_filter_indexes.sql`
  - index for monitoring latest-job lookup and layer filter scans

### Changed

- Workflow script execution now enforces explicit runtime policy:
  - `disabled` (default), `allowlist`, `sandboxed`
  - allowlist and sandbox options are configurable by env
- Web console now supports runtime language switching with persisted preference storage.
- CMDB assets list/filter hot paths now include descending composite indexes for `class` and `status`.
- Monitoring layer endpoint now computes summary counters in SQL aggregation and reuses summary `asset_total` as response total.
- Monitoring layer filter now uses explicit normalized class sets instead of CASE classification SQL for better index usage.
- Audit best-effort writes now run in detached async tasks and emit slow-write warnings when single inserts exceed 500ms.
- Monitoring sync enqueue path now resolves asset existence/class in a single query to cut one write-path round trip.
- API benchmark script now supports parallel execution via `--concurrency` and emits stage-level utilization snapshots.
- SSE benchmark script now supports `--burst-count` stress profile and reports `alert.monitoring_sync` lag distribution (`min/avg/p50/p95/p99/max`).

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
