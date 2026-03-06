#!/usr/bin/env bash
set -Eeuo pipefail

RUN_ID="${RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)}"
POLICY_FILE="${POLICY_FILE:-scripts/benchmark-threshold-policy.json}"
OUTPUT_DIR="${OUTPUT_DIR:-.run/benchmarks/gate-${RUN_ID}}"
SUMMARY_JSON="${OUTPUT_DIR}/gate-summary.json"
SUMMARY_MD="${OUTPUT_DIR}/gate-summary.md"
PROFILE_LABEL="${PROFILE_LABEL:-smoke}"

declare -a API_SUMMARIES=()
SSE_SUMMARY=""
TMP_RESULTS_FILE=""
RESOLVED_POLICY_FILE=""
POLICY_SOURCE_PATH="root"

log() {
  printf '[benchmark-gate] %s\n' "$*"
}

fatal() {
  printf '[benchmark-gate][ERROR] %s\n' "$*" >&2
  exit 1
}

usage() {
  cat <<'USAGE'
Benchmark threshold gate

Usage:
  bash scripts/benchmark-threshold-gate.sh [options]

Options:
  --api-summary <path>     API benchmark summary csv (repeatable)
  --sse-summary <path>     SSE benchmark summary json
  --policy <path>          Threshold policy json (default: scripts/benchmark-threshold-policy.json)
  --output-dir <path>      Output directory for gate summary artifacts
  --profile <label>        Profile label (for example: smoke, scale-1k, scale-5k, scale-10k)
  -h, --help               Show this help
USAGE
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || fatal "missing required command: $1"
}

is_number() {
  [[ "$1" =~ ^-?[0-9]+([.][0-9]+)?$ ]]
}

num_ge() {
  local actual="$1"
  local threshold="$2"
  awk -v a="${actual}" -v b="${threshold}" 'BEGIN { if ((a + 0) >= (b + 0)) print 1; else print 0 }'
}

num_le() {
  local actual="$1"
  local threshold="$2"
  awk -v a="${actual}" -v b="${threshold}" 'BEGIN { if ((a + 0) <= (b + 0)) print 1; else print 0 }'
}

resolve_profile_policy() {
  RESOLVED_POLICY_FILE="$(mktemp)"

  if jq -e '.profiles? | type == "object"' "${POLICY_FILE}" >/dev/null; then
    local available_profiles
    available_profiles="$(jq -r '.profiles | keys | join(", ")' "${POLICY_FILE}")"

    if ! jq -e --arg profile "${PROFILE_LABEL}" '.profiles[$profile] != null' "${POLICY_FILE}" >/dev/null; then
      fatal "profile '${PROFILE_LABEL}' not found in policy '${POLICY_FILE}'. available profiles: ${available_profiles}"
    fi

    jq --arg profile "${PROFILE_LABEL}" '.profiles[$profile]' "${POLICY_FILE}" >"${RESOLVED_POLICY_FILE}"
    POLICY_SOURCE_PATH="profiles.${PROFILE_LABEL}"
  else
    cp "${POLICY_FILE}" "${RESOLVED_POLICY_FILE}"
    POLICY_SOURCE_PATH="root"
  fi
}

append_check() {
  local category="$1"
  local source="$2"
  local target="$3"
  local metric="$4"
  local actual="$5"
  local threshold="$6"
  local operator="$7"
  local pass="$8"
  local note="$9"

  local pass_json="false"
  if [[ "${pass}" -eq 1 ]]; then
    pass_json="true"
  fi

  jq -nc \
    --arg category "${category}" \
    --arg source "${source}" \
    --arg target "${target}" \
    --arg metric "${metric}" \
    --arg actual "${actual}" \
    --arg threshold "${threshold}" \
    --arg operator "${operator}" \
    --arg note "${note}" \
    --argjson pass "${pass_json}" \
    '{
      category: $category,
      source: $source,
      target: $target,
      metric: $metric,
      actual: $actual,
      threshold: $threshold,
      operator: $operator,
      pass: $pass,
      note: $note
    }' >>"${TMP_RESULTS_FILE}"
}

