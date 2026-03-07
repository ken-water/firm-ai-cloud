#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

RUN_ID="${RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)}"
POLICY_FILE="${POLICY_FILE:-scripts/ops-escalation-failover-policy.json}"
OUTPUT_DIR="${OUTPUT_DIR:-.run/sim/v0.1.5-escalation-failover-${RUN_ID}}"
SUMMARY_JSON="${OUTPUT_DIR}/summary.json"
SUMMARY_MD="${OUTPUT_DIR}/summary.md"
API_BASE_URL="${API_BASE_URL:-http://127.0.0.1:8080}"
AUTH_USER="${AUTH_USER:-admin}"
USE_LIVE_BASELINE=0
ALLOW_FAIL=0

TMP_SCENARIOS=""
TMP_CHECKS=""
TMP_VIOLATIONS=""

BASE_ESC_BACKLOG=""
BASE_NEAR_BREACH=""
BASE_BREACHED=""
BASE_FAILOVER_RTO=""
BASE_FAILOVER_ERROR=""
BASELINE_SOURCE="policy"

log() {
  printf '[ops-sim] %s\n' "$*"
}

fatal() {
  printf '[ops-sim][ERROR] %s\n' "$*" >&2
  exit 1
}

usage() {
  cat <<'USAGE'
Escalation and failover simulation dry-run

Usage:
  bash scripts/ops-escalation-failover-sim.sh [options]

Options:
  --policy <path>          Simulation policy file
  --output-dir <path>      Output directory
  --run-id <id>            Stable run id used in artifact paths
  --api-base-url <url>     API base URL for optional live baseline fetch
  --auth-user <username>   Auth user header for optional live baseline fetch
  --live-baseline          Pull escalation queue baseline from API before simulation
  --allow-fail             Always exit 0 even when budget violations exist
  -h, --help               Show help
USAGE
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || fatal "missing required command: $1"
}

num_le() {
  local actual="$1"
  local threshold="$2"
  awk -v a="${actual}" -v b="${threshold}" 'BEGIN { if ((a + 0) <= (b + 0)) print 1; else print 0 }'
}

calc_value() {
  local baseline="$1"
  local multiplier="$2"
  local additive="$3"
  awk -v b="${baseline}" -v m="${multiplier}" -v a="${additive}" 'BEGIN { printf "%.3f", ((b + 0) * (m + 0)) + (a + 0) }'
}

append_check() {
  local scenario="$1"
  local metric="$2"
  local actual="$3"
  local threshold="$4"
  local pass="$5"
  local hint="$6"

  local pass_json="false"
  if [[ "${pass}" == "1" ]]; then
    pass_json="true"
  fi

  jq -nc \
    --arg scenario "${scenario}" \
    --arg metric "${metric}" \
    --arg actual "${actual}" \
    --arg threshold "${threshold}" \
    --arg hint "${hint}" \
    --argjson pass "${pass_json}" \
    '{
      scenario: $scenario,
      metric: $metric,
      actual: $actual,
      threshold: $threshold,
      pass: $pass,
      hint: $hint
    }' >>"${TMP_CHECKS}"
}

append_violation() {
  local scenario="$1"
  local metric="$2"
  local actual="$3"
  local threshold="$4"
  local hint="$5"

  jq -nc \
    --arg scenario "${scenario}" \
    --arg metric "${metric}" \
    --arg actual "${actual}" \
    --arg threshold "${threshold}" \
    --arg hint "${hint}" \
    '{
      scenario: $scenario,
      metric: $metric,
      actual: $actual,
      threshold: $threshold,
      hint: $hint
    }' >>"${TMP_VIOLATIONS}"
}

validate_policy() {
  [[ -f "${POLICY_FILE}" ]] || fatal "policy file not found: ${POLICY_FILE}"

  jq -e '.baseline.escalation_backlog' "${POLICY_FILE}" >/dev/null || fatal "policy missing baseline.escalation_backlog"
  jq -e '.baseline.near_breach_tickets' "${POLICY_FILE}" >/dev/null || fatal "policy missing baseline.near_breach_tickets"
  jq -e '.baseline.breached_tickets' "${POLICY_FILE}" >/dev/null || fatal "policy missing baseline.breached_tickets"
  jq -e '.baseline.failover_rto_minutes' "${POLICY_FILE}" >/dev/null || fatal "policy missing baseline.failover_rto_minutes"
  jq -e '.baseline.failover_error_rate' "${POLICY_FILE}" >/dev/null || fatal "policy missing baseline.failover_error_rate"

  jq -e '.budgets.escalation.backlog_max' "${POLICY_FILE}" >/dev/null || fatal "policy missing budgets.escalation.backlog_max"
  jq -e '.budgets.escalation.near_breach_max' "${POLICY_FILE}" >/dev/null || fatal "policy missing budgets.escalation.near_breach_max"
  jq -e '.budgets.escalation.breached_max' "${POLICY_FILE}" >/dev/null || fatal "policy missing budgets.escalation.breached_max"
  jq -e '.budgets.failover.rto_minutes_max' "${POLICY_FILE}" >/dev/null || fatal "policy missing budgets.failover.rto_minutes_max"
  jq -e '.budgets.failover.error_rate_max' "${POLICY_FILE}" >/dev/null || fatal "policy missing budgets.failover.error_rate_max"

  jq -e '.scenarios | type == "array" and length > 0' "${POLICY_FILE}" >/dev/null || fatal "policy must include non-empty scenarios[]"
}

