# Changelog

All notable changes to this project are documented in this file.

The format follows Keep a Changelog principles and uses Semantic Versioning.

## [Unreleased]

### Added

- None yet.

### Changed

- None yet.

### Fixed

- None yet.

## [0.1.7] - 2026-03-09

### Added

- v0.1.7 planning and release-gate documentation:
  - `docs/34-v0.1.7-runbook-execution-closure-plan.md`
  - `docs/35-v0.1.7-release-gate-checklist.md`
- v0.1.7 issue baseline and closure track: GitHub issues `#145` to `#150`.
- Runbook execution policy baseline:
  - migration: `202603090001_create_runbook_execution_policy.sql`
  - endpoints:
    - `GET /api/v1/ops/cockpit/runbook-templates/execution-policy`
    - `PUT /api/v1/ops/cockpit/runbook-templates/execution-policy`
- Runbook execution records now persist execution-mode/runtime metadata:
  - `execution_mode` (`simulate` / `live`)
  - `runtime_summary` JSON payload
- Hybrid live runbook execution baseline:
  - `POST /api/v1/ops/cockpit/runbook-templates/{key}/execute` now supports `execution_mode`
  - guarded live adapter for `dependency-check` with bounded TCP probe and runtime latency summary
- v0.1.7 one-command operator validation suite:
  - script: `scripts/qa-v0.1.7-operator-journey.sh`
  - artifacts: `summary.json`, `summary.md`, `artifact-index.json`, stage logs.

### Changed

- v0.1.x planning document now includes v0.1.7 track and issue map (`docs/20-v0.1x-operator-simplicity-plan.md`).
- Runbook template catalog response now includes `execution_modes` capability field.
- Web console runbook panel now adds:
  - execution-policy read/write controls (role-gated),
  - per-template execution mode selection,
  - execution runtime summary rendering in timeline table.

### Fixed

- None yet.

## [0.1.6] - 2026-03-08

### Added

- v0.1.6 planning and release-gate documentation:
  - `docs/32-v0.1.6-operator-simplicity-v2-plan.md`
  - `docs/33-v0.1.6-release-gate-checklist.md`
- v0.1.6 issue baseline and closure track: GitHub issues `#136` to `#144`.
- Operator profile preset workflow baseline:
  - migration: `202603080001_create_setup_operator_profile_runs.sql`
  - endpoints:
    - `GET /api/v1/setup/profiles`
    - `POST /api/v1/setup/profiles/{key}/preview`
    - `POST /api/v1/setup/profiles/{key}/apply`
    - `GET /api/v1/setup/profiles/history`
    - `POST /api/v1/setup/profiles/history/{id}/revert`
- Cockpit next-best-action assistant:
  - endpoint: `GET /api/v1/ops/cockpit/next-actions`
- Change calendar reservation and auto-slot recommendation:
  - migration: `202603080002_create_change_calendar_reservations.sql`
  - endpoints:
    - `GET /api/v1/ops/cockpit/change-calendar/reservations`
    - `POST /api/v1/ops/cockpit/change-calendar/reservations`
    - `GET /api/v1/ops/cockpit/change-calendar/slot-recommendations`
- Handover overdue and ownership quality gates:
  - endpoints:
    - `GET /api/v1/ops/cockpit/handover-digest/reminders`
    - `GET /api/v1/ops/cockpit/handover-digest/reminders/export?format=csv|json`
- Restore evidence compliance policy and weekly scorecard:
  - migration: `202603080003_create_restore_evidence_compliance_policy.sql`
  - endpoints:
    - `GET /api/v1/ops/cockpit/backup/evidence-compliance/policy`
    - `PUT /api/v1/ops/cockpit/backup/evidence-compliance/policy`
    - `GET /api/v1/ops/cockpit/backup/evidence-compliance/scorecard`
    - `GET /api/v1/ops/cockpit/backup/evidence-compliance/scorecard/export?format=csv|json`
- One-click runbook templates with preflight and evidence:
  - migration: `202603080004_create_ops_runbook_template_executions.sql`
  - endpoints:
    - `GET /api/v1/ops/cockpit/runbook-templates`
    - `POST /api/v1/ops/cockpit/runbook-templates/{key}/execute`
    - `GET /api/v1/ops/cockpit/runbook-templates/executions`
    - `GET /api/v1/ops/cockpit/runbook-templates/executions/{id}`
