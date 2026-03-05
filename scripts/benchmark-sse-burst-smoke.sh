#!/usr/bin/env bash
set -Eeuo pipefail

API_BASE_URL="${API_BASE_URL:-http://127.0.0.1:8080}"
AUTH_USER="${AUTH_USER:-admin}"
ASSET_ID="${ASSET_ID:-}"
STREAM_DURATION_SECONDS="${STREAM_DURATION_SECONDS:-35}"
BURST_EVENTS="${BURST_EVENTS:-30}"
BURST_INTERVAL_MS="${BURST_INTERVAL_MS:-120}"
RUN_ID="${RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)}"
OUTPUT_DIR="${OUTPUT_DIR:-.run/benchmarks/sse-${RUN_ID}}"
RAW_FILE="${OUTPUT_DIR}/sse.raw"
ERR_FILE="${OUTPUT_DIR}/sse.stderr.log"
DATA_FILE="${OUTPUT_DIR}/sse.data.jsonl"
EVENT_TYPES_FILE="${OUTPUT_DIR}/event-types.txt"
LAG_FILE="${OUTPUT_DIR}/alert-monitoring-sync-lag-ms.txt"
SUMMARY_JSON="${OUTPUT_DIR}/summary.json"
SUMMARY_MD="${OUTPUT_DIR}/summary.md"

CREATED_ASSET_ID=""

log() {
  printf '[benchmark-sse] %s\n' "$*" >&2
}

fatal() {
  printf '[benchmark-sse][ERROR] %s\n' "$*" >&2
  exit 1
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || fatal "missing required command: $1"
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

api_auth_curl() {
  curl -sS -H "x-auth-user: ${AUTH_USER}" "$@"
}

resolve_asset_id() {
  if [[ -n "${ASSET_ID}" ]]; then
    log "Using provided asset id: ${ASSET_ID}"
    return
  fi

  local asset_name payload response
  asset_name="bench-sse-${RUN_ID}"
  payload="$(jq -nc --arg name "${asset_name}" '{
    asset_class: "server",
    name: $name,
    hostname: ($name + ".local"),
    status: "active",
    site: "dc-a",
    department: "platform"
  }')"

  response="$(api_auth_curl -X POST "${API_BASE_URL}/api/v1/cmdb/assets" \
    -H 'Content-Type: application/json' \
    -d "${payload}")"

  ASSET_ID="$(echo "${response}" | jq -er '.id')"
  CREATED_ASSET_ID="${ASSET_ID}"
  log "Created temporary benchmark asset id=${ASSET_ID}"
}

trigger_burst_events() {
  local interval_seconds success_count failure_count
  interval_seconds="$(awk -v ms="${BURST_INTERVAL_MS}" 'BEGIN { printf "%.3f", ms / 1000.0 }')"
  success_count=0
  failure_count=0

  log "Triggering burst events: count=${BURST_EVENTS}, interval_ms=${BURST_INTERVAL_MS}"
  for ((i = 1; i <= BURST_EVENTS; i++)); do
    local status
    status="$(api_auth_curl -o /dev/null -w '%{http_code}' -X POST \
      "${API_BASE_URL}/api/v1/cmdb/assets/${ASSET_ID}/monitoring-sync" \
      -H 'Content-Type: application/json' \
      -d "{\"reason\":\"sse-burst-${RUN_ID}-${i}\"}")"
    if [[ "${status}" == "200" ]]; then
      success_count=$((success_count + 1))
    else
      failure_count=$((failure_count + 1))
    fi

    if (( i < BURST_EVENTS )); then
      sleep "${interval_seconds}"
    fi
  done

  echo "${success_count} ${failure_count}"
}

count_event_type() {
  local event_name="$1"
  if [[ ! -s "${EVENT_TYPES_FILE}" ]]; then
    echo "0"
    return
  fi
  grep -c "^${event_name}$" "${EVENT_TYPES_FILE}" || true
}

