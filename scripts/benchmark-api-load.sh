#!/usr/bin/env bash
set -Eeuo pipefail

API_BASE_URL="${API_BASE_URL:-http://127.0.0.1:8080}"
AUTH_USER="${AUTH_USER:-admin}"
REQUESTS_PER_ENDPOINT="${REQUESTS_PER_ENDPOINT:-80}"
WARMUP_REQUESTS="${WARMUP_REQUESTS:-10}"
REQUEST_TIMEOUT_SECONDS="${REQUEST_TIMEOUT_SECONDS:-8}"
CONCURRENCY="${CONCURRENCY:-1}"
RUN_ID="${RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)}"
OUTPUT_DIR="${OUTPUT_DIR:-.run/benchmarks/api-${RUN_ID}}"
RAW_DIR="${OUTPUT_DIR}/raw"
SUMMARY_CSV="${OUTPUT_DIR}/summary.csv"
SUMMARY_MD="${OUTPUT_DIR}/summary.md"
UTILIZATION_CSV="${OUTPUT_DIR}/utilization.csv"

ENDPOINT_SPECS=(
  "health|GET|/health|none"
  "cmdb_assets|GET|/api/v1/cmdb/assets?limit=50&offset=0|auth"
  "cmdb_asset_stats|GET|/api/v1/cmdb/assets/stats|auth"
  "monitoring_overview|GET|/api/v1/monitoring/overview|auth"
  "monitoring_layer_hardware|GET|/api/v1/monitoring/layers/hardware?limit=20&offset=0|auth"
  "workflow_requests|GET|/api/v1/workflow/requests?limit=20|auth"
)

log() {
  printf '[benchmark-api] %s\n' "$*"
}

fatal() {
  printf '[benchmark-api][ERROR] %s\n' "$*" >&2
  exit 1
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || fatal "missing required command: $1"
}

to_ms() {
  local seconds="$1"
  awk -v s="${seconds}" 'BEGIN { printf "%.3f", s * 1000 }'
}

percentile_from_sorted() {
  local sorted_file="$1"
  local n="$2"
  local percentile="$3"

  if (( n == 0 )); then
    echo "0.000"
    return
  fi

  local index=$(( (n * percentile + 99) / 100 ))
  if (( index < 1 )); then
    index=1
  fi
  if (( index > n )); then
    index="$n"
  fi

  sed -n "${index}p" "${sorted_file}"
}

run_request() {
  local method="$1"
  local path="$2"
  local auth_mode="$3"

  local response
  if [[ "${auth_mode}" == "auth" ]]; then
    if ! response="$(curl -sS -o /dev/null -w '%{http_code} %{time_total}' \
      --max-time "${REQUEST_TIMEOUT_SECONDS}" \
      -X "${method}" \
      -H "x-auth-user: ${AUTH_USER}" \
      "${API_BASE_URL}${path}")"; then
      echo "000 $(to_ms "${REQUEST_TIMEOUT_SECONDS}")"
      return
    fi
  else
    if ! response="$(curl -sS -o /dev/null -w '%{http_code} %{time_total}' \
      --max-time "${REQUEST_TIMEOUT_SECONDS}" \
      -X "${method}" \
      "${API_BASE_URL}${path}")"; then
      echo "000 $(to_ms "${REQUEST_TIMEOUT_SECONDS}")"
      return
    fi
  fi

  local status="${response%% *}"
  local seconds="${response##* }"
  echo "${status} $(to_ms "${seconds}")"
}

