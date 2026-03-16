#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

RUN_ID="${RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)}"
OUTPUT_DIR="${OUTPUT_DIR:-.run/qa/v0.1.18-daily-loop-${RUN_ID}}"
SUMMARY_JSON="${OUTPUT_DIR}/summary.json"
SUMMARY_MD="${OUTPUT_DIR}/summary.md"
ARTIFACT_INDEX_JSON="${OUTPUT_DIR}/artifact-index.json"
API_BASE_URL="${API_BASE_URL:-http://127.0.0.1:8080}"
AUTH_USER="${AUTH_USER:-admin}"
GUARD_USER="${READ_ONLY_AUTH_USER:-viewer}"
READ_ONLY_MODE=0

TMP_STAGES=""
LAST_STATUS=""
LAST_BODY_FILE=""
STAGE_ENDPOINT=""
STAGE_REMEDIATION=""
BRIEFING_FILE="${OUTPUT_DIR}/daily-ops-briefing.json"
ACTION_FILE="${OUTPUT_DIR}/daily-ops-action.json"
SEED_TICKET_FILE="${OUTPUT_DIR}/seed-ticket.json"
SELECTED_TASK_KEY=""
SELECTED_ITEM_TYPE=""
SELECTED_SUMMARY=""
SEED_TICKET_NO=""
SEED_TICKET_ID=""

log() {
  printf '[qa-v0.1.18] %s\n' "$*"
}

fatal() {
  printf '[qa-v0.1.18][ERROR] %s\n' "$*" >&2
  exit 1
}

usage() {
  cat <<'USAGE'
Daily ops return-loop validation (v0.1.18)

Usage:
  bash scripts/qa-v0.1.18-daily-ops-loop.sh [options]

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

fetch_briefing() {
  request_api GET "/api/v1/ops/cockpit/daily-ops/briefing?limit=24"
  status_is_success "${LAST_STATUS}" || {
    echo "daily ops briefing read failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  cp "${LAST_BODY_FILE}" "${BRIEFING_FILE}"
}

ensure_seed_ticket() {
  if (( READ_ONLY_MODE == 1 )); then
    return 0
  fi

  fetch_briefing || return 1
  if jq -e '.summary.total > 0 and (.items | length > 0)' "${BRIEFING_FILE}" >/dev/null; then
    return 0
  fi

  local payload
  payload="$(jq -nc --arg run_id "${RUN_ID}" '{title: ("qa-v0.1.18 daily ops seed " + $run_id), description: "Seed ticket for v0.1.18 daily ops validation", priority: "high", category: "incident"}')"
  request_api POST "/api/v1/tickets" "${payload}"
  status_is_success "${LAST_STATUS}" || {
    echo "seed ticket create failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  cp "${LAST_BODY_FILE}" "${SEED_TICKET_FILE}"
  SEED_TICKET_ID="$(jq -r '.ticket.id // empty' "${SEED_TICKET_FILE}")"
  SEED_TICKET_NO="$(jq -r '.ticket.ticket_no // empty' "${SEED_TICKET_FILE}")"
  [[ -n "${SEED_TICKET_ID}" && -n "${SEED_TICKET_NO}" ]] || {
    echo "seed ticket response missing ticket id/no"
    cat "${SEED_TICKET_FILE}" || true
    return 1
  }

  fetch_briefing || return 1
}

select_follow_up_item() {
  local jq_filter='first(.items[]? | select(.item_type == "ticket")) // first(.items[]?)'
  SELECTED_TASK_KEY="$(jq -r "${jq_filter} | .task_key // empty" "${BRIEFING_FILE}")"
  SELECTED_ITEM_TYPE="$(jq -r "${jq_filter} | .item_type // empty" "${BRIEFING_FILE}")"
  SELECTED_SUMMARY="$(jq -r "${jq_filter} | .summary // empty" "${BRIEFING_FILE}")"
  [[ -n "${SELECTED_TASK_KEY}" ]] || {
    echo "no selectable follow-up item found"
    cat "${BRIEFING_FILE}" || true
    return 1
  }
}

stage_daily_briefing_read() {
  set_stage_context \
    "/api/v1/ops/cockpit/daily-ops/briefing" \
    "Verify the daily ops briefing endpoint returns a prioritized queue with summary counts and at least one visible item."

  ensure_seed_ticket || return 1
  if (( READ_ONLY_MODE == 1 )); then
    fetch_briefing || return 1
  fi

  jq -e '
    (.summary.total >= 1)
    and (.items | length >= 1)
    and (.summary | has("overdue"))
    and (.summary | has("due_today"))
    and (.summary | has("blocked"))
    and (.summary | has("completed"))
    and (.summary | has("deferred"))
  ' "${BRIEFING_FILE}" >/dev/null || {
    echo "daily ops briefing payload missing summary structure"
    cat "${BRIEFING_FILE}" || true
    return 1
  }

  jq '{recommended_next_task_key, summary, items: [.items[] | {task_key, item_type, status, priority}]}' "${BRIEFING_FILE}"
}

stage_follow_up_queue_read() {
  set_stage_context \
    "/api/v1/ops/cockpit/daily-ops/briefing" \
    "Verify the follow-up queue exposes deterministic task fields including status, follow_up_state, priority, and recommended_action metadata."

  fetch_briefing || return 1
  select_follow_up_item || return 1

  jq -e --arg task_key "${SELECTED_TASK_KEY}" '
    (.items[] | select(.task_key == $task_key)) as $item
    | ($item.status | IN("due_today", "overdue", "blocked", "completed", "deferred"))
      and ($item.follow_up_state | IN("new", "acknowledged", "completed", "deferred"))
      and ($item.priority | IN("critical", "high", "medium", "low"))
      and ($item.summary | type == "string")
      and ($item.reason | type == "string")
      and ($item.available_actions | type == "array")
  ' "${BRIEFING_FILE}" >/dev/null || {
    echo "selected follow-up item missing expected fields"
    cat "${BRIEFING_FILE}" || true
    return 1
  }

  jq --arg task_key "${SELECTED_TASK_KEY}" '.items[] | select(.task_key == $task_key)' "${BRIEFING_FILE}"
}

stage_follow_up_action_apply() {
  set_stage_context \
    "/api/v1/ops/cockpit/daily-ops/follow-up-actions (POST)" \
    "Apply a complete action to a selected daily ops task and persist auditable follow-up state."

  if (( READ_ONLY_MODE == 1 )); then
    echo "read-only mode: follow-up action apply skipped"
    return 2
  fi

  [[ -n "${SELECTED_TASK_KEY}" ]] || {
    echo "selected task key is empty"
    return 1
  }

  local payload
  payload="$(jq -nc --arg task_key "${SELECTED_TASK_KEY}" '{task_key: $task_key, action: "complete", note: "qa v0.1.18 completed from validation"}')"
  request_api POST "/api/v1/ops/cockpit/daily-ops/follow-up-actions" "${payload}"
  status_is_success "${LAST_STATUS}" || {
    echo "follow-up action apply failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  cp "${LAST_BODY_FILE}" "${ACTION_FILE}"

  jq -e --arg task_key "${SELECTED_TASK_KEY}" '
    .task_key == $task_key
    and .action == "complete"
    and .status_after == "completed"
    and (.item_after.follow_up_state == "completed")
  ' "${ACTION_FILE}" >/dev/null || {
    echo "follow-up action response missing expected completion state"
    cat "${ACTION_FILE}" || true
    return 1
  }

  cat "${ACTION_FILE}"
}

stage_follow_up_verify() {
  set_stage_context \
    "/api/v1/ops/cockpit/daily-ops/briefing" \
    "Re-read the daily ops briefing and confirm the selected task shows completed follow-up state after apply."

  if (( READ_ONLY_MODE == 1 )); then
    echo "read-only mode: follow-up verify skipped"
    return 2
  fi

  fetch_briefing || return 1

  jq -e --arg task_key "${SELECTED_TASK_KEY}" '
    (.items[] | select(.task_key == $task_key)) as $item
    | $item.status == "completed"
      and $item.follow_up_state == "completed"
  ' "${BRIEFING_FILE}" >/dev/null || {
    echo "selected task did not remain completed after refresh"
    cat "${BRIEFING_FILE}" || true
    return 1
  }

  jq --arg task_key "${SELECTED_TASK_KEY}" '.items[] | select(.task_key == $task_key) | {task_key, status, follow_up_state, completed_at}' "${BRIEFING_FILE}"
}

