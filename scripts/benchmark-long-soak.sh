#!/usr/bin/env bash
set -Eeuo pipefail

API_BASE_URL="${API_BASE_URL:-http://127.0.0.1:8080}"
AUTH_USER="${AUTH_USER:-admin}"
PROFILE_LABEL="${BENCHMARK_PROFILE:-scale-1k}"
RUN_ID="${RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)}"
OUTPUT_DIR="${OUTPUT_DIR:-.run/benchmarks/long-soak-${RUN_ID}}"
POLICY_FILE="${POLICY_FILE:-scripts/benchmark-long-soak-policy.json}"
DURATION_HOURS="${DURATION_HOURS:-24}"
SAMPLE_INTERVAL_MINUTES="${SAMPLE_INTERVAL_MINUTES:-240}"
MAX_SAMPLES="${MAX_SAMPLES:-}"
SKIP_GATE=0
NO_SLEEP=0

SAMPLES_DIR="${OUTPUT_DIR}/samples"
DRIFT_DIR="${OUTPUT_DIR}/drift"
GATE_DIR="${OUTPUT_DIR}/gate"
SUMMARY_JSON="${OUTPUT_DIR}/summary.json"
SUMMARY_MD="${OUTPUT_DIR}/summary.md"
DRIFT_SUMMARY_JSON="${DRIFT_DIR}/summary.json"
DRIFT_SUMMARY_MD="${DRIFT_DIR}/summary.md"
TMP_METRICS=""

log() {
  printf '[long-soak] %s\n' "$*"
}

fatal() {
  printf '[long-soak][ERROR] %s\n' "$*" >&2
  exit 1
}

usage() {
  cat <<'USAGE'
Long-soak benchmark profile runner

Usage:
  bash scripts/benchmark-long-soak.sh [options]

Options:
  --profile <label>             Profile label for child benchmark run (default: scale-1k)
  --api-base-url <url>          API base URL (default: http://127.0.0.1:8080)
  --auth-user <username>        Auth user header value (default: admin)
  --duration-hours <num>        Total soak duration in hours (default: 24)
  --sample-interval-minutes <n> Interval between benchmark samples (default: 240)
  --max-samples <num>           Optional hard cap for number of samples
  --policy <path>               Long-soak gate policy file (default: scripts/benchmark-long-soak-policy.json)
  --output-dir <path>           Output directory
  --skip-gate                   Skip gate evaluation step
  --no-sleep                    Skip waiting between samples (useful for local dry verification)
  -h, --help                    Show this help
USAGE
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || fatal "missing required command: $1"
}

is_positive_integer() {
  [[ "$1" =~ ^[0-9]+$ ]] && (( "$1" > 0 ))
}

calc_delta() {
  local current="$1"
  local baseline="$2"
  awk -v cur="${current}" -v base="${baseline}" 'BEGIN { printf "%.3f", (cur + 0) - (base + 0) }'
}

metric_from_api_csv() {
  local file="$1"
  local metric="$2"
  case "${metric}" in
    success_rate_avg)
      awk -F',' 'NR > 1 {sum += $7; count += 1} END { if (count == 0) printf "0.000"; else printf "%.3f", sum / count }' "${file}"
      ;;
    p95_ms_max)
      awk -F',' 'NR > 1 { if ($11 + 0 > max + 0 || NR == 2) max = $11 } END { if (NR <= 1) printf "0.000"; else printf "%.3f", max + 0 }' "${file}"
      ;;
    p99_ms_max)
      awk -F',' 'NR > 1 { if ($12 + 0 > max + 0 || NR == 2) max = $12 } END { if (NR <= 1) printf "0.000"; else printf "%.3f", max + 0 }' "${file}"
      ;;
    *)
      fatal "unsupported API metric: ${metric}"
      ;;
  esac
}

