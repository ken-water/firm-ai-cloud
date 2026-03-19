#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

RUN_ID="${RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)}"
OUTPUT_DIR="${OUTPUT_DIR:-.run/qa/v0.1.24-demo-scenarios-${RUN_ID}}"
SUMMARY_JSON="${OUTPUT_DIR}/summary.json"
SUMMARY_MD="${OUTPUT_DIR}/summary.md"
ARTIFACT_INDEX_JSON="${OUTPUT_DIR}/artifact-index.json"
API_BASE_URL="${API_BASE_URL:-http://127.0.0.1:8080}"
AUTH_USER="${AUTH_USER:-admin}"

TMP_STAGES=""
LAST_STATUS=""
LAST_BODY_FILE=""
STAGE_ENDPOINT=""
STAGE_REMEDIATION=""

MONITORING_FILE="${OUTPUT_DIR}/monitoring-overview.json"
TOPOLOGY_BOARD_FILE="${OUTPUT_DIR}/topology-board.json"
HANDOVER_READINESS_FILE="${OUTPUT_DIR}/handover-readiness.json"
AI_EVIDENCE_FILE="${OUTPUT_DIR}/ai-evidence.json"

log() {
  printf '[qa-v0.1.24] %s\n' "$*"
}

fatal() {
  printf '[qa-v0.1.24][ERROR] %s\n' "$*" >&2
  exit 1
}

