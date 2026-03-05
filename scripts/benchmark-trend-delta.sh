#!/usr/bin/env bash
set -Eeuo pipefail

RUN_ID="${RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)}"
PROFILE_LABEL="${PROFILE_LABEL:-smoke}"
OUTPUT_DIR="${OUTPUT_DIR:-.run/benchmarks/trend-delta-${RUN_ID}}"
SUMMARY_JSON="${OUTPUT_DIR}/summary.json"
SUMMARY_MD="${OUTPUT_DIR}/summary.md"

CURRENT_API_SUMMARY=""
BASELINE_API_SUMMARY=""
CURRENT_SSE_SUMMARY=""
BASELINE_SSE_SUMMARY=""
TMP_RESULTS_FILE=""

log() {
  printf '[benchmark-delta] %s\n' "$*"
}

fatal() {
  printf '[benchmark-delta][ERROR] %s\n' "$*" >&2
  exit 1
}

usage() {
  cat <<'USAGE'
Benchmark trend delta report

Usage:
  bash scripts/benchmark-trend-delta.sh [options]

Options:
  --current-api-summary <path>     Current API summary csv
  --baseline-api-summary <path>    Baseline API summary csv
  --current-sse-summary <path>     Current SSE summary json (optional)
  --baseline-sse-summary <path>    Baseline SSE summary json (optional)
  --profile <label>                Profile label in report metadata
  --output-dir <path>              Output directory
  -h, --help                       Show this help
USAGE
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || fatal "missing required command: $1"
}

is_number() {
  [[ "$1" =~ ^-?[0-9]+([.][0-9]+)?$ ]]
}

calc_delta() {
  local current="$1"
  local baseline="$2"
  awk -v cur="${current}" -v base="${baseline}" 'BEGIN { printf "%.3f", (cur + 0) - (base + 0) }'
}

direction_from_delta() {
  local kind="$1"
  local delta="$2"
  if ! is_number "${delta}"; then
    echo "missing"
    return
  fi

  case "${kind}" in
    higher_better)
      awk -v d="${delta}" 'BEGIN { if (d > 0) print "improved"; else if (d < 0) print "regressed"; else print "same" }'
      ;;
    lower_better)
      awk -v d="${delta}" 'BEGIN { if (d < 0) print "improved"; else if (d > 0) print "regressed"; else print "same" }'
      ;;
    *)
      echo "same"
      ;;
  esac
}

append_delta() {
  local category="$1"
  local target="$2"
  local metric="$3"
  local baseline="$4"
  local current="$5"
  local delta="$6"
  local trend="$7"
  local note="$8"

  jq -nc \
    --arg category "${category}" \
    --arg target "${target}" \
    --arg metric "${metric}" \
    --arg baseline "${baseline}" \
    --arg current "${current}" \
    --arg delta "${delta}" \
    --arg trend "${trend}" \
    --arg note "${note}" \
    '{
      category: $category,
      target: $target,
      metric: $metric,
      baseline: $baseline,
      current: $current,
      delta: $delta,
      trend: $trend,
      note: $note
    }' >>"${TMP_RESULTS_FILE}"
}

read_api_metric() {
  local file="$1"
  local endpoint="$2"
  local column="$3"
  awk -F',' -v endpoint="${endpoint}" -v column="${column}" '
    NR > 1 && $1 == endpoint { print $column; found=1; exit }
    END { if (!found) print "missing" }
  ' "${file}"
}

build_api_delta() {
  local current_file="$1"
  local baseline_file="$2"

  [[ -f "${current_file}" ]] || fatal "current api summary file not found: ${current_file}"
  [[ -f "${baseline_file}" ]] || fatal "baseline api summary file not found: ${baseline_file}"

  local endpoints
  endpoints="$( { awk -F',' 'NR > 1 {print $1}' "${current_file}"; awk -F',' 'NR > 1 {print $1}' "${baseline_file}"; } | sed '/^$/d' | sort -u )"
  [[ -n "${endpoints}" ]] || fatal "no API endpoints found in provided summaries"

  local endpoint
  while IFS= read -r endpoint; do
    [[ -n "${endpoint}" ]] || continue

    local baseline_success baseline_p95 baseline_p99
    local current_success current_p95 current_p99
    baseline_success="$(read_api_metric "${baseline_file}" "${endpoint}" 7)"
    baseline_p95="$(read_api_metric "${baseline_file}" "${endpoint}" 11)"
    baseline_p99="$(read_api_metric "${baseline_file}" "${endpoint}" 12)"
    current_success="$(read_api_metric "${current_file}" "${endpoint}" 7)"
    current_p95="$(read_api_metric "${current_file}" "${endpoint}" 11)"
    current_p99="$(read_api_metric "${current_file}" "${endpoint}" 12)"

    build_one_metric "api" "${endpoint}" "success_rate" "${baseline_success}" "${current_success}" "higher_better" "percentage"
    build_one_metric "api" "${endpoint}" "p95_ms" "${baseline_p95}" "${current_p95}" "lower_better" "milliseconds"
    build_one_metric "api" "${endpoint}" "p99_ms" "${baseline_p99}" "${current_p99}" "lower_better" "milliseconds"
  done <<<"${endpoints}"
}