benchmark_endpoint() {
  local name="$1"
  local method="$2"
  local path="$3"
  local auth_mode="$4"

  local result_file="${RAW_DIR}/${name}.result"
  local times_file="${RAW_DIR}/${name}.times_ms"
  local status_file="${RAW_DIR}/${name}.status"
  local sorted_file="${RAW_DIR}/${name}.times_ms.sorted"

  : >"${result_file}"
  : >"${times_file}"
  : >"${status_file}"

  log "Warmup ${name}: ${WARMUP_REQUESTS} requests"
  for ((i = 1; i <= WARMUP_REQUESTS; i++)); do
    run_request "${method}" "${path}" "${auth_mode}" >/dev/null
  done

  log "Benchmark ${name}: ${REQUESTS_PER_ENDPOINT} requests (concurrency=${CONCURRENCY})"
  local start_ns end_ns
  start_ns="$(date +%s%N)"
  if (( CONCURRENCY <= 1 )); then
    for ((i = 1; i <= REQUESTS_PER_ENDPOINT; i++)); do
      run_request "${method}" "${path}" "${auth_mode}" >>"${result_file}"
    done
  else
    local in_flight=0
    for ((i = 1; i <= REQUESTS_PER_ENDPOINT; i++)); do
      (
        run_request "${method}" "${path}" "${auth_mode}" >>"${result_file}"
      ) &
      in_flight=$((in_flight + 1))
      if (( in_flight >= CONCURRENCY )); then
        wait -n
        in_flight=$((in_flight - 1))
      fi
    done
    wait
  fi
  end_ns="$(date +%s%N)"

  awk '{print $1}' "${result_file}" >"${status_file}"
  awk '{print $2}' "${result_file}" >"${times_file}"
  sort -n "${times_file}" >"${sorted_file}"

  local total success failed
  total="$(wc -l <"${status_file}" | tr -d ' ')"
  success="$(awk '$1 >= 200 && $1 < 300 { c++ } END { print c + 0 }' "${status_file}")"
  failed=$((total - success))

  local success_rate
  success_rate="$(awk -v ok="${success}" -v all="${total}" 'BEGIN { if (all == 0) { printf "0.00" } else { printf "%.2f", (ok * 100.0) / all } }')"

  local min max avg p50 p90 p95 p99
  if (( total == 0 )); then
    min="0.000"
    max="0.000"
    avg="0.000"
    p50="0.000"
    p90="0.000"
    p95="0.000"
    p99="0.000"
  else
    min="$(head -n 1 "${sorted_file}")"
    max="$(tail -n 1 "${sorted_file}")"
    avg="$(awk '{ sum += $1 } END { if (NR == 0) { printf "0.000" } else { printf "%.3f", sum / NR } }' "${times_file}")"
    p50="$(percentile_from_sorted "${sorted_file}" "${total}" 50)"
    p90="$(percentile_from_sorted "${sorted_file}" "${total}" 90)"
    p95="$(percentile_from_sorted "${sorted_file}" "${total}" 95)"
    p99="$(percentile_from_sorted "${sorted_file}" "${total}" 99)"
  fi

  local elapsed_ns elapsed_seconds rps_success
  elapsed_ns=$((end_ns - start_ns))
  elapsed_seconds="$(awk -v ns="${elapsed_ns}" 'BEGIN { if (ns <= 0) { printf "0.001" } else { printf "%.3f", ns / 1000000000.0 } }')"
  rps_success="$(awk -v ok="${success}" -v sec="${elapsed_seconds}" 'BEGIN { if (sec <= 0) { printf "0.00" } else { printf "%.2f", ok / sec } }')"

  printf '%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s\n' \
    "${name}" "${method}" "${path}" "${total}" "${success}" "${failed}" "${success_rate}" \
    "${avg}" "${p50}" "${p90}" "${p95}" "${p99}" "${rps_success}" >>"${SUMMARY_CSV}"

  capture_utilization_snapshot "after_${name}"
}

write_markdown_report() {
  cat >"${SUMMARY_MD}" <<MARKDOWN
# API Load Benchmark Summary

- Run ID: ${RUN_ID}
- API Base URL: ${API_BASE_URL}
- Auth User: ${AUTH_USER}
- Requests per endpoint: ${REQUESTS_PER_ENDPOINT}
- Warmup requests per endpoint: ${WARMUP_REQUESTS}
- Concurrency: ${CONCURRENCY}

| Endpoint | Method | Total | Success | Failed | Success % | Avg ms | P50 ms | P90 ms | P95 ms | P99 ms | Success RPS |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
MARKDOWN

  tail -n +2 "${SUMMARY_CSV}" | while IFS=',' read -r name method path total success failed success_rate avg p50 p90 p95 p99 rps_success; do
    printf '| `%s` | `%s` | %s | %s | %s | %s | %s | %s | %s | %s | %s | %s |\n' \
      "${name}" "${method}" "${total}" "${success}" "${failed}" "${success_rate}" "${avg}" "${p50}" "${p90}" "${p95}" "${p99}" "${rps_success}" \
      >>"${SUMMARY_MD}"
  done

  {
    echo
    echo "Raw artifacts:"
    echo "- summary csv: \`${SUMMARY_CSV}\`"
    echo "- per-endpoint raw files: \`${RAW_DIR}\`"
    echo "- utilization snapshot csv: \`${UTILIZATION_CSV}\`"
    echo
    echo "## Utilization Snapshots"
    echo
    echo "| Stage | Timestamp (UTC) | Load 1m | Load 5m | Load 15m | Mem Used MB | Mem Total MB | DB CPU | DB Mem |"
    echo "| --- | --- | --- | --- | --- | --- | --- | --- | --- |"
  } >>"${SUMMARY_MD}"

  tail -n +2 "${UTILIZATION_CSV}" | while IFS=',' read -r stage timestamp load1 load5 load15 mem_used mem_total db_cpu db_mem; do
    printf '| `%s` | `%s` | %s | %s | %s | %s | %s | %s | %s |\n' \
      "${stage}" "${timestamp}" "${load1}" "${load5}" "${load15}" "${mem_used}" "${mem_total}" "${db_cpu}" "${db_mem}" \
      >>"${SUMMARY_MD}"
  done
}

