#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

RUN_ID="${RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)}"
OUTPUT_DIR="${OUTPUT_DIR:-.run/qa/v0.1.15-journey-${RUN_ID}}"
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

BOOTSTRAP_OWNER_KEY="qa_bootstrap_${RUN_ID//[^a-zA-Z0-9]/}"
BOOTSTRAP_OWNER_REF="qa-bootstrap-owner"
BOOTSTRAP_OWNER_TARGET="bootstrap-${RUN_ID//[^a-zA-Z0-9]/}@example.com"
NOTIFICATION_CHANNEL_NAME="operator-bootstrap-primary"
WORKSPACE_CATALOG_FILE="${OUTPUT_DIR}/integration-catalog.json"

log() {
  printf '[qa-v0.1.15] %s\n' "$*"
}

fatal() {
  printf '[qa-v0.1.15][ERROR] %s\n' "$*" >&2
  exit 1
}

usage() {
  cat <<'USAGE'
Integration bootstrap workspace validation (v0.1.15)

Usage:
  bash scripts/qa-v0.1.15-operator-journey.sh [options]

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

fetch_catalog() {
  request_api GET "/api/v1/ops/cockpit/integrations/bootstrap"
  status_is_success "${LAST_STATUS}" || {
    echo "catalog read failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  cp "${LAST_BODY_FILE}" "${WORKSPACE_CATALOG_FILE}"
}

ensure_owner_directory_seed() {
  request_api GET "/api/v1/ops/cockpit/runbook-templates/analytics/owners"
  status_is_success "${LAST_STATUS}" || {
    echo "failed to read owner directory for bootstrap seed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  local merged_payload
  merged_payload="$(jq -c \
    --arg owner_key "${BOOTSTRAP_OWNER_KEY}" \
    --arg owner_ref "${BOOTSTRAP_OWNER_REF}" \
    --arg target "${BOOTSTRAP_OWNER_TARGET}" \
    '
      .items as $items
      | if any($items[]?; .owner_key == $owner_key) then
          {items: $items}
        else
          {items: ($items + [{
            owner_key: $owner_key,
            display_name: "Bootstrap Owner",
            owner_type: "team",
            owner_ref: $owner_ref,
            notification_target: $target,
            note: "qa v0.1.15 seeded owner",
            is_enabled: true
          }])}
        end
    ' "${LAST_BODY_FILE}")"

  request_api PUT "/api/v1/ops/cockpit/runbook-templates/analytics/owners" "${merged_payload}"
  status_is_success "${LAST_STATUS}" || {
    echo "failed to seed owner directory, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
}

build_apply_payload() {
  local integration_key="$1"
  local payload
  payload="$(jq -c \
    --arg integration_key "${integration_key}" \
    --arg channel_name "${NOTIFICATION_CHANNEL_NAME}" \
    --arg target "${BOOTSTRAP_OWNER_TARGET}" \
    --arg escalation_owner "${BOOTSTRAP_OWNER_REF}" \
    '
      (.items[] | select(.integration_key == $integration_key)) as $item
      | {
          integration_key: $integration_key,
          channel_name: (if $integration_key == "operator_notifications" then ($item.default_payload.channel_name // $channel_name) else null end),
          channel_type: (if $integration_key == "operator_notifications" then ($item.default_payload.channel_type // "email") else null end),
          target: (if $integration_key == "operator_notifications" then ($item.default_payload.target // $target) else null end),
          escalation_owner: (if $integration_key == "ticket_followup_policy" then ($item.default_payload.escalation_owner // $escalation_owner) else null end)
        }
    ' "${WORKSPACE_CATALOG_FILE}")"
  [[ -n "${payload}" && "${payload}" != "null" ]] || return 1
  printf '%s' "${payload}"
}

apply_bootstrap_item() {
  local integration_key="$1"
  local payload="$2"
  request_api POST "/api/v1/ops/cockpit/integrations/bootstrap/apply" "${payload}"
  status_is_success "${LAST_STATUS}" || {
    echo "apply failed for ${integration_key}, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  jq -e --arg key "${integration_key}" '.integration_key == $key and (.results | type == "array") and (.item_after.integration_key == $key)' "${LAST_BODY_FILE}" >/dev/null || {
    echo "unexpected apply response for ${integration_key}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  cat "${LAST_BODY_FILE}"
}

stage_integration_catalog_read() {
  set_stage_context \
    "/api/v1/ops/cockpit/integrations/bootstrap" \
    "Verify the integration bootstrap catalog is reachable and exposes both bootstrap items."

  fetch_catalog || return 1

  jq -e '
    .total >= 2
    and ([.items[]?.integration_key] | index("operator_notifications") != null)
    and ([.items[]?.integration_key] | index("ticket_followup_policy") != null)
  ' "${WORKSPACE_CATALOG_FILE}" >/dev/null || {
    echo "catalog is missing expected integration keys"
    cat "${WORKSPACE_CATALOG_FILE}" || true
    return 1
  }

  jq '{generated_at, total, recommended_next_key, keys: [.items[].integration_key]}' "${WORKSPACE_CATALOG_FILE}"
}

stage_bootstrap_apply() {
  set_stage_context \
    "/api/v1/ops/cockpit/integrations/bootstrap/apply" \
    "Seed one enabled owner directory entry, then apply both bootstrap items with catalog defaults."

  if (( READ_ONLY_MODE == 1 )); then
    echo "read-only mode: bootstrap apply skipped"
    return 2
  fi

  ensure_owner_directory_seed || return 1
  fetch_catalog || return 1

  local notifications_payload
  notifications_payload="$(build_apply_payload "operator_notifications")" || {
    echo "failed to build operator_notifications payload"
    return 1
  }
  local policy_payload
  policy_payload="$(build_apply_payload "ticket_followup_policy")" || {
    echo "failed to build ticket_followup_policy payload"
    return 1
  }

  echo "operator_notifications payload=${notifications_payload}"
  apply_bootstrap_item "operator_notifications" "${notifications_payload}" || return 1
  echo
  echo "ticket_followup_policy payload=${policy_payload}"
  apply_bootstrap_item "ticket_followup_policy" "${policy_payload}" || return 1
}

stage_bootstrap_verify() {
  set_stage_context \
    "/api/v1/ops/cockpit/integrations/bootstrap" \
    "Confirm both bootstrap items converge to ready after apply."

  if (( READ_ONLY_MODE == 1 )); then
    echo "read-only mode: bootstrap verify skipped"
    return 2
  fi

  fetch_catalog || return 1

  jq -e '
    ([.items[] | select(.status == "ready") | .integration_key] | index("operator_notifications") != null)
    and ([.items[] | select(.status == "ready") | .integration_key] | index("ticket_followup_policy") != null)
  ' "${WORKSPACE_CATALOG_FILE}" >/dev/null || {
    echo "not all integration bootstrap items are ready"
    cat "${WORKSPACE_CATALOG_FILE}" || true
    return 1
  }

  jq '{ready_items: [.items[] | {integration_key, status, gap_reason}]}' "${WORKSPACE_CATALOG_FILE}"
}

stage_workspace_visibility_read() {
  set_stage_context \
    "/api/v1/ops/cockpit/integrations/bootstrap" \
    "Validate the workspace can render the catalog by checking required fields used by the admin panel."

  fetch_catalog || return 1

  jq -e '
    [.items[] | {
      integration_key,
      name,
      summary,
      status,
      gap_reason,
      recommended_apply_order,
      auto_applicable,
      required_inputs,
      default_payload,
      evidence
    }]
    | all(
        .integration_key != null
        and (.name | type == "string")
        and (.summary | type == "string")
        and (.status | IN("ready", "action_required"))
        and (.gap_reason | type == "string")
        and (.recommended_apply_order | type == "number")
        and (.auto_applicable | type == "boolean")
        and (.required_inputs | type == "array")
        and (.default_payload | type == "object")
        and (.evidence | type == "object")
      )
  ' "${WORKSPACE_CATALOG_FILE}" >/dev/null || {
    echo "workspace catalog is missing required render fields"
    cat "${WORKSPACE_CATALOG_FILE}" || true
    return 1
  }

  jq '{workspace_items: [.items[] | {integration_key, status, required_inputs, default_payload}]}' "${WORKSPACE_CATALOG_FILE}"
}

stage_bootstrap_idempotency_verify() {
  set_stage_context \
    "/api/v1/ops/cockpit/integrations/bootstrap/apply" \
    "Re-apply both items and ensure the second pass only returns noop or reused-style operations."

  if (( READ_ONLY_MODE == 1 )); then
    echo "read-only mode: bootstrap idempotency skipped"
    return 2
  fi

  fetch_catalog || return 1

  local notifications_payload
  notifications_payload="$(build_apply_payload "operator_notifications")" || return 1
  request_api POST "/api/v1/ops/cockpit/integrations/bootstrap/apply" "${notifications_payload}"
  status_is_success "${LAST_STATUS}" || {
    echo "idempotency apply failed for operator_notifications, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  jq -e 'all(.results[]?; (.operation == "reused" or .operation == "noop"))' "${LAST_BODY_FILE}" >/dev/null || {
    echo "operator_notifications returned a mutating operation on idempotency pass"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  local policy_payload
  policy_payload="$(build_apply_payload "ticket_followup_policy")" || return 1
  request_api POST "/api/v1/ops/cockpit/integrations/bootstrap/apply" "${policy_payload}"
  status_is_success "${LAST_STATUS}" || {
    echo "idempotency apply failed for ticket_followup_policy, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  jq -e 'all(.results[]?; .operation == "noop")' "${LAST_BODY_FILE}" >/dev/null || {
    echo "ticket_followup_policy returned a mutating operation on idempotency pass"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  echo "idempotency verified for operator_notifications and ticket_followup_policy"
}

stage_read_only_guard() {
  set_stage_context \
    "/api/v1/ops/cockpit/integrations/bootstrap/apply" \
    "Use a read-only user to confirm apply is rejected while catalog read remains available."

  fetch_catalog || return 1
  local guard_payload
  guard_payload="$(build_apply_payload "ticket_followup_policy")" || return 1

  request_api_as_user "${READ_ONLY_AUTH_USER}" GET "/api/v1/ops/cockpit/integrations/bootstrap"
  status_is_success "${LAST_STATUS}" || {
    echo "read-only catalog read failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  request_api_as_user "${READ_ONLY_AUTH_USER}" POST "/api/v1/ops/cockpit/integrations/bootstrap/apply" "${guard_payload}"
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
    --arg version "0.1.15" \
    --arg run_id "${RUN_ID}" \
    --arg output_dir "${OUTPUT_DIR}" \
    --arg overall "${overall//\"/}" \
    '{
      version: $version,
      run_id: $run_id,
      output_dir: $output_dir,
      overall: $overall,
      generated_at: (now | todateiso8601),
      stages: .
    }' "${TMP_STAGES}" >"${SUMMARY_JSON}"

  jq -r '
    ["# v0.1.15 Operator Journey Summary", "", "- overall=" + .overall, "- run_id=" + .run_id, "- output_dir=" + .output_dir, "", "## Stages"]
    + (.stages | map("- " + .key + ": " + .status + " | endpoint=" + .endpoint + " | remediation=" + .remediation))
    + (if .overall == "fail" then ["", "## Failed Stages"] + (.stages | map(select(.status == "fail") | "- " + .key + " | endpoint=" + .endpoint + " | remediation=" + .remediation + " | diagnostics=" + .diagnostics)) else [] end)
    | join("\n")
  ' "${SUMMARY_JSON}" >"${SUMMARY_MD}"

  jq -n \
    --arg summary_json "${SUMMARY_JSON}" \
    --arg summary_md "${SUMMARY_MD}" \
    --arg artifact_index_json "${ARTIFACT_INDEX_JSON}" \
    --argjson stages "$(jq -s '.' "${TMP_STAGES}")" \
    '{
      version: "0.1.15",
      summary_json: $summary_json,
      summary_md: $summary_md,
      artifact_index_json: $artifact_index_json,
      stage_logs: ($stages | map({key, log_file}))
    }' >"${ARTIFACT_INDEX_JSON}"
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
        WORKSPACE_CATALOG_FILE="${OUTPUT_DIR}/integration-catalog.json"
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
    OUTPUT_DIR=".run/qa/v0.1.15-journey-${RUN_ID}"
    SUMMARY_JSON="${OUTPUT_DIR}/summary.json"
    SUMMARY_MD="${OUTPUT_DIR}/summary.md"
    ARTIFACT_INDEX_JSON="${OUTPUT_DIR}/artifact-index.json"
    WORKSPACE_CATALOG_FILE="${OUTPUT_DIR}/integration-catalog.json"
  fi

  mkdir -p "${OUTPUT_DIR}"
  TMP_STAGES="$(mktemp)"

  run_stage "integration_catalog_read" "Integration catalog read" stage_integration_catalog_read
  run_stage "bootstrap_apply" "Bootstrap apply" stage_bootstrap_apply
  run_stage "bootstrap_verify" "Bootstrap verify" stage_bootstrap_verify
  run_stage "workspace_visibility_read" "Workspace visibility read" stage_workspace_visibility_read
  run_stage "bootstrap_idempotency_verify" "Bootstrap idempotency verify" stage_bootstrap_idempotency_verify
  run_stage "read_only_guard" "Read-only guard" stage_read_only_guard

  write_summary
  cat "${SUMMARY_MD}"

  if jq -e '.overall == "pass"' "${SUMMARY_JSON}" >/dev/null; then
    exit 0
  fi
  exit 1
}

main "$@"
