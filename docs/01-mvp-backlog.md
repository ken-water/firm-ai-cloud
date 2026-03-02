# CloudOps One MVP Backlog (Epic -> Story -> Acceptance Criteria)

Version: v1.2  
Date: 2026-03-02  
Phase: MVP (P0)

## 1. Scope

- Goal: deliver an integrated loop across CMDB, monitoring, ticket automation, and identity access.
- Target orgs: enterprise teams with datacenter/network assets and existing Zabbix operations.
- Timeline baseline: 12 weeks build + 2 weeks pilot.
- Documentation language: English-only for all public/open-source docs.

## 2. Priority Rules

- `P0`: required for initial release.
- `P1`: release + 1 to 2 iterations.
- `P2`: enhancement backlog.

## 3. Epic List

| Epic ID | Epic Name | Priority | Outcome |
|---|---|---|---|
| E1 | CMDB and Asset Management | P0 | Assets, relationships, and lifecycle fully traceable |
| E2 | Auto Discovery and Change Notification | P0 | Device onboarding/offboarding detected and notified automatically |
| E3 | Zabbix Integration and Monitoring Views | P0 | Unified multi-layer monitoring experience |
| E4 | Topology and Big-Screen Visualization | P1 | Auto-generated topology, multi-link visualization, NOC screens |
| E5 | Ticketing and Workflow Automation | P0 | Request-approval-execution-close loop with automation |
| E6 | Unified Auth and RBAC | P0 | Enterprise SSO + local fallback + full auditability |
| E7 | Third-Party System Integration | P1 | CMDB/ticket synchronization with external systems |
| E8 | Internationalization Foundation | P0 | English UI now, scalable to additional languages later |
| E9 | Real-Time Monitoring and Alert Streaming | P0 | Live dashboard updates and low-latency alert delivery |

## 4. Story Breakdown

### E1: CMDB and Asset Management (P0)

| Story ID | User Story | Acceptance Criteria (AC) | Priority |
|---|---|---|---|
| E1-S1 | As an asset admin, I can define asset classes and field templates | At least 8 built-in asset classes; field types include text, enum, date, and relation; template versioning available | P0 |
| E1-S2 | As an operator, I can create and update assets | Single-create and batch-import are supported; required field validation; import error report downloadable | P0 |
| E1-S3 | As an admin, I can manage lifecycle states | States include active, maintenance, decommissioned, retired; each transition stores who/when/why | P0 |
| E1-S4 | As a manager, I can see asset analytics by site/department/type | Dashboards include total count, active rate, decommission rate; day/week/month aggregation | P0 |
| E1-S5 | As a platform user, I can inspect device-business-owner-system relationships | Any asset has an upstream/downstream relationship view; relationship filters are supported | P0 |

### E2: Auto Discovery and Change Notification (P0)

| Story ID | User Story | Acceptance Criteria (AC) | Priority |
|---|---|---|---|
| E2-S1 | As an admin, I can configure multiple discovery methods | SNMP, SSH, and API discovery supported in MVP; each method has scan scope and schedule | P0 |
| E2-S2 | As a platform, I can detect and stage new devices | New devices go to a review queue; dedup by configurable policy (IP+SN, hostname, etc.) | P0 |
| E2-S3 | As a platform, I can detect down/offboarded devices | Consecutive failed scans trigger suspected-offboarded event; manual confirm flow supported | P0 |
| E2-S4 | As a stakeholder, I receive change notifications | In-app, email, and webhook channels supported; template by event type | P0 |

### E3: Zabbix Integration and Monitoring Views (P0)

| Story ID | User Story | Acceptance Criteria (AC) | Priority |
|---|---|---|---|
| E3-S1 | As an admin, I can connect Zabbix Server/Proxy data sources | Multiple Zabbix instances supported; connection health visible; credentials encrypted at rest | P0 |
| E3-S2 | As an operator, I can view hardware/network/service/business layers | Four-layer dashboards include status summary, top anomalies, and alert list; filter by site and department | P0 |
| E3-S3 | As an on-call engineer, I can use a unified alert center | Alert dedup, aggregation, acknowledge, and close are supported; Zabbix event ID mapping retained | P0 |
| E3-S5 | As an operator, I can see near-real-time metric updates in monitoring views | Critical metric widgets refresh in <= 5 seconds for live mode; stale-state indicator shown when stream is interrupted | P0 |
| E3-S4 | As a manager, I can review SLA and service health score | Availability, latency, and error metrics supported; daily/weekly export supported | P1 |

### E4: Topology and Big-Screen Visualization (P1)