check_api_summary() {
  local summary_file="$1"
  [[ -f "${summary_file}" ]] || fatal "api summary file not found: ${summary_file}"

  declare -A endpoint_success=()
  declare -A endpoint_p95=()
  declare -A endpoint_p99=()

  while IFS=',' read -r endpoint method path total success failed success_rate avg p50 p90 p95 p99 rps; do
    if [[ "${endpoint}" == "endpoint" ]]; then
      continue
    fi
    endpoint_success["${endpoint}"]="${success_rate}"
    endpoint_p95["${endpoint}"]="${p95}"
    endpoint_p99["${endpoint}"]="${p99}"
  done <"${summary_file}"

  mapfile -t policy_endpoints < <(jq -r '.api.endpoints | keys[]' "${RESOLVED_POLICY_FILE}")
  for endpoint in "${policy_endpoints[@]}"; do
    local success_rate_min p95_max p99_max
    success_rate_min="$(jq -r --arg endpoint "${endpoint}" '.api.endpoints[$endpoint].success_rate_min // .api.default.success_rate_min' "${RESOLVED_POLICY_FILE}")"
    p95_max="$(jq -r --arg endpoint "${endpoint}" '.api.endpoints[$endpoint].p95_ms_max // .api.default.p95_ms_max' "${RESOLVED_POLICY_FILE}")"
    p99_max="$(jq -r --arg endpoint "${endpoint}" '.api.endpoints[$endpoint].p99_ms_max // .api.default.p99_ms_max' "${RESOLVED_POLICY_FILE}")"

    local actual_success actual_p95 actual_p99
    actual_success="${endpoint_success[${endpoint}]:-}"
    actual_p95="${endpoint_p95[${endpoint}]:-}"
    actual_p99="${endpoint_p99[${endpoint}]:-}"

    if [[ -z "${actual_success}" || -z "${actual_p95}" || -z "${actual_p99}" ]]; then
      append_check "api" "${summary_file}" "${endpoint}" "present" "missing" "present" "==" 0 "endpoint missing from benchmark summary"
      continue
    fi

    local pass_success=0 pass_p95=0 pass_p99=0
    if is_number "${actual_success}" && is_number "${success_rate_min}"; then
      pass_success="$(num_ge "${actual_success}" "${success_rate_min}")"
    fi
    if is_number "${actual_p95}" && is_number "${p95_max}"; then
      pass_p95="$(num_le "${actual_p95}" "${p95_max}")"
    fi
    if is_number "${actual_p99}" && is_number "${p99_max}"; then
      pass_p99="$(num_le "${actual_p99}" "${p99_max}")"
    fi

    append_check "api" "${summary_file}" "${endpoint}" "success_rate" "${actual_success}" "${success_rate_min}" ">=" "${pass_success}" "percentage"
    append_check "api" "${summary_file}" "${endpoint}" "p95_ms" "${actual_p95}" "${p95_max}" "<=" "${pass_p95}" "milliseconds"
    append_check "api" "${summary_file}" "${endpoint}" "p99_ms" "${actual_p99}" "${p99_max}" "<=" "${pass_p99}" "milliseconds"
  done
}