read_json_metric() {
  local file="$1"
  local expr="$2"
  jq -r "${expr} // \"missing\"" "${file}"
}

build_sse_delta() {
  local current_file="$1"
  local baseline_file="$2"

  [[ -f "${current_file}" ]] || fatal "current sse summary file not found: ${current_file}"
  [[ -f "${baseline_file}" ]] || fatal "baseline sse summary file not found: ${baseline_file}"

  local baseline_lag_p95 baseline_lag_p99 baseline_lag_max
  local baseline_stream_error baseline_alert_sync
  local current_lag_p95 current_lag_p99 current_lag_max
  local current_stream_error current_alert_sync

  baseline_lag_p95="$(read_json_metric "${baseline_file}" '.lag_ms.p95')"
  baseline_lag_p99="$(read_json_metric "${baseline_file}" '.lag_ms.p99')"
  baseline_lag_max="$(read_json_metric "${baseline_file}" '.lag_ms.max')"
  baseline_stream_error="$(read_json_metric "${baseline_file}" '.stream_events.stream_error')"
  baseline_alert_sync="$(read_json_metric "${baseline_file}" '.stream_events.alert_monitoring_sync')"

  current_lag_p95="$(read_json_metric "${current_file}" '.lag_ms.p95')"
  current_lag_p99="$(read_json_metric "${current_file}" '.lag_ms.p99')"
  current_lag_max="$(read_json_metric "${current_file}" '.lag_ms.max')"
  current_stream_error="$(read_json_metric "${current_file}" '.stream_events.stream_error')"
  current_alert_sync="$(read_json_metric "${current_file}" '.stream_events.alert_monitoring_sync')"

  build_one_metric "sse" "alert.monitoring_sync" "lag_p95_ms" "${baseline_lag_p95}" "${current_lag_p95}" "lower_better" "milliseconds"
  build_one_metric "sse" "alert.monitoring_sync" "lag_p99_ms" "${baseline_lag_p99}" "${current_lag_p99}" "lower_better" "milliseconds"
  build_one_metric "sse" "alert.monitoring_sync" "lag_max_ms" "${baseline_lag_max}" "${current_lag_max}" "lower_better" "milliseconds"
  build_one_metric "sse" "stream" "stream_error_count" "${baseline_stream_error}" "${current_stream_error}" "lower_better" "count"
  build_one_metric "sse" "stream" "alert_monitoring_sync_count" "${baseline_alert_sync}" "${current_alert_sync}" "higher_better" "count"
}

build_one_metric() {
  local category="$1"
  local target="$2"
  local metric="$3"
  local baseline="$4"
  local current="$5"
  local trend_kind="$6"
  local note="$7"

  local delta="missing"
  local trend="missing"
  if is_number "${baseline}" && is_number "${current}"; then
    delta="$(calc_delta "${current}" "${baseline}")"
    trend="$(direction_from_delta "${trend_kind}" "${delta}")"
  fi

  append_delta "${category}" "${target}" "${metric}" "${baseline}" "${current}" "${delta}" "${trend}" "${note}"
}

