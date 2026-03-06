#!/usr/bin/env bash
set -Eeuo pipefail

API_BASE_URL="${API_BASE_URL:-http://127.0.0.1:8080}"
AUTH_USER="${AUTH_USER:-admin}"
PROFILE_LABEL="${BENCHMARK_PROFILE:-scale-5k}"
RUN_ID="${RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)}"
OUTPUT_DIR="${OUTPUT_DIR:-.run/benchmarks/multiregion-${RUN_ID}}"
POLICY_FILE="${POLICY_FILE:-scripts/benchmark-multiregion-policy.json}"

SKIP_BENCHMARK=0
BASELINE_API_SUMMARY=""
BASELINE_SSE_SUMMARY=""
BASELINE_DIR="${OUTPUT_DIR}/baseline"
SUMMARY_JSON="${OUTPUT_DIR}/summary.json"
SUMMARY_MD="${OUTPUT_DIR}/summary.md"
TMP_CHECKS=""
TMP_VIOLATIONS=""
TMP_SCENARIOS=""

log() {
  printf '[multiregion] %s\n' "$*"
}

fatal() {
  printf '[multiregion][ERROR] %s\n' "$*" >&2
  exit 1
}

usage() {
  cat <<'USAGE'
Multi-region benchmark simulation

Usage:
  bash scripts/benchmark-multiregion-sim.sh [options]

Options:
  --profile <label>         Benchmark profile for baseline run (default: scale-5k)
  --api-base-url <url>      API base URL (default: http://127.0.0.1:8080)
  --auth-user <username>    Auth user header value (default: admin)
  --policy <path>           Policy file (default: scripts/benchmark-multiregion-policy.json)
  --output-dir <path>       Output directory
  --skip-benchmark          Skip baseline run and use provided summaries
  --api-summary <path>      Existing API summary csv (required with --skip-benchmark)
  --sse-summary <path>      Existing SSE summary json (required with --skip-benchmark)
  -h, --help                Show this help
USAGE
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || fatal "missing required command: $1"
}

is_number() {
  [[ "$1" =~ ^-?[0-9]+([.][0-9]+)?$ ]]
}

num_le() {
  local actual="$1"
  local threshold="$2"
  awk -v a="${actual}" -v b="${threshold}" 'BEGIN { if ((a + 0) <= (b + 0)) print 1; else print 0 }'
}

append_check() {
  local category="$1"
  local scenario="$2"
  local target="$3"
  local metric="$4"
  local actual="$5"
  local threshold="$6"
  local operator="$7"
  local pass="$8"
  local note="$9"

  local pass_json="false"
  if [[ "${pass}" == "1" ]]; then
    pass_json="true"
  fi

  jq -nc \
    --arg category "${category}" \
    --arg scenario "${scenario}" \
    --arg target "${target}" \
    --arg metric "${metric}" \
    --arg actual "${actual}" \
    --arg threshold "${threshold}" \
    --arg operator "${operator}" \
    --arg note "${note}" \
    --argjson pass "${pass_json}" \
    '{
      category: $category,
      scenario: $scenario,
      target: $target,
      metric: $metric,
      actual: $actual,
      threshold: $threshold,
      operator: $operator,
      pass: $pass,
      note: $note
    }' >>"${TMP_CHECKS}"
}

append_violation() {
  local category="$1"
  local scenario="$2"
  local target="$3"
  local metric="$4"
  local actual="$5"
  local threshold="$6"
  local hint="$7"

  jq -nc \
    --arg category "${category}" \
    --arg scenario "${scenario}" \
    --arg target "${target}" \
    --arg metric "${metric}" \
    --arg actual "${actual}" \
    --arg threshold "${threshold}" \
    --arg hint "${hint}" \
    '{
      category: $category,
      scenario: $scenario,
      target: $target,
      metric: $metric,
      actual: $actual,
      threshold: $threshold,
      hint: $hint
    }' >>"${TMP_VIOLATIONS}"
}

