#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

RUN_ID="${RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)}"
OUTPUT_DIR="${OUTPUT_DIR:-.run/qa/v0.1.7-journey-${RUN_ID}}"
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

SIM_EXECUTION_ID=""
LIVE_EXECUTION_ID=""
POLICY_MODE=""

log() {
  printf '[qa-v0.1.7] %s\n' "$*"
}

fatal() {
  printf '[qa-v0.1.7][ERROR] %s\n' "$*" >&2
  exit 1
}

usage() {
  cat <<'USAGE'
Runbook execution closure validation (v0.1.7)

Usage:
  bash scripts/qa-v0.1.7-operator-journey.sh [options]

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

stage_policy_read() {
  set_stage_context \
    "/api/v1/ops/cockpit/runbook-templates and /execution-policy" \
    "Verify runbook template catalog and execution policy endpoint RBAC mapping."

  request_api GET "/api/v1/ops/cockpit/runbook-templates"
  status_is_success "${LAST_STATUS}" || {
    echo "runbook templates load failed, status=${LAST_STATUS}"
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

  echo "execution policy read validated mode=${POLICY_MODE}"
  return 0
}

stage_policy_write() {
  set_stage_context \
    "/api/v1/ops/cockpit/runbook-templates/execution-policy (PUT)" \
    "Use operator/admin account to enable hybrid_live and allowlist dependency-check template."

  if (( READ_ONLY_MODE == 1 )); then
    echo "read-only mode enabled, skipping execution-policy write-path"
    return 2
  fi

  request_api PUT "/api/v1/ops/cockpit/runbook-templates/execution-policy" \
    '{"mode":"hybrid_live","live_templates":["dependency-check"],"max_live_step_timeout_seconds":10,"allow_simulate_failure":false,"note":"qa v0.1.7 policy update"}'

  if [[ "${LAST_STATUS}" == "403" ]]; then
    echo "execution policy update forbidden under current RBAC; skipped"
    cat "${LAST_BODY_FILE}" || true
    return 2
  fi

  status_is_success "${LAST_STATUS}" || {
    echo "execution policy update failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  request_api GET "/api/v1/ops/cockpit/runbook-templates/execution-policy"
  status_is_success "${LAST_STATUS}" || {
    echo "execution policy reload failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  POLICY_MODE="$(jq -r '.policy.mode // empty' "${LAST_BODY_FILE}")"
  [[ "${POLICY_MODE}" == "hybrid_live" ]] || {
    echo "execution policy mode mismatch after update: ${POLICY_MODE}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  local allowlisted
  allowlisted="$(jq -r '.policy.live_templates[]? | select(. == "dependency-check")' "${LAST_BODY_FILE}" | head -n 1)"
  [[ -n "${allowlisted}" ]] || {
    echo "dependency-check template is not allowlisted"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  echo "execution policy write validated mode=${POLICY_MODE}"
  return 0
}

stage_simulate_execute() {
  set_stage_context \
    "/api/v1/ops/cockpit/runbook-templates/{key}/execute (simulate)" \
    "Ensure simulate execution remains available with full preflight/evidence payload."

  if (( READ_ONLY_MODE == 1 )); then
    echo "read-only mode enabled, skipping simulate execute write-path"
    return 2
  fi

  request_api POST "/api/v1/ops/cockpit/runbook-templates/dependency-check/execute" \
    "{\"execution_mode\":\"simulate\",\"params\":{\"asset_ref\":\"qa-${RUN_ID}\",\"dependency_target\":\"127.0.0.1:8080\",\"probe_timeout_seconds\":5},\"preflight_confirmations\":[\"confirm_probe_source\",\"confirm_dependency_owner\",\"confirm_ticket_context\"],\"evidence\":{\"summary\":\"qa v0.1.7 simulate execution\",\"ticket_ref\":\"QA-SIM-${RUN_ID}\",\"artifact_url\":\"https://example.invalid/qa/${RUN_ID}/simulate\"},\"note\":\"qa simulate\"}"

  if [[ "${LAST_STATUS}" == "403" ]]; then
    echo "simulate execution forbidden under current RBAC; skipped"
    cat "${LAST_BODY_FILE}" || true
    return 2
  fi

  status_is_success "${LAST_STATUS}" || {
    echo "simulate execution failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  SIM_EXECUTION_ID="$(jq -r '.execution.id // empty' "${LAST_BODY_FILE}")"
  [[ -n "${SIM_EXECUTION_ID}" ]] || {
    echo "simulate execution response missing execution id"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  local mode
  mode="$(jq -r '.execution.execution_mode // empty' "${LAST_BODY_FILE}")"
  [[ "${mode}" == "simulate" ]] || {
    echo "simulate execution returned mode=${mode}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  echo "simulate execution validated execution_id=${SIM_EXECUTION_ID}"
  return 0
}

stage_live_execute() {
  set_stage_context \
    "/api/v1/ops/cockpit/runbook-templates/{key}/execute (live)" \
    "Enable hybrid_live policy and verify dependency-check live TCP probe path."

  if (( READ_ONLY_MODE == 1 )); then
    echo "read-only mode enabled, skipping live execute write-path"
    return 2
  fi

  request_api POST "/api/v1/ops/cockpit/runbook-templates/dependency-check/execute" \
    "{\"execution_mode\":\"live\",\"params\":{\"asset_ref\":\"qa-${RUN_ID}\",\"dependency_target\":\"127.0.0.1:8080\",\"probe_timeout_seconds\":5},\"preflight_confirmations\":[\"confirm_probe_source\",\"confirm_dependency_owner\",\"confirm_ticket_context\"],\"evidence\":{\"summary\":\"qa v0.1.7 live execution\",\"ticket_ref\":\"QA-LIVE-${RUN_ID}\",\"artifact_url\":\"https://example.invalid/qa/${RUN_ID}/live\"},\"note\":\"qa live\"}"

  if [[ "${LAST_STATUS}" == "403" ]]; then
    echo "live execution forbidden under current RBAC; skipped"
    cat "${LAST_BODY_FILE}" || true
    return 2
  fi

  status_is_success "${LAST_STATUS}" || {
    echo "live execution failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  LIVE_EXECUTION_ID="$(jq -r '.execution.id // empty' "${LAST_BODY_FILE}")"
  [[ -n "${LIVE_EXECUTION_ID}" ]] || {
    echo "live execution response missing execution id"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  local mode
  mode="$(jq -r '.execution.execution_mode // empty' "${LAST_BODY_FILE}")"
  [[ "${mode}" == "live" ]] || {
    echo "live execution returned mode=${mode}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  echo "live execution validated execution_id=${LIVE_EXECUTION_ID}"
  return 0
}

stage_execution_records() {
  set_stage_context \
    "/api/v1/ops/cockpit/runbook-templates/executions and /executions/{id}" \
    "Ensure execution list/detail include execution_mode and runtime_summary payload."

  request_api GET "/api/v1/ops/cockpit/runbook-templates/executions?template_key=dependency-check&limit=20"
  status_is_success "${LAST_STATUS}" || {
    echo "execution list failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  if [[ -n "${LIVE_EXECUTION_ID}" ]]; then
    request_api GET "/api/v1/ops/cockpit/runbook-templates/executions/${LIVE_EXECUTION_ID}"
    status_is_success "${LAST_STATUS}" || {
      echo "live execution detail failed, status=${LAST_STATUS}"
      cat "${LAST_BODY_FILE}" || true
      return 1
    }

    local runtime_mode
    runtime_mode="$(jq -r '.item.runtime_summary.mode // empty' "${LAST_BODY_FILE}")"
    [[ "${runtime_mode}" == "live" ]] || {
      echo "runtime summary mode mismatch: ${runtime_mode}"
      cat "${LAST_BODY_FILE}" || true
      return 1
    }
  fi

  echo "execution record read path validated"
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
    --arg version "v0.1.7" \
    --arg run_id "${RUN_ID}" \
    --arg generated_at "${generated_at}" \
    --arg api_base_url "${API_BASE_URL}" \
    --arg auth_user "${AUTH_USER}" \
    --argjson read_only "$( (( READ_ONLY_MODE == 1 )) && echo true || echo false )" \
    --arg overall "${overall}" \
    --arg policy_mode "${POLICY_MODE}" \
    --arg simulate_execution_id "${SIM_EXECUTION_ID}" \
    --arg live_execution_id "${LIVE_EXECUTION_ID}" \
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
        live_execution_id: $live_execution_id
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
    --arg version "v0.1.7" \
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
    echo "# v0.1.7 Operator Journey Validation Summary"
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
    echo "- live_execution_id=${LIVE_EXECUTION_ID:-none}"

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

  run_stage "policy_read" "runbook execution policy read path" stage_policy_read
  run_stage "policy_write" "runbook execution policy write path" stage_policy_write
  run_stage "simulate_execute" "runbook simulate execution path" stage_simulate_execute
  run_stage "live_execute" "runbook live execution path" stage_live_execute
  run_stage "execution_records" "runbook execution records path" stage_execution_records

  write_summary
}

main "$@"
