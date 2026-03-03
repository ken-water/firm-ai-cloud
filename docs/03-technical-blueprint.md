# CloudOps One Technical Blueprint

Version: v1.1  
Date: 2026-03-02  
Scope: MVP implementation baseline (CMDB + Zabbix + Ticketing + IAM + Rust backend + live dashboard streaming + i18n-ready UI)
Decision status: Data stack locked as PostgreSQL + Redis + OpenSearch + MinIO (see ADR-0001)

## 1. Architecture Overview

## 1.1 Logical Layers

1. `Web Console`
2. `API Gateway`
3. `Domain Services` (CMDB, Monitoring, Ticketing, IAM, Notification, Reporting)
4. `Streaming Layer` (ingestion, normalization, real-time fanout)
5. `Workflow/Automation Engine`
6. `Integration Adapters` (Zabbix, LDAP/OIDC, Webhook, 3rd-party systems)
7. `Data Layer` (PostgreSQL, Redis, Object Storage, Search/TSDB optional)

## 1.2 Technology Stack Decision

| Layer | Choice |
|---|---|
| Frontend | React + TypeScript + Vite |
| Frontend UI | Ant Design |
| Visualization | ECharts + Cytoscape.js |
| Frontend i18n | react-i18next |
| Backend runtime | Rust + Tokio |
| Backend framework | Axum (REST/WebSocket), Tower middleware |
| ORM/DB access | SQLx |
| Workflow | Temporal (SDK in Rust integration path) or isolated workflow worker with API bridge |
| Event bus | NATS JetStream |
| Cache | Redis |
| Primary DB | PostgreSQL |
| Search | OpenSearch (alerts/audit/tickets) |
| Object storage | MinIO |
| Deployment | Docker + Kubernetes + Helm, with docker-compose for local demo |

## 1.3 Service Boundaries

| Service | Responsibility |
|---|---|
| `cmdb-service` | Asset model, lifecycle, relationships, discovery candidate handling |
| `discovery-service` | SNMP/SSH/API scan jobs, dedup, change event generation |
| `monitoring-service` | Unified monitoring API, alert normalization, layer views |
| `zabbix-adapter` | Pulls host/item/trigger/event data from Zabbix Server/Proxy |
| `stream-gateway` | Real-time channel fanout to frontend clients (WebSocket/SSE) |
| `stream-processor` | Event normalization, deduplication, throttling, and routing |
| `topology-service` | Topology graph generation, multi-link modeling, link diagnostics |
| `ticket-service` | Ticket CRUD, approval flow state, SLA timers |
| `workflow-service` | Auto execution graph, task retry, rollback and manual fallback |
| `iam-service` | SSO/local auth, RBAC, org/site/dept scoping |
| `notification-service` | In-app/email/webhook dispatch and template management |
| `reporting-service` | KPI/SLA/asset analytics and exports |
| `audit-service` | Immutable operation/event logs |

## 2. Integration Design

## 2.1 Zabbix Integration

- Adapter mode: pull-based scheduled sync + incremental short-cycle polling for near-real-time updates.
- Supported entities in MVP: hosts, host groups, items, triggers, events, proxies.
- Core mapping:
  - `zabbix.host` -> `asset.monitoring_binding`
  - `zabbix.trigger/event` -> `unified_alert`
  - `zabbix.proxy` -> `site/collector context`
- Sync modes:
  - `full sync` every 1 to 5 minutes for consistency
  - `live sync` every 1 to 3 seconds for trigger/event deltas
- Data freshness target:
  - live dashboard path <= 5 seconds
  - full reconciliation path <= 60 seconds

## 2.2 Identity Integration

- Protocols: LDAP or OIDC in MVP.
- User mapping: external group -> platform role binding.
- Fallback: local admin account (break-glass path).

## 2.3 Third-Party Interfaces

- Outbound webhook events: asset changed, alert raised, ticket updated, workflow failed.
- Inbound callbacks: ticket status update, execution result callback, external asset sync.

## 2.4 Real-Time Streaming Path

1. `zabbix-adapter` receives or polls latest event/item updates.
2. `stream-processor` normalizes records into platform event schema.
3. Dedup/throttle/fingerprint rules are applied.
4. Events are published to NATS subjects by scope (`site`, `department`, `business`, `severity`).
5. `stream-gateway` pushes updates to frontend via WebSocket (SSE fallback).
6. UI widgets render incremental updates and maintain stream health state.

## 3. Draft Data Model

## 3.1 Core Entities

| Entity | Key Fields |
|---|---|
| `asset` | id, class, name, hostname, ip, serial_no, status, site_id, dept_id, owner_id |
| `asset_relation` | id, src_asset_id, dst_asset_id, relation_type, source, updated_at |
| `discovery_job` | id, method, scope, schedule, status, last_run_at |
| `discovery_candidate` | id, fingerprint, payload, match_score, review_status |
| `monitor_source` | id, type, endpoint, auth_ref, status |
| `monitor_binding` | id, asset_id, source_id, external_host_id |
| `alert` | id, source, severity, status, fingerprint, first_seen_at, last_seen_at |
| `metric_point` | id, source_id, asset_id, metric_key, value, ts |
| `stream_event` | id, channel, event_type, payload, ts, dedup_key |
| `stream_subscriber` | id, user_id, scope, protocol, connected_at, last_seen_at |
| `topology_node` | id, asset_id, node_type, attrs |
| `topology_link` | id, src_node_id, dst_node_id, link_key, attrs |
| `ticket` | id, type, status, requester_id, approver_chain, priority, due_at |
| `ticket_task` | id, ticket_id, mode(auto/manual), executor, status, result |
| `workflow_template` | id, name, trigger_type, definition_json, enabled |
| `org_unit` | id, type(org/site/dept), parent_id, name |
| `role_binding` | id, subject_id, role_id, scope_type, scope_id |
| `audit_log` | id, actor, action, object_type, object_id, payload, created_at |
| `i18n_key` | key, module, description |
| `i18n_translation` | key, locale, value, updated_at |