capture_utilization_snapshot() {
  local stage="$1"
  local timestamp
  timestamp="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

  local load1="n/a" load5="n/a" load15="n/a"
  if [[ -r /proc/loadavg ]]; then
    read -r load1 load5 load15 _ < /proc/loadavg || true
  fi

  local mem_total="n/a" mem_used="n/a"
  if command -v free >/dev/null 2>&1; then
    mem_total="$(free -m | awk '/^Mem:/ {print $2}')"
    mem_used="$(free -m | awk '/^Mem:/ {print $3}')"
  fi

  local db_cpu="n/a" db_mem="n/a"
  if command -v docker >/dev/null 2>&1; then
    local db_line
    db_line="$(docker stats --no-stream --format '{{.Name}},{{.CPUPerc}},{{.MemUsage}}' 2>/dev/null | awk -F, '/postgres/ {print $0; exit}')"
    if [[ -n "${db_line}" ]]; then
      db_cpu="$(echo "${db_line}" | awk -F, '{print $2}')"
      db_mem="$(echo "${db_line}" | awk -F, '{print $3}')"
    fi
  fi

  printf '%s,%s,%s,%s,%s,%s,%s,%s,%s\n' \
    "${stage}" "${timestamp}" "${load1}" "${load5}" "${load15}" "${mem_used}" "${mem_total}" "${db_cpu}" "${db_mem}" \
    >>"${UTILIZATION_CSV}"
}

main() {
  require_cmd curl
  require_cmd awk
  require_cmd sort
  require_cmd sed

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --concurrency)
        [[ $# -ge 2 ]] || fatal "--concurrency requires a numeric value"
        CONCURRENCY="$2"
        shift 2
        ;;
      *)
        fatal "unknown argument: $1"
        ;;
    esac
  done

  [[ "${CONCURRENCY}" =~ ^[0-9]+$ ]] || fatal "CONCURRENCY must be a positive integer"
  (( CONCURRENCY > 0 )) || fatal "CONCURRENCY must be >= 1"

  mkdir -p "${RAW_DIR}"

  log "Health check: ${API_BASE_URL}/health"
  curl -fsS "${API_BASE_URL}/health" >/dev/null || fatal "api health check failed"

  printf 'endpoint,method,path,total,success,failed,success_rate,avg_ms,p50_ms,p90_ms,p95_ms,p99_ms,success_rps\n' >"${SUMMARY_CSV}"
  printf 'stage,timestamp_utc,load_1m,load_5m,load_15m,mem_used_mb,mem_total_mb,db_cpu,db_mem\n' >"${UTILIZATION_CSV}"
  capture_utilization_snapshot "before_all"

  local spec name method path auth_mode
  for spec in "${ENDPOINT_SPECS[@]}"; do
    IFS='|' read -r name method path auth_mode <<<"${spec}"
    benchmark_endpoint "${name}" "${method}" "${path}" "${auth_mode}"
  done

  capture_utilization_snapshot "after_all"

  write_markdown_report

  log "Benchmark completed."
  log "Summary CSV: ${SUMMARY_CSV}"
  log "Summary Markdown: ${SUMMARY_MD}"
}

main "$@"