metric_from_sse_json() {
  local file="$1"
  local metric="$2"
  case "${metric}" in
    lag_p95_ms)
      jq -r '.lag_ms.p95 // 0' "${file}"
      ;;
    lag_p99_ms)
      jq -r '.lag_ms.p99 // 0' "${file}"
      ;;
    lag_max_ms)
      jq -r '.lag_ms.max // 0' "${file}"
      ;;
    stream_error_count)
      jq -r '.stream_events.stream_error // 0' "${file}"
      ;;
    *)
      fatal "unsupported SSE metric: ${metric}"
      ;;
  esac
}

append_sample_metrics() {
  local sample_index="$1"
  local sample_dir="$2"
  local started_at="$3"
  local finished_at="$4"
  local api_summary="${sample_dir}/api/summary.csv"
  local sse_summary="${sample_dir}/sse/summary.json"

  [[ -f "${api_summary}" ]] || fatal "missing API summary for sample ${sample_index}: ${api_summary}"
  [[ -f "${sse_summary}" ]] || fatal "missing SSE summary for sample ${sample_index}: ${sse_summary}"

  local api_success_rate_avg api_p95_ms_max api_p99_ms_max
  local sse_lag_p95_ms sse_lag_p99_ms sse_lag_max_ms sse_stream_error_count

  api_success_rate_avg="$(metric_from_api_csv "${api_summary}" "success_rate_avg")"
  api_p95_ms_max="$(metric_from_api_csv "${api_summary}" "p95_ms_max")"
  api_p99_ms_max="$(metric_from_api_csv "${api_summary}" "p99_ms_max")"
  sse_lag_p95_ms="$(metric_from_sse_json "${sse_summary}" "lag_p95_ms")"
  sse_lag_p99_ms="$(metric_from_sse_json "${sse_summary}" "lag_p99_ms")"
  sse_lag_max_ms="$(metric_from_sse_json "${sse_summary}" "lag_max_ms")"
  sse_stream_error_count="$(metric_from_sse_json "${sse_summary}" "stream_error_count")"

  jq -nc \
    --argjson sample_index "${sample_index}" \
    --arg sample_dir "${sample_dir}" \
    --arg started_at "${started_at}" \
    --arg finished_at "${finished_at}" \
    --arg api_summary "${api_summary}" \
    --arg sse_summary "${sse_summary}" \
    --argjson api_success_rate_avg "${api_success_rate_avg}" \
    --argjson api_p95_ms_max "${api_p95_ms_max}" \
    --argjson api_p99_ms_max "${api_p99_ms_max}" \
    --argjson sse_lag_p95_ms "${sse_lag_p95_ms}" \
    --argjson sse_lag_p99_ms "${sse_lag_p99_ms}" \
    --argjson sse_lag_max_ms "${sse_lag_max_ms}" \
    --argjson sse_stream_error_count "${sse_stream_error_count}" \
    '{
      sample_index: $sample_index,
      sample_dir: $sample_dir,
      started_at: $started_at,
      finished_at: $finished_at,
      artifacts: {
        api_summary_csv: $api_summary,
        sse_summary_json: $sse_summary
      },
      metrics: {
        api_success_rate_avg: $api_success_rate_avg,
        api_p95_ms_max: $api_p95_ms_max,
        api_p99_ms_max: $api_p99_ms_max,
        sse_lag_p95_ms: $sse_lag_p95_ms,
        sse_lag_p99_ms: $sse_lag_p99_ms,
        sse_lag_max_ms: $sse_lag_max_ms,
        sse_stream_error_count: $sse_stream_error_count
      }
    }' >>"${TMP_METRICS}"
}