- v0.1.6 one-command operator validation suite:
  - script: `scripts/qa-v0.1.6-operator-journey.sh`
  - artifacts: `summary.json`, `summary.md`, `artifact-index.json`, stage logs.

### Changed

- v0.1.x planning document now includes v0.1.6 track and issue map (`docs/20-v0.1x-operator-simplicity-plan.md`).
- Web console daily cockpit now adds:
  - deterministic next-action panel and reservation-assisted scheduling workflow,
  - handover overdue/ownership reminder panel and export,
  - evidence SLA compliance policy + scorecard widget/export,
  - one-click runbook template execution panel with preflight/evidence inputs and timeline.
- Playbook dry-run/execute path now supports reservation context (`reservation_id`) for change-calendar-aligned execution.
- RBAC mapping/integration checks now include v0.1.6 cockpit routes (reservation, handover reminders, evidence compliance, runbook templates).

### Fixed

- None yet.

## [0.1.5] - 2026-03-07

### Added

- v0.1.5 planning document: `docs/29-v0.1.5-operator-autonomy-plan.md`.
- v0.1.5 issue baseline: GitHub issues `#127` to `#135` (incident command flow, escalation policy, change calendar, handover digest, restore evidence, simulation drill, validation, release closure).
- Incident command baseline for cockpit:
  - migration: `202603070001_create_incident_command_tables.sql`
  - endpoints:
    - `GET /api/v1/ops/cockpit/incidents`
    - `GET /api/v1/ops/cockpit/incidents/{alert_id}`
    - `POST /api/v1/ops/cockpit/incidents/{alert_id}/command`
- Ticket SLA escalation policy and queue guardrails baseline:
  - migration: `202603070002_create_ticket_escalation_tables.sql`
  - endpoints:
    - `GET /api/v1/tickets/escalation/policy`
    - `PUT /api/v1/tickets/escalation/policy`
    - `POST /api/v1/tickets/escalation/policy/preview`
    - `GET /api/v1/tickets/escalation/queue`
    - `GET /api/v1/tickets/escalation/actions`
    - `POST /api/v1/tickets/escalation/run`
- Restore verification evidence baseline for backup/drill continuity:
  - migration: `202603070003_create_restore_verification_evidence.sql`
  - endpoints:
    - `GET /api/v1/ops/cockpit/backup/restore-evidence`
    - `POST /api/v1/ops/cockpit/backup/runs/{id}/restore-evidence`
    - `PATCH /api/v1/ops/cockpit/backup/restore-evidence/{id}`
- Unified change calendar baseline for maintenance/freeze/approval overlays:
  - endpoints:
    - `GET /api/v1/ops/cockpit/change-calendar`
    - `POST /api/v1/ops/cockpit/change-calendar/conflicts`
- Shift handover digest baseline for unresolved risk carryover:
  - migration: `202603070004_create_handover_digest_tables.sql`
  - endpoints:
    - `GET /api/v1/ops/cockpit/handover-digest`
    - `GET /api/v1/ops/cockpit/handover-digest/export?format=csv|json`
    - `POST /api/v1/ops/cockpit/handover-digest/items/{item_key}/close`
- Escalation/failover dry-run simulation tooling:
  - script: `scripts/ops-escalation-failover-sim.sh`
  - policy: `scripts/ops-escalation-failover-policy.json`
  - guide: `docs/30-v0.1.5-escalation-failover-simulation.md`
- No-code operator journey validation suite:
  - script: `scripts/qa-v0.1.5-operator-journey.sh`
  - runbook: `docs/31-v0.1.5-operator-journey-validation.md`

### Changed

- v0.1.x planning document now includes v0.1.5 track and issue map (`docs/20-v0.1x-operator-simplicity-plan.md`).
- Web console daily cockpit adds incident command panel (owner/ETA/status update and timeline view).
- RBAC integration checks and permission mapping now cover incident command cockpit routes.
- Web console tickets page now includes SLA escalation policy controls, policy preview/run actions, risk queue view, and escalation action timeline.
- Ticket list and detail payloads now expose escalation state markers (`normal` / `near_breach` / `breached`) and latest action evidence.
- RBAC permission mapping and integration checks now include escalation policy/queue/actions/run allow-deny matrix (viewer read-only for writes).
- Backup run payloads now expose restore-evidence coverage hints (`restore_evidence_count` and latest evidence closure state).
- Weekly digest now reports restore-evidence coverage and missing continuity evidence gaps.
- Web console backup/dr panel now includes restore evidence attach/close workflow, coverage summary, and missing-run highlighting.
- High-risk playbook execute path now includes change-calendar conflict detail in policy-block responses.
- Web console daily cockpit now includes unified change calendar panel with date-range overlays and conflict check workflow.
- Shift handover digest export now uses deterministic digest timestamp derived from shift context for reproducible artifacts.

