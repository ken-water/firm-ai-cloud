#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

RUN_ID="${RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)}"
OUTPUT_DIR="${OUTPUT_DIR:-.run/qa/v0.1.10-journey-${RUN_ID}}"
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
ANALYTICS_POLICY_KEY=""
ANALYTICS_THRESHOLD=""
ANALYTICS_SAMPLE=""
ANALYTICS_NOTE=""

SIM_EXECUTION_ID=""
REPLAY_EXECUTION_ID=""
FAILED_EXECUTION_ID=""

log() {
  printf '[qa-v0.1.10] %s\n' "$*"
}

fatal() {
  printf '[qa-v0.1.10][ERROR] %s\n' "$*" >&2
  exit 1
}

usage() {
  cat <<'USAGE'
Runbook risk policy and alert validation (v0.1.10)

Usage:
  bash scripts/qa-v0.1.10-operator-journey.sh [options]

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
    "/api/v1/ops/cockpit/runbook-templates, /execution-policy, /analytics/policy" \
    "Verify baseline runbook reads and analytics risk policy payload shape."

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

  request_api GET "/api/v1/ops/cockpit/runbook-templates/analytics/policy"
  status_is_success "${LAST_STATUS}" || {
    echo "analytics policy load failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  ANALYTICS_POLICY_KEY="$(jq -r '.policy.policy_key // empty' "${LAST_BODY_FILE}")"
  ANALYTICS_THRESHOLD="$(jq -r '.policy.failure_rate_threshold_percent // empty' "${LAST_BODY_FILE}")"
  ANALYTICS_SAMPLE="$(jq -r '.policy.minimum_sample_size // empty' "${LAST_BODY_FILE}")"
  ANALYTICS_NOTE="$(jq -r '.policy.note // empty' "${LAST_BODY_FILE}")"

  [[ -n "${ANALYTICS_POLICY_KEY}" ]] || {
    echo "analytics policy response missing policy_key"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  [[ "${ANALYTICS_THRESHOLD}" =~ ^[0-9]+$ ]] || {
    echo "analytics policy threshold is invalid: ${ANALYTICS_THRESHOLD}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  [[ "${ANALYTICS_SAMPLE}" =~ ^[0-9]+$ ]] || {
    echo "analytics policy sample is invalid: ${ANALYTICS_SAMPLE}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  echo "policy read validated mode=${POLICY_MODE} policy=${ANALYTICS_POLICY_KEY} threshold=${ANALYTICS_THRESHOLD}% sample=${ANALYTICS_SAMPLE}"
  return 0
}

stage_policy_write() {
  set_stage_context \
    "/api/v1/ops/cockpit/runbook-templates/analytics/policy (PUT)" \
    "Verify write-capable actor can submit analytics policy payload with valid bounds."

  if (( READ_ONLY_MODE == 1 )); then
    echo "read-only mode enabled, skipping policy write path"
    return 2
  fi

  [[ -n "${ANALYTICS_POLICY_KEY}" ]] || {
    echo "analytics policy read stage must run before policy write"
    return 1
  }

  local body
  body="$(jq -nc \
    --argjson threshold "${ANALYTICS_THRESHOLD}" \
    --argjson sample "${ANALYTICS_SAMPLE}" \
    --arg note "${ANALYTICS_NOTE}" \
    '{
      failure_rate_threshold_percent: $threshold,
      minimum_sample_size: $sample,
      note: (if ($note | length) > 0 then $note else null end)
    }')"

  request_api PUT "/api/v1/ops/cockpit/runbook-templates/analytics/policy" "${body}"

  if [[ "${LAST_STATUS}" == "403" ]]; then
    echo "policy write forbidden under current RBAC; skipped"
    cat "${LAST_BODY_FILE}" || true
    return 2
  fi

  status_is_success "${LAST_STATUS}" || {
    echo "policy write failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  local returned_key returned_threshold returned_sample
  returned_key="$(jq -r '.policy.policy_key // empty' "${LAST_BODY_FILE}")"
  returned_threshold="$(jq -r '.policy.failure_rate_threshold_percent // empty' "${LAST_BODY_FILE}")"
  returned_sample="$(jq -r '.policy.minimum_sample_size // empty' "${LAST_BODY_FILE}")"

  [[ "${returned_key}" == "${ANALYTICS_POLICY_KEY}" ]] || {
    echo "policy key mismatch: expected=${ANALYTICS_POLICY_KEY} actual=${returned_key}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  [[ "${returned_threshold}" == "${ANALYTICS_THRESHOLD}" ]] || {
    echo "policy threshold mismatch: expected=${ANALYTICS_THRESHOLD} actual=${returned_threshold}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  [[ "${returned_sample}" == "${ANALYTICS_SAMPLE}" ]] || {
    echo "policy sample mismatch: expected=${ANALYTICS_SAMPLE} actual=${returned_sample}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  request_api GET "/api/v1/ops/cockpit/runbook-templates/analytics/policy"
  status_is_success "${LAST_STATUS}" || {
    echo "analytics policy re-read after write failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  local persisted_threshold persisted_sample
  persisted_threshold="$(jq -r '.policy.failure_rate_threshold_percent // empty' "${LAST_BODY_FILE}")"
  persisted_sample="$(jq -r '.policy.minimum_sample_size // empty' "${LAST_BODY_FILE}")"

  [[ "${persisted_threshold}" == "${ANALYTICS_THRESHOLD}" ]] || {
    echo "persisted threshold mismatch: expected=${ANALYTICS_THRESHOLD} actual=${persisted_threshold}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  [[ "${persisted_sample}" == "${ANALYTICS_SAMPLE}" ]] || {
    echo "persisted sample mismatch: expected=${ANALYTICS_SAMPLE} actual=${persisted_sample}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  echo "policy write validated policy=${ANALYTICS_POLICY_KEY} threshold=${ANALYTICS_THRESHOLD}% sample=${ANALYTICS_SAMPLE}"
  return 0
}

stage_simulate_execute() {
  set_stage_context \
    "/api/v1/ops/cockpit/runbook-templates/{key}/execute (simulate)" \
    "Create one successful and one failed dependency-check execution for policy/alert verification."

  if (( READ_ONLY_MODE == 1 )); then
    echo "read-only mode enabled, skipping simulate execute write-path"
    return 2
  fi

  local success_payload
  success_payload="$(jq -nc \
    --arg run_id "${RUN_ID}" \
    '{
      execution_mode: "simulate",
      params: {
        asset_ref: ("qa-" + $run_id),
        dependency_target: "127.0.0.1:8080",
        probe_timeout_seconds: 5
      },
      preflight_confirmations: [
        "confirm_probe_source",
        "confirm_dependency_owner",
        "confirm_ticket_context"
      ],
      evidence: {
        summary: "qa v0.1.10 simulate success",
        ticket_ref: ("QA-SIM-" + $run_id),
        artifact_url: ("https://example.invalid/qa/" + $run_id + "/simulate-success")
      },
      note: "qa success simulate"
    }')"

  request_api POST "/api/v1/ops/cockpit/runbook-templates/dependency-check/execute" "${success_payload}"

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

  local failed_payload
  failed_payload="$(jq -nc \
    --arg run_id "${RUN_ID}" \
    '{
      execution_mode: "simulate",
      params: {
        asset_ref: ("qa-" + $run_id),
        dependency_target: "unstable-cache:6379",
        probe_timeout_seconds: 5
      },
      preflight_confirmations: [
        "confirm_probe_source",
        "confirm_dependency_owner",
        "confirm_ticket_context"
      ],
      evidence: {
        summary: "qa v0.1.10 simulate failure",
        ticket_ref: ("QA-FAIL-" + $run_id),
        artifact_url: ("https://example.invalid/qa/" + $run_id + "/simulate-failure")
      },
      note: "qa failed simulate"
    }')"

  request_api POST "/api/v1/ops/cockpit/runbook-templates/dependency-check/execute" "${failed_payload}"
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

  local replay_payload
  replay_payload="$(jq -nc --arg run_id "${RUN_ID}" '{note: ("qa v0.1.10 replay " + $run_id)}')"

  request_api POST "/api/v1/ops/cockpit/runbook-templates/executions/${SIM_EXECUTION_ID}/replay" "${replay_payload}"

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

stage_alerts_read() {
  set_stage_context \
    "/api/v1/ops/cockpit/runbook-templates/analytics/alerts" \
    "Verify risk alert response envelope, policy snapshot, and alert item structure."

  request_api GET "/api/v1/ops/cockpit/runbook-templates/analytics/alerts?days=14&template_key=dependency-check&limit=20"
  status_is_success "${LAST_STATUS}" || {
    echo "risk alerts read failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  local total threshold sample
  total="$(jq -r '.total // empty' "${LAST_BODY_FILE}")"
  threshold="$(jq -r '.policy.failure_rate_threshold_percent // empty' "${LAST_BODY_FILE}")"
  sample="$(jq -r '.policy.minimum_sample_size // empty' "${LAST_BODY_FILE}")"

  [[ "${total}" =~ ^[0-9]+$ ]] || {
    echo "risk alerts total is invalid: ${total}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  [[ "${threshold}" =~ ^[0-9]+$ ]] || {
    echo "risk alerts policy threshold is invalid: ${threshold}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  [[ "${sample}" =~ ^[0-9]+$ ]] || {
    echo "risk alerts policy sample is invalid: ${sample}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  if jq -e '.items | length > 0' "${LAST_BODY_FILE}" >/dev/null 2>&1; then
    local first_template first_severity first_action
    first_template="$(jq -r '.items[0].template_key // empty' "${LAST_BODY_FILE}")"
    first_severity="$(jq -r '.items[0].severity // empty' "${LAST_BODY_FILE}")"
    first_action="$(jq -r '.items[0].recommended_action // empty' "${LAST_BODY_FILE}")"

    [[ -n "${first_template}" ]] || {
      echo "first alert item missing template_key"
      cat "${LAST_BODY_FILE}" || true
      return 1
    }
    [[ "${first_severity}" == "warning" || "${first_severity}" == "critical" ]] || {
      echo "first alert item has invalid severity=${first_severity}"
      cat "${LAST_BODY_FILE}" || true
      return 1
    }
    [[ -n "${first_action}" ]] || {
      echo "first alert item missing recommended_action"
      cat "${LAST_BODY_FILE}" || true
      return 1
    }
  fi

  echo "alerts read validated total=${total} threshold=${threshold}% sample=${sample}"
  return 0
}

stage_records_verify() {
  set_stage_context \
    "/api/v1/ops/cockpit/runbook-templates/executions, /executions/{id}" \
    "Ensure replay linkage and failed execution timeline are persisted in execution records."

  request_api GET "/api/v1/ops/cockpit/runbook-templates/executions?template_key=dependency-check&limit=40"
  status_is_success "${LAST_STATUS}" || {
    echo "runbook execution list verification failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  if [[ -n "${SIM_EXECUTION_ID}" ]]; then
    local sim_found
    sim_found="$(jq -r --arg eid "${SIM_EXECUTION_ID}" '.items[]? | select((.id|tostring)==$eid) | .id' "${LAST_BODY_FILE}" | head -n 1)"
    [[ -n "${sim_found}" ]] || {
      echo "simulate execution ${SIM_EXECUTION_ID} not found in execution list"
      cat "${LAST_BODY_FILE}" || true
      return 1
    }
  fi

  if [[ -n "${REPLAY_EXECUTION_ID}" ]]; then
    local replay_found
    replay_found="$(jq -r --arg eid "${REPLAY_EXECUTION_ID}" '.items[]? | select((.id|tostring)==$eid) | .id' "${LAST_BODY_FILE}" | head -n 1)"
    [[ -n "${replay_found}" ]] || {
      echo "replay execution ${REPLAY_EXECUTION_ID} not found in execution list"
      cat "${LAST_BODY_FILE}" || true
      return 1
    }

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
    local failed_found
    failed_found="$(jq -r --arg eid "${FAILED_EXECUTION_ID}" '.items[]? | select((.id|tostring)==$eid) | .id' "${LAST_BODY_FILE}" | head -n 1)"
    [[ -n "${failed_found}" ]] || {
      echo "failed execution ${FAILED_EXECUTION_ID} not found in execution list"
      cat "${LAST_BODY_FILE}" || true
      return 1
    }

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
    --arg version "v0.1.10" \
    --arg run_id "${RUN_ID}" \
    --arg generated_at "${generated_at}" \
    --arg api_base_url "${API_BASE_URL}" \
    --arg auth_user "${AUTH_USER}" \
    --argjson read_only "$( (( READ_ONLY_MODE == 1 )) && echo true || echo false )" \
    --arg overall "${overall}" \
    --arg policy_mode "${POLICY_MODE}" \
    --arg analytics_policy_key "${ANALYTICS_POLICY_KEY}" \
    --arg analytics_threshold "${ANALYTICS_THRESHOLD}" \
    --arg analytics_sample "${ANALYTICS_SAMPLE}" \
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
        analytics_policy_key: $analytics_policy_key,
        analytics_failure_rate_threshold_percent: $analytics_threshold,
        analytics_minimum_sample_size: $analytics_sample,
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
    --arg version "v0.1.10" \
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
    echo "# v0.1.10 Operator Journey Validation Summary"
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
    echo "- analytics_policy_key=${ANALYTICS_POLICY_KEY:-none}"
    echo "- analytics_failure_rate_threshold_percent=${ANALYTICS_THRESHOLD:-none}"
    echo "- analytics_minimum_sample_size=${ANALYTICS_SAMPLE:-none}"
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

  run_stage "policy_read" "runbook policy read path" stage_policy_read
  run_stage "policy_write" "runbook policy write path" stage_policy_write
  run_stage "simulate_execute" "runbook simulate execution path" stage_simulate_execute
  run_stage "replay_execute" "runbook replay execution path" stage_replay_execute
  run_stage "alerts_read" "runbook proactive risk alerts read path" stage_alerts_read
  run_stage "records_verify" "runbook execution records verify path" stage_records_verify

  write_summary
}

main "$@"