run_long_soak_samples() {
  local sample_count="$1"
  local total_minutes="$2"

  mkdir -p "${SAMPLES_DIR}"
  : >"${TMP_METRICS}"

  local i
  for ((i = 1; i <= sample_count; i++)); do
    local sample_name sample_dir started_at finished_at sample_run_id
    sample_name="$(printf 'sample-%03d' "${i}")"
    sample_dir="${SAMPLES_DIR}/${sample_name}"
    sample_run_id="${RUN_ID}-${sample_name}"
    started_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

    log "running sample ${i}/${sample_count} (profile=${PROFILE_LABEL})"
    API_BASE_URL="${API_BASE_URL}" \
      AUTH_USER="${AUTH_USER}" \
      BENCHMARK_PROFILE="${PROFILE_LABEL}" \
      RUN_ID="${sample_run_id}" \
      OUTPUT_DIR="${sample_dir}" \
      bash scripts/benchmark-scale-profiles.sh \
        --profile "${PROFILE_LABEL}" \
        --api-base-url "${API_BASE_URL}" \
        --auth-user "${AUTH_USER}" \
        --output-dir "${sample_dir}" \
        --run-id "${sample_run_id}" \
        --skip-gate

    finished_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    append_sample_metrics "${i}" "${sample_dir}" "${started_at}" "${finished_at}"

    if (( i < sample_count && NO_SLEEP == 0 )); then
      log "sleeping ${SAMPLE_INTERVAL_MINUTES} minute(s) before next sample"
      sleep "${SAMPLE_INTERVAL_MINUTES}m"
    fi
  done

  log "completed ${sample_count} samples over planned ${total_minutes} minutes"
}