check_sse_summary() {
  local summary_file="$1"
  [[ -f "${summary_file}" ]] || fatal "sse summary file not found: ${summary_file}"

  local lag_p95 lag_p99 lag_max stream_error alert_sync heartbeat connected
  lag_p95="$(jq -r '.lag_ms.p95 // "0"' "${summary_file}")"
  lag_p99="$(jq -r '.lag_ms.p99 // "0"' "${summary_file}")"
  lag_max="$(jq -r '.lag_ms.max // "0"' "${summary_file}")"
  stream_error="$(jq -r '.stream_events.stream_error // "0"' "${summary_file}")"
  alert_sync="$(jq -r '.stream_events.alert_monitoring_sync // "0"' "${summary_file}")"
  heartbeat="$(jq -r '.stream_events.stream_heartbeat // "0"' "${summary_file}")"
  connected="$(jq -r '.stream_events.stream_connected // "0"' "${summary_file}")"

  local lag_p95_max lag_p99_max lag_max_max stream_error_max alert_sync_min heartbeat_min connected_min
  lag_p95_max="$(jq -r '.sse.lag_ms.p95_ms_max' "${RESOLVED_POLICY_FILE}")"
  lag_p99_max="$(jq -r '.sse.lag_ms.p99_ms_max' "${RESOLVED_POLICY_FILE}")"
  lag_max_max="$(jq -r '.sse.lag_ms.max_ms_max' "${RESOLVED_POLICY_FILE}")"
  stream_error_max="$(jq -r '.sse.stream_error_max' "${RESOLVED_POLICY_FILE}")"
  alert_sync_min="$(jq -r '.sse.alert_sync_min' "${RESOLVED_POLICY_FILE}")"
  heartbeat_min="$(jq -r '.sse.heartbeat_min' "${RESOLVED_POLICY_FILE}")"
  connected_min="$(jq -r '.sse.connected_min' "${RESOLVED_POLICY_FILE}")"

  append_check "sse" "${summary_file}" "alert.monitoring_sync" "lag_p95_ms" "${lag_p95}" "${lag_p95_max}" "<=" "$(num_le "${lag_p95}" "${lag_p95_max}")" "milliseconds"
  append_check "sse" "${summary_file}" "alert.monitoring_sync" "lag_p99_ms" "${lag_p99}" "${lag_p99_max}" "<=" "$(num_le "${lag_p99}" "${lag_p99_max}")" "milliseconds"
  append_check "sse" "${summary_file}" "alert.monitoring_sync" "lag_max_ms" "${lag_max}" "${lag_max_max}" "<=" "$(num_le "${lag_max}" "${lag_max_max}")" "milliseconds"
  append_check "sse" "${summary_file}" "stream" "stream_error_count" "${stream_error}" "${stream_error_max}" "<=" "$(num_le "${stream_error}" "${stream_error_max}")" "count"
  append_check "sse" "${summary_file}" "stream" "alert_monitoring_sync_count" "${alert_sync}" "${alert_sync_min}" ">=" "$(num_ge "${alert_sync}" "${alert_sync_min}")" "count"
  append_check "sse" "${summary_file}" "stream" "stream_heartbeat_count" "${heartbeat}" "${heartbeat_min}" ">=" "$(num_ge "${heartbeat}" "${heartbeat_min}")" "count"
  append_check "sse" "${summary_file}" "stream" "stream_connected_count" "${connected}" "${connected_min}" ">=" "$(num_ge "${connected}" "${connected_min}")" "count"
}