load_policy_baseline() {
  BASE_ESC_BACKLOG="$(jq -r '.baseline.escalation_backlog' "${POLICY_FILE}")"
  BASE_NEAR_BREACH="$(jq -r '.baseline.near_breach_tickets' "${POLICY_FILE}")"
  BASE_BREACHED="$(jq -r '.baseline.breached_tickets' "${POLICY_FILE}")"
  BASE_FAILOVER_RTO="$(jq -r '.baseline.failover_rto_minutes' "${POLICY_FILE}")"
  BASE_FAILOVER_ERROR="$(jq -r '.baseline.failover_error_rate' "${POLICY_FILE}")"
}

try_live_baseline() {
  if (( USE_LIVE_BASELINE != 1 )); then
    return
  fi

  if ! curl -fsS "${API_BASE_URL}/health" >/dev/null 2>&1; then
    log "live baseline skipped: API health probe failed, fallback to policy baseline"
    return
  fi

  local queue_json
  if ! queue_json="$(curl -fsS -H "x-auth-user: ${AUTH_USER}" "${API_BASE_URL}/api/v1/tickets/escalation/queue?limit=300&offset=0")"; then
    log "live baseline skipped: escalation queue fetch failed, fallback to policy baseline"
    return
  fi

  local queue_total near_total breached_total
  queue_total="$(echo "${queue_json}" | jq -r '.items | length')"
  near_total="$(echo "${queue_json}" | jq -r '[.items[] | select(.escalation_state == "near_breach")] | length')"
  breached_total="$(echo "${queue_json}" | jq -r '[.items[] | select(.escalation_state == "breached")] | length')"

  BASE_ESC_BACKLOG="${queue_total}"
  BASE_NEAR_BREACH="${near_total}"
  BASE_BREACHED="${breached_total}"
  BASELINE_SOURCE="api:ticket-escalation-queue + policy:failover"
  log "live baseline enabled: queue=${BASE_ESC_BACKLOG}, near=${BASE_NEAR_BREACH}, breached=${BASE_BREACHED}"
}