write_drift_summary() {
  mkdir -p "${DRIFT_DIR}"

  local sample_count
  sample_count="$(jq -s 'length' "${TMP_METRICS}")"
  (( sample_count > 0 )) || fatal "no sample metrics captured"

  local baseline_file current_file
  baseline_file="$(mktemp)"
  current_file="$(mktemp)"
  trap 'rm -f "${baseline_file}" "${current_file}" "${TMP_METRICS}"' EXIT
  jq -s '.[0]' "${TMP_METRICS}" >"${baseline_file}"
  jq -s '.[-1]' "${TMP_METRICS}" >"${current_file}"

  local baseline_api_success current_api_success
  local baseline_api_p95 current_api_p95
  local baseline_api_p99 current_api_p99
  local baseline_sse_p95 current_sse_p95
  local baseline_sse_p99 current_sse_p99
  local baseline_sse_max current_sse_max
  local baseline_sse_error current_sse_error

  baseline_api_success="$(jq -r '.metrics.api_success_rate_avg' "${baseline_file}")"
  current_api_success="$(jq -r '.metrics.api_success_rate_avg' "${current_file}")"
  baseline_api_p95="$(jq -r '.metrics.api_p95_ms_max' "${baseline_file}")"
  current_api_p95="$(jq -r '.metrics.api_p95_ms_max' "${current_file}")"
  baseline_api_p99="$(jq -r '.metrics.api_p99_ms_max' "${baseline_file}")"
  current_api_p99="$(jq -r '.metrics.api_p99_ms_max' "${current_file}")"
  baseline_sse_p95="$(jq -r '.metrics.sse_lag_p95_ms' "${baseline_file}")"
  current_sse_p95="$(jq -r '.metrics.sse_lag_p95_ms' "${current_file}")"
  baseline_sse_p99="$(jq -r '.metrics.sse_lag_p99_ms' "${baseline_file}")"
  current_sse_p99="$(jq -r '.metrics.sse_lag_p99_ms' "${current_file}")"
  baseline_sse_max="$(jq -r '.metrics.sse_lag_max_ms' "${baseline_file}")"
  current_sse_max="$(jq -r '.metrics.sse_lag_max_ms' "${current_file}")"
  baseline_sse_error="$(jq -r '.metrics.sse_stream_error_count' "${baseline_file}")"
  current_sse_error="$(jq -r '.metrics.sse_stream_error_count' "${current_file}")"

  jq -n \
    --arg run_id "${RUN_ID}" \
    --arg generated_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    --arg profile "${PROFILE_LABEL}" \
    --argjson sample_count "${sample_count}" \
    --argjson sample_interval_minutes "${SAMPLE_INTERVAL_MINUTES}" \
    --argjson duration_hours "${DURATION_HOURS}" \
    --argjson baseline "$(cat "${baseline_file}")" \
    --argjson current "$(cat "${current_file}")" \
    --slurpfile samples "${TMP_METRICS}" \
    --arg api_success_delta "$(calc_delta "${current_api_success}" "${baseline_api_success}")" \
    --arg api_p95_delta "$(calc_delta "${current_api_p95}" "${baseline_api_p95}")" \
    --arg api_p99_delta "$(calc_delta "${current_api_p99}" "${baseline_api_p99}")" \
    --arg sse_p95_delta "$(calc_delta "${current_sse_p95}" "${baseline_sse_p95}")" \
    --arg sse_p99_delta "$(calc_delta "${current_sse_p99}" "${baseline_sse_p99}")" \
    --arg sse_max_delta "$(calc_delta "${current_sse_max}" "${baseline_sse_max}")" \
    --arg sse_error_delta "$(calc_delta "${current_sse_error}" "${baseline_sse_error}")" \
    '{
      run_id: $run_id,
      generated_at: $generated_at,
      profile: $profile,
      duration_hours: $duration_hours,
      sample_interval_minutes: $sample_interval_minutes,
      sample_count: $sample_count,
      baseline_sample: $baseline,
      current_sample: $current,
      deltas: [
        { metric: "api_success_rate_avg", baseline: ($baseline.metrics.api_success_rate_avg | tostring), current: ($current.metrics.api_success_rate_avg | tostring), delta: $api_success_delta },
        { metric: "api_p95_ms_max", baseline: ($baseline.metrics.api_p95_ms_max | tostring), current: ($current.metrics.api_p95_ms_max | tostring), delta: $api_p95_delta },
        { metric: "api_p99_ms_max", baseline: ($baseline.metrics.api_p99_ms_max | tostring), current: ($current.metrics.api_p99_ms_max | tostring), delta: $api_p99_delta },
        { metric: "sse_lag_p95_ms", baseline: ($baseline.metrics.sse_lag_p95_ms | tostring), current: ($current.metrics.sse_lag_p95_ms | tostring), delta: $sse_p95_delta },
        { metric: "sse_lag_p99_ms", baseline: ($baseline.metrics.sse_lag_p99_ms | tostring), current: ($current.metrics.sse_lag_p99_ms | tostring), delta: $sse_p99_delta },
        { metric: "sse_lag_max_ms", baseline: ($baseline.metrics.sse_lag_max_ms | tostring), current: ($current.metrics.sse_lag_max_ms | tostring), delta: $sse_max_delta },
        { metric: "sse_stream_error_count", baseline: ($baseline.metrics.sse_stream_error_count | tostring), current: ($current.metrics.sse_stream_error_count | tostring), delta: $sse_error_delta }
      ],
      samples: $samples
    }' >"${DRIFT_SUMMARY_JSON}"

  {
    echo "# Long-Soak Drift Summary"
    echo
    echo "- Run ID: ${RUN_ID}"
    echo "- Profile: ${PROFILE_LABEL}"
    echo "- Planned duration hours: ${DURATION_HOURS}"
    echo "- Sample interval minutes: ${SAMPLE_INTERVAL_MINUTES}"
    echo "- Captured samples: ${sample_count}"
    echo
    echo "| Metric | Baseline | Current | Delta (Current-Baseline) |"
    echo "| --- | --- | --- | --- |"
    jq -r '.deltas[] | "| `\(.metric)` | `\(.baseline)` | `\(.current)` | `\(.delta)` |"' "${DRIFT_SUMMARY_JSON}"
  } >"${DRIFT_SUMMARY_MD}"

  log "drift summary JSON: ${DRIFT_SUMMARY_JSON}"
  log "drift summary Markdown: ${DRIFT_SUMMARY_MD}"
}