write_summaries() {
  local generated_at api_summaries_json overall_pass_json
  generated_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  api_summaries_json="$(printf '%s\n' "${API_SUMMARIES[@]}" | sed '/^$/d' | jq -R . | jq -s '.')"

  if jq -s -e 'any(.[]; .pass == false)' "${TMP_RESULTS_FILE}" >/dev/null; then
    overall_pass_json="false"
  else
    overall_pass_json="true"
  fi

  jq -n \
    --arg run_id "${RUN_ID}" \
    --arg generated_at "${generated_at}" \
    --arg policy_file "${POLICY_FILE}" \
    --arg policy_source "${POLICY_SOURCE_PATH}" \
    --arg profile "${PROFILE_LABEL}" \
    --arg sse_summary "${SSE_SUMMARY}" \
    --argjson api_summaries "${api_summaries_json}" \
    --argjson pass "${overall_pass_json}" \
    --slurpfile checks "${TMP_RESULTS_FILE}" \
    '{
      run_id: $run_id,
      generated_at: $generated_at,
      profile: $profile,
      policy_file: $policy_file,
      policy_source: $policy_source,
      api_summaries: $api_summaries,
      sse_summary: ($sse_summary | if . == "" then null else . end),
      checks: $checks,
      pass: $pass,
      totals: {
        checks: ($checks | length),
        failed: ($checks | map(select(.pass == false)) | length)
      }
    }' >"${SUMMARY_JSON}"

  local result_text="PASS"
  if [[ "${overall_pass_json}" != "true" ]]; then
    result_text="FAIL"
  fi

  {
    echo "# Benchmark Threshold Gate Summary"
    echo
    echo "- Run ID: ${RUN_ID}"
    echo "- Generated at: ${generated_at}"
    echo "- Profile: ${PROFILE_LABEL}"
    echo "- Policy: \`${POLICY_FILE}\`"
    echo "- Policy source: \`${POLICY_SOURCE_PATH}\`"
    echo "- Result: **${result_text}**"
    echo
    echo "| Category | Source | Target | Metric | Actual | Threshold | Operator | Result | Note |"
    echo "| --- | --- | --- | --- | --- | --- | --- | --- | --- |"
    jq -s -r '.[] | "| `\(.category)` | `\(.source)` | `\(.target)` | `\(.metric)` | `\(.actual)` | `\(.threshold)` | `\(.operator)` | \((if .pass then "PASS" else "FAIL" end)) | \(.note) |"' "${TMP_RESULTS_FILE}"
  } >"${SUMMARY_MD}"

  log "Gate summary JSON: ${SUMMARY_JSON}"
  log "Gate summary Markdown: ${SUMMARY_MD}"

  if [[ "${overall_pass_json}" != "true" ]]; then
    fatal "benchmark threshold gate failed"
  fi
}

main() {
  require_cmd jq
  require_cmd awk

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --api-summary)
        [[ $# -ge 2 ]] || fatal "--api-summary requires a file path"
        API_SUMMARIES+=("$2")
        shift 2
        ;;
      --sse-summary)
        [[ $# -ge 2 ]] || fatal "--sse-summary requires a file path"
        SSE_SUMMARY="$2"
        shift 2
        ;;
      --policy)
        [[ $# -ge 2 ]] || fatal "--policy requires a file path"
        POLICY_FILE="$2"
        shift 2
        ;;
      --output-dir)
        [[ $# -ge 2 ]] || fatal "--output-dir requires a directory path"
        OUTPUT_DIR="$2"
        SUMMARY_JSON="${OUTPUT_DIR}/gate-summary.json"
        SUMMARY_MD="${OUTPUT_DIR}/gate-summary.md"
        shift 2
        ;;
      --profile)
        [[ $# -ge 2 ]] || fatal "--profile requires a label"
        PROFILE_LABEL="$2"
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

  [[ -f "${POLICY_FILE}" ]] || fatal "policy file not found: ${POLICY_FILE}"

  if [[ ${#API_SUMMARIES[@]} -eq 0 && -z "${SSE_SUMMARY}" ]]; then
    fatal "at least one --api-summary or --sse-summary must be provided"
  fi

  mkdir -p "${OUTPUT_DIR}"
  TMP_RESULTS_FILE="$(mktemp)"
  resolve_profile_policy
  trap 'rm -f "${TMP_RESULTS_FILE}" "${RESOLVED_POLICY_FILE}"' EXIT
  : >"${TMP_RESULTS_FILE}"

  local api_summary
  for api_summary in "${API_SUMMARIES[@]}"; do
    check_api_summary "${api_summary}"
  done
  if [[ -n "${SSE_SUMMARY}" ]]; then
    check_sse_summary "${SSE_SUMMARY}"
  fi

  local checks_count
  checks_count="$(jq -s 'length' "${TMP_RESULTS_FILE}")"
  if [[ "${checks_count}" == "0" ]]; then
    fatal "no checks were evaluated"
  fi

  write_summaries
}

main "$@"
