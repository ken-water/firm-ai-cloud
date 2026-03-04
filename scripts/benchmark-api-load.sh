#!/usr/bin/env bash
set -Eeuo pipefail

API_BASE_URL="${API_BASE_URL:-http://127.0.0.1:8080}"
AUTH_USER="${AUTH_USER:-admin}"
REQUESTS_PER_ENDPOINT="${REQUESTS_PER_ENDPOINT:-80}"
WARMUP_REQUESTS="${WARMUP_REQUESTS:-10}"
REQUEST_TIMEOUT_SECONDS="${REQUEST_TIMEOUT_SECONDS:-8}"
RUN_ID="${RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)}"
OUTPUT_DIR="${OUTPUT_DIR:-.run/benchmarks/api-${RUN_ID}}"
RAW_DIR="${OUTPUT_DIR}/raw"
SUMMARY_CSV="${OUTPUT_DIR}/summary.csv"
SUMMARY_MD="${OUTPUT_DIR}/summary.md"

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

  local times_file="${RAW_DIR}/${name}.times_ms"
  local status_file="${RAW_DIR}/${name}.status"
  local sorted_file="${RAW_DIR}/${name}.times_ms.sorted"

  : >"${times_file}"
  : >"${status_file}"

  log "Warmup ${name}: ${WARMUP_REQUESTS} requests"
  for ((i = 1; i <= WARMUP_REQUESTS; i++)); do
    run_request "${method}" "${path}" "${auth_mode}" >/dev/null
  done

  log "Benchmark ${name}: ${REQUESTS_PER_ENDPOINT} requests"
  local start_ns end_ns
  start_ns="$(date +%s%N)"
  for ((i = 1; i <= REQUESTS_PER_ENDPOINT; i++)); do
    local result status latency_ms
    result="$(run_request "${method}" "${path}" "${auth_mode}")"
    status="${result%% *}"
    latency_ms="${result##* }"
    echo "${status}" >>"${status_file}"
    echo "${latency_ms}" >>"${times_file}"
  done
  end_ns="$(date +%s%N)"

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
}

write_markdown_report() {
  cat >"${SUMMARY_MD}" <<MARKDOWN
# API Load Benchmark Summary

- Run ID: ${RUN_ID}
- API Base URL: ${API_BASE_URL}
- Auth User: ${AUTH_USER}
- Requests per endpoint: ${REQUESTS_PER_ENDPOINT}
- Warmup requests per endpoint: ${WARMUP_REQUESTS}

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
  } >>"${SUMMARY_MD}"
}

main() {
  require_cmd curl
  require_cmd awk
  require_cmd sort
  require_cmd sed

  mkdir -p "${RAW_DIR}"

  log "Health check: ${API_BASE_URL}/health"
  curl -fsS "${API_BASE_URL}/health" >/dev/null || fatal "api health check failed"

  printf 'endpoint,method,path,total,success,failed,success_rate,avg_ms,p50_ms,p90_ms,p95_ms,p99_ms,success_rps\n' >"${SUMMARY_CSV}"

  local spec name method path auth_mode
  for spec in "${ENDPOINT_SPECS[@]}"; do
    IFS='|' read -r name method path auth_mode <<<"${spec}"
    benchmark_endpoint "${name}" "${method}" "${path}" "${auth_mode}"
  done

  write_markdown_report

  log "Benchmark completed."
  log "Summary CSV: ${SUMMARY_CSV}"
  log "Summary Markdown: ${SUMMARY_MD}"
}

main "$@"