simulate_scenarios() {
  local backlog_max near_max breached_max rto_max error_max
  backlog_max="$(jq -r '.budgets.escalation.backlog_max' "${POLICY_FILE}")"
  near_max="$(jq -r '.budgets.escalation.near_breach_max' "${POLICY_FILE}")"
  breached_max="$(jq -r '.budgets.escalation.breached_max' "${POLICY_FILE}")"
  rto_max="$(jq -r '.budgets.failover.rto_minutes_max' "${POLICY_FILE}")"
  error_max="$(jq -r '.budgets.failover.error_rate_max' "${POLICY_FILE}")"

  while IFS= read -r scenario; do
    [[ -n "${scenario}" ]] || continue

    local key description
    key="$(echo "${scenario}" | jq -r '.key')"
    description="$(echo "${scenario}" | jq -r '.description // ""')"

    local m_backlog a_backlog m_near a_near m_breached a_breached p_rto p_error
    m_backlog="$(echo "${scenario}" | jq -r '.escalation.backlog_multiplier // 1')"
    a_backlog="$(echo "${scenario}" | jq -r '.escalation.backlog_additive // 0')"
    m_near="$(echo "${scenario}" | jq -r '.escalation.near_breach_multiplier // 1')"
    a_near="$(echo "${scenario}" | jq -r '.escalation.near_breach_additive // 0')"
    m_breached="$(echo "${scenario}" | jq -r '.escalation.breached_multiplier // 1')"
    a_breached="$(echo "${scenario}" | jq -r '.escalation.breached_additive // 0')"
    p_rto="$(echo "${scenario}" | jq -r '.failover.rto_penalty_minutes // 0')"
    p_error="$(echo "${scenario}" | jq -r '.failover.error_penalty // 0')"

    local projected_backlog projected_near projected_breached projected_rto projected_error
    projected_backlog="$(calc_value "${BASE_ESC_BACKLOG}" "${m_backlog}" "${a_backlog}")"
    projected_near="$(calc_value "${BASE_NEAR_BREACH}" "${m_near}" "${a_near}")"
    projected_breached="$(calc_value "${BASE_BREACHED}" "${m_breached}" "${a_breached}")"
    projected_rto="$(calc_value "${BASE_FAILOVER_RTO}" "1" "${p_rto}")"
    projected_error="$(calc_value "${BASE_FAILOVER_ERROR}" "1" "${p_error}")"

    local hint_escalation hint_failover
    hint_escalation="$(echo "${scenario}" | jq -r '.hints.escalation // "Review escalation policy thresholds and queue ownership."')"
    hint_failover="$(echo "${scenario}" | jq -r '.hints.failover // "Review failover readiness and rollback drill cadence."')"

    local pass_backlog pass_near pass_breached pass_rto pass_error scenario_pass
    pass_backlog="$(num_le "${projected_backlog}" "${backlog_max}")"
    pass_near="$(num_le "${projected_near}" "${near_max}")"
    pass_breached="$(num_le "${projected_breached}" "${breached_max}")"
    pass_rto="$(num_le "${projected_rto}" "${rto_max}")"
    pass_error="$(num_le "${projected_error}" "${error_max}")"
    scenario_pass=1

    append_check "${key}" "escalation.backlog" "${projected_backlog}" "${backlog_max}" "${pass_backlog}" "${hint_escalation}"
    append_check "${key}" "escalation.near_breach" "${projected_near}" "${near_max}" "${pass_near}" "${hint_escalation}"
    append_check "${key}" "escalation.breached" "${projected_breached}" "${breached_max}" "${pass_breached}" "${hint_escalation}"
    append_check "${key}" "failover.rto_minutes" "${projected_rto}" "${rto_max}" "${pass_rto}" "${hint_failover}"
    append_check "${key}" "failover.error_rate" "${projected_error}" "${error_max}" "${pass_error}" "${hint_failover}"

    if [[ "${pass_backlog}" != "1" ]]; then
      append_violation "${key}" "escalation.backlog" "${projected_backlog}" "<=${backlog_max}" "${hint_escalation}"
      scenario_pass=0
    fi
    if [[ "${pass_near}" != "1" ]]; then
      append_violation "${key}" "escalation.near_breach" "${projected_near}" "<=${near_max}" "${hint_escalation}"
      scenario_pass=0
    fi
    if [[ "${pass_breached}" != "1" ]]; then
      append_violation "${key}" "escalation.breached" "${projected_breached}" "<=${breached_max}" "${hint_escalation}"
      scenario_pass=0
    fi
    if [[ "${pass_rto}" != "1" ]]; then
      append_violation "${key}" "failover.rto_minutes" "${projected_rto}" "<=${rto_max}" "${hint_failover}"
      scenario_pass=0
    fi
    if [[ "${pass_error}" != "1" ]]; then
      append_violation "${key}" "failover.error_rate" "${projected_error}" "<=${error_max}" "${hint_failover}"
      scenario_pass=0
    fi

    local pass_json="false"
    if (( scenario_pass == 1 )); then
      pass_json="true"
    fi

    jq -nc \
      --arg key "${key}" \
      --arg description "${description}" \
      --arg projected_backlog "${projected_backlog}" \
      --arg projected_near "${projected_near}" \
      --arg projected_breached "${projected_breached}" \
      --arg projected_rto "${projected_rto}" \
      --arg projected_error "${projected_error}" \
      --argjson pass "${pass_json}" \
      '{
        key: $key,
        description: $description,
        projected: {
          escalation_backlog: $projected_backlog,
          escalation_near_breach: $projected_near,
          escalation_breached: $projected_breached,
          failover_rto_minutes: $projected_rto,
          failover_error_rate: $projected_error
        },
        pass: $pass
      }' >>"${TMP_SCENARIOS}"
  done < <(jq -c '.scenarios[]' "${POLICY_FILE}")
}

