# Changelog

All notable changes to this project are documented in this file.

The format follows Keep a Changelog principles and uses Semantic Versioning.

## [Unreleased]

### Added

- LAN helper scripts:
  - `scripts/dev-lan-up.sh`
  - `scripts/dev-lan-down.sh`
- v0.0.4 UI iteration planning document: `docs/12-v0.0.4-ui-iteration-plan.md`.

### Changed

- CI trigger policy updated: GitHub Actions CI now runs by manual dispatch only (`workflow_dispatch`) to avoid automatic resource consumption.
- Web console default API base now follows current host (`<current-host>:8080`) when `VITE_API_BASE_URL` is not provided, improving LAN access.
- Web console now uses a UI baseline shell with sidebar/topbar layout and shared design-token stylesheet.

### Fixed

- None

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
