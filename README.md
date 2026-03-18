# CloudOps One

CloudOps One is an open-source operations platform focused on:

- CMDB and asset relationships
- Zabbix-based monitoring visualization
- real-time alerting and big-screen operations views
- ticketing and workflow automation
- unified authentication and RBAC

## One-Click Install (Full Stack)

From repository root:

```bash
bash scripts/install.sh
```

What it does:

1. Detects Docker.
2. Installs Docker automatically if missing (Linux/macOS).
3. Builds CloudOps API/Web images if missing.
4. Starts CloudOps API, CloudOps web console, PostgreSQL, Redis, OpenSearch, MinIO, and bundled Zabbix stack (server/web/proxy/local agent) via Compose.
5. Bootstraps Zabbix defaults: creates proxy `cloudops-proxy` and local host `cloudops-local-agent`.

More details: [docs/05-installation.md](docs/05-installation.md)

For China-network environments:

```bash
bash scripts/install.sh --skip-docker-install --mirror cn
```

For developer mode (dependencies only, local API/Web by source):

```bash
bash scripts/install.sh --skip-docker-install --mirror cn --dependencies-only
```

## Manage Local Stack

```bash
# install / bootstrap
bash scripts/install.sh

# upgrade dependency images, rebuild app images if needed, and recreate services
bash scripts/upgrade.sh

# start api + web for LAN access
bash scripts/dev-lan-up.sh

# stop LAN-mode api + web
bash scripts/dev-lan-down.sh

# uninstall containers only (keep data)
bash scripts/uninstall.sh

# uninstall including persisted data volumes
bash scripts/uninstall.sh --purge-data

# create backup artifacts
bash scripts/backup-stack.sh

# restore stack data from backup artifacts
bash scripts/restore-stack.sh --input-dir backups/stack-backup-<timestamp> --yes

# run end-to-end DR drill and generate report artifacts
bash scripts/dr-drill.sh --env-file deploy/.env --output-dir .run/dr-drill/<run-id> --yes

# run benchmark threshold gate from benchmark artifacts
bash scripts/benchmark-threshold-gate.sh \
  --api-summary .run/benchmarks/api-<run-id>/summary.csv \
  --sse-summary .run/benchmarks/sse-<run-id>/summary.json
```

## Fully Offline Delivery

Build an offline package on an internet-connected machine:

```bash
bash scripts/build-offline-bundle.sh --mirror cn
```

At customer site (after extracting the bundle), use one command:

```bash
bash scripts/install-offline.sh
```

Details: [docs/06-offline-installation.md](docs/06-offline-installation.md)

Default bundled Zabbix access:

- CloudOps Web Console: `http://127.0.0.1:8081`
- CloudOps API: `http://127.0.0.1:8080`
- Zabbix Web UI: `http://127.0.0.1:8082`
- Default login: `Admin / zabbix`
- Server trapper port: `10051` (for proxy uplink)
- Proxy port: `10061` (for local/remote agents)
- Auto-provisioned local host: `cloudops-local-agent` (via proxy `cloudops-proxy`)

Manual rerun (if needed):

```bash
bash scripts/bootstrap-zabbix.sh --env-file deploy/.env
```

## Product Planning Docs

