# CloudOps One Disaster Recovery Runbook

Version: v0.1  
Date: 2026-03-04

## 1. Purpose

Define the minimum backup/restore operating baseline for CloudOps One MVP deployments.

## 2. Scope

This runbook covers:

- PostgreSQL logical backup/restore (`pg_dump` + `psql`)
- Named volume backup/restore for critical services:
  - PostgreSQL data volume
  - MinIO data volume
  - OpenSearch data volume
  - Zabbix DB data volume
  - Zabbix Proxy data volume

This runbook does not cover cross-region replication or zero-downtime failover.

## 3. Baseline Targets (MVP)

- Backup frequency: daily full backup
- Retention: at least 7 daily copies
- Target RPO: <= 24 hours
- Target RTO: <= 2 hours for single-node recovery

These are baseline targets for self-hosted MVP. Production targets can be stricter per customer SLA.

## 4. Prerequisites

- Docker and Docker Compose are available on target host.
- Stack env file exists (`deploy/.env` or custom path).
- Operator has filesystem access to store backup artifacts.
- Stack is healthy before backup (`docker compose ... ps`).

## 5. Backup Procedure

Run from repository root:

```bash
bash scripts/backup-stack.sh
```

Optional parameters:

```bash
# custom output directory
bash scripts/backup-stack.sh --output-dir /data/backups/cloudops-$(date +%F)

# custom compose/env/project
bash scripts/backup-stack.sh \
  --compose-file deploy/docker-compose.yml \
  --env-file deploy/.env \
  --project-name deploy
```

Generated artifacts include:

- `postgres/pg_dump.sql.gz`
- `volumes/*.tar.gz`
- `metadata.env`
- `SHA256SUMS`

Integrity check:

```bash
cd <backup-dir>
sha256sum -c SHA256SUMS
```

## 6. Restore Procedure

Restore is destructive to current stack data volumes in the selected compose project.

```bash
bash scripts/restore-stack.sh --input-dir <backup-dir> --yes
```

Optional parameters:

```bash
# skip PostgreSQL restore
bash scripts/restore-stack.sh --input-dir <backup-dir> --skip-postgres --yes

# skip volume restore
bash scripts/restore-stack.sh --input-dir <backup-dir> --skip-volumes --yes
```

Restore sequence implemented by script:

1. Stop running stack.
2. Restore selected named volumes from tar archives.
3. Start PostgreSQL and restore SQL dump if present.
4. Start full stack.

If full stack startup fails (for example, app images are not locally available and registry is unreachable), restore script falls back to infrastructure services startup (`postgres/redis/opensearch/minio/zabbix-*`) so core data services remain recoverable.

## 7. Recovery Drill (Recommended Quarterly)

Use one command in a non-production environment:

```bash
bash scripts/dr-drill.sh --env-file deploy/.env --output-dir .run/dr-drill/<run-id> --yes
```

The drill automation performs:

1. backup (`scripts/backup-stack.sh`)
2. restore (`scripts/restore-stack.sh`)
3. verification checks (compose status, API/web/Zabbix reachability, CMDB/monitoring query)
4. report generation with explicit pass/fail per check

Generated drill artifacts:

- `report.json` (machine-readable)
- `report.md` (human-readable summary)
- `logs/*.log` (per-check command outputs for failure diagnosis)
- `backup/` (backup artifacts produced during this drill)

If any check fails, script exits non-zero and report includes actionable failure references.

## 8. Verification Checklist

After restore, verify:

- Compose services are running:

```bash
docker compose --env-file deploy/.env -f deploy/docker-compose.yml ps
```

- API health:

```bash
curl -fsS http://127.0.0.1:8080/health
```

- Web console is reachable:

```bash
curl -I http://127.0.0.1:8081
```

- Zabbix Web is reachable:

```bash
curl -I http://127.0.0.1:8082
```

- CMDB data exists (sample check):

```bash
curl -H 'x-auth-user: admin' http://127.0.0.1:8080/api/v1/cmdb/assets
```

- Monitoring source and latest sync jobs can be queried without error.

## 9. Operational Notes

- For production, encrypt and protect backup storage.
- Keep backups on different storage/media from the running host.
- Run periodic restore drills; backup without restore test is insufficient.
- If `POSTGRES_IMAGE` is customized in env, scripts reuse that image as tool image unless overridden with `--tool-image`.
