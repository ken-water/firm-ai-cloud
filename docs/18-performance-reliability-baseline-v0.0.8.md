# CloudOps One Performance and Reliability Baseline (v0.0.8)

Version: v0.1  
Date: 2026-03-04

## 1. Goal

Establish a repeatable benchmark baseline for:

- Core API read endpoints (latency, success rate, throughput)
- SSE stream behavior under burst monitoring-sync events

## 2. Benchmark Scripts

- `scripts/benchmark-api-load.sh`
- `scripts/benchmark-sse-burst-smoke.sh`

## 3. Test Environment Snapshot

- Host OS: Ubuntu 24.04 kernel `6.17.0-14-generic`
- CPU: AMD Ryzen 7 5800H (8C/16T)
- Memory: 15 GiB
- Runtime stack:
  - API: local `cargo run -p api`
  - Infra: Docker Compose (`postgres`, `redis`, `opensearch`, `minio`, `zabbix-*`)

## 4. Execution Parameters

API benchmark run:

- Run ID: `20260304T124211Z`
- Requests per endpoint: `80`
- Warmup per endpoint: `10`
- Base URL: `http://127.0.0.1:8080`
- Output:
  - `.run/benchmarks/api-20260304T124211Z/summary.csv`
  - `.run/benchmarks/api-20260304T124211Z/summary.md`

SSE smoke run:

- Run ID: `20260304T124514Z`
- Stream duration: `35s`
- Burst events: `30` (interval `120ms`)
- Output:
  - `.run/benchmarks/sse-20260304T124514Z/summary.json`
  - `.run/benchmarks/sse-20260304T124514Z/summary.md`

## 5. API Baseline Metrics

| Endpoint | Success % | Avg (ms) | P95 (ms) | P99 (ms) | Success RPS |
| --- | --- | --- | --- | --- | --- |
| `/health` | 100.00 | 1.070 | 1.324 | 1.924 | 114.29 |
| `/api/v1/cmdb/assets` | 100.00 | 208.151 | 1206.309 | 2500.726 | 4.10 |
| `/api/v1/cmdb/assets/stats` | 100.00 | 108.211 | 484.155 | 1035.908 | 8.40 |
| `/api/v1/monitoring/overview` | 100.00 | 143.699 | 723.184 | 1129.664 | 6.32 |
| `/api/v1/monitoring/layers/hardware` | 100.00 | 159.947 | 863.577 | 1285.437 | 5.59 |
| `/api/v1/workflow/requests` | 100.00 | 127.542 | 684.002 | 850.146 | 6.62 |

## 6. SSE Burst Smoke Metrics

- Burst API trigger result: `30` success / `0` failure
- Stream events observed:
  - `stream.connected`: `1`
  - `stream.heartbeat`: `7`
  - `alert.test`: `1`
  - `alert.monitoring_sync`: `4`
  - `stream.stale`: `1`
  - `stream.recovered`: `0`
  - `stream.error`: `0`
- Smoke result: `PASS`

## 7. MVP KPI Comparison

| KPI | Target | Baseline | Status |
| --- | --- | --- | --- |
| API success rate (core read endpoints) | >= 99.0% | 100.0% | Pass |
| API P95 (`/health`) | <= 20ms | 1.324ms | Pass |
| API P95 (`/api/v1/cmdb/assets`) | <= 800ms | 1206.309ms | Fail |
| API P95 (`/api/v1/monitoring/overview`) | <= 800ms | 723.184ms | Pass |
| API P95 (`/api/v1/monitoring/layers/hardware`) | <= 800ms | 863.577ms | Fail |
| API P95 (`/api/v1/workflow/requests`) | <= 800ms | 684.002ms | Pass |
| SSE stream error events | 0 | 0 | Pass |
| SSE burst visibility | >= 1 `alert.monitoring_sync` event under burst | 4 events | Pass |

## 8. Known Bottlenecks

1. High tail latency on CMDB/monitoring list endpoints (`p95`/`p99` spikes).
2. SSE burst events are observable but not 1:1 with trigger count; stream emits aggregated job-state events because backend polling interval is 5s.
3. Under sustained benchmark traffic, API logs reported slow SQL statements on audit and monitoring binding writes (for example `INSERT INTO audit_logs` and monitoring sync upsert paths >1s).
4. Current benchmark is single-client sequential load; no multi-client concurrency baseline yet.

## 9. Next Actions

1. Add SQL explain/trace for `cmdb/assets` and `monitoring/layers` and create missing indexes for common filter/sort paths.
2. Optimize audit log write path (batching and/or async queue) and review monitoring sync write hot paths.
3. Add a second benchmark profile with parallel clients (for example 10/20 workers) and capture CPU/DB utilization.
4. Add SSE stress profile for 300+ burst jobs and measure event lag distribution.
5. Publish baseline delta tracking in release notes from v0.0.8 onward.

## 10. Re-run Commands

```bash
# API latency/throughput baseline
bash scripts/benchmark-api-load.sh

# SSE burst stability smoke
bash scripts/benchmark-sse-burst-smoke.sh
```