stage_recommended_next_action_visibility() {
  set_stage_context \
    "/api/v1/ops/cockpit/daily-ops/briefing" \
    "Verify the briefing exposes a recommended next task and at least one task-level recommended action contract."

  fetch_briefing || return 1

  jq -e '
    (.recommended_next_task_key == null or (.recommended_next_task_key | type == "string"))
    and ([.items[]? | select(.recommended_action != null)] | length >= 1)
  ' "${BRIEFING_FILE}" >/dev/null || {
    echo "briefing did not expose recommended next task/action visibility"
    cat "${BRIEFING_FILE}" || true
    return 1
  }

  jq '{recommended_next_task_key, recommended_items: [.items[] | select(.recommended_action != null) | {task_key, action_key: .recommended_action.action_key, label: .recommended_action.label}]}' "${BRIEFING_FILE}"
}

stage_read_only_guard() {
  set_stage_context \
    "/api/v1/ops/cockpit/daily-ops/follow-up-actions (POST)" \
    "Verify a read-only user cannot apply daily ops follow-up actions."

  [[ -n "${SELECTED_TASK_KEY}" ]] || {
    fetch_briefing || return 1
    select_follow_up_item || return 1
  }

  local payload
  payload="$(jq -nc --arg task_key "${SELECTED_TASK_KEY}" '{task_key: $task_key, action: "acknowledge", note: "read only guard probe"}')"
  request_api_as_user "${GUARD_USER}" POST "/api/v1/ops/cockpit/daily-ops/follow-up-actions" "${payload}"
  if [[ "${LAST_STATUS}" != "403" ]]; then
    echo "expected 403 for read-only guard, got status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  fi

  cat "${LAST_BODY_FILE}"
}

