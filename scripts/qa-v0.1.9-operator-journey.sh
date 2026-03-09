#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

RUN_ID="${RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)}"
OUTPUT_DIR="${OUTPUT_DIR:-.run/qa/v0.1.9-journey-${RUN_ID}}"
SUMMARY_JSON="${OUTPUT_DIR}/summary.json"
SUMMARY_MD="${OUTPUT_DIR}/summary.md"
ARTIFACT_INDEX_JSON="${OUTPUT_DIR}/artifact-index.json"
API_BASE_URL="${API_BASE_URL:-http://127.0.0.1:8080}"
AUTH_USER="${AUTH_USER:-admin}"
READ_ONLY_MODE=0

TMP_STAGES=""
LAST_STATUS=""
LAST_BODY_FILE=""

STAGE_ENDPOINT=""
STAGE_REMEDIATION=""

POLICY_MODE=""
SIM_EXECUTION_ID=""
REPLAY_EXECUTION_ID=""
FAILED_EXECUTION_ID=""

log() {
  printf '[qa-v0.1.9] %s\n' "$*"
}

fatal() {
  printf '[qa-v0.1.9][ERROR] %s\n' "$*" >&2
  exit 1
}

usage() {
  cat <<'USAGE'
Runbook analytics closure validation (v0.1.9)

Usage:
  bash scripts/qa-v0.1.9-operator-journey.sh [options]

Options:
  --api-base-url <url>     API base URL
  --auth-user <username>   Auth user header (default: admin)
  --output-dir <path>      Output directory
  --run-id <id>            Run id
  --read-only              Skip write-path stages (for read-only validation)
  -h, --help               Show help
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
    args+=( -H "Content-Type: application/json" -d "${body}" )
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
  diag="$(tail -n 6 "${log_file}" | tr '\n' ' ' | sed 's/[[:space:]]\+/ /g' | sed 's/^ //;s/ $//')"

  jq -nc \
    --arg key "${stage_key}" \
    --arg name "${stage_name}" \
    --arg status "${status}" \
    --arg endpoint "${STAGE_ENDPOINT}" \
    --arg remediation "${STAGE_REMEDIATION}" \
    --arg log_file "${log_file}" \
    --arg diagnostics "${diag}" \
    '{
      key: $key,
      name: $name,
      status: $status,
      endpoint: $endpoint,
      remediation: $remediation,
      log_file: $log_file,
      diagnostics: $diagnostics
    }' >>"${TMP_STAGES}"
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

  if (( rc == 2 )); then
    append_stage "${stage_key}" "${stage_name}" "skipped" "${log_file}"
    return
  fi

  append_stage "${stage_key}" "${stage_name}" "fail" "${log_file}"
}

