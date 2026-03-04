# CloudOps One Installation Guide (One-Click)

Version: v1.1  
Date: 2026-03-03

## 1. Goal

Provide a one-click installation path for open-source users:

- If Docker is already installed: start immediately.
- If Docker is missing: installer auto-installs Docker (Linux/macOS supported) and continues.

## 2. What Gets Installed

The current installer starts CloudOps One full stack for MVP development:

- CloudOps API service
- CloudOps Web Console

- PostgreSQL
- Redis
- OpenSearch
- MinIO
- Zabbix MySQL
- Zabbix Server
- Zabbix Web
- Zabbix Proxy
- Local Zabbix Agent (containerized)

Compose files and env templates are located under:

- `deploy/docker-compose.yml`
- `deploy/.env.example`
- `deploy/.env.cn.example` (China mirror profile, currently `docker.1ms.run`)

## 3. Quick Start

Run from repository root:

```bash
bash scripts/install.sh
```

For China-network environments, apply the mirror profile first:

```bash
bash scripts/install.sh --skip-docker-install --mirror cn
```

This writes China mirror image sources into `deploy/.env` automatically.

The installer will:

1. Detect Docker.
2. Install Docker automatically if missing (unless disabled).
3. Generate `deploy/.env` from `deploy/.env.example` if absent.
4. Pull images.
5. Build CloudOps API/Web images locally if missing.
6. Start all services and wait for health checks.
7. Bootstrap Zabbix defaults (proxy + local agent host) via API.

## 4. Useful Options

```bash
# Do not install Docker automatically if missing
bash scripts/install.sh --skip-docker-install

# Skip image pulling and reuse local cache
bash scripts/install.sh --no-pull

# Start infra dependencies only (skip CloudOps API/Web)
bash scripts/install.sh --dependencies-only

# Explicit mode (same behavior as default for now)
bash scripts/install.sh --mode container
```

Key runtime env knobs in `deploy/.env`:

- `MONITORING_SECRET_ENCRYPTION_KEY` / `MONITORING_SECRET_INLINE_POLICY`
- `MONITORING_SYNC_WORKER_ENABLED` / `MONITORING_SYNC_POLL_SECONDS`
- `DISCOVERY_SCHEDULER_ENABLED` / `DISCOVERY_SCHEDULER_POLL_SECONDS`
- `WORKFLOW_EXECUTION_POLICY_MODE` / `WORKFLOW_EXECUTION_ALLOWLIST` / `WORKFLOW_EXECUTION_SANDBOX_DIR`

## 5. Endpoints

After installation, default local endpoints are:

- CloudOps API: `http://127.0.0.1:8080`
- CloudOps Web Console: `http://127.0.0.1:8081`
- PostgreSQL: `127.0.0.1:5432`
- Redis: `127.0.0.1:6379`
- OpenSearch: `http://127.0.0.1:9200`
- MinIO API: `http://127.0.0.1:9000`
- MinIO Console: `http://127.0.0.1:9001`
- Zabbix Web: `http://127.0.0.1:8082`
- Zabbix Server trapper: `0.0.0.0:10051`
- Zabbix Proxy for agents: `0.0.0.0:10061`

Default Zabbix login:

- Username: `Admin`
- Password: `zabbix`

Default auto-provisioned monitoring objects:

- Proxy: `cloudops-proxy`
- Local host: `cloudops-local-agent` (through proxy, interface DNS `zabbix-agent-local`)

## 6. Stop / Restart

```bash
docker compose --env-file deploy/.env -f deploy/docker-compose.yml down
docker compose --env-file deploy/.env -f deploy/docker-compose.yml up -d
docker compose --env-file deploy/.env -f deploy/docker-compose.yml logs -f
```

## 7. Upgrade

Upgrade dependency images and recreate services (CloudOps API/Web images are rebuilt if missing):

```bash
bash scripts/upgrade.sh
```

Common options:

```bash
# Recreate from local images only
bash scripts/upgrade.sh --no-pull

# Skip waiting for health checks
bash scripts/upgrade.sh --skip-healthcheck
```

Manual Zabbix bootstrap rerun:

```bash
bash scripts/bootstrap-zabbix.sh --env-file deploy/.env
```

## 8. Backup and Restore

Backup and restore scripts are included for operations baseline:

```bash
# Create one backup artifact set under backups/stack-backup-<timestamp>
bash scripts/backup-stack.sh

# Restore from a backup directory (destructive to current stack data)
bash scripts/restore-stack.sh --input-dir backups/stack-backup-<timestamp> --yes
```

Detailed runbook:

- `docs/17-disaster-recovery-runbook.md`

## 9. Uninstall

```bash
# Stop and remove containers, keep persisted data volumes
bash scripts/uninstall.sh

# Fully clean containers + volumes
bash scripts/uninstall.sh --purge-data

# Also remove deploy/.env
bash scripts/uninstall.sh --purge-data --remove-env
```

## 10. Security Notes

- Default passwords in `deploy/.env.example` are for local development only.
- Before production use, update all credentials and limit exposed ports.

## 11. Air-Gapped Environments

Use the dedicated offline flow documented in:

- `docs/06-offline-installation.md`