validate_policy() {
  [[ -f "${POLICY_FILE}" ]] || fatal "policy file not found: ${POLICY_FILE}"
  jq -e '.regions | type == "object" and length > 0' "${POLICY_FILE}" >/dev/null \
    || fatal "policy must define non-empty regions object"
  jq -e '.budgets.api_classes.control_plane.p95_ms_max' "${POLICY_FILE}" >/dev/null \
    || fatal "policy missing budgets.api_classes.control_plane.p95_ms_max"
  jq -e '.budgets.api_classes.control_plane.p99_ms_max' "${POLICY_FILE}" >/dev/null \
    || fatal "policy missing budgets.api_classes.control_plane.p99_ms_max"
  jq -e '.budgets.api_classes.data_plane.p95_ms_max' "${POLICY_FILE}" >/dev/null \
    || fatal "policy missing budgets.api_classes.data_plane.p95_ms_max"
  jq -e '.budgets.api_classes.data_plane.p99_ms_max' "${POLICY_FILE}" >/dev/null \
    || fatal "policy missing budgets.api_classes.data_plane.p99_ms_max"
  jq -e '.budgets.sse.lag_p95_ms_max' "${POLICY_FILE}" >/dev/null \
    || fatal "policy missing budgets.sse.lag_p95_ms_max"
  jq -e '.budgets.sse.lag_p99_ms_max' "${POLICY_FILE}" >/dev/null \
    || fatal "policy missing budgets.sse.lag_p99_ms_max"
  jq -e '.budgets.sse.lag_max_ms_max' "${POLICY_FILE}" >/dev/null \
    || fatal "policy missing budgets.sse.lag_max_ms_max"
  jq -e '.budgets.sse.stream_error_count_max' "${POLICY_FILE}" >/dev/null \
    || fatal "policy missing budgets.sse.stream_error_count_max"

  local failover_from failover_to
  failover_from="$(jq -r '.failover.from_region' "${POLICY_FILE}")"
  failover_to="$(jq -r '.failover.to_region' "${POLICY_FILE}")"
  jq -e --arg region "${failover_from}" '.regions[$region] != null' "${POLICY_FILE}" >/dev/null \
    || fatal "policy failover.from_region not found in regions"
  jq -e --arg region "${failover_to}" '.regions[$region] != null' "${POLICY_FILE}" >/dev/null \
    || fatal "policy failover.to_region not found in regions"
}

prepare_scenarios() {
  : >"${TMP_SCENARIOS}"

  mapfile -t regions < <(jq -r '.regions | keys[]' "${POLICY_FILE}" | sort)
  local region
  for region in "${regions[@]}"; do
    local overhead
    overhead="$(jq -r --arg region "${region}" '.regions[$region].latency_overhead_ms' "${POLICY_FILE}")"
    jq -nc \
      --arg key "steady-${region}" \
      --arg type "steady" \
      --arg region "${region}" \
      --argjson latency_overhead_ms "${overhead}" \
      --argjson stream_error_penalty "0" \
      '{
        key: $key,
        type: $type,
        region: $region,
        latency_overhead_ms: $latency_overhead_ms,
        stream_error_penalty: $stream_error_penalty
      }' >>"${TMP_SCENARIOS}"
  done

  local failover_from failover_to region_overhead extra_penalty stream_error_penalty total_overhead
  failover_from="$(jq -r '.failover.from_region' "${POLICY_FILE}")"
  failover_to="$(jq -r '.failover.to_region' "${POLICY_FILE}")"
  region_overhead="$(jq -r --arg region "${failover_to}" '.regions[$region].latency_overhead_ms' "${POLICY_FILE}")"
  extra_penalty="$(jq -r '.failover.extra_latency_penalty_ms' "${POLICY_FILE}")"
  stream_error_penalty="$(jq -r '.failover.stream_error_penalty // 0' "${POLICY_FILE}")"
  total_overhead="$(awk -v r="${region_overhead}" -v e="${extra_penalty}" 'BEGIN { printf "%.3f", (r + 0) + (e + 0) }')"

  jq -nc \
    --arg key "failover-${failover_from}-to-${failover_to}" \
    --arg type "failover" \
    --arg region "${failover_to}" \
    --arg from_region "${failover_from}" \
    --argjson latency_overhead_ms "${total_overhead}" \
    --argjson stream_error_penalty "${stream_error_penalty}" \
    '{
      key: $key,
      type: $type,
      region: $region,
      from_region: $from_region,
      latency_overhead_ms: $latency_overhead_ms,
      stream_error_penalty: $stream_error_penalty
    }' >>"${TMP_SCENARIOS}"
}

