#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

RUN_ID="${RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)}"
OUTPUT_DIR="${OUTPUT_DIR:-.run/qa/v0.1.17-journey-${RUN_ID}}"
SUMMARY_JSON="${OUTPUT_DIR}/summary.json"
SUMMARY_MD="${OUTPUT_DIR}/summary.md"
ARTIFACT_INDEX_JSON="${OUTPUT_DIR}/artifact-index.json"
API_BASE_URL="${API_BASE_URL:-http://127.0.0.1:8080}"
AUTH_USER="${AUTH_USER:-admin}"
READ_ONLY_AUTH_USER="${READ_ONLY_AUTH_USER:-viewer}"
READ_ONLY_MODE=0
OUTPUT_DIR_EXPLICIT=0

TMP_STAGES=""
LAST_STATUS=""
LAST_BODY_FILE=""
STAGE_ENDPOINT=""
STAGE_REMEDIATION=""
ACTIVATION_FILE="${OUTPUT_DIR}/activation.json"
STARTER_TEMPLATES_FILE="${OUTPUT_DIR}/starter-templates.json"
PROFILE_APPLY_FILE="${OUTPUT_DIR}/profile-apply.json"
FEEDBACK_FILE="${OUTPUT_DIR}/feedback.json"

log() {
  printf '[qa-v0.1.17] %s\n' "$*"
}

fatal() {
  printf '[qa-v0.1.17][ERROR] %s\n' "$*" >&2
  exit 1
}