## 3.2 Relationship Notes

- `asset` links to monitoring by `monitor_binding`.
- `topology_link` supports multiple entries for same node pair via `link_key`.
- `ticket` links to `alert` and `asset` through reference tables for traceability.

## 4. API Baseline (MVP)

## 4.1 CMDB APIs

- `GET /api/v1/assets`
- `POST /api/v1/assets`
- `GET /api/v1/assets/{id}`
- `PATCH /api/v1/assets/{id}`
- `POST /api/v1/assets/import`
- `GET /api/v1/assets/{id}/relations`

## 4.2 Discovery APIs

- `POST /api/v1/discovery/jobs`
- `GET /api/v1/discovery/jobs`
- `POST /api/v1/discovery/jobs/{id}/run`
- `GET /api/v1/discovery/candidates`
- `POST /api/v1/discovery/candidates/{id}/approve`

## 4.3 Monitoring and Alert APIs

- `GET /api/v1/monitoring/overview`
- `GET /api/v1/monitoring/layers/{layer}`
- `GET /api/v1/monitoring/live/snapshot`
- `GET /api/v1/alerts`
- `POST /api/v1/alerts/{id}/ack`
- `POST /api/v1/alerts/{id}/close`

## 4.4 Topology APIs

- `GET /api/v1/topology/maps/{scope}`
- `GET /api/v1/topology/links/{id}`

## 4.5 Ticketing APIs

- `POST /api/v1/tickets`
- `GET /api/v1/tickets`
- `GET /api/v1/tickets/{id}`
- `POST /api/v1/tickets/{id}/approve`
- `POST /api/v1/tickets/{id}/execute`
- `POST /api/v1/tickets/{id}/close`

## 4.6 IAM and i18n APIs

- `POST /api/v1/auth/login`
- `POST /api/v1/auth/sso/callback`
- `GET /api/v1/me/permissions`
- `GET /api/v1/i18n/locales`
- `GET /api/v1/i18n/bundles/{locale}`
- `PUT /api/v1/me/preferences/language`

## 4.7 Real-Time Streaming APIs

- `GET /api/v1/streams/channels`
- `GET /api/v1/streams/ws` (WebSocket upgrade)
- `GET /api/v1/streams/sse` (fallback)
- `POST /api/v1/streams/subscriptions`
- `DELETE /api/v1/streams/subscriptions/{id}`

## 5. Workflow and Automation Design

## 5.1 Execution Modes

- `Auto`: runs scripts/API actions defined in workflow template.
- `Manual`: creates actionable task for operator with due time and form fields.
- `Hybrid`: auto steps plus manual approval/checkpoints.

## 5.2 Failure Strategy

- Retry policy: exponential backoff with capped attempts.
- Rollback policy: optional rollback graph per workflow template.
- Escalation policy: auto-notify on-call and convert to manual task.

## 6. i18n Technical Strategy

## 6.1 Frontend

- Use key-based translation framework (for example, i18next format).
- Organize locales by module: `home`, `cmdb`, `monitoring`, `topology`, `ticketing`, `admin`.
- Persist user language preference in profile.
- Fallback chain: `selected locale -> en-US`.

## 6.2 Backend

- Error/message payloads return stable codes with localizable message templates.
- Keep API contracts language-neutral; localize only presentation text.
- Optional admin API for translation key health check.

## 6.3 Quality Gates

- CI check: fail if new UI key has no `en-US` translation.
- Lint check: detect hardcoded strings in front-end pages/components.
- Release check: i18n coverage report for MVP routes.

## 7. Security and Compliance Baseline

- SSO integration with signed token validation.
- Secrets encrypted at rest and masked in UI/logs.
- Action-level audit logs for access, CMDB updates, approval actions, and automation execution.
- Least-privilege RBAC across org/site/dept/resource scopes.
- Stream channel authorization enforced per subscription scope and user role.
- Rate limits and connection caps enforced for WebSocket/SSE endpoints.

## 8. Delivery Plan (Engineering)

| Phase | Duration | Output |
|---|---|---|
| Phase 1 | Week 1-3 | Rust service foundation, IAM, org model, CMDB core, i18n skeleton |
| Phase 2 | Week 4-7 | Discovery engine, Zabbix adapter, stream-processor, monitoring overview |
| Phase 3 | Week 8-10 | Real-time gateway, live big-screen baseline, alert-to-ticket automation |
| Phase 4 | Week 11-12 | Topology P1 baseline, reporting, stream hardening |
| Phase 5 | Week 13-14 | Pilot, stabilization, docs for open-source release |

## 9. MVP Exit Criteria

- All P0 stories from backlog are complete and validated.
- Zabbix integration and alert lifecycle run stably under pilot load.
- Live dashboard and alert streams meet latency and reconnect KPIs.
- English UI is complete with no hardcoded non-localized text.
- i18n framework is active and capable of adding at least one extra locale without code refactor.

## 10. One-Click Installation Strategy

- Primary path: containerized install via `scripts/install.sh`.
- If Docker is missing, installer auto-bootstraps Docker (Linux/macOS).
- Dependencies start from `deploy/docker-compose.yml` with health checks.
- Installer guarantees MVP stack bootstrap: PostgreSQL, Redis, OpenSearch, MinIO, Zabbix server/proxy/web, and a local Zabbix agent container.
- Offline path: `scripts/build-offline-bundle.sh` creates a full air-gapped package with bundled images.
- Customer-side offline install uses a single command: `bash scripts/install-offline.sh`.