run_baseline_benchmark() {
  mkdir -p "${BASELINE_DIR}"
  API_BASE_URL="${API_BASE_URL}" \
    AUTH_USER="${AUTH_USER}" \
    BENCHMARK_PROFILE="${PROFILE_LABEL}" \
    RUN_ID="${RUN_ID}" \
    OUTPUT_DIR="${BASELINE_DIR}" \
    bash scripts/benchmark-scale-profiles.sh \
      --profile "${PROFILE_LABEL}" \
      --api-base-url "${API_BASE_URL}" \
      --auth-user "${AUTH_USER}" \
      --output-dir "${BASELINE_DIR}" \
      --run-id "${RUN_ID}" \
      --skip-gate

  BASELINE_API_SUMMARY="${BASELINE_DIR}/api/summary.csv"
  BASELINE_SSE_SUMMARY="${BASELINE_DIR}/sse/summary.json"
}

prepare_baseline_inputs() {
  if (( SKIP_BENCHMARK == 1 )); then
    [[ -n "${BASELINE_API_SUMMARY}" ]] || fatal "--api-summary is required with --skip-benchmark"
    [[ -n "${BASELINE_SSE_SUMMARY}" ]] || fatal "--sse-summary is required with --skip-benchmark"
  else
    log "running baseline benchmark profile=${PROFILE_LABEL}"
    run_baseline_benchmark
  fi

  [[ -f "${BASELINE_API_SUMMARY}" ]] || fatal "API summary not found: ${BASELINE_API_SUMMARY}"
  [[ -f "${BASELINE_SSE_SUMMARY}" ]] || fatal "SSE summary not found: ${BASELINE_SSE_SUMMARY}"
}

projected_value() {
  local baseline="$1"
  local overhead="$2"
  awk -v b="${baseline}" -v o="${overhead}" 'BEGIN { printf "%.3f", (b + 0) + (o + 0) }'
}