| Story ID | User Story | Acceptance Criteria (AC) | Priority |
|---|---|---|---|
| E4-S1 | As a network admin, I can auto-generate topology maps | Topology generated from CMDB relations + monitoring edges; manual correction with audit log | P1 |
| E4-S2 | As an operator, I can view multiple lines between two devices | Multi-link rendering between same device pair supported; each link shows bandwidth/latency/loss | P1 |
| E4-S3 | As an on-call engineer, I can click a link for deep diagnostics | Link detail includes 24h trend, active alerts, metadata, and recent changes | P1 |
| E4-S4 | As leadership, I can use role-specific big screens | At least three templates: NOC overview, business health, and capacity trend | P1 |

### E9: Real-Time Monitoring and Alert Streaming (P0)

| Story ID | User Story | Acceptance Criteria (AC) | Priority |
|---|---|---|---|
| E9-S1 | As a NOC user, I can open a live big-screen mode | Big-screen pages support WebSocket/SSE live mode with auto reconnect and stream status indicator | P0 |
| E9-S2 | As an on-call engineer, I receive alert updates in seconds | New critical alerts are pushed to UI in <= 3 seconds from ingestion in normal load | P0 |
| E9-S3 | As an admin, I can control real-time channels by site/department | Stream subscriptions support scope filtering (site, department, business) and role-based access control | P0 |
| E9-S4 | As an operator, I can keep live dashboards stable under burst alerts | Burst handling includes dedup/throttle rules and UI queueing without page freeze | P0 |

### E5: Ticketing and Workflow Automation (P0)

| Story ID | User Story | Acceptance Criteria (AC) | Priority |
|---|---|---|---|
| E5-S1 | As a user, I can submit standardized ticket forms | At least 6 ticket templates; form and attachment validation; unique ticket ID on submit | P0 |
| E5-S2 | As an approver, I can complete multi-step approvals | Sequential and parallel approval supported; timeout reminders; decision comments retained | P0 |
| E5-S3 | As a platform, I can run auto-execution after approval | Supports script/API/task graph actions; execution logs replayable; failover to rollback/manual path | P0 |
| E5-S4 | As an executor, I can process manual tasks | Claim/reassign supported; required result fields; optional second-level review | P0 |
| E5-S5 | As a platform, I can auto-create tickets from alerts | Rule-matched alerts create tickets automatically; ticket closure writes back to alert and CMDB change history | P0 |

### E6: Unified Auth and RBAC (P0)

| Story ID | User Story | Acceptance Criteria (AC) | Priority |
|---|---|---|---|
| E6-S1 | As an admin, I can connect enterprise identity providers | LDAP or OIDC required for MVP; fallback behavior configurable; user mapping policy supported | P0 |
| E6-S2 | As a platform, I preserve local account fallback | Local login enable/disable/reset supported; local login can be restricted to break-glass roles | P0 |
| E6-S3 | As an admin, I can configure granular authorization | Organization/department/resource scopes supported; menu/data/action permissions included | P0 |
| E6-S4 | As an auditor, I can query operation logs | Login, permission, CMDB, and ticket actions fully logged; searchable and CSV-exportable | P0 |

### E7: Third-Party System Integration (P1)

| Story ID | User Story | Acceptance Criteria (AC) | Priority |
|---|---|---|---|
| E7-S1 | As an admin, I can sync ticket states with external ITSM | Webhook + callback integration; customizable status mapping | P1 |
| E7-S2 | As a platform, I can sync asset data with external asset tools | Full + incremental sync supported; conflict policy configurable | P1 |

### E8: Internationalization Foundation (P0)

| Story ID | User Story | Acceptance Criteria (AC) | Priority |
|---|---|---|---|
| E8-S1 | As a user, I can use the product in English by default | Default locale is `en-US`; no hardcoded non-English text in UI | P0 |
| E8-S2 | As a maintainer, I can add new languages without code rewrite | All UI strings externalized as translation keys; locale pack loading supported | P0 |
| E8-S3 | As a user, I can switch language at runtime | Language switcher available in profile/global header; preference persisted per user | P1 |
| E8-S4 | As a global user, I see locale-aware format styles | Date/time/number formatting follows selected locale; timezone support included | P1 |

## 5. Non-Functional Acceptance (MVP)

| Category | KPI |
|---|---|
| Performance | Core pages P95 <= 2s; alert list query P95 <= 3s |
| Availability | Monthly availability >= 99.9% |
| Data Freshness | Monitoring sync lag <= 60s for full pull paths; live mode update lag <= 5s |
| Real-Time Alerting | Critical alert push latency <= 3s for subscribed clients in normal load |
| Realtime Resilience | Stream reconnect <= 10s after transient disconnect; no duplicate alert cards after reconnect |
| Security | Audit coverage for login, permission change, asset change, and approvals |
| Reliability | Automated workflow success rate >= 95% |
| Localization | 100% UI copy key coverage for MVP pages in English |

## 6. MVP Definition of Done

- All `P0` stories delivered and passed UAT.
- Pilot in at least 2 sites and 2 departments with production-like traffic.
- Auto discovery and alert-to-ticket loop runs stably for 2 weeks.
- Public documents are English and include install, operations, and user guides.