write_summary() {
  local generated_at checks_total violations_total overall_pass_json
  generated_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  checks_total="$(jq -s 'length' "${TMP_CHECKS}")"
  violations_total="$(jq -s 'length' "${TMP_VIOLATIONS}")"

  overall_pass_json="true"
  if [[ "${violations_total}" != "0" ]]; then
    overall_pass_json="false"
  fi

  jq -n \
    --arg run_id "${RUN_ID}" \
    --arg generated_at "${generated_at}" \
    --arg policy_file "${POLICY_FILE}" \
    --arg baseline_source "${BASELINE_SOURCE}" \
    --arg base_esc_backlog "${BASE_ESC_BACKLOG}" \
    --arg base_near_breach "${BASE_NEAR_BREACH}" \
    --arg base_breached "${BASE_BREACHED}" \
    --arg base_failover_rto "${BASE_FAILOVER_RTO}" \
    --arg base_failover_error "${BASE_FAILOVER_ERROR}" \
    --argjson pass "${overall_pass_json}" \
    --slurpfile scenarios "${TMP_SCENARIOS}" \
    --slurpfile checks "${TMP_CHECKS}" \
    --slurpfile violations "${TMP_VIOLATIONS}" \
    '{
      run_id: $run_id,
      generated_at: $generated_at,
      policy_file: $policy_file,
      baseline_source: $baseline_source,
      baseline: {
        escalation_backlog: $base_esc_backlog,
        near_breach_tickets: $base_near_breach,
        breached_tickets: $base_breached,
        failover_rto_minutes: $base_failover_rto,
        failover_error_rate: $base_failover_error
      },
      scenarios: $scenarios,
      checks: $checks,
      violations: $violations,
      pass: $pass,
      totals: {
        scenarios: ($scenarios | length),
        checks: ($checks | length),
        violations: ($violations | length)
      }
    }' >"${SUMMARY_JSON}"

  local result_text="PASS"
  if [[ "${overall_pass_json}" != "true" ]]; then
    result_text="FAIL"
  fi

  {
    echo "# v0.1.5 Escalation and Failover Simulation Summary"
    echo
    echo "- Run ID: ${RUN_ID}"
    echo "- Generated at: ${generated_at}"
    echo "- Policy: \`${POLICY_FILE}\`"
    echo "- Baseline source: ${BASELINE_SOURCE}"
    echo "- Result: **${result_text}**"
    echo
    echo "## Baseline"
    echo
    echo "- escalation_backlog=${BASE_ESC_BACKLOG}"
    echo "- near_breach_tickets=${BASE_NEAR_BREACH}"
    echo "- breached_tickets=${BASE_BREACHED}"
    echo "- failover_rto_minutes=${BASE_FAILOVER_RTO}"
    echo "- failover_error_rate=${BASE_FAILOVER_ERROR}"
    echo
    echo "## Scenario Matrix"
    echo
    echo "| Scenario | Esc Backlog | Near Breach | Breached | Failover RTO(min) | Failover Error(%) | Result |"
    echo "| --- | --- | --- | --- | --- | --- | --- |"
    jq -r '.scenarios[] | "| `\(.key)` | \(.projected.escalation_backlog) | \(.projected.escalation_near_breach) | \(.projected.escalation_breached) | \(.projected.failover_rto_minutes) | \(.projected.failover_error_rate) | " + (if .pass then "PASS" else "FAIL" end) + " |"' "${SUMMARY_JSON}"

    if [[ "${violations_total}" != "0" ]]; then
      echo
      echo "## Budget Violations"
      jq -r '.violations[] | "- scenario=\(.scenario), metric=\(.metric), actual=\(.actual), threshold=\(.threshold)\n  - remediation: \(.hint)"' "${SUMMARY_JSON}"

      echo
      echo "## Recommended Actions"
      jq -r '.violations | map(.hint) | unique[] | "- " + .' "${SUMMARY_JSON}"
    else
      echo
      echo "No budget violations detected."
    fi

    echo
    echo "## Artifacts"
    echo
    echo "- JSON summary: ${SUMMARY_JSON}"
    echo "- Markdown summary: ${SUMMARY_MD}"
  } >"${SUMMARY_MD}"

  log "simulation summary JSON: ${SUMMARY_JSON}"
  log "simulation summary Markdown: ${SUMMARY_MD}"

  if [[ "${overall_pass_json}" != "true" && ${ALLOW_FAIL} -ne 1 ]]; then
    fatal "budget violations detected"
  fi
}

main() {
  require_cmd jq
  require_cmd awk
  require_cmd curl

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --policy)
        POLICY_FILE="$2"
        shift 2
        ;;
      --output-dir)
        OUTPUT_DIR="$2"
        shift 2
        ;;
      --run-id)
        RUN_ID="$2"
        shift 2
        ;;
      --api-base-url)
        API_BASE_URL="$2"
        shift 2
        ;;
      --auth-user)
        AUTH_USER="$2"
        shift 2
        ;;
      --live-baseline)
        USE_LIVE_BASELINE=1
        shift
        ;;
      --allow-fail)
        ALLOW_FAIL=1
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

  mkdir -p "${OUTPUT_DIR}"
  SUMMARY_JSON="${OUTPUT_DIR}/summary.json"
  SUMMARY_MD="${OUTPUT_DIR}/summary.md"

  TMP_SCENARIOS="$(mktemp)"
  TMP_CHECKS="$(mktemp)"
  TMP_VIOLATIONS="$(mktemp)"
  trap 'rm -f "${TMP_SCENARIOS}" "${TMP_CHECKS}" "${TMP_VIOLATIONS}"' EXIT

  validate_policy
  load_policy_baseline
  try_live_baseline
  simulate_scenarios
  write_summary
}

main "$@"