generate_summary() {
  local stream_exit="$1"
  local burst_success="$2"
  local burst_failed="$3"

  awk -F $'\t' '$2 ~ /^data: / {payload=$2; sub(/^data: /, "", payload); print $1 "\t" payload}' "${RAW_FILE}" >"${DATA_FILE}" || true
  if [[ -s "${DATA_FILE}" ]]; then
    cut -f2 "${DATA_FILE}" | jq -r '.event_type' >"${EVENT_TYPES_FILE}" 2>/dev/null || true
  else
    : >"${EVENT_TYPES_FILE}"
  fi

  local total_events connected_count heartbeat_count alert_test_count alert_sync_count stale_count recovered_count error_count
  total_events="$(wc -l <"${EVENT_TYPES_FILE}" | tr -d ' ')"
  connected_count="$(count_event_type "stream.connected")"
  heartbeat_count="$(count_event_type "stream.heartbeat")"
  alert_test_count="$(count_event_type "alert.test")"
  alert_sync_count="$(count_event_type "alert.monitoring_sync")"
  stale_count="$(count_event_type "stream.stale")"
  recovered_count="$(count_event_type "stream.recovered")"
  error_count="$(count_event_type "stream.error")"

  : >"${LAG_FILE}"
  while IFS=$'\t' read -r received_at payload; do
    [[ -n "${payload}" ]] || continue

    local event_type event_timestamp
    IFS=$'\t' read -r event_type event_timestamp < <(
      echo "${payload}" | jq -r '[.event_type // "", .timestamp // ""] | @tsv' 2>/dev/null || true
    )
    if [[ "${event_type}" != "alert.monitoring_sync" ]]; then
      continue
    fi
    if [[ -z "${event_timestamp}" ]]; then
      continue
    fi

    local recv_ms event_ms lag_ms
    recv_ms="$(date -u -d "${received_at}" +%s%3N 2>/dev/null || true)"
    event_ms="$(date -u -d "${event_timestamp}" +%s%3N 2>/dev/null || true)"
    if [[ -z "${recv_ms}" || -z "${event_ms}" ]]; then
      continue
    fi

    lag_ms=$((recv_ms - event_ms))
    if (( lag_ms < 0 )); then
      lag_ms=0
    fi
    printf '%s\n' "${lag_ms}" >>"${LAG_FILE}"
  done <"${DATA_FILE}"

  local lag_count lag_min lag_max lag_avg lag_p50 lag_p95 lag_p99 lag_sorted_file
  lag_sorted_file="${OUTPUT_DIR}/alert-monitoring-sync-lag-ms.sorted"
  if [[ -s "${LAG_FILE}" ]]; then
    sort -n "${LAG_FILE}" >"${lag_sorted_file}"
    lag_count="$(wc -l <"${lag_sorted_file}" | tr -d ' ')"
    lag_min="$(head -n 1 "${lag_sorted_file}")"
    lag_max="$(tail -n 1 "${lag_sorted_file}")"
    lag_avg="$(awk '{sum += $1} END {if (NR == 0) {printf "0.000"} else {printf "%.3f", sum / NR}}' "${lag_sorted_file}")"
    lag_p50="$(percentile_from_sorted "${lag_sorted_file}" "${lag_count}" 50)"
    lag_p95="$(percentile_from_sorted "${lag_sorted_file}" "${lag_count}" 95)"
    lag_p99="$(percentile_from_sorted "${lag_sorted_file}" "${lag_count}" 99)"
  else
    : >"${lag_sorted_file}"
    lag_count=0
    lag_min="0"
    lag_max="0"
    lag_avg="0.000"
    lag_p50="0.000"
    lag_p95="0.000"
    lag_p99="0.000"
  fi

  local pass=1
  if (( connected_count < 1 )); then
    pass=0
  fi
  if (( heartbeat_count < 2 )); then
    pass=0
  fi
  if (( alert_sync_count < 1 )); then
    pass=0
  fi
  if (( error_count > 0 )); then
    pass=0
  fi
  if (( stream_exit != 0 && stream_exit != 124 )); then
    pass=0
  fi

  jq -n \
    --arg run_id "${RUN_ID}" \
    --arg api_base_url "${API_BASE_URL}" \
    --arg auth_user "${AUTH_USER}" \
    --argjson stream_duration_seconds "${STREAM_DURATION_SECONDS}" \
    --argjson burst_events "${BURST_EVENTS}" \
    --argjson burst_interval_ms "${BURST_INTERVAL_MS}" \
    --argjson asset_id "${ASSET_ID}" \
    --argjson stream_exit_code "${stream_exit}" \
    --argjson burst_success "${burst_success}" \
    --argjson burst_failed "${burst_failed}" \
    --argjson total_events "${total_events}" \
    --argjson connected_count "${connected_count}" \
    --argjson heartbeat_count "${heartbeat_count}" \
    --argjson alert_test_count "${alert_test_count}" \
    --argjson alert_sync_count "${alert_sync_count}" \
    --argjson stale_count "${stale_count}" \
    --argjson recovered_count "${recovered_count}" \
    --argjson error_count "${error_count}" \
    --argjson lag_count "${lag_count}" \
    --argjson lag_min "${lag_min}" \
    --argjson lag_avg "${lag_avg}" \
    --argjson lag_p50 "${lag_p50}" \
    --argjson lag_p95 "${lag_p95}" \
    --argjson lag_p99 "${lag_p99}" \
    --argjson lag_max "${lag_max}" \
    --argjson pass "${pass}" \
    '{
      run_id: $run_id,
      api_base_url: $api_base_url,
      auth_user: $auth_user,
      stream_duration_seconds: $stream_duration_seconds,
      burst_events: $burst_events,
      burst_interval_ms: $burst_interval_ms,
      asset_id: $asset_id,
      stream_exit_code: $stream_exit_code,
      burst_result: {
        success: $burst_success,
        failed: $burst_failed
      },
      stream_events: {
        total: $total_events,
        stream_connected: $connected_count,
        stream_heartbeat: $heartbeat_count,
        alert_test: $alert_test_count,
        alert_monitoring_sync: $alert_sync_count,
        stream_stale: $stale_count,
        stream_recovered: $recovered_count,
        stream_error: $error_count
      },
      lag_ms: {
        samples: $lag_count,
        min: $lag_min,
        avg: $lag_avg,
        p50: $lag_p50,
        p95: $lag_p95,
        p99: $lag_p99,
        max: $lag_max
      },
      pass: ($pass == 1)
    }' >"${SUMMARY_JSON}"

  local pass_text="PASS"
  if (( pass == 0 )); then
    pass_text="FAIL"
  fi

  cat >"${SUMMARY_MD}" <<MARKDOWN