### Fixed

- None yet.

## [0.1.4] - 2026-03-06

### Added

- v0.1.4 planning and continuity documentation:
  - `docs/25-v0.1.4-safe-ops-planning.md`
  - `docs/26-v0.1.4-ops-continuity-and-digest.md`
  - `docs/27-v0.1.4-long-soak-stability.md`
  - `docs/28-v0.1.4-multi-region-simulation.md`
- v0.1.4 issue baseline and release-closure track: GitHub issues `#118` to `#126`.
- High-risk playbook policy governance:
  - migration: `202603060004_create_playbook_execution_policy.sql`
  - endpoints:
    - `GET /api/v1/workflow/playbooks/policy`
    - `PUT /api/v1/workflow/playbooks/policy`
- Two-person approval flow for high-risk playbook execution:
  - migration: `202603060005_create_playbook_approval_requests.sql`
  - endpoints:
    - `GET /api/v1/workflow/playbooks/approvals`
    - `POST /api/v1/workflow/playbooks/{key}/approval-request`
    - `POST /api/v1/workflow/playbooks/approvals/{id}/approve`
    - `POST /api/v1/workflow/playbooks/approvals/{id}/reject`
- Alert suppression governance and explainability:
  - endpoint: `POST /api/v1/alerts/policies/preview`
  - `GET /api/v1/alerts` adds `suppressed` filter support
  - `GET /api/v1/alerts/{id}` adds governance detail payload
- Backup/DR no-code policy and run tracking:
  - migration: `202603060006_create_backup_dr_policy_tables.sql`
  - endpoints:
    - `GET /api/v1/ops/cockpit/backup/policies`
    - `POST /api/v1/ops/cockpit/backup/policies`
    - `PATCH /api/v1/ops/cockpit/backup/policies/{id}`
    - `POST /api/v1/ops/cockpit/backup/policies/{id}/run`
    - `GET /api/v1/ops/cockpit/backup/runs`
    - `POST /api/v1/ops/cockpit/backup/scheduler/tick`
- Weekly operations digest:
  - `GET /api/v1/ops/cockpit/weekly-digest`
  - `GET /api/v1/ops/cockpit/weekly-digest/export?format=csv|json`
- Release-grade stability evidence scripts:
  - `scripts/benchmark-long-soak.sh`
  - `scripts/benchmark-long-soak-gate.sh`
  - `scripts/benchmark-long-soak-policy.json`
  - `scripts/benchmark-multiregion-sim.sh`
  - `scripts/benchmark-multiregion-policy.json`

### Changed

- Web console daily cockpit now includes backup/DR policy wizard, recent run evidence panel, and weekly digest generation/export actions.
- Alert remediation execution path now requests approval first for high-risk playbooks instead of direct execution.
- RBAC integration test script now covers backup/DR policy APIs and weekly digest/export routes.
- Release process now includes publish/reconcile and synchronization gate scripts:
  - `scripts/release-publish.sh`
  - `scripts/release-sync-check.sh`
  - `make release-publish`
  - `make release-publish-dry`
  - `make release-check`

### Fixed

- None yet.

## [0.1.3] - 2026-03-06

### Added

- LDAP live connector production baseline with startup validation and environment controls:
  - `AUTH_LDAP_LIVE_URL`
  - `AUTH_LDAP_LIVE_BIND_DN`
  - `AUTH_LDAP_LIVE_BIND_PASSWORD`
  - `AUTH_LDAP_LIVE_BASE_DN`
  - `AUTH_LDAP_LIVE_USER_FILTER`
  - `AUTH_LDAP_LIVE_ATTR_EMAIL`
  - `AUTH_LDAP_LIVE_ATTR_DISPLAY_NAME`
  - `AUTH_LDAP_LIVE_ATTR_GROUPS`
  - `AUTH_LDAP_LIVE_STARTTLS`
  - `AUTH_LDAP_LIVE_TLS_INSECURE_SKIP_VERIFY`