evaluate_api_checks() {
  while IFS=',' read -r endpoint _method _path _total _success _failed _success_rate _avg _p50 _p90 p95 p99 _rps; do
    if [[ "${endpoint}" == "endpoint" ]]; then
      continue
    fi

    local endpoint_class p95_budget p99_budget
    endpoint_class="$(jq -r --arg endpoint "${endpoint}" '.endpoint_classes[$endpoint] // .endpoint_default_class // "data_plane"' "${POLICY_FILE}")"
    p95_budget="$(jq -r --arg cls "${endpoint_class}" '.budgets.api_classes[$cls].p95_ms_max' "${POLICY_FILE}")"
    p99_budget="$(jq -r --arg cls "${endpoint_class}" '.budgets.api_classes[$cls].p99_ms_max' "${POLICY_FILE}")"

    while IFS= read -r scenario; do
      [[ -n "${scenario}" ]] || continue
      local scenario_key scenario_type overhead proj_p95 proj_p99 pass_p95 pass_p99
      scenario_key="$(jq -r '.key' <<<"${scenario}")"
      scenario_type="$(jq -r '.type' <<<"${scenario}")"
      overhead="$(jq -r '.latency_overhead_ms' <<<"${scenario}")"

      proj_p95="$(projected_value "${p95}" "${overhead}")"
      proj_p99="$(projected_value "${p99}" "${overhead}")"
      pass_p95="$(num_le "${proj_p95}" "${p95_budget}")"
      pass_p99="$(num_le "${proj_p99}" "${p99_budget}")"

      append_check "api" "${scenario_key}" "${endpoint}" "p95_ms" "${proj_p95}" "${p95_budget}" "<=" "${pass_p95}" "${endpoint_class}"
      append_check "api" "${scenario_key}" "${endpoint}" "p99_ms" "${proj_p99}" "${p99_budget}" "<=" "${pass_p99}" "${endpoint_class}"

      if [[ "${pass_p95}" != "1" ]]; then
        if [[ "${scenario_type}" == "failover" ]]; then
          append_violation "api" "${scenario_key}" "${endpoint}" "p95_ms" "${proj_p95}" "${p95_budget}" \
            "Pre-warm failover region caches and trim cross-region dependencies for this endpoint."
        else
          append_violation "api" "${scenario_key}" "${endpoint}" "p95_ms" "${proj_p95}" "${p95_budget}" \
            "Investigate inter-region RTT and optimize endpoint payload/query path."
        fi
      fi
      if [[ "${pass_p99}" != "1" ]]; then
        if [[ "${scenario_type}" == "failover" ]]; then
          append_violation "api" "${scenario_key}" "${endpoint}" "p99_ms" "${proj_p99}" "${p99_budget}" \
            "Tune failover routing and retry policy to reduce tail latency during switchover."
        else
          append_violation "api" "${scenario_key}" "${endpoint}" "p99_ms" "${proj_p99}" "${p99_budget}" \
            "Investigate queue backpressure and database tail latency contributors."
        fi
      fi
    done <"${TMP_SCENARIOS}"
  done <"${BASELINE_API_SUMMARY}"
}

evaluate_sse_checks() {
  local base_lag_p95 base_lag_p99 base_lag_max base_stream_error
  local budget_lag_p95 budget_lag_p99 budget_lag_max budget_stream_error

  base_lag_p95="$(jq -r '.lag_ms.p95 // 0' "${BASELINE_SSE_SUMMARY}")"
  base_lag_p99="$(jq -r '.lag_ms.p99 // 0' "${BASELINE_SSE_SUMMARY}")"
  base_lag_max="$(jq -r '.lag_ms.max // 0' "${BASELINE_SSE_SUMMARY}")"
  base_stream_error="$(jq -r '.stream_events.stream_error // 0' "${BASELINE_SSE_SUMMARY}")"

  budget_lag_p95="$(jq -r '.budgets.sse.lag_p95_ms_max' "${POLICY_FILE}")"
  budget_lag_p99="$(jq -r '.budgets.sse.lag_p99_ms_max' "${POLICY_FILE}")"
  budget_lag_max="$(jq -r '.budgets.sse.lag_max_ms_max' "${POLICY_FILE}")"
  budget_stream_error="$(jq -r '.budgets.sse.stream_error_count_max' "${POLICY_FILE}")"

  while IFS= read -r scenario; do
    [[ -n "${scenario}" ]] || continue
    local scenario_key scenario_type overhead stream_error_penalty
    local proj_lag_p95 proj_lag_p99 proj_lag_max proj_stream_error
    local pass_lag_p95 pass_lag_p99 pass_lag_max pass_stream_error

    scenario_key="$(jq -r '.key' <<<"${scenario}")"
    scenario_type="$(jq -r '.type' <<<"${scenario}")"
    overhead="$(jq -r '.latency_overhead_ms' <<<"${scenario}")"
    stream_error_penalty="$(jq -r '.stream_error_penalty // 0' <<<"${scenario}")"

    proj_lag_p95="$(projected_value "${base_lag_p95}" "${overhead}")"
    proj_lag_p99="$(projected_value "${base_lag_p99}" "${overhead}")"
    proj_lag_max="$(projected_value "${base_lag_max}" "${overhead}")"
    proj_stream_error="$(awk -v b="${base_stream_error}" -v p="${stream_error_penalty}" 'BEGIN { printf "%.0f", (b + 0) + (p + 0) }')"

    pass_lag_p95="$(num_le "${proj_lag_p95}" "${budget_lag_p95}")"
    pass_lag_p99="$(num_le "${proj_lag_p99}" "${budget_lag_p99}")"
    pass_lag_max="$(num_le "${proj_lag_max}" "${budget_lag_max}")"
    pass_stream_error="$(num_le "${proj_stream_error}" "${budget_stream_error}")"

    append_check "sse" "${scenario_key}" "alert.monitoring_sync" "lag_p95_ms" "${proj_lag_p95}" "${budget_lag_p95}" "<=" "${pass_lag_p95}" "milliseconds"
    append_check "sse" "${scenario_key}" "alert.monitoring_sync" "lag_p99_ms" "${proj_lag_p99}" "${budget_lag_p99}" "<=" "${pass_lag_p99}" "milliseconds"
    append_check "sse" "${scenario_key}" "alert.monitoring_sync" "lag_max_ms" "${proj_lag_max}" "${budget_lag_max}" "<=" "${pass_lag_max}" "milliseconds"
    append_check "sse" "${scenario_key}" "stream" "stream_error_count" "${proj_stream_error}" "${budget_stream_error}" "<=" "${pass_stream_error}" "count"

    if [[ "${pass_lag_p95}" != "1" || "${pass_lag_p99}" != "1" || "${pass_lag_max}" != "1" ]]; then
      if [[ "${scenario_type}" == "failover" ]]; then
        append_violation "sse" "${scenario_key}" "alert.monitoring_sync" "lag" "${proj_lag_max}" "${budget_lag_max}" \
          "Increase stream worker capacity and broker headroom for failover region."
      else
        append_violation "sse" "${scenario_key}" "alert.monitoring_sync" "lag" "${proj_lag_max}" "${budget_lag_max}" \
          "Tune stream fanout and event payload size for cross-region delivery."
      fi
    fi
    if [[ "${pass_stream_error}" != "1" ]]; then
      append_violation "sse" "${scenario_key}" "stream" "stream_error_count" "${proj_stream_error}" "${budget_stream_error}" \
        "Review stream reconnect/backoff policy and broker stability under multi-region traffic."
    fi
  done <"${TMP_SCENARIOS}"
}

