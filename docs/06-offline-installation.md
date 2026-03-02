# CloudOps One Fully Offline Installation

Version: v1.0  
Date: 2026-03-02

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
4. Starts stack without pulling from external registries.

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