usage() {
  cat <<'USAGE'
Demo scenario pack validation (v0.1.24)

Usage:
  bash scripts/qa-v0.1.24-demo-scenarios.sh [options]

Options:
  --api-base-url <url>       API base URL (default: http://127.0.0.1:8080)
  --auth-user <username>     Auth user header (default: admin)
  --output-dir <path>        Output directory
  --run-id <id>              Run id
  -h, --help                 Show help
USAGE
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || fatal "missing required command: $1"
}

set_stage_context() {
  STAGE_ENDPOINT="$1"
  STAGE_REMEDIATION="$2"
}

request_api() {
  local method="$1"
  local path="$2"
  local body="${3-}"

  LAST_BODY_FILE="$(mktemp)"
  local -a args=(-sS -o "${LAST_BODY_FILE}" -w "%{http_code}" -X "${method}" "${API_BASE_URL}${path}" -H "x-auth-user: ${AUTH_USER}")
  if [[ -n "${body}" ]]; then
    args+=(-H "Content-Type: application/json" -d "${body}")
  fi
  if ! LAST_STATUS="$(curl "${args[@]}")"; then
    LAST_STATUS="000"
  fi
}

status_is_success() {
  [[ "$1" == "200" || "$1" == "201" ]]
}

append_stage() {
  local stage_key="$1"
  local stage_name="$2"
  local status="$3"
  local log_file="$4"
  local diag
  diag="$(tail -n 8 "${log_file}" | tr '\n' ' ' | sed 's/[[:space:]]\+/ /g' | sed 's/^ //;s/ $//')"

  jq -nc \
    --arg key "${stage_key}" \
    --arg name "${stage_name}" \
    --arg status "${status}" \
    --arg endpoint "${STAGE_ENDPOINT}" \
    --arg remediation "${STAGE_REMEDIATION}" \
    --arg log_file "${log_file}" \
    --arg diagnostics "${diag}" \
    '{key: $key, name: $name, status: $status, endpoint: $endpoint, remediation: $remediation, log_file: $log_file, diagnostics: $diagnostics}' >>"${TMP_STAGES}"
}

run_stage() {
  local stage_key="$1"
  local stage_name="$2"
  local stage_fn="$3"
  local log_file="${OUTPUT_DIR}/${stage_key}.log"

  STAGE_ENDPOINT=""
  STAGE_REMEDIATION=""
  log "running stage=${stage_key}"

  set +e
  "${stage_fn}" >"${log_file}" 2>&1
  local rc=$?
  set -e

  if (( rc == 0 )); then
    append_stage "${stage_key}" "${stage_name}" "pass" "${log_file}"
    return
  fi
  append_stage "${stage_key}" "${stage_name}" "fail" "${log_file}"
}

stage_monitoring_read() {
  set_stage_context \
    "/api/v1/monitoring/overview" \
    "Verify monitoring overview endpoint is reachable with expected summary fields."

  request_api GET "/api/v1/monitoring/overview"
  status_is_success "${LAST_STATUS}" || {
    echo "monitoring overview read failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  cp "${LAST_BODY_FILE}" "${MONITORING_FILE}"
  jq -e '.summary | has("source_total") and has("source_unreachable_total")' "${MONITORING_FILE}" >/dev/null
}

stage_topology_board_read() {
  set_stage_context \
    "/api/v1/ops/cockpit/topology-board" \
    "Verify topology board returns summary and item fields for demo risk scan."

  request_api GET "/api/v1/ops/cockpit/topology-board?limit=20"
  status_is_success "${LAST_STATUS}" || {
    echo "topology board read failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  cp "${LAST_BODY_FILE}" "${TOPOLOGY_BOARD_FILE}"
  jq -e '.summary | has("service_total") and has("critical_total") and has("unassigned_owner_total")' "${TOPOLOGY_BOARD_FILE}" >/dev/null
}

stage_handover_readiness_read() {
  set_stage_context \
    "/api/v1/ops/cockpit/handover-readiness" \
    "Verify handover readiness state and reason model are returned for shift decisions."

  request_api GET "/api/v1/ops/cockpit/handover-readiness"
  status_is_success "${LAST_STATUS}" || {
    echo "handover readiness read failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  cp "${LAST_BODY_FILE}" "${HANDOVER_READINESS_FILE}"
  jq -e 'has("readiness_state") and (.summary | has("blocking")) and (.reasons | type == "array")' "${HANDOVER_READINESS_FILE}" >/dev/null
}

stage_ai_evidence_read() {
  set_stage_context \
    "/api/v1/ops/cockpit/ai/evidence-query" \
    "Verify AI evidence flow and guided actions are returned for demo narrative."

  local payload
  payload="$(jq -nc '{module: "monitoring", intent: "health_summary", question: "Demo monitoring risk summary", time_window_hours: 24}')"
  request_api POST "/api/v1/ops/cockpit/ai/evidence-query" "${payload}"
  status_is_success "${LAST_STATUS}" || {
    echo "ai evidence query failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  cp "${LAST_BODY_FILE}" "${AI_EVIDENCE_FILE}"
  jq -e '.answer | has("summary") and has("evidence_total")' "${AI_EVIDENCE_FILE}" >/dev/null
  jq -e 'has("guided_actions") and (.guided_actions | type == "array")' "${AI_EVIDENCE_FILE}" >/dev/null
}

stage_demo_chain_assertions() {
  set_stage_context \
    "cross-module" \
    "Verify demo chain metrics are simultaneously available across monitoring, topology, handoff, and AI."

  jq -e '.summary.source_total >= 0' "${MONITORING_FILE}" >/dev/null
  jq -e '.summary.service_total >= 0 and .summary.critical_total >= 0' "${TOPOLOGY_BOARD_FILE}" >/dev/null
  jq -e '(.readiness_state == "ready" or .readiness_state == "at_risk" or .readiness_state == "blocking")' "${HANDOVER_READINESS_FILE}" >/dev/null
  jq -e '.answer.evidence_total >= 0' "${AI_EVIDENCE_FILE}" >/dev/null
}

write_summary() {
  local total pass fail
  total="$(wc -l <"${TMP_STAGES}" | tr -d ' ')"
  pass="$(jq -s '[.[] | select(.status == "pass")] | length' "${TMP_STAGES}")"
  fail="$(jq -s '[.[] | select(.status == "fail")] | length' "${TMP_STAGES}")"

  jq -n \
    --arg run_id "${RUN_ID}" \
    --arg generated_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    --arg api_base_url "${API_BASE_URL}" \
    --arg auth_user "${AUTH_USER}" \
    --argjson total "${total}" \
    --argjson pass "${pass}" \
    --argjson fail "${fail}" \
    --slurpfile stages "${TMP_STAGES}" \
    '{
      run_id: $run_id,
      generated_at: $generated_at,
      api_base_url: $api_base_url,
      auth_user: $auth_user,
      summary: {
        total: $total,
        pass: $pass,
        fail: $fail
      },
      stages: ($stages[0] // [])
    }' >"${SUMMARY_JSON}"

  {
    echo "# v0.1.24 Demo Scenario Validation"
    echo
    echo "- run_id: ${RUN_ID}"
    echo "- api_base_url: ${API_BASE_URL}"
    echo "- auth_user: ${AUTH_USER}"
    echo "- total stages: ${total}"
    echo "- pass: ${pass}"
    echo "- fail: ${fail}"
    echo
    echo "## Stage Results"
    jq -r '.stages[] | "- [\(.status == \"pass\" ? \"x\" : \" \" )] \(.key): \(.name)"' "${SUMMARY_JSON}"
  } >"${SUMMARY_MD}"

  jq -n \
    --arg summary_json "${SUMMARY_JSON}" \
    --arg summary_md "${SUMMARY_MD}" \
    --arg monitoring_file "${MONITORING_FILE}" \
    --arg topology_board_file "${TOPOLOGY_BOARD_FILE}" \
    --arg handover_readiness_file "${HANDOVER_READINESS_FILE}" \
    --arg ai_evidence_file "${AI_EVIDENCE_FILE}" \
    '{
      artifacts: [
        {key: "summary_json", path: $summary_json},
        {key: "summary_md", path: $summary_md},
        {key: "monitoring_overview", path: $monitoring_file},
        {key: "topology_board", path: $topology_board_file},
        {key: "handover_readiness", path: $handover_readiness_file},
        {key: "ai_evidence", path: $ai_evidence_file}
      ]
    }' >"${ARTIFACT_INDEX_JSON}"
}

main() {
  require_cmd curl
  require_cmd jq

  while [[ $# -gt 0 ]]; do
    case "$1" in
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
      --output-dir)
        [[ $# -ge 2 ]] || fatal "--output-dir requires a value"
        OUTPUT_DIR="$2"
        SUMMARY_JSON="${OUTPUT_DIR}/summary.json"
        SUMMARY_MD="${OUTPUT_DIR}/summary.md"
        ARTIFACT_INDEX_JSON="${OUTPUT_DIR}/artifact-index.json"
        MONITORING_FILE="${OUTPUT_DIR}/monitoring-overview.json"
        TOPOLOGY_BOARD_FILE="${OUTPUT_DIR}/topology-board.json"
        HANDOVER_READINESS_FILE="${OUTPUT_DIR}/handover-readiness.json"
        AI_EVIDENCE_FILE="${OUTPUT_DIR}/ai-evidence.json"
        shift 2
        ;;
      --run-id)
        [[ $# -ge 2 ]] || fatal "--run-id requires a value"
        RUN_ID="$2"
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
  TMP_STAGES="$(mktemp)"

  run_stage "monitoring_overview_read" "Monitoring overview read" stage_monitoring_read
  run_stage "topology_board_read" "Topology board read" stage_topology_board_read
  run_stage "handover_readiness_read" "Handover readiness read" stage_handover_readiness_read
  run_stage "ai_evidence_read" "AI evidence read" stage_ai_evidence_read
  run_stage "demo_chain_assertions" "Cross-module demo chain assertions" stage_demo_chain_assertions

  write_summary

  if jq -e '.summary.fail == 0' "${SUMMARY_JSON}" >/dev/null; then
    log "PASS: all stages passed"
    log "artifacts: ${OUTPUT_DIR}"
    exit 0
  fi

  log "FAIL: one or more stages failed"
  log "artifacts: ${OUTPUT_DIR}"
  exit 1
}

main "$@"