run_gate() {
  local gate_pass=true
  if (( SKIP_GATE == 1 )); then
    log "skipping long-soak gate (--skip-gate)"
    return 0
  fi

  mkdir -p "${GATE_DIR}"
  if bash scripts/benchmark-long-soak-gate.sh \
    --drift-summary "${DRIFT_SUMMARY_JSON}" \
    --policy "${POLICY_FILE}" \
    --output-dir "${GATE_DIR}"; then
    gate_pass=true
  else
    gate_pass=false
  fi

  if [[ "${gate_pass}" != "true" ]]; then
    return 1
  fi
  return 0
}

write_overall_summary() {
  local gate_summary_json=""
  local gate_summary_md=""
  local gate_result="skipped"
  if (( SKIP_GATE == 0 )); then
    gate_summary_json="${GATE_DIR}/summary.json"
    gate_summary_md="${GATE_DIR}/summary.md"
    if [[ -f "${gate_summary_json}" ]]; then
      if [[ "$(jq -r '.pass' "${gate_summary_json}")" == "true" ]]; then
        gate_result="pass"
      else
        gate_result="fail"
      fi
    else
      gate_result="missing"
    fi
  fi

  jq -n \
    --arg run_id "${RUN_ID}" \
    --arg generated_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    --arg profile "${PROFILE_LABEL}" \
    --arg api_base_url "${API_BASE_URL}" \
    --arg auth_user "${AUTH_USER}" \
    --argjson duration_hours "${DURATION_HOURS}" \
    --argjson sample_interval_minutes "${SAMPLE_INTERVAL_MINUTES}" \
    --arg policy_file "${POLICY_FILE}" \
    --arg output_dir "${OUTPUT_DIR}" \
    --arg samples_dir "${SAMPLES_DIR}" \
    --arg drift_summary_json "${DRIFT_SUMMARY_JSON}" \
    --arg drift_summary_md "${DRIFT_SUMMARY_MD}" \
    --arg gate_summary_json "${gate_summary_json}" \
    --arg gate_summary_md "${gate_summary_md}" \
    --arg gate_result "${gate_result}" \
    --slurpfile samples "${TMP_METRICS}" \
    '{
      run_id: $run_id,
      generated_at: $generated_at,
      profile: $profile,
      api_base_url: $api_base_url,
      auth_user: $auth_user,
      duration_hours: $duration_hours,
      sample_interval_minutes: $sample_interval_minutes,
      policy_file: $policy_file,
      output_dir: $output_dir,
      samples_dir: $samples_dir,
      artifacts: {
        drift_summary_json: $drift_summary_json,
        drift_summary_md: $drift_summary_md,
        gate_summary_json: (if $gate_result == "skipped" then null else $gate_summary_json end),
        gate_summary_md: (if $gate_result == "skipped" then null else $gate_summary_md end)
      },
      gate_result: $gate_result,
      samples: $samples
    }' >"${SUMMARY_JSON}"

  {
    echo "# Long-Soak Benchmark Summary"
    echo
    echo "- Run ID: ${RUN_ID}"
    echo "- Profile: ${PROFILE_LABEL}"
    echo "- API Base URL: ${API_BASE_URL}"
    echo "- Auth User: ${AUTH_USER}"
    echo "- Planned duration hours: ${DURATION_HOURS}"
    echo "- Sample interval minutes: ${SAMPLE_INTERVAL_MINUTES}"
    echo "- Output directory: \`${OUTPUT_DIR}\`"
    echo "- Drift summary: \`${DRIFT_SUMMARY_MD}\`"
    if (( SKIP_GATE == 1 )); then
      echo "- Gate summary: skipped"
    else
      echo "- Gate summary: \`${GATE_DIR}/summary.md\`"
      echo "- Gate result: **${gate_result^^}**"
    fi
    echo
    echo "| Sample | API Success Avg | API P95 Max | API P99 Max | SSE Lag P95 | SSE Lag P99 | SSE Lag Max | SSE Stream Errors |"
    echo "| --- | --- | --- | --- | --- | --- | --- | --- |"
    jq -r '.samples[] | "| #\(.sample_index) | `\(.metrics.api_success_rate_avg)` | `\(.metrics.api_p95_ms_max)` | `\(.metrics.api_p99_ms_max)` | `\(.metrics.sse_lag_p95_ms)` | `\(.metrics.sse_lag_p99_ms)` | `\(.metrics.sse_lag_max_ms)` | `\(.metrics.sse_stream_error_count)` |"' "${SUMMARY_JSON}"
  } >"${SUMMARY_MD}"

  log "long-soak summary JSON: ${SUMMARY_JSON}"
  log "long-soak summary Markdown: ${SUMMARY_MD}"
}