- Argon2id local password hashing baseline with legacy hash migration on successful local login.
- Local MFA recovery lifecycle:
  - migration: `202603060001_create_local_mfa_recovery_codes.sql`
  - endpoints:
    - `GET /api/v1/auth/local/mfa/recovery/status`
    - `POST /api/v1/auth/local/mfa/recovery/rotate`
    - `POST /api/v1/auth/local/mfa/recovery/admin-reset`
  - one-time recovery-code login via `POST /api/v1/auth/local/login` (`recovery_code` payload field)
- No-code setup bootstrap templates for operator onboarding:
  - migration: `202603060002_create_setup_bootstrap_templates.sql`
  - endpoints:
    - `GET /api/v1/setup/templates`
    - `POST /api/v1/setup/templates/{key}/preview`
    - `POST /api/v1/setup/templates/{key}/apply`
- Alert remediation planning and execution linkage:
  - `GET /api/v1/alerts/{id}/remediation`
  - remediation timeline events persisted from playbook dry-run/execute/replay
- Guided cockpit checklist tracking:
  - migration: `202603060003_create_ops_checklist_tables.sql`
  - endpoints:
    - `GET /api/v1/ops/cockpit/checklists`
    - `POST /api/v1/ops/cockpit/checklists/{template_key}/complete`
    - `POST /api/v1/ops/cockpit/checklists/{template_key}/exception`
- Benchmark regression guard and baseline artifacts:
  - `scripts/benchmark-regression-guard.sh`
  - `scripts/benchmark-regression-policy.json`
  - `benchmarks/baselines/scale-10k/{api-summary.csv,sse-summary.json}`
- Capacity guidance document: `docs/24-v0.1.3-capacity-guidance.md`.

### Changed

- Web console alert center adds one-click mapped remediation with confirmation-token flow for high-risk playbooks.
- Daily cockpit queue now includes latest alert remediation context and a guided checklist panel (summary, overdue/skipped highlighting, completion/exception actions).
- Benchmark scripts now support `scale-10k` profile defaults:
  - `scripts/benchmark-api-load.sh`
  - `scripts/benchmark-sse-burst-smoke.sh`
  - `scripts/benchmark-scale-profiles.sh`
  - `scripts/benchmark-threshold-policy.json`
- Profile runner now validates required benchmark artifacts before producing combined summary output.
- CI benchmark workflow now runs profile-based benchmark suites and publishes profile/gate/trend/regression summaries as artifacts.
- RBAC docs and integration checks now include cockpit checklist write permissions (`ops.cockpit.write`).

### Fixed

- Shared required-setting validation message is now identity-provider-agnostic (`<KEY> is required`).

## [0.1.2] - 2026-03-05

### Added

- Scale benchmark profile orchestration script: `scripts/benchmark-scale-profiles.sh`.
- Benchmark trend delta reporting script: `scripts/benchmark-trend-delta.sh`.
- LDAP dev connector login baseline API: `POST /api/v1/auth/ldap/login`.
- Local auth hardening APIs:
  - `POST /api/v1/auth/local/password`
  - `POST /api/v1/auth/local/mfa/enroll`
  - `POST /api/v1/auth/local/login`
- Local auth lockout integration script: `scripts/test-local-auth-hardening.sh`.

### Changed

- Benchmark scripts now support profile presets (`smoke`, `scale-1k`, `scale-5k`) with profile metadata artifacts:
  - `scripts/benchmark-api-load.sh`
  - `scripts/benchmark-sse-burst-smoke.sh`
- Benchmark threshold policy and gate are now profile-aware (`smoke`, `scale-1k`, `scale-5k`) with strict profile existence validation.
- Profile orchestration script can optionally generate trend delta artifacts when baseline summaries are provided.
- Developer quickstart benchmark section now includes profile-aware commands and one-command profile run flow.
- LDAP login now supports startup-validated group-to-role mapping policy via `AUTH_LDAP_GROUP_ROLE_MAPPING_JSON` with audit metadata.
- Local fallback header auth now supports governance modes (`allow_all`, `break_glass_only`, `disabled`) with break-glass allowlist and audit events.
- Local auth session and brute-force controls are configurable via env (idle/max-age/concurrency, lockout, and rate-limit knobs).

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