finalize_summary() {
  local overall="pass"
  if jq -s -e 'map(select(.status == "fail")) | length > 0' "${TMP_STAGES}" >/dev/null 2>&1; then
    overall="fail"
  fi

  jq -s \
    --arg run_id "${RUN_ID}" \
    --arg overall "${overall}" \
    --arg api_base_url "${API_BASE_URL}" \
    --arg auth_user "${AUTH_USER}" \
    --arg read_only_mode "${READ_ONLY_MODE}" \
    --arg selected_task_key "${SELECTED_TASK_KEY:-}" \
    --arg selected_item_type "${SELECTED_ITEM_TYPE:-}" \
    --arg selected_summary "${SELECTED_SUMMARY:-}" \
    --arg seed_ticket_no "${SEED_TICKET_NO:-}" \
    --arg seed_ticket_id "${SEED_TICKET_ID:-}" \
    '{
      run_id: $run_id,
      overall: $overall,
      api_base_url: $api_base_url,
      auth_user: $auth_user,
      read_only_mode: ($read_only_mode == "1"),
      selected_task_key: (if $selected_task_key == "" then null else $selected_task_key end),
      selected_item_type: (if $selected_item_type == "" then null else $selected_item_type end),
      selected_summary: (if $selected_summary == "" then null else $selected_summary end),
      seed_ticket_id: (if $seed_ticket_id == "" then null else ($seed_ticket_id | tonumber) end),
      seed_ticket_no: (if $seed_ticket_no == "" then null else $seed_ticket_no end),
      stages: .
    }' "${TMP_STAGES}" >"${SUMMARY_JSON}"

  {
    echo "# v0.1.18 Daily Ops Loop Summary"
    echo
    echo "- overall=$(jq -r '.overall' "${SUMMARY_JSON}")"
    echo "- run_id=${RUN_ID}"
    echo "- api_base_url=${API_BASE_URL}"
    echo "- auth_user=${AUTH_USER}"
    echo "- read_only_mode=$([[ "${READ_ONLY_MODE}" == "1" ]] && echo true || echo false)"
    echo "- selected_task_key=${SELECTED_TASK_KEY:-none}"
    echo "- selected_item_type=${SELECTED_ITEM_TYPE:-none}"
    echo "- seed_ticket_no=${SEED_TICKET_NO:-none}"
    echo
    echo "## Stages"
    jq -r '.stages[] | "- \(.key): \(.status) | endpoint=\(.endpoint) | remediation=\(.remediation) | diagnostics=\(.diagnostics)"' "${SUMMARY_JSON}"
  } >"${SUMMARY_MD}"

  jq -n \
    --arg summary_json "${SUMMARY_JSON}" \
    --arg summary_md "${SUMMARY_MD}" \
    --arg briefing_file "${BRIEFING_FILE}" \
    --arg action_file "${ACTION_FILE}" \
    --arg seed_ticket_file "${SEED_TICKET_FILE}" \
    '{
      artifacts: [
        {key: "summary_json", path: $summary_json},
        {key: "summary_md", path: $summary_md},
        {key: "daily_ops_briefing", path: $briefing_file},
        {key: "follow_up_action", path: $action_file},
        {key: "seed_ticket", path: $seed_ticket_file}
      ]
    }' >"${ARTIFACT_INDEX_JSON}"
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
      --read-only-user)
        GUARD_USER="$2"
        shift 2
        ;;
      --output-dir)
        OUTPUT_DIR="$2"
        SUMMARY_JSON="${OUTPUT_DIR}/summary.json"
        SUMMARY_MD="${OUTPUT_DIR}/summary.md"
        ARTIFACT_INDEX_JSON="${OUTPUT_DIR}/artifact-index.json"
        BRIEFING_FILE="${OUTPUT_DIR}/daily-ops-briefing.json"
        ACTION_FILE="${OUTPUT_DIR}/daily-ops-action.json"
        SEED_TICKET_FILE="${OUTPUT_DIR}/seed-ticket.json"
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
  TMP_STAGES="$(mktemp)"
  trap 'rm -f "${TMP_STAGES}"' EXIT

  run_stage "daily_briefing_read" "daily ops briefing read path" stage_daily_briefing_read
  run_stage "follow_up_queue_read" "follow-up queue read path" stage_follow_up_queue_read
  run_stage "follow_up_action_apply" "follow-up action apply path" stage_follow_up_action_apply
  run_stage "follow_up_verify" "follow-up verify path" stage_follow_up_verify
  run_stage "recommended_next_action_visibility" "recommended next action visibility" stage_recommended_next_action_visibility
  run_stage "read_only_guard" "read-only guard path" stage_read_only_guard

  finalize_summary

  if jq -e '.overall == "fail"' "${SUMMARY_JSON}" >/dev/null; then
    cat "${SUMMARY_MD}"
    return 1
  fi

  cat "${SUMMARY_MD}"
}

main "$@"
