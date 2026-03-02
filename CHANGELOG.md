# Changelog

All notable changes to this project are documented in this file.

The format follows Keep a Changelog principles and uses Semantic Versioning.

## [Unreleased]

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

### Changed

- RBAC principal resolution now supports either `x-auth-user` header or `Authorization: Bearer <session_token>`.
- Developer quickstart now documents OIDC env settings, bearer-token flow, and OIDC smoke validation.

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
