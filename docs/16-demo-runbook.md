# CloudOps One Demo Runbook

Version: v0.1  
Date: 2026-03-03

## 1. Goal

This runbook prepares a repeatable customer demo with:

- CMDB hierarchy and relationship data
- Monitoring source and sync job activity
- Discovery candidates and notification delivery records
- Local snapshot artifacts for quick proof during presentation

## 2. Prerequisites

- Dependency stack running (`postgres`, `redis`, `opensearch`, `minio`, `zabbix-*`)
- API running at `http://127.0.0.1:8080`
- `curl` and `jq` installed
- RBAC bootstrap user available (`admin` by default)

Optional check:

```bash
curl -fsS http://127.0.0.1:8080/health
```

## 3. Prepare Demo Data

Run one command:

```bash
bash scripts/demo-seed-data.sh
```

Outputs:

- Manifest: `.run/demo/<demo-tag>-manifest.json`
- Snapshots: `.run/demo/<demo-tag>-snapshots/`

Example with fixed tag:

```bash
DEMO_TAG=demo-customer-a bash scripts/demo-seed-data.sh
```

## 4. Validate Demo Dataset

Run:

```bash
bash scripts/demo-health-check.sh
```

Optional explicit target:

```bash
bash scripts/demo-health-check.sh --tag demo-customer-a
```

Output report:

- `.run/demo/<demo-tag>-health-check.json`

## 5. Suggested 30-Minute Demo Flow

1. Open CMDB asset list and show mixed asset classes (`physical_host`, `virtual_machine`, `container`, `database`, `network_device`, `business_service`, `team`).
2. Open an asset detail, show bindings and lifecycle readiness (idle vs operational).
3. Show relation graph and impact API output (`contains`, `depends_on`, `runs_service`, `owned_by`).
4. Show monitoring source list and probe status.
5. Show monitoring overview and layer pages (`hardware`, `network`, `service`, `business`).
6. Show discovery jobs, run history, candidates, and notification deliveries.
7. Show generated snapshot files and manifest as evidence package.

Useful API checks during demo:

```bash
AUTH_HEADER='x-auth-user: admin'
curl -H "$AUTH_HEADER" "http://127.0.0.1:8080/api/v1/monitoring/overview?site=dc-a"
curl -H "$AUTH_HEADER" "http://127.0.0.1:8080/api/v1/cmdb/discovery/candidates?review_status=pending&limit=50"
curl -H "$AUTH_HEADER" "http://127.0.0.1:8080/api/v1/cmdb/discovery/notification-deliveries?limit=50"
```

## 6. Screen Recording Commands

### Linux (X11 + FFmpeg)

```bash
ffmpeg -y -video_size 1920x1080 -framerate 30 -f x11grab -i :0.0 \
  -c:v libx264 -preset veryfast -crf 23 demo-cloudops-one.mp4
```

### macOS (QuickTime)

Use QuickTime Player -> `File` -> `New Screen Recording`, then export to `.mp4`.

## 7. Cleanup Demo Data

Dry-run preview:

```bash
bash scripts/demo-cleanup-data.sh --tag demo-customer-a --dry-run
```

Actual cleanup:

```bash
bash scripts/demo-cleanup-data.sh --tag demo-customer-a
```

Or use manifest directly:

```bash
bash scripts/demo-cleanup-data.sh --manifest .run/demo/demo-customer-a-manifest.json
```