write_summary_files() {
  local overall_pass_json
  if jq -s -e 'any(.[]; .pass == false)' "${TMP_CHECKS}" >/dev/null; then
    overall_pass_json="false"
  else
    overall_pass_json="true"
  fi

  jq -n \
    --arg run_id "${RUN_ID}" \
    --arg generated_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    --arg profile "${PROFILE_LABEL}" \
    --arg api_base_url "${API_BASE_URL}" \
    --arg auth_user "${AUTH_USER}" \
    --arg policy_file "${POLICY_FILE}" \
    --arg api_summary "${BASELINE_API_SUMMARY}" \
    --arg sse_summary "${BASELINE_SSE_SUMMARY}" \
    --argjson pass "${overall_pass_json}" \
    --slurpfile scenarios "${TMP_SCENARIOS}" \
    --slurpfile checks "${TMP_CHECKS}" \
    --slurpfile violations "${TMP_VIOLATIONS}" \
    '{
      run_id: $run_id,
      generated_at: $generated_at,
      profile: $profile,
      api_base_url: $api_base_url,
      auth_user: $auth_user,
      policy_file: $policy_file,
      sources: {
        api_summary_csv: $api_summary,
        sse_summary_json: $sse_summary
      },
      scenarios: $scenarios,
      checks: $checks,
      violations: $violations,
      recommendations: ($violations | map(.hint) | unique),
      pass: $pass,
      totals: {
        scenarios: ($scenarios | length),
        checks: ($checks | length),
        failed_checks: ($checks | map(select(.pass == false)) | length),
        violations: ($violations | length)
      }
    }' >"${SUMMARY_JSON}"

  {
    echo "# Multi-Region Simulation Summary"
    echo
    echo "- Run ID: ${RUN_ID}"
    echo "- Profile: ${PROFILE_LABEL}"
    echo "- API Base URL: ${API_BASE_URL}"
    echo "- Auth User: ${AUTH_USER}"
    echo "- Policy file: \`${POLICY_FILE}\`"
    echo "- API summary: \`${BASELINE_API_SUMMARY}\`"
    echo "- SSE summary: \`${BASELINE_SSE_SUMMARY}\`"
    echo "- Result: **$(jq -r 'if .pass then "PASS" else "FAIL" end' "${SUMMARY_JSON}")**"
    echo
    echo "## Scenarios"
    echo
    echo "| Scenario | Type | Region | Latency Overhead ms | Stream Error Penalty |"
    echo "| --- | --- | --- | --- | --- |"
    jq -r '.scenarios[] | "| `\(.key)` | `\(.type)` | `\(.region)` | `\(.latency_overhead_ms)` | `\(.stream_error_penalty)` |"' "${SUMMARY_JSON}"
    echo
    echo "## Failed Checks"
    echo
    echo "| Category | Scenario | Target | Metric | Actual | Threshold | Note |"
    echo "| --- | --- | --- | --- | --- | --- | --- |"
  } >"${SUMMARY_MD}"

  if [[ "$(jq -r '.totals.failed_checks' "${SUMMARY_JSON}")" == "0" ]]; then
    echo "| - | - | - | - | - | - | no failed check |" >>"${SUMMARY_MD}"
  else
    jq -r '.checks[] | select(.pass == false) | "| `\(.category)` | `\(.scenario)` | `\(.target)` | `\(.metric)` | `\(.actual)` | `\(.threshold)` | \(.note) |"' "${SUMMARY_JSON}" >>"${SUMMARY_MD}"
  fi

  {
    echo
    echo "## Recommendations"
  } >>"${SUMMARY_MD}"
  if [[ "$(jq -r '.recommendations | length' "${SUMMARY_JSON}")" == "0" ]]; then
    echo "- No recommendation; all checks passed." >>"${SUMMARY_MD}"
  else
    jq -r '.recommendations[] | "- " + .' "${SUMMARY_JSON}" >>"${SUMMARY_MD}"
  fi

  log "summary JSON: ${SUMMARY_JSON}"
  log "summary Markdown: ${SUMMARY_MD}"
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
      --policy)
        [[ $# -ge 2 ]] || fatal "--policy requires a path"
        POLICY_FILE="$2"
        shift 2
        ;;
      --output-dir)
        [[ $# -ge 2 ]] || fatal "--output-dir requires a path"
        OUTPUT_DIR="$2"
        BASELINE_DIR="${OUTPUT_DIR}/baseline"
        SUMMARY_JSON="${OUTPUT_DIR}/summary.json"
        SUMMARY_MD="${OUTPUT_DIR}/summary.md"
        shift 2
        ;;
      --skip-benchmark)
        SKIP_BENCHMARK=1
        shift
        ;;
      --api-summary)
        [[ $# -ge 2 ]] || fatal "--api-summary requires a path"
        BASELINE_API_SUMMARY="$2"
        shift 2
        ;;
      --sse-summary)
        [[ $# -ge 2 ]] || fatal "--sse-summary requires a path"
        BASELINE_SSE_SUMMARY="$2"
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

  mkdir -p "${OUTPUT_DIR}"
  TMP_CHECKS="$(mktemp)"
  TMP_VIOLATIONS="$(mktemp)"
  TMP_SCENARIOS="$(mktemp)"
  trap 'rm -f "${TMP_CHECKS}" "${TMP_VIOLATIONS}" "${TMP_SCENARIOS}"' EXIT
  : >"${TMP_CHECKS}"
  : >"${TMP_VIOLATIONS}"
  : >"${TMP_SCENARIOS}"

  validate_policy
  prepare_scenarios
  prepare_baseline_inputs
  evaluate_api_checks
  evaluate_sse_checks
  write_summary_files

  if [[ "$(jq -r '.pass' "${SUMMARY_JSON}")" != "true" ]]; then
    fatal "multi-region simulation exceeded budget; inspect ${SUMMARY_MD}"
  fi
}

main "$@"
