# CloudOps One

CloudOps One is an open-source operations platform focused on:

- CMDB and asset relationships
- Zabbix-based monitoring visualization
- real-time alerting and big-screen operations views
- ticketing and workflow automation
- unified authentication and RBAC

## One-Click Install (Dependencies)

From repository root:

```bash
bash scripts/install.sh
```

What it does:

1. Detects Docker.
2. Installs Docker automatically if missing (Linux/macOS).
3. Starts PostgreSQL, Redis, OpenSearch, MinIO, and bundled Zabbix stack (server/web/proxy/local agent) via Compose.

More details: [docs/05-installation.md](docs/05-installation.md)

For China-network environments:

```bash
bash scripts/install.sh --skip-docker-install --mirror cn
```

## Manage Local Stack

```bash
# install / bootstrap
bash scripts/install.sh

# upgrade dependency images and recreate services
bash scripts/upgrade.sh

# start api + web for LAN access
bash scripts/dev-lan-up.sh

# stop LAN-mode api + web
bash scripts/dev-lan-down.sh

# uninstall containers only (keep data)
bash scripts/uninstall.sh

# uninstall including persisted data volumes
bash scripts/uninstall.sh --purge-data
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

- Zabbix Web UI: `http://127.0.0.1:8082`
- Default login: `Admin / zabbix`
- Server trapper port: `10051` (for proxy uplink)
- Proxy port: `10061` (for local/remote agents)

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
- [Release governance](docs/08-release-governance.md)
- [Changelog](CHANGELOG.md)

## Release Standards

Every version release must include detailed English release notes.

GitHub Actions CI is manual-by-default to control resource usage (`workflow_dispatch` only).

Use:

```bash
bash scripts/validate-release-note.sh release-notes/vX.Y.Z.md
```

Template:

- [`release-notes/TEMPLATE.md`](release-notes/TEMPLATE.md)