main() {
  require_cmd bash
  require_cmd jq
  require_cmd awk

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --profile)
        [[ $# -ge 2 ]] || fatal "--profile requires a value"
        PROFILE_LABEL="$2"
        shift 2
        ;;
      --api-base-url)
        [[ $# -ge 2 ]] || fatal "--api-base-url requires a value"
        API_BASE_URL="$2"
        shift 2
        ;;
      --auth-user)
        [[ $# -ge 2 ]] || fatal "--auth-user requires a value"
        AUTH_USER="$2"
        shift 2
        ;;
      --duration-hours)
        [[ $# -ge 2 ]] || fatal "--duration-hours requires a value"
        DURATION_HOURS="$2"
        shift 2
        ;;
      --sample-interval-minutes)
        [[ $# -ge 2 ]] || fatal "--sample-interval-minutes requires a value"
        SAMPLE_INTERVAL_MINUTES="$2"
        shift 2
        ;;
      --max-samples)
        [[ $# -ge 2 ]] || fatal "--max-samples requires a value"
        MAX_SAMPLES="$2"
        shift 2
        ;;
      --policy)
        [[ $# -ge 2 ]] || fatal "--policy requires a value"
        POLICY_FILE="$2"
        shift 2
        ;;
      --output-dir)
        [[ $# -ge 2 ]] || fatal "--output-dir requires a path"
        OUTPUT_DIR="$2"
        SAMPLES_DIR="${OUTPUT_DIR}/samples"
        DRIFT_DIR="${OUTPUT_DIR}/drift"
        GATE_DIR="${OUTPUT_DIR}/gate"
        SUMMARY_JSON="${OUTPUT_DIR}/summary.json"
        SUMMARY_MD="${OUTPUT_DIR}/summary.md"
        DRIFT_SUMMARY_JSON="${DRIFT_DIR}/summary.json"
        DRIFT_SUMMARY_MD="${DRIFT_DIR}/summary.md"
        shift 2
        ;;
      --skip-gate)
        SKIP_GATE=1
        shift
        ;;
      --no-sleep)
        NO_SLEEP=1
        shift
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

  is_positive_integer "${DURATION_HOURS}" || fatal "duration hours must be a positive integer"
  is_positive_integer "${SAMPLE_INTERVAL_MINUTES}" || fatal "sample interval minutes must be a positive integer"
  [[ -f "${POLICY_FILE}" ]] || fatal "policy file not found: ${POLICY_FILE}"

  local total_minutes sample_count
  total_minutes=$(( DURATION_HOURS * 60 ))
  sample_count=$(( total_minutes / SAMPLE_INTERVAL_MINUTES + 1 ))
  if [[ -n "${MAX_SAMPLES}" ]]; then
    is_positive_integer "${MAX_SAMPLES}" || fatal "max samples must be a positive integer"
    if (( sample_count > MAX_SAMPLES )); then
      sample_count="${MAX_SAMPLES}"
    fi
  fi
  (( sample_count > 0 )) || sample_count=1

  mkdir -p "${OUTPUT_DIR}"
  TMP_METRICS="$(mktemp)"

  run_long_soak_samples "${sample_count}" "${total_minutes}"
  write_drift_summary

  local gate_exit=0
  if ! run_gate; then
    gate_exit=1
  fi

  write_overall_summary

  if (( gate_exit != 0 )); then
    fatal "long-soak gate failed; inspect ${GATE_DIR}/summary.md"
  fi
}

main "$@"