- [MVP backlog](docs/01-mvp-backlog.md)
- [Information architecture](docs/02-information-architecture.md)
- [Technical blueprint](docs/03-technical-blueprint.md)
- [ADR-0001 data stack decision](docs/04-adr-data-and-search-stack.md)
- [Developer quickstart](docs/07-dev-quickstart.md)
- [v0.0.2 sprint backlog](docs/09-v0.0.2-sprint-backlog.md)
- [Security operations guide (v0.0.3)](docs/11-security-operations-v0.0.3.md)
- [v0.0.4 UI iteration plan](docs/12-v0.0.4-ui-iteration-plan.md)
- [v0.0.5 monitoring bootstrap plan](docs/14-v0.0.5-monitoring-bootstrap-plan.md)
- [Disaster recovery runbook](docs/17-disaster-recovery-runbook.md)
- [Performance and reliability baseline (v0.0.8)](docs/18-performance-reliability-baseline-v0.0.8.md)
- [v0.0.9 topology + reliability plan](docs/19-v0.0.9-topology-reliability-plan.md)
- [v0.1.x operator simplicity + efficiency plan](docs/20-v0.1x-operator-simplicity-plan.md)
- [v0.1.1 validation suite](docs/21-v0.1.1-validation-suite.md)
- [v0.1.1 operator runbook](docs/22-v0.1.1-operator-runbook.md)
- [v0.1.2 security and scale evidence](docs/23-v0.1.2-security-scale-evidence.md)
- [v0.1.3 capacity guidance](docs/24-v0.1.3-capacity-guidance.md)
- [v0.1.4 safe-ops planning](docs/25-v0.1.4-safe-ops-planning.md)
- [v0.1.4 ops continuity and digest](docs/26-v0.1.4-ops-continuity-and-digest.md)
- [v0.1.4 long-soak stability](docs/27-v0.1.4-long-soak-stability.md)
- [v0.1.4 multi-region simulation](docs/28-v0.1.4-multi-region-simulation.md)
- [v0.1.5 operator autonomy plan](docs/29-v0.1.5-operator-autonomy-plan.md)
- [v0.1.5 escalation failover simulation](docs/30-v0.1.5-escalation-failover-simulation.md)
- [v0.1.5 operator journey validation](docs/31-v0.1.5-operator-journey-validation.md)
- [v0.1.6 operator simplicity v2 plan](docs/32-v0.1.6-operator-simplicity-v2-plan.md)
- [v0.1.6 release gate checklist](docs/33-v0.1.6-release-gate-checklist.md)
- [v0.1.7 runbook execution closure plan](docs/34-v0.1.7-runbook-execution-closure-plan.md)
- [v0.1.7 release gate checklist](docs/35-v0.1.7-release-gate-checklist.md)
- [v0.1.8 runbook standardization plan](docs/36-v0.1.8-runbook-standardization-plan.md)
- [v0.1.8 release gate checklist](docs/37-v0.1.8-release-gate-checklist.md)
- [v0.1.9 runbook analytics visibility plan](docs/38-v0.1.9-runbook-analytics-visibility-plan.md)
- [v0.1.9 release gate checklist](docs/39-v0.1.9-release-gate-checklist.md)
- [v0.1.10 runbook risk policy plan](docs/40-v0.1.10-runbook-risk-policy-plan.md)
- [v0.1.10 release gate checklist](docs/41-v0.1.10-release-gate-checklist.md)
- [v0.1.11 runbook risk-to-ticket closure plan](docs/42-v0.1.11-runbook-risk-ticket-closure-plan.md)
- [v0.1.11 release gate checklist](docs/43-v0.1.11-release-gate-checklist.md)
- [v0.1.12 runbook risk dispatch and ownership routing plan](docs/44-v0.1.12-runbook-risk-dispatch-routing-plan.md)
- [v0.1.12 release gate checklist](docs/45-v0.1.12-release-gate-checklist.md)
- [v0.1.13 owner directory and notification readiness plan](docs/46-v0.1.13-owner-directory-readiness-plan.md)
- [v0.1.13 release gate checklist](docs/47-v0.1.13-release-gate-checklist.md)
- [v0.1.14 owner readiness repair and bootstrap plan](docs/48-v0.1.14-owner-readiness-repair-plan.md)
- [v0.1.14 release gate checklist](docs/49-v0.1.14-release-gate-checklist.md)
- [v0.1.15 integration productization and guided bootstrap plan](docs/50-v0.1.15-integration-bootstrap-plan.md)
- [v0.1.15 release gate checklist](docs/51-v0.1.15-release-gate-checklist.md)
- [v0.1.16 go-live readiness workspace plan](docs/52-v0.1.16-go-live-readiness-plan.md)
- [v0.1.16 release gate checklist](docs/53-v0.1.16-release-gate-checklist.md)
- [v0.1.17 first-value activation and pilot feedback plan](docs/54-v0.1.17-first-value-activation-plan.md)
- [v0.1.17 release gate checklist](docs/55-v0.1.17-release-gate-checklist.md)
- [v0.1.18 daily ops return loop and follow-up closure plan](docs/56-v0.1.18-daily-ops-return-loop-plan.md)
- [v0.1.18 release gate checklist](docs/57-v0.1.18-release-gate-checklist.md)
- [v0.1.19 daily ops ownership and closure continuity plan](docs/58-v0.1.19-daily-ops-ownership-closure-plan.md)
- [v0.1.19 release gate checklist](docs/59-v0.1.19-release-gate-checklist.md)
- [v0.1.20 owner assignment and escalation action loop plan](docs/60-v0.1.20-owner-assignment-escalation-loop-plan.md)
- [v0.1.20 release gate checklist](docs/61-v0.1.20-release-gate-checklist.md)
- [SMB intelligent cloud product strategy baseline](docs/62-smb-intelligent-cloud-product-strategy-baseline.md)
- [v0.1.21 dashboard + business/org/AI baseline plan](docs/63-v0.1.21-dashboard-business-org-ai-plan.md)
- [v0.1.21 release gate checklist](docs/64-v0.1.21-release-gate-checklist.md)
- [Release governance](docs/08-release-governance.md)
- [Changelog](CHANGELOG.md)

## Release Standards

Every version release must include detailed English release notes.

GitHub Actions CI is manual-by-default to control resource usage (`workflow_dispatch` only).

Release command flow:

```bash
make release-publish-dry VERSION=X.Y.Z
make release-publish VERSION=X.Y.Z
make release-check VERSION=X.Y.Z
```

Template:

- [`release-notes/TEMPLATE.md`](release-notes/TEMPLATE.md)

Direct scripts:

- `bash scripts/release-publish.sh --version X.Y.Z`
- `bash scripts/release-sync-check.sh --version X.Y.Z`
