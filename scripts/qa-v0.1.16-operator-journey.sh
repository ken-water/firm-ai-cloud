#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

RUN_ID="${RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)}"
OUTPUT_DIR="${OUTPUT_DIR:-.run/qa/v0.1.16-journey-${RUN_ID}}"
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
READINESS_FILE="${OUTPUT_DIR}/go-live-readiness.json"
SELECTED_ACTION_FILE="${OUTPUT_DIR}/selected-action.json"
INTEGRATION_CATALOG_FILE="${OUTPUT_DIR}/integration-catalog.json"
SEEDED_OWNER_KEY="qa_go_live_${RUN_ID//[^a-zA-Z0-9]/}"
SEEDED_OWNER_REF="qa-go-live-owner"
SEEDED_OWNER_TARGET="go-live-${RUN_ID//[^a-zA-Z0-9]/}@example.com"

log() {
  printf '[qa-v0.1.16] %s\n' "$*"
}

fatal() {
  printf '[qa-v0.1.16][ERROR] %s\n' "$*" >&2
  exit 1
}

usage() {
  cat <<'USAGE'
Go-live readiness workspace validation (v0.1.16)

Usage:
  bash scripts/qa-v0.1.16-operator-journey.sh [options]

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

fetch_go_live_readiness() {
  request_api GET "/api/v1/ops/cockpit/go-live/readiness"
  status_is_success "${LAST_STATUS}" || {
    echo "go-live readiness read failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  cp "${LAST_BODY_FILE}" "${READINESS_FILE}"
}

fetch_integration_catalog() {
  request_api GET "/api/v1/ops/cockpit/integrations/bootstrap"
  status_is_success "${LAST_STATUS}" || {
    echo "integration catalog read failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  cp "${LAST_BODY_FILE}" "${INTEGRATION_CATALOG_FILE}"
}

ensure_owner_directory_seed() {
  request_api GET "/api/v1/ops/cockpit/runbook-templates/analytics/owners"
  status_is_success "${LAST_STATUS}" || {
    echo "failed to read owner directory, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  local merged_payload
  merged_payload="$(jq -c \
    --arg owner_key "${SEEDED_OWNER_KEY}" \
    --arg owner_ref "${SEEDED_OWNER_REF}" \
    --arg target "${SEEDED_OWNER_TARGET}" \
    '
      .items as $items
      | if any($items[]?; .owner_key == $owner_key) then
          {items: $items}
        else
          {items: ($items + [{
            owner_key: $owner_key,
            display_name: "QA Go-live Owner",
            owner_type: "team",
            owner_ref: $owner_ref,
            notification_target: $target,
            note: "qa v0.1.16 seeded owner",
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

select_guided_action() {
  jq -c '
    .domains
    | map(select(
        .recommended_action != null
        and .recommended_action.action_type == "api"
        and .recommended_action.auto_applicable == true
        and .recommended_action.api_path != null
        and .recommended_action.method != null
      ))
    | first
    | if . == null then empty else {
        domain_key,
        status,
        action: .recommended_action
      } end
  ' "${READINESS_FILE}" >"${SELECTED_ACTION_FILE}"

  [[ -s "${SELECTED_ACTION_FILE}" ]]
}

apply_selected_guided_action() {
  local api_path
  api_path="$(jq -r '.action.api_path' "${SELECTED_ACTION_FILE}")"
  local method
  method="$(jq -r '.action.method' "${SELECTED_ACTION_FILE}")"
  local body
  body="$(jq -c '.action.body // {}' "${SELECTED_ACTION_FILE}")"

  request_api "${method}" "${api_path}" "${body}"
  status_is_success "${LAST_STATUS}" || {
    echo "guided action apply failed, status=${LAST_STATUS}, method=${method}, path=${api_path}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  jq -e '(.results | type == "array") and (.results | length > 0)' "${LAST_BODY_FILE}" >/dev/null || {
    echo "guided action response missing results"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  cat "${LAST_BODY_FILE}"
}

stage_go_live_readiness_read() {
  set_stage_context \
    "/api/v1/ops/cockpit/go-live/readiness" \
    "Verify the readiness API returns all six go-live domains and summary metadata."

  fetch_go_live_readiness || return 1

  jq -e '
    .summary.total == 6
    and (.domains | length == 6)
    and ([.domains[]?.domain_key] | index("authentication") != null)
    and ([.domains[]?.domain_key] | index("monitoring_sources") != null)
    and ([.domains[]?.domain_key] | index("operator_notifications") != null)
    and ([.domains[]?.domain_key] | index("ticket_followup") != null)
    and ([.domains[]?.domain_key] | index("backup_restore") != null)
    and ([.domains[]?.domain_key] | index("runbook_execution") != null)
  ' "${READINESS_FILE}" >/dev/null || {
    echo "readiness payload is missing required domains"
    cat "${READINESS_FILE}" || true
    return 1
  }

  jq '{generated_at, overall_status, recommended_next_domain, summary, domains: [.domains[] | {domain_key, status}]}' "${READINESS_FILE}"
}

stage_remediation_visibility_read() {
  set_stage_context \
    "/api/v1/ops/cockpit/go-live/readiness" \
    "Validate that each domain can expose deterministic remediation metadata and at least one action is visible."

  fetch_go_live_readiness || return 1

  jq -e '
    ([.domains[] | select(.recommended_action != null)] | length) >= 1
    and ([.domains[] | select(.recommended_action != null) | .recommended_action] | all(
      (.action_key | type == "string")
      and (.label | type == "string")
      and (.description | type == "string")
      and (.action_type | IN("api", "link"))
      and (.requires_write | type == "boolean")
      and (.auto_applicable | type == "boolean")
    ))
  ' "${READINESS_FILE}" >/dev/null || {
    echo "remediation metadata is missing required fields"
    cat "${READINESS_FILE}" || true
    return 1
  }

  jq '{actions: [.domains[] | select(.recommended_action != null) | {domain_key, status, action: .recommended_action}]}' "${READINESS_FILE}"
}

stage_guided_action_apply() {
  set_stage_context \
    "/api/v1/ops/cockpit/integrations/bootstrap/apply" \
    "Pick the first auto-applicable go-live remediation action and execute it through the advertised API contract."

  if (( READ_ONLY_MODE == 1 )); then
    echo "read-only mode: guided action apply skipped"
    return 2
  fi

  fetch_go_live_readiness || return 1
  if ! select_guided_action; then
    ensure_owner_directory_seed || return 1
    fetch_go_live_readiness || return 1
    select_guided_action || {
      echo "no auto-applicable guided action available after owner seeding"
      cat "${READINESS_FILE}" || true
      return 1
    }
  fi

  jq '{selected_domain: .domain_key, action_key: .action.action_key, method: .action.method, api_path: .action.api_path, body: .action.body}' "${SELECTED_ACTION_FILE}"
  apply_selected_guided_action || return 1
}

stage_go_live_verify() {
  set_stage_context \
    "/api/v1/ops/cockpit/go-live/readiness" \
    "Re-read readiness after apply and confirm the targeted guided-action domain converges to ready state."

  if (( READ_ONLY_MODE == 1 )); then
    echo "read-only mode: go-live verify skipped"
    return 2
  fi

  [[ -s "${SELECTED_ACTION_FILE}" ]] || {
    echo "selected action artifact is missing"
    return 1
  }

  fetch_go_live_readiness || return 1

  local domain_key
  domain_key="$(jq -r '.domain_key' "${SELECTED_ACTION_FILE}")"

  jq -e --arg domain_key "${domain_key}" '
    (.domains[] | select(.domain_key == $domain_key)) as $item
    | $item.status == "ready"
    and ($item.recommended_action != null)
  ' "${READINESS_FILE}" >/dev/null || {
    echo "targeted domain did not converge to ready"
    jq --arg domain_key "${domain_key}" '.domains[] | select(.domain_key == $domain_key)' "${READINESS_FILE}" || true
    return 1
  }

  jq --arg domain_key "${domain_key}" '{verified_domain: (.domains[] | select(.domain_key == $domain_key))}' "${READINESS_FILE}"
}

stage_workspace_visibility_read() {
  set_stage_context \
    "/api/v1/ops/cockpit/go-live/readiness" \
    "Validate the readiness payload contains all fields required by the cockpit workspace renderer."

  fetch_go_live_readiness || return 1

  jq -e '
    [.domains[] | {
      domain_key,
      name,
      status,
      summary,
      reason,
      recommended_action,
      evidence
    }]
    | all(
        (.domain_key | type == "string")
        and (.name | type == "string")
        and (.status | IN("ready", "warning", "blocking"))
        and (.summary | type == "string")
        and (.reason | type == "string")
        and (.evidence | type == "object")
        and (
          .recommended_action == null
          or (
            (.recommended_action.action_key | type == "string")
            and (.recommended_action.label | type == "string")
            and (.recommended_action.description | type == "string")
            and (.recommended_action.action_type | IN("api", "link"))
            and (.recommended_action.requires_write | type == "boolean")
            and (.recommended_action.auto_applicable | type == "boolean")
          )
        )
      )
  ' "${READINESS_FILE}" >/dev/null || {
    echo "workspace payload is missing required render fields"
    cat "${READINESS_FILE}" || true
    return 1
  }

  jq '{workspace: {overall_status, recommended_next_domain, summary, domains: [.domains[] | {domain_key, status, action_type: (.recommended_action.action_type // null)}]}}' "${READINESS_FILE}"
}

stage_read_only_guard() {
  set_stage_context \
    "/api/v1/ops/cockpit/integrations/bootstrap/apply" \
    "Use a read-only user to confirm readiness remains readable while guided apply is rejected with 403."

  if (( READ_ONLY_MODE == 1 )); then
    request_api_as_user "${AUTH_USER}" GET "/api/v1/ops/cockpit/go-live/readiness"
  else
    request_api_as_user "${READ_ONLY_AUTH_USER}" GET "/api/v1/ops/cockpit/go-live/readiness"
  fi
  status_is_success "${LAST_STATUS}" || {
    echo "read-only readiness read failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  cp "${LAST_BODY_FILE}" "${READINESS_FILE}"
  if ! select_guided_action; then
    echo "no guided action available for read-only guard"
    cat "${READINESS_FILE}" || true
    return 1
  fi

  local guard_user
  if (( READ_ONLY_MODE == 1 )); then
    guard_user="${AUTH_USER}"
  else
    guard_user="${READ_ONLY_AUTH_USER}"
  fi

  local api_path
  api_path="$(jq -r '.action.api_path' "${SELECTED_ACTION_FILE}")"
  local method
  method="$(jq -r '.action.method' "${SELECTED_ACTION_FILE}")"
  local body
  body="$(jq -c '.action.body // {}' "${SELECTED_ACTION_FILE}")"

  request_api_as_user "${guard_user}" "${method}" "${api_path}" "${body}"
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
    --arg version "0.1.16" \
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
    ["# v0.1.16 Operator Journey Summary", "", "- overall=" + .overall, "- run_id=" + .run_id, "- output_dir=" + .output_dir, "", "## Stages"]
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
      version: "0.1.16",
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
        READINESS_FILE="${OUTPUT_DIR}/go-live-readiness.json"
        SELECTED_ACTION_FILE="${OUTPUT_DIR}/selected-action.json"
        INTEGRATION_CATALOG_FILE="${OUTPUT_DIR}/integration-catalog.json"
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
    OUTPUT_DIR=".run/qa/v0.1.16-journey-${RUN_ID}"
    SUMMARY_JSON="${OUTPUT_DIR}/summary.json"
    SUMMARY_MD="${OUTPUT_DIR}/summary.md"
    ARTIFACT_INDEX_JSON="${OUTPUT_DIR}/artifact-index.json"
    READINESS_FILE="${OUTPUT_DIR}/go-live-readiness.json"
    SELECTED_ACTION_FILE="${OUTPUT_DIR}/selected-action.json"
    INTEGRATION_CATALOG_FILE="${OUTPUT_DIR}/integration-catalog.json"
  fi

  mkdir -p "${OUTPUT_DIR}"
  TMP_STAGES="$(mktemp)"

  run_stage "go_live_readiness_read" "Go-live readiness read" stage_go_live_readiness_read
  run_stage "remediation_visibility_read" "Remediation visibility read" stage_remediation_visibility_read
  run_stage "guided_action_apply" "Guided action apply" stage_guided_action_apply
  run_stage "go_live_verify" "Go-live verify" stage_go_live_verify
  run_stage "workspace_visibility_read" "Workspace visibility read" stage_workspace_visibility_read
  run_stage "read_only_guard" "Read-only guard" stage_read_only_guard

  write_summary
  cat "${SUMMARY_MD}"

  if jq -e '.overall == "pass"' "${SUMMARY_JSON}" >/dev/null; then
    exit 0
  fi
  exit 1
}

main "$@"
