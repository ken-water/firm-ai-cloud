# CloudOps One Information Architecture and Page Map

Version: v1.1  
Date: 2026-03-02  
Language policy: English-first UI, localization-ready architecture

## 1. IA Goals

- Deliver a unified user experience across CMDB, monitoring, and ticketing.
- Reduce context switching for operators and on-call engineers.
- Keep navigation consistent for both regular console and NOC big-screen modes.
- Ensure all user-facing text is localization-ready from day one.
- Support live data and live alerts in big-screen views without manual refresh.

## 2. Top-Level Navigation

| Menu | Purpose | Primary Roles |
|---|---|---|
| Home | Unified workspace and personal queue | All roles |
| CMDB | Asset inventory, relationships, lifecycle | Asset Admin, Ops |
| Monitoring | Multi-layer monitoring views and alert center | Ops, NOC, SRE |
| Topology | Auto-generated network/service topology and link analytics | Network Ops, NOC |
| Ticketing | Request, approval, execution, and closure | All roles |
| Reports | KPI, SLA, asset and process analytics | Managers, Finance, Audit |
| Integrations | Zabbix/SSO/Webhook/3rd-party connectors | Platform Admin |
| Administration | RBAC, org/site/dept settings, audit logs | Platform Admin, Security |

## 3. Page Inventory (MVP)

| Page ID | Route | Module | Description | MVP |
|---|---|---|---|---|
| P-HOME-01 | `/home` | Home | Personal dashboard: alerts, tickets, asset changes | P0 |
| P-CMDB-01 | `/cmdb/assets` | CMDB | Asset list with search, filters, batch actions | P0 |
| P-CMDB-02 | `/cmdb/assets/:id` | CMDB | Asset detail with lifecycle timeline and relations | P0 |
| P-CMDB-03 | `/cmdb/relations` | CMDB | Business-service-device-owner relationship graph | P0 |
| P-CMDB-04 | `/cmdb/discovery` | CMDB | Discovery jobs, candidate queue, merge decisions | P0 |
| P-MON-01 | `/monitoring/overview` | Monitoring | Layer summary: hardware/network/service/business | P0 |
| P-MON-02 | `/monitoring/alerts` | Monitoring | Unified alert center with dedup and status actions | P0 |
| P-MON-03 | `/monitoring/sla` | Monitoring | SLA and health score dashboard | P1 |
| P-TOPO-01 | `/topology/map` | Topology | Auto-drawn topology with multi-link visualization | P1 |
| P-TOPO-02 | `/topology/link/:id` | Topology | Link deep-dive: trend, alerts, metadata, changes | P1 |
| P-TKT-01 | `/tickets` | Ticketing | Ticket list and filters by status/type/owner | P0 |
| P-TKT-02 | `/tickets/new` | Ticketing | New request form from predefined templates | P0 |
| P-TKT-03 | `/tickets/:id` | Ticketing | Ticket detail: approval, execution logs, comments | P0 |
| P-TKT-04 | `/tickets/workflows` | Ticketing | Workflow templates and auto-execution policies | P0 |
| P-RPT-01 | `/reports/assets` | Reports | Asset analytics by site/dept/type/status | P0 |
| P-RPT-02 | `/reports/operations` | Reports | Ticket efficiency and automation rate | P1 |
| P-RPT-03 | `/reports/executive` | Reports | Executive KPIs and capacity trends | P1 |
| P-INT-01 | `/integrations/zabbix` | Integrations | Zabbix endpoints, mapping, sync health | P0 |
| P-INT-02 | `/integrations/sso` | Integrations | LDAP/OIDC config and mapping rules | P0 |
| P-INT-03 | `/integrations/webhooks` | Integrations | Outbound event and callback endpoints | P1 |
| P-ADM-01 | `/admin/users-roles` | Administration | Users, groups, role policies | P0 |
| P-ADM-02 | `/admin/org-structure` | Administration | Organization, datacenter, department hierarchy | P0 |
| P-ADM-03 | `/admin/audit-logs` | Administration | Security and operation audit logs | P0 |
| P-BIG-01 | `/screens/noc` | Big Screen | NOC status wall (live stream) | P0 |
| P-BIG-02 | `/screens/business-health` | Big Screen | Business health wall (live stream) | P1 |
| P-BIG-03 | `/screens/capacity` | Big Screen | Capacity and growth wall (live stream) | P1 |
| P-RT-01 | `/monitoring/live` | Monitoring | Real-time operations console with stream health and alert feed | P0 |

## 4. Key User Flows

### 4.1 Asset Onboarding and Monitoring Binding

1. Discovery job detects a new device.
2. Asset enters CMDB candidate queue.
3. Operator reviews and confirms merge/create.
4. Platform links asset to monitoring entity from Zabbix.
5. Asset appears in CMDB relations, monitoring views, and topology.

### 4.2 Alert to Ticket Closed Loop

1. Alert arrives from Zabbix adapter.
2. Rule engine matches policy and creates ticket.
3. Approval flow runs.
4. Execution auto-runs or routes to manual task.
5. Ticket close writes back to alert state and CMDB change timeline.

### 4.3 Topology Troubleshooting

1. NOC opens topology map and filters by site.
2. Operator selects a red link between two devices.
3. Link detail page shows metrics, active alerts, and recent changes.
4. Operator opens related ticket or creates a new one.

### 4.4 Big-Screen Live Operations

1. User opens `/screens/noc` in live mode.
2. Frontend subscribes to scoped stream channels (site/department/business).
3. Live metrics and alerts update continuously without full-page refresh.
4. Stream degradation switches widgets to stale state and shows reconnect progress.

## 5. UX Rules for Unified Experience

- One shared global header and left navigation across CMDB, monitoring, and ticketing.
- Consistent object entry points: every page can deep-link to asset, service, ticket, and alert.
- Unified status language: only use `Healthy`, `Warning`, `Critical`, `Unknown`.
- Cross-module side panel: quick context panel available for asset/alert/ticket references.
- Live widgets must show `Live`/`Delayed` status and last update timestamp.

## 6. Internationalization (i18n) Requirements

| Area | Requirement |
|---|---|
| Default language | English (`en-US`) |
| Translation model | Key-based dictionaries, no hardcoded UI copy |
| Runtime switching | User can switch language from profile/header |
| Formatting | Locale-aware date/time/number; timezone-aware timestamps |
| Fallback strategy | Missing key falls back to English |
| Governance | New UI changes must include i18n keys in PR checklist |

## 7. IA Notes for Future Languages

- Keep labels short to avoid layout breaks in translated languages.
- Use dynamic width components for table headers and tabs.
- Avoid embedding grammar-sensitive text in code; use interpolation templates.
- Keep all alert/ticket status labels canonical and map localized labels in UI only.