write_summaries() {
  local generated_at
  generated_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

  jq -n \
    --arg run_id "${RUN_ID}" \
    --arg generated_at "${generated_at}" \
    --arg profile "${PROFILE_LABEL}" \
    --arg current_api_summary "${CURRENT_API_SUMMARY}" \
    --arg baseline_api_summary "${BASELINE_API_SUMMARY}" \
    --arg current_sse_summary "${CURRENT_SSE_SUMMARY}" \
    --arg baseline_sse_summary "${BASELINE_SSE_SUMMARY}" \
    --slurpfile deltas "${TMP_RESULTS_FILE}" \
    '{
      run_id: $run_id,
      generated_at: $generated_at,
      profile: $profile,
      sources: {
        current_api_summary: $current_api_summary,
        baseline_api_summary: $baseline_api_summary,
        current_sse_summary: ($current_sse_summary | if . == "" then null else . end),
        baseline_sse_summary: ($baseline_sse_summary | if . == "" then null else . end)
      },
      deltas: $deltas,
      totals: {
        checks: ($deltas | length),
        improved: ($deltas | map(select(.trend == "improved")) | length),
        regressed: ($deltas | map(select(.trend == "regressed")) | length),
        same: ($deltas | map(select(.trend == "same")) | length),
        missing: ($deltas | map(select(.trend == "missing")) | length)
      }
    }' >"${SUMMARY_JSON}"

  {
    echo "# Benchmark Trend Delta Summary"
    echo
    echo "- Run ID: ${RUN_ID}"
    echo "- Generated at: $(jq -r '.generated_at' "${SUMMARY_JSON}")"
    echo "- Profile: ${PROFILE_LABEL}"
    echo "- Current API Summary: \`${CURRENT_API_SUMMARY}\`"
    echo "- Baseline API Summary: \`${BASELINE_API_SUMMARY}\`"
    if [[ -n "${CURRENT_SSE_SUMMARY}" && -n "${BASELINE_SSE_SUMMARY}" ]]; then
      echo "- Current SSE Summary: \`${CURRENT_SSE_SUMMARY}\`"
      echo "- Baseline SSE Summary: \`${BASELINE_SSE_SUMMARY}\`"
    fi
    echo
    echo "| Category | Target | Metric | Baseline | Current | Delta (Current-Baseline) | Trend | Note |"
    echo "| --- | --- | --- | --- | --- | --- | --- | --- |"
    jq -r '.deltas[] | "| `\(.category)` | `\(.target)` | `\(.metric)` | `\(.baseline)` | `\(.current)` | `\(.delta)` | \(.trend) | \(.note) |"' "${SUMMARY_JSON}"
  } >"${SUMMARY_MD}"

  log "Trend delta summary JSON: ${SUMMARY_JSON}"
  log "Trend delta summary Markdown: ${SUMMARY_MD}"
}

main() {
  require_cmd jq
  require_cmd awk
  require_cmd sort
  require_cmd sed

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --current-api-summary)
        [[ $# -ge 2 ]] || fatal "--current-api-summary requires a file path"
        CURRENT_API_SUMMARY="$2"
        shift 2
        ;;
      --baseline-api-summary)
        [[ $# -ge 2 ]] || fatal "--baseline-api-summary requires a file path"
        BASELINE_API_SUMMARY="$2"
        shift 2
        ;;
      --current-sse-summary)
        [[ $# -ge 2 ]] || fatal "--current-sse-summary requires a file path"
        CURRENT_SSE_SUMMARY="$2"
        shift 2
        ;;
      --baseline-sse-summary)
        [[ $# -ge 2 ]] || fatal "--baseline-sse-summary requires a file path"
        BASELINE_SSE_SUMMARY="$2"
        shift 2
        ;;
      --profile)
        [[ $# -ge 2 ]] || fatal "--profile requires a label"
        PROFILE_LABEL="$2"
        shift 2
        ;;
      --output-dir)
        [[ $# -ge 2 ]] || fatal "--output-dir requires a path"
        OUTPUT_DIR="$2"
        SUMMARY_JSON="${OUTPUT_DIR}/summary.json"
        SUMMARY_MD="${OUTPUT_DIR}/summary.md"
        shift 2
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      *)
        fatal "unknown argument: $1"
        ;;
    esac
  done

  [[ -n "${CURRENT_API_SUMMARY}" ]] || fatal "--current-api-summary is required"
  [[ -n "${BASELINE_API_SUMMARY}" ]] || fatal "--baseline-api-summary is required"
  if [[ -n "${CURRENT_SSE_SUMMARY}" || -n "${BASELINE_SSE_SUMMARY}" ]]; then
    [[ -n "${CURRENT_SSE_SUMMARY}" && -n "${BASELINE_SSE_SUMMARY}" ]] || fatal "both --current-sse-summary and --baseline-sse-summary are required when comparing SSE"
  fi

  mkdir -p "${OUTPUT_DIR}"
  TMP_RESULTS_FILE="$(mktemp)"
  trap 'rm -f "${TMP_RESULTS_FILE}"' EXIT
  : >"${TMP_RESULTS_FILE}"

  build_api_delta "${CURRENT_API_SUMMARY}" "${BASELINE_API_SUMMARY}"
  if [[ -n "${CURRENT_SSE_SUMMARY}" && -n "${BASELINE_SSE_SUMMARY}" ]]; then
    build_sse_delta "${CURRENT_SSE_SUMMARY}" "${BASELINE_SSE_SUMMARY}"
  fi

  local checks_count
  checks_count="$(jq -s 'length' "${TMP_RESULTS_FILE}")"
  [[ "${checks_count}" != "0" ]] || fatal "no trend delta checks were generated"

  write_summaries
}

main "$@"