# SSE Burst Smoke Summary

- Run ID: ${RUN_ID}
- API Base URL: ${API_BASE_URL}
- Auth User: ${AUTH_USER}
- Asset ID: ${ASSET_ID}
- Stream Duration: ${STREAM_DURATION_SECONDS}s
- Burst Events: ${BURST_EVENTS}
- Burst Interval: ${BURST_INTERVAL_MS}ms
- Stream Exit Code: ${stream_exit} (124 is expected when timeout stops the stream)
- Result: **${pass_text}**

| Metric | Value |
| --- | --- |
| Burst request success | ${burst_success} |
| Burst request failed | ${burst_failed} |
| Total stream events | ${total_events} |
| stream.connected | ${connected_count} |
| stream.heartbeat | ${heartbeat_count} |
| alert.test | ${alert_test_count} |
| alert.monitoring_sync | ${alert_sync_count} |
| stream.stale | ${stale_count} |
| stream.recovered | ${recovered_count} |
| stream.error | ${error_count} |
| Lag samples (alert.monitoring_sync) | ${lag_count} |
| Lag min (ms) | ${lag_min} |
| Lag avg (ms) | ${lag_avg} |
| Lag p50 (ms) | ${lag_p50} |
| Lag p95 (ms) | ${lag_p95} |
| Lag p99 (ms) | ${lag_p99} |
| Lag max (ms) | ${lag_max} |