usage() {
  cat <<'USAGE'
First-value activation journey validation (v0.1.17)

Usage:
  bash scripts/qa-v0.1.17-activation-journey.sh [options]

Options:
  --api-base-url <url>       API base URL
  --auth-user <username>     Auth user header for normal stages (default: admin)
  --read-only-user <user>    Auth user used for guard probe (default: viewer)
  --output-dir <path>        Output directory
  --run-id <id>              Run id
  --read-only                Skip write-path stages
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

request_api_as_user() {
  local auth_user="$1"
  local method="$2"
  local path="$3"
  local body="${4-}"

  LAST_BODY_FILE="$(mktemp)"
  local -a args=(-sS -o "${LAST_BODY_FILE}" -w "%{http_code}" -X "${method}" "${API_BASE_URL}${path}" -H "x-auth-user: ${auth_user}")
  if [[ -n "${body}" ]]; then
    args+=(-H "Content-Type: application/json" -d "${body}")
  fi
  if ! LAST_STATUS="$(curl "${args[@]}")"; then
    LAST_STATUS="000"
  fi
}

request_api() {
  request_api_as_user "${AUTH_USER}" "$@"
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
  if (( rc == 2 )); then
    append_stage "${stage_key}" "${stage_name}" "skipped" "${log_file}"
    return
  fi
  append_stage "${stage_key}" "${stage_name}" "fail" "${log_file}"
}

fetch_activation() {
  request_api GET "/api/v1/setup/activation"
  status_is_success "${LAST_STATUS}" || {
    echo "activation read failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  cp "${LAST_BODY_FILE}" "${ACTIVATION_FILE}"
}

fetch_starter_templates() {
  request_api GET "/api/v1/setup/activation/starter-templates"
  status_is_success "${LAST_STATUS}" || {
    echo "starter template read failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  cp "${LAST_BODY_FILE}" "${STARTER_TEMPLATES_FILE}"
}

recommended_profile_key() {
  jq -r '.recommended_profile_key // empty' "${ACTIVATION_FILE}"
}

stage_activation_status_read() {
  set_stage_context \
    "/api/v1/setup/activation" \
    "Verify the activation status endpoint exposes overall status, summary, and deterministic next step metadata."

  fetch_activation || return 1

  jq -e '
    (.overall_status | IN("ready", "warning", "blocking"))
    and (.summary.total >= 3)
    and (.items | length >= 3)
    and ([.items[]?.item_key] | index("environment_preflight") != null)
    and ([.items[]?.item_key] | index("integration_checklist") != null)
    and ([.items[]?.item_key] | index("operator_profile") != null)
    and (.recommended_profile_key | type == "string")
  ' "${ACTIVATION_FILE}" >/dev/null || {
    echo "activation payload missing required structure"
    cat "${ACTIVATION_FILE}" || true
    return 1
  }

  jq '{overall_status, recommended_next_step_key, recommended_profile_key, summary, items: [.items[] | {item_key, status}]}' "${ACTIVATION_FILE}"
}

stage_starter_template_visibility_read() {
  set_stage_context \
    "/api/v1/setup/activation/starter-templates" \
    "Validate the starter-template catalog is readable and contains the recommended SMB template metadata."

  fetch_starter_templates || return 1

  jq -e '
    . as $root
    | .total >= 3
    and (.recommended_template_key | type == "string")
    and ([.items[]?.template_key] | index($root.recommended_template_key) != null)
    and ([.items[] | {
      template_key,
      name,
      summary,
      target_scale,
      first_value_goal,
      recommended_when,
      profile_key,
      defaults
    }] | all(
      (.template_key | type == "string")
      and (.name | type == "string")
      and (.summary | type == "string")
      and (.target_scale | type == "string")
      and (.first_value_goal | type == "string")
      and (.recommended_when | type == "string")
      and (.profile_key | type == "string")
      and (.defaults | type == "object")
    ))
  ' "${STARTER_TEMPLATES_FILE}" >/dev/null || {
    echo "starter-template payload missing required render fields"
    cat "${STARTER_TEMPLATES_FILE}" || true
    return 1
  }

  jq '{recommended_template_key, items: [.items[] | {template_key, profile_key, target_scale}]}' "${STARTER_TEMPLATES_FILE}"
}

stage_activation_progress_apply() {
  set_stage_context \
    "/api/v1/setup/profiles/<recommended>/apply" \
    "Apply the recommended SMB starter profile so activation can advance from blank-page setup toward first value."

  if (( READ_ONLY_MODE == 1 )); then
    echo "read-only mode: activation apply skipped"
    return 2
  fi

  fetch_activation || return 1
  local profile_key
  profile_key="$(recommended_profile_key)"
  [[ -n "${profile_key}" ]] || {
    echo "activation payload did not include recommended_profile_key"
    cat "${ACTIVATION_FILE}" || true
    return 1
  }

  request_api POST "/api/v1/setup/profiles/${profile_key}/apply" '{"note":"qa v0.1.17 activation apply"}'
  status_is_success "${LAST_STATUS}" || {
    echo "profile apply failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  cp "${LAST_BODY_FILE}" "${PROFILE_APPLY_FILE}"

  jq -e --arg key "${profile_key}" '.profile_key == $key and .status == "applied" and (.actions | type == "array")' "${PROFILE_APPLY_FILE}" >/dev/null || {
    echo "profile apply response missing expected shape"
    cat "${PROFILE_APPLY_FILE}" || true
    return 1
  }

  cat "${PROFILE_APPLY_FILE}"
}

stage_activation_verify() {
  set_stage_context \
    "/api/v1/setup/activation" \
    "Re-read activation and confirm the operator-profile activation item converges after starter-profile apply."

  if (( READ_ONLY_MODE == 1 )); then
    echo "read-only mode: activation verify skipped"
    return 2
  fi

  fetch_activation || return 1

  jq -e '
    (.items[] | select(.item_key == "operator_profile")) as $item
    | $item.status == "ready"
  ' "${ACTIVATION_FILE}" >/dev/null || {
    echo "operator_profile activation item did not converge to ready"
    jq '.items[] | select(.item_key == "operator_profile")' "${ACTIVATION_FILE}" || true
    return 1
  }

  jq '{verified_item: (.items[] | select(.item_key == "operator_profile"))}' "${ACTIVATION_FILE}"
}

stage_feedback_capture_apply() {
  set_stage_context \
    "/api/v1/setup/activation/feedback" \
    "Submit structured activation feedback so pilot friction is captured in product-owned form."

  if (( READ_ONLY_MODE == 1 )); then
    echo "read-only mode: feedback apply skipped"
    return 2
  fi

  fetch_activation || return 1
  local profile_key
  profile_key="$(recommended_profile_key)"
  [[ -n "${profile_key}" ]] || profile_key="smb-small-office"

  request_api POST "/api/v1/setup/activation/feedback" "{\"step_key\":\"operator_profile\",\"template_key\":\"${profile_key}\",\"feedback_kind\":\"confused\",\"comment\":\"qa v0.1.17 structured feedback\",\"context\":{\"source\":\"qa-script\"}}"
  status_is_success "${LAST_STATUS}" || {
    echo "feedback capture failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  cp "${LAST_BODY_FILE}" "${FEEDBACK_FILE}"

  jq -e '.step_key == "operator_profile" and .feedback_kind == "confused" and (.id | type == "number")' "${FEEDBACK_FILE}" >/dev/null || {
    echo "feedback response missing expected shape"
    cat "${FEEDBACK_FILE}" || true
    return 1
  }

  cat "${FEEDBACK_FILE}"
}

stage_read_only_guard() {
  set_stage_context \
    "/api/v1/setup/profiles/<recommended>/apply" \
    "Use a read-only user to confirm activation remains readable while starter-profile apply is rejected."

  local guard_user="${READ_ONLY_AUTH_USER}"
  if (( READ_ONLY_MODE == 1 )); then
    guard_user="${AUTH_USER}"
  fi

  request_api_as_user "${guard_user}" GET "/api/v1/setup/activation"
  status_is_success "${LAST_STATUS}" || {
    echo "read-only activation read failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  cp "${LAST_BODY_FILE}" "${ACTIVATION_FILE}"

  local profile_key
  profile_key="$(recommended_profile_key)"
  [[ -n "${profile_key}" ]] || {
    echo "activation payload did not include recommended_profile_key"
    cat "${ACTIVATION_FILE}" || true
    return 1
  }

  request_api_as_user "${guard_user}" POST "/api/v1/setup/profiles/${profile_key}/apply" '{"note":"qa read only guard"}'
  [[ "${LAST_STATUS}" == "403" ]] || {
    echo "read-only guard expected 403, got status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  cat "${LAST_BODY_FILE}"
}

write_summary() {
  local overall
  overall="$(jq -s 'if any(.[]; .status == "fail") then "fail" else "pass" end' "${TMP_STAGES}")"

  jq -s \
    --arg version "0.1.17" \
    --arg run_id "${RUN_ID}" \
    --arg output_dir "${OUTPUT_DIR}" \
    --arg overall "${overall//\"/}" \
    '{version: $version, run_id: $run_id, output_dir: $output_dir, overall: $overall, generated_at: (now | todateiso8601), stages: .}' "${TMP_STAGES}" >"${SUMMARY_JSON}"

  jq -r '
    ["# v0.1.17 Activation Journey Summary", "", "- overall=" + .overall, "- run_id=" + .run_id, "- output_dir=" + .output_dir, "", "## Stages"]
    + (.stages | map("- " + .key + ": " + .status + " | endpoint=" + .endpoint + " | remediation=" + .remediation))
    + (if .overall == "fail" then ["", "## Failed Stages"] + (.stages | map(select(.status == "fail") | "- " + .key + " | endpoint=" + .endpoint + " | remediation=" + .remediation + " | diagnostics=" + .diagnostics)) else [] end)
    | join("\n")
  ' "${SUMMARY_JSON}" >"${SUMMARY_MD}"

  jq -n \
    --arg summary_json "${SUMMARY_JSON}" \
    --arg summary_md "${SUMMARY_MD}" \
    --arg artifact_index_json "${ARTIFACT_INDEX_JSON}" \
    --argjson stages "$(jq -s '.' "${TMP_STAGES}")" \
    '{version: "0.1.17", summary_json: $summary_json, summary_md: $summary_md, artifact_index_json: $artifact_index_json, stage_logs: ($stages | map({key, log_file}))}' >"${ARTIFACT_INDEX_JSON}"
}

main() {
  require_cmd curl
  require_cmd jq
  require_cmd sed
  require_cmd tail
  require_cmd mktemp

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
      --read-only-user)
        [[ $# -ge 2 ]] || fatal "--read-only-user requires a value"
        READ_ONLY_AUTH_USER="$2"
        shift 2
        ;;
      --output-dir)
        [[ $# -ge 2 ]] || fatal "--output-dir requires a value"
        OUTPUT_DIR="$2"
        OUTPUT_DIR_EXPLICIT=1
        SUMMARY_JSON="${OUTPUT_DIR}/summary.json"
        SUMMARY_MD="${OUTPUT_DIR}/summary.md"
        ARTIFACT_INDEX_JSON="${OUTPUT_DIR}/artifact-index.json"
        ACTIVATION_FILE="${OUTPUT_DIR}/activation.json"
        STARTER_TEMPLATES_FILE="${OUTPUT_DIR}/starter-templates.json"
        PROFILE_APPLY_FILE="${OUTPUT_DIR}/profile-apply.json"
        FEEDBACK_FILE="${OUTPUT_DIR}/feedback.json"
        shift 2
        ;;
      --run-id)
        [[ $# -ge 2 ]] || fatal "--run-id requires a value"
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

  if (( OUTPUT_DIR_EXPLICIT == 0 )); then
    OUTPUT_DIR=".run/qa/v0.1.17-journey-${RUN_ID}"
    SUMMARY_JSON="${OUTPUT_DIR}/summary.json"
    SUMMARY_MD="${OUTPUT_DIR}/summary.md"
    ARTIFACT_INDEX_JSON="${OUTPUT_DIR}/artifact-index.json"
    ACTIVATION_FILE="${OUTPUT_DIR}/activation.json"
    STARTER_TEMPLATES_FILE="${OUTPUT_DIR}/starter-templates.json"
    PROFILE_APPLY_FILE="${OUTPUT_DIR}/profile-apply.json"
    FEEDBACK_FILE="${OUTPUT_DIR}/feedback.json"
  fi

  mkdir -p "${OUTPUT_DIR}"
  TMP_STAGES="$(mktemp)"

  run_stage "activation_status_read" "Activation status read" stage_activation_status_read
  run_stage "starter_template_visibility_read" "Starter template visibility read" stage_starter_template_visibility_read
  run_stage "activation_progress_apply" "Activation progress apply" stage_activation_progress_apply
  run_stage "activation_verify" "Activation verify" stage_activation_verify
  run_stage "feedback_capture_apply" "Feedback capture apply" stage_feedback_capture_apply
  run_stage "read_only_guard" "Read-only guard" stage_read_only_guard

  write_summary
  cat "${SUMMARY_MD}"

  if jq -e '.overall == "pass"' "${SUMMARY_JSON}" >/dev/null; then
    exit 0
  fi
  exit 1
}

main "$@"