stage_baseline_read() {
  set_stage_context \
    "/api/v1/ops/cockpit/runbook-templates, /execution-policy, /executions" \
    "Verify runbook baseline read path and policy payload shape."

  request_api GET "/api/v1/ops/cockpit/runbook-templates"
  status_is_success "${LAST_STATUS}" || {
    echo "runbook templates load failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  local template_total
  template_total="$(jq -r '.total // 0' "${LAST_BODY_FILE}")"
  [[ "${template_total}" =~ ^[0-9]+$ ]] || {
    echo "runbook templates total is invalid: ${template_total}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  request_api GET "/api/v1/ops/cockpit/runbook-templates/execution-policy"
  status_is_success "${LAST_STATUS}" || {
    echo "execution policy load failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  POLICY_MODE="$(jq -r '.policy.mode // empty' "${LAST_BODY_FILE}")"
  [[ -n "${POLICY_MODE}" ]] || {
    echo "execution policy response missing mode"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  request_api GET "/api/v1/ops/cockpit/runbook-templates/executions?template_key=dependency-check&limit=10"
  status_is_success "${LAST_STATUS}" || {
    echo "runbook execution list failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  echo "baseline read validated policy_mode=${POLICY_MODE} templates=${template_total}"
  return 0
}

stage_simulate_execute() {
  set_stage_context \
    "/api/v1/ops/cockpit/runbook-templates/{key}/execute (simulate)" \
    "Create one successful and one failed dependency-check execution for analytics verification."

  if (( READ_ONLY_MODE == 1 )); then
    echo "read-only mode enabled, skipping simulate execute write-path"
    return 2
  fi

  request_api POST "/api/v1/ops/cockpit/runbook-templates/dependency-check/execute" \
    "{\"execution_mode\":\"simulate\",\"params\":{\"asset_ref\":\"qa-${RUN_ID}\",\"dependency_target\":\"127.0.0.1:8080\",\"probe_timeout_seconds\":5},\"preflight_confirmations\":[\"confirm_probe_source\",\"confirm_dependency_owner\",\"confirm_ticket_context\"],\"evidence\":{\"summary\":\"qa v0.1.9 simulate success\",\"ticket_ref\":\"QA-SIM-${RUN_ID}\",\"artifact_url\":\"https://example.invalid/qa/${RUN_ID}/simulate-success\"},\"note\":\"qa success simulate\"}"

  if [[ "${LAST_STATUS}" == "403" ]]; then
    echo "simulate execution forbidden under current RBAC; skipped"
    cat "${LAST_BODY_FILE}" || true
    return 2
  fi

  status_is_success "${LAST_STATUS}" || {
    echo "simulate success execution failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  SIM_EXECUTION_ID="$(jq -r '.execution.id // empty' "${LAST_BODY_FILE}")"
  [[ -n "${SIM_EXECUTION_ID}" ]] || {
    echo "simulate success execution response missing execution id"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  request_api POST "/api/v1/ops/cockpit/runbook-templates/dependency-check/execute" \
    "{\"execution_mode\":\"simulate\",\"params\":{\"asset_ref\":\"qa-${RUN_ID}\",\"dependency_target\":\"unstable-cache:6379\",\"probe_timeout_seconds\":5},\"preflight_confirmations\":[\"confirm_probe_source\",\"confirm_dependency_owner\",\"confirm_ticket_context\"],\"evidence\":{\"summary\":\"qa v0.1.9 simulate failure\",\"ticket_ref\":\"QA-FAIL-${RUN_ID}\",\"artifact_url\":\"https://example.invalid/qa/${RUN_ID}/simulate-failure\"},\"note\":\"qa failed simulate\"}"

  status_is_success "${LAST_STATUS}" || {
    echo "simulate failure execution call failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  FAILED_EXECUTION_ID="$(jq -r '.execution.id // empty' "${LAST_BODY_FILE}")"
  [[ -n "${FAILED_EXECUTION_ID}" ]] || {
    echo "simulate failure execution response missing execution id"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  local failed_status
  failed_status="$(jq -r '.execution.status // empty' "${LAST_BODY_FILE}")"
  [[ "${failed_status}" == "failed" ]] || {
    echo "expected failed status for unstable execution, got=${failed_status}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  echo "simulate executions validated success_id=${SIM_EXECUTION_ID} failed_id=${FAILED_EXECUTION_ID}"
  return 0
}

stage_replay_execute() {
  set_stage_context \
    "/api/v1/ops/cockpit/runbook-templates/executions/{id}/replay" \
    "Replay successful baseline execution and verify source linkage."

  if (( READ_ONLY_MODE == 1 )); then
    echo "read-only mode enabled, skipping replay write-path"
    return 2
  fi

  [[ -n "${SIM_EXECUTION_ID}" ]] || {
    echo "simulate execution id is required before replay"
    return 1
  }

  request_api POST "/api/v1/ops/cockpit/runbook-templates/executions/${SIM_EXECUTION_ID}/replay" \
    "{\"note\":\"qa v0.1.9 replay\"}"

  if [[ "${LAST_STATUS}" == "403" ]]; then
    echo "replay execution forbidden under current RBAC; skipped"
    cat "${LAST_BODY_FILE}" || true
    return 2
  fi

  status_is_success "${LAST_STATUS}" || {
    echo "replay execution failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  REPLAY_EXECUTION_ID="$(jq -r '.execution.id // empty' "${LAST_BODY_FILE}")"
  [[ -n "${REPLAY_EXECUTION_ID}" ]] || {
    echo "replay response missing execution id"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  local replay_source
  replay_source="$(jq -r '.execution.replay_source_execution_id // empty' "${LAST_BODY_FILE}")"
  [[ "${replay_source}" == "${SIM_EXECUTION_ID}" ]] || {
    echo "replay source mismatch: expected=${SIM_EXECUTION_ID} actual=${replay_source}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  echo "replay execution validated execution_id=${REPLAY_EXECUTION_ID} source=${SIM_EXECUTION_ID}"
  return 0
}

stage_analytics_summary() {
  set_stage_context \
    "/api/v1/ops/cockpit/runbook-templates/analytics/summary" \
    "Verify summary response includes totals and hotspot breakdown."

  request_api GET "/api/v1/ops/cockpit/runbook-templates/analytics/summary?days=14&template_key=dependency-check"
  status_is_success "${LAST_STATUS}" || {
    echo "analytics summary failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  local execution_total success_rate
  execution_total="$(jq -r '.totals.executions // empty' "${LAST_BODY_FILE}")"
  success_rate="$(jq -r '.totals.success_rate_percent // empty' "${LAST_BODY_FILE}")"
  [[ "${execution_total}" =~ ^[0-9]+$ ]] || {
    echo "analytics summary total is invalid: ${execution_total}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  [[ "${success_rate}" =~ ^[0-9]+(\.[0-9]+)?$ ]] || {
    echo "analytics summary success_rate_percent is invalid: ${success_rate}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  if [[ -n "${FAILED_EXECUTION_ID}" ]]; then
    local hotspot_found
    hotspot_found="$(jq -r '.failed_steps[]? | select(.step_id == "reachability_probe") | .step_id' "${LAST_BODY_FILE}" | head -n 1)"
    [[ -n "${hotspot_found}" ]] || {
      echo "expected reachability_probe hotspot not found"
      cat "${LAST_BODY_FILE}" || true
      return 1
    }
  fi

  echo "analytics summary validated totals.executions=${execution_total} success_rate=${success_rate}"
  return 0
}

stage_failure_feed() {
  set_stage_context \
    "/api/v1/ops/cockpit/runbook-templates/analytics/failures" \
    "Verify failure feed payload and failed execution visibility."

  request_api GET "/api/v1/ops/cockpit/runbook-templates/analytics/failures?days=14&template_key=dependency-check&limit=20"
  status_is_success "${LAST_STATUS}" || {
    echo "failure feed failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  local total
  total="$(jq -r '.total // empty' "${LAST_BODY_FILE}")"
  [[ "${total}" =~ ^[0-9]+$ ]] || {
    echo "failure feed total is invalid: ${total}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  if [[ -n "${FAILED_EXECUTION_ID}" ]]; then
    local failed_item
    failed_item="$(jq -r --arg eid "${FAILED_EXECUTION_ID}" '.items[]? | select((.id|tostring)==$eid) | .id' "${LAST_BODY_FILE}" | head -n 1)"
    [[ -n "${failed_item}" ]] || {
      echo "failed execution ${FAILED_EXECUTION_ID} not found in failure feed"
      cat "${LAST_BODY_FILE}" || true
      return 1
    }
  fi

  echo "failure feed validated total=${total}"
  return 0
}

stage_records_verify() {
  set_stage_context \
    "/api/v1/ops/cockpit/runbook-templates/executions/{id}" \
    "Ensure replay linkage and failed execution timeline are persisted."

  if [[ -n "${REPLAY_EXECUTION_ID}" ]]; then
    request_api GET "/api/v1/ops/cockpit/runbook-templates/executions/${REPLAY_EXECUTION_ID}"
    status_is_success "${LAST_STATUS}" || {
      echo "replay execution detail failed, status=${LAST_STATUS}"
      cat "${LAST_BODY_FILE}" || true
      return 1
    }

    local replay_source runtime_replay_source
    replay_source="$(jq -r '.item.replay_source_execution_id // empty' "${LAST_BODY_FILE}")"
    runtime_replay_source="$(jq -r '.item.runtime_summary.replay_source_execution_id // empty' "${LAST_BODY_FILE}")"
    [[ "${replay_source}" == "${SIM_EXECUTION_ID}" ]] || {
      echo "replay detail source mismatch: ${replay_source}"
      cat "${LAST_BODY_FILE}" || true
      return 1
    }
    [[ "${runtime_replay_source}" == "${SIM_EXECUTION_ID}" ]] || {
      echo "runtime summary replay source mismatch: ${runtime_replay_source}"
      cat "${LAST_BODY_FILE}" || true
      return 1
    }
  fi

  if [[ -n "${FAILED_EXECUTION_ID}" ]]; then
    request_api GET "/api/v1/ops/cockpit/runbook-templates/executions/${FAILED_EXECUTION_ID}"
    status_is_success "${LAST_STATUS}" || {
      echo "failed execution detail lookup failed, status=${LAST_STATUS}"
      cat "${LAST_BODY_FILE}" || true
      return 1
    }

    local failed_status failed_step
    failed_status="$(jq -r '.item.status // empty' "${LAST_BODY_FILE}")"
    failed_step="$(jq -r '.item.timeline[]? | select(.status == "failed") | .step_id' "${LAST_BODY_FILE}" | head -n 1)"
    [[ "${failed_status}" == "failed" ]] || {
      echo "failed execution status mismatch: ${failed_status}"
      cat "${LAST_BODY_FILE}" || true
      return 1
    }
    [[ -n "${failed_step}" ]] || {
      echo "failed execution timeline missing failed step"
      cat "${LAST_BODY_FILE}" || true
      return 1
    }
  fi

  echo "record verification completed"
  return 0
}

write_summary() {
  local generated_at fail_count overall
  generated_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

  fail_count="$(jq -s 'map(select(.status == "fail")) | length' "${TMP_STAGES}")"
  overall="pass"
  if [[ "${fail_count}" != "0" ]]; then
    overall="fail"
  fi

  jq -n \
    --arg version "v0.1.9" \
    --arg run_id "${RUN_ID}" \
    --arg generated_at "${generated_at}" \
    --arg api_base_url "${API_BASE_URL}" \
    --arg auth_user "${AUTH_USER}" \
    --argjson read_only "$( (( READ_ONLY_MODE == 1 )) && echo true || echo false )" \
    --arg overall "${overall}" \
    --arg policy_mode "${POLICY_MODE}" \
    --arg simulate_execution_id "${SIM_EXECUTION_ID}" \
    --arg replay_execution_id "${REPLAY_EXECUTION_ID}" \
    --arg failed_execution_id "${FAILED_EXECUTION_ID}" \
    --slurpfile stages "${TMP_STAGES}" \
    '{
      version: $version,
      run_id: $run_id,
      generated_at: $generated_at,
      api_base_url: $api_base_url,
      auth_user: $auth_user,
      read_only_mode: $read_only,
      overall: $overall,
      entities: {
        policy_mode: $policy_mode,
        simulate_execution_id: $simulate_execution_id,
        replay_execution_id: $replay_execution_id,
        failed_execution_id: $failed_execution_id
      },
      stages: $stages,
      totals: {
        total: ($stages | length),
        pass: ($stages | map(select(.status == "pass")) | length),
        fail: ($stages | map(select(.status == "fail")) | length),
        skipped: ($stages | map(select(.status == "skipped")) | length)
      }
    }' >"${SUMMARY_JSON}"

  jq -n \
    --arg generated_at "${generated_at}" \
    --arg summary_json "${SUMMARY_JSON}" \
    --arg summary_md "${SUMMARY_MD}" \
    --arg version "v0.1.9" \
    --slurpfile stages "${TMP_STAGES}" \
    '{
      generated_at: $generated_at,
      version: $version,
      artifacts: ([
        {kind: "summary_json", path: $summary_json},
        {kind: "summary_md", path: $summary_md}
      ] + ($stages | map({kind: "stage_log", stage: .key, path: .log_file})))
    }' >"${ARTIFACT_INDEX_JSON}"

  {
    echo "# v0.1.9 Operator Journey Validation Summary"
    echo
    echo "- Run ID: ${RUN_ID}"
    echo "- Generated at: ${generated_at}"
    echo "- API base URL: ${API_BASE_URL}"
    echo "- Auth user: ${AUTH_USER}"
    echo "- Read-only mode: $([[ ${READ_ONLY_MODE} -eq 1 ]] && echo true || echo false)"
    echo "- Overall: **${overall}**"
    echo
    echo "## Stage Results"
    echo
    echo "| Stage | Result | Endpoint | Remediation Pointer | Diagnostics | Log |"
    echo "| --- | --- | --- | --- | --- | --- |"
    jq -r '.stages[] | "| `\(.key)` | \(.status) | `\(.endpoint)` | \(.remediation) | \(.diagnostics) | `\(.log_file)` |"' "${SUMMARY_JSON}"

    echo
    echo "## Entity References"
    echo
    echo "- policy_mode=${POLICY_MODE:-none}"
    echo "- simulate_execution_id=${SIM_EXECUTION_ID:-none}"
    echo "- replay_execution_id=${REPLAY_EXECUTION_ID:-none}"
    echo "- failed_execution_id=${FAILED_EXECUTION_ID:-none}"

    if [[ "${fail_count}" != "0" ]]; then
      echo
      echo "## Failure Localization"
      jq -r '.stages[] | select(.status == "fail") | "- stage=\(.key) endpoint=\(.endpoint) remediation=\(.remediation) diagnostics=\(.diagnostics)"' "${SUMMARY_JSON}"
    fi

    echo
    echo "## Artifacts"
    echo
    echo "- JSON summary: ${SUMMARY_JSON}"
    echo "- Markdown summary: ${SUMMARY_MD}"
    echo "- Artifact index: ${ARTIFACT_INDEX_JSON}"
  } >"${SUMMARY_MD}"

  log "summary JSON: ${SUMMARY_JSON}"
  log "summary Markdown: ${SUMMARY_MD}"
  log "artifact index: ${ARTIFACT_INDEX_JSON}"

  if [[ "${overall}" != "pass" ]]; then
    exit 1
  fi
}

main() {
  require_cmd curl
  require_cmd jq

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --api-base-url)
        API_BASE_URL="$2"
        shift 2
        ;;
      --auth-user)
        AUTH_USER="$2"
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
      --read-only)
        READ_ONLY_MODE=1
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
  ARTIFACT_INDEX_JSON="${OUTPUT_DIR}/artifact-index.json"

  TMP_STAGES="$(mktemp)"
  trap 'rm -f "${TMP_STAGES}" "${LAST_BODY_FILE:-}"' EXIT

  run_stage "baseline_read" "runbook baseline read path" stage_baseline_read
  run_stage "simulate_execute" "runbook simulate execution path" stage_simulate_execute
  run_stage "replay_execute" "runbook replay execution path" stage_replay_execute
  run_stage "analytics_summary" "runbook analytics summary path" stage_analytics_summary
  run_stage "failure_feed" "runbook analytics failure feed path" stage_failure_feed
  run_stage "records_verify" "runbook execution records verify path" stage_records_verify

  write_summary
}

main "$@"