Artifacts:
- summary json: \`${SUMMARY_JSON}\`
- raw sse output: \`${RAW_FILE}\`
- stderr log: \`${ERR_FILE}\`
- lag samples: \`${LAG_FILE}\`
MARKDOWN

  if (( pass == 0 )); then
    fatal "SSE burst smoke failed. Inspect ${SUMMARY_MD} and ${ERR_FILE}"
  fi
}

main() {
  require_cmd curl
  require_cmd jq
  require_cmd awk
  require_cmd timeout
  require_cmd date

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --burst-count)
        [[ $# -ge 2 ]] || fatal "--burst-count requires a numeric value"
        BURST_EVENTS="$2"
        shift 2
        ;;
      --burst-interval-ms)
        [[ $# -ge 2 ]] || fatal "--burst-interval-ms requires a numeric value"
        BURST_INTERVAL_MS="$2"
        shift 2
        ;;
      --stream-duration)
        [[ $# -ge 2 ]] || fatal "--stream-duration requires a numeric value"
        STREAM_DURATION_SECONDS="$2"
        shift 2
        ;;
      *)
        fatal "unknown argument: $1"
        ;;
    esac
  done

  [[ "${BURST_EVENTS}" =~ ^[0-9]+$ ]] || fatal "BURST_EVENTS must be a positive integer"
  [[ "${BURST_INTERVAL_MS}" =~ ^[0-9]+$ ]] || fatal "BURST_INTERVAL_MS must be a positive integer"
  [[ "${STREAM_DURATION_SECONDS}" =~ ^[0-9]+$ ]] || fatal "STREAM_DURATION_SECONDS must be a positive integer"
  (( BURST_EVENTS > 0 )) || fatal "BURST_EVENTS must be >= 1"
  (( BURST_INTERVAL_MS > 0 )) || fatal "BURST_INTERVAL_MS must be >= 1"
  (( STREAM_DURATION_SECONDS > 0 )) || fatal "STREAM_DURATION_SECONDS must be >= 1"

  local min_stream_duration
  min_stream_duration=$(( (BURST_EVENTS * BURST_INTERVAL_MS + 999) / 1000 + 20 ))
  if (( STREAM_DURATION_SECONDS < min_stream_duration )); then
    log "Adjusting stream duration from ${STREAM_DURATION_SECONDS}s to ${min_stream_duration}s for burst coverage"
    STREAM_DURATION_SECONDS="${min_stream_duration}"
  fi

  mkdir -p "${OUTPUT_DIR}"

  log "Health check: ${API_BASE_URL}/health"
  curl -fsS "${API_BASE_URL}/health" >/dev/null || fatal "api health check failed"

  resolve_asset_id

  log "Starting SSE stream capture"
  timeout "${STREAM_DURATION_SECONDS}" \
    curl -sS -N \
      -H "x-auth-user: ${AUTH_USER}" \
      "${API_BASE_URL}/api/v1/streams/sse?severity=all" \
      2>"${ERR_FILE}" \
    | while IFS= read -r line; do
        printf '%s\t%s\n' "$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)" "${line}"
      done >"${RAW_FILE}" &
  local stream_pid=$!

  sleep 2
  read -r burst_success burst_failed < <(trigger_burst_events)

  local stream_exit=0
  set +e
  wait "${stream_pid}"
  stream_exit=$?
  set -e

  generate_summary "${stream_exit}" "${burst_success}" "${burst_failed}"

  log "SSE burst smoke completed."
  log "Summary JSON: ${SUMMARY_JSON}"
  log "Summary Markdown: ${SUMMARY_MD}"
  if [[ -n "${CREATED_ASSET_ID}" ]]; then
    log "Temporary asset created for benchmark: ${CREATED_ASSET_ID}"
  fi
}

main "$@"
