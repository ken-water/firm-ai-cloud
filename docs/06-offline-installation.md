# CloudOps One Fully Offline Installation

Version: v1.1  
Date: 2026-03-03

## 1. Goal

Ship a complete offline bundle that can be installed in an air-gapped environment with one install command:

```bash
bash scripts/install-offline.sh
```

## 2. Build Bundle (Internet-Connected Build Host)

Build a China-mirror based offline package:

```bash
bash scripts/build-offline-bundle.sh --mirror cn
```

Build with default upstream image tags:

```bash
bash scripts/build-offline-bundle.sh --mirror default
```

Output archive:

- `dist/cloudops-one-offline-<timestamp>.tar.gz`

## 3. Transfer to Air-Gapped Customer Environment

Copy the generated `.tar.gz` archive to the target environment (USB/internal transfer).

Extract:

```bash
tar -xzf cloudops-one-offline-<timestamp>.tar.gz
cd cloudops-one-offline-<timestamp>
```

## 4. One-Command Offline Install

```bash
bash scripts/install-offline.sh
```

What this command does:

1. Optionally runs bundled offline Docker installer if Docker is missing.
2. Loads all bundled images from `images/cloudops-images.tar`.
3. Prepares `deploy/.env` from `deploy/.env.offline`.
4. Starts stack without pulling from external registries, including bundled Zabbix server/proxy/local-agent containers.
5. Bootstraps Zabbix defaults (proxy + local agent host) automatically.
6. Starts CloudOps API and CloudOps Web Console from bundled images.

## 5. Optional: Host Without Docker

To support hosts that do not have Docker:

1. Put offline Docker packages into `docker/packages/`.
2. Keep `docker/install-docker-offline.sh` in the bundle.
3. Run the same install command:

```bash
bash scripts/install-offline.sh
```

## 6. Offline Upgrade and Uninstall

```bash
# Upgrade from locally loaded images only
bash scripts/upgrade.sh --no-pull

# Stop/remove containers, keep data volumes
bash scripts/uninstall.sh

# Remove containers and volumes
bash scripts/uninstall.sh --purge-data
```

## 7. Validation

Check running services:

```bash
docker ps
```

Expected healthy containers:

- `deploy_postgres_1`
- `deploy_redis_1`
- `deploy_opensearch_1`
- `deploy_minio_1`
- `deploy_zabbix-db_1`
- `deploy_zabbix-server_1`
- `deploy_zabbix-web_1`
- `deploy_zabbix-proxy_1`
- `deploy_zabbix-agent-local_1`
- `deploy_api_1`
- `deploy_web_1`

Bundled monitoring access defaults:

- CloudOps Web Console: `http://127.0.0.1:8081`
- CloudOps API: `http://127.0.0.1:8080`
- Zabbix Web: `http://127.0.0.1:8082` (`Admin / zabbix`)
- Zabbix Proxy listener for agents: `<host-ip>:10061`
- Auto-provisioned local host: `cloudops-local-agent`
