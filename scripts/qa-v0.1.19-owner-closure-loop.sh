#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

RUN_ID="${RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)}"
OUTPUT_DIR="${OUTPUT_DIR:-.run/qa/v0.1.19-owner-closure-${RUN_ID}}"
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
CLOSURE_FILE="${OUTPUT_DIR}/daily-ops-closure-continuity.json"
ACTION_FILE="${OUTPUT_DIR}/daily-ops-owner-action.json"
SEED_TICKET_FILE="${OUTPUT_DIR}/seed-ticket.json"
SELECTED_TASK_KEY=""
SELECTED_ITEM_TYPE=""
SELECTED_OWNER_REF=""
SEED_TICKET_NO=""
SEED_TICKET_ID=""

log() {
  printf '[qa-v0.1.19] %s\n' "$*"
}

fatal() {
  printf '[qa-v0.1.19][ERROR] %s\n' "$*" >&2
  exit 1
}

usage() {
  cat <<'USAGE'
Daily ops owner-closure loop validation (v0.1.19)

Usage:
  bash scripts/qa-v0.1.19-owner-closure-loop.sh [options]

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

fetch_closure() {
  request_api GET "/api/v1/ops/cockpit/daily-ops/closure-continuity?limit=24"
  status_is_success "${LAST_STATUS}" || {
    echo "daily ops closure continuity read failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  cp "${LAST_BODY_FILE}" "${CLOSURE_FILE}"
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
  payload="$(jq -nc --arg run_id "${RUN_ID}" '{title: ("qa-v0.1.19 owner closure seed " + $run_id), description: "Seed ticket for v0.1.19 owner-closure validation", priority: "high", category: "incident"}')"
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
  local jq_filter='first(.items[]? | select((.owner.owner_ref // "") != "" and (.status != "completed") and (.status != "deferred"))) // first(.items[]?)'
  SELECTED_TASK_KEY="$(jq -r "${jq_filter} | .task_key // empty" "${BRIEFING_FILE}")"
  SELECTED_ITEM_TYPE="$(jq -r "${jq_filter} | .item_type // empty" "${BRIEFING_FILE}")"
  SELECTED_OWNER_REF="$(jq -r "${jq_filter} | .owner.owner_ref // empty" "${BRIEFING_FILE}")"
  [[ -n "${SELECTED_TASK_KEY}" ]] || {
    echo "no selectable follow-up item found"
    cat "${BRIEFING_FILE}" || true
    return 1
  }
}

stage_daily_briefing_read() {
  set_stage_context \
    "/api/v1/ops/cockpit/daily-ops/briefing" \
    "Verify daily ops briefing exposes owner + due policy contract and at least one visible item."

  ensure_seed_ticket || return 1
  fetch_briefing || return 1
  select_follow_up_item || return 1

  jq -e --arg task_key "${SELECTED_TASK_KEY}" '
    (.summary.total >= 1)
    and (.items | length >= 1)
    and ((.items[] | select(.task_key == $task_key)) as $item
      | ($item.owner | type == "object")
      and ($item.owner | has("owner_ref"))
      and ($item.owner | has("owner_state"))
      and ($item.due_policy | type == "object")
      and ($item.due_policy | has("policy_key"))
      and ($item | has("escalate_at")))
  ' "${BRIEFING_FILE}" >/dev/null || {
    echo "daily briefing payload missing owner/due policy fields"
    cat "${BRIEFING_FILE}" || true
    return 1
  }

  jq --arg task_key "${SELECTED_TASK_KEY}" '.items[] | select(.task_key == $task_key) | {task_key, status, priority, owner, due_policy, due_at, escalate_at}' "${BRIEFING_FILE}"
}

stage_ownership_apply() {
  set_stage_context \
    "/api/v1/ops/cockpit/daily-ops/follow-up-actions (POST acknowledge)" \
    "Apply owner-oriented follow-up action with owner confirmation note for selected task."

  if (( READ_ONLY_MODE == 1 )); then
    echo "read-only mode: ownership apply skipped"
    return 2
  fi

  [[ -n "${SELECTED_TASK_KEY}" ]] || {
    echo "selected task key is empty"
    return 1
  }

  local owner_note
  owner_note="owner_confirmed=${SELECTED_OWNER_REF:-unknown}"
  local payload
  payload="$(jq -nc --arg task_key "${SELECTED_TASK_KEY}" --arg note "${owner_note}" '{task_key: $task_key, action: "acknowledge", note: $note}')"
  request_api POST "/api/v1/ops/cockpit/daily-ops/follow-up-actions" "${payload}"
  status_is_success "${LAST_STATUS}" || {
    echo "ownership apply failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  cp "${LAST_BODY_FILE}" "${ACTION_FILE}"

  jq -e --arg task_key "${SELECTED_TASK_KEY}" '
    .task_key == $task_key
    and .action == "acknowledge"
    and (.item_after.follow_up_state == "acknowledged")
  ' "${ACTION_FILE}" >/dev/null || {
    echo "ownership apply response missing expected acknowledged state"
    cat "${ACTION_FILE}" || true
    return 1
  }

  cat "${ACTION_FILE}"
}

stage_ownership_verify() {
  set_stage_context \
    "/api/v1/ops/cockpit/daily-ops/briefing" \
    "Re-read briefing and confirm selected task keeps owner metadata and acknowledged/closed follow-up state."

  if (( READ_ONLY_MODE == 1 )); then
    echo "read-only mode: ownership verify skipped"
    return 2
  fi

  fetch_briefing || return 1

  jq -e --arg task_key "${SELECTED_TASK_KEY}" '
    (.items[] | select(.task_key == $task_key)) as $item
    | (($item.follow_up_state == "acknowledged") or ($item.follow_up_state == "completed"))
      and ($item.owner | has("owner_state"))
      and ($item.due_policy | has("policy_key"))
  ' "${BRIEFING_FILE}" >/dev/null || {
    echo "ownership verify failed for selected task"
    cat "${BRIEFING_FILE}" || true
    return 1
  }

  jq --arg task_key "${SELECTED_TASK_KEY}" '.items[] | select(.task_key == $task_key) | {task_key, follow_up_state, owner, due_policy}' "${BRIEFING_FILE}"
}

stage_handoff_digest_verify() {
  set_stage_context \
    "/api/v1/ops/cockpit/daily-ops/closure-continuity" \
    "Verify closure continuity exposes carryover digest linked to daily follow-up task keys and owner state."

  fetch_closure || return 1

  jq -e '
    (.summary | has("carryover_total"))
    and (.summary | has("owner_gap_total"))
    and (.summary | has("overdue_total"))
    and (.summary | has("blocked_total"))
    and (.summary | has("escalation_candidate_total"))
    and (.carryover_items | type == "array")
    and ([.carryover_items[]? | has("task_key") and has("owner") and (.owner | has("owner_state"))] | all)
  ' "${CLOSURE_FILE}" >/dev/null || {
    echo "closure continuity payload missing carryover summary/owner linkage"
    cat "${CLOSURE_FILE}" || true
    return 1
  }

  jq '{summary, carryover_items: [.carryover_items[] | {task_key, status, owner_state: .owner.owner_state}]}' "${CLOSURE_FILE}"
}

stage_escalation_trigger_verify() {
  set_stage_context \
    "/api/v1/ops/cockpit/daily-ops/closure-continuity" \
    "Verify escalation candidate semantics are queryable and include deterministic trigger metadata."

  fetch_closure || return 1

  jq -e '
    (.escalation_candidates | type == "array")
    and ([.escalation_candidates[]? |
      has("task_key")
      and has("trigger_state")
      and has("trigger_reason")
      and has("owner_ref")
      and has("due_policy")
      and (.due_policy | has("policy_key"))
    ] | all)
  ' "${CLOSURE_FILE}" >/dev/null || {
    echo "closure continuity payload missing escalation trigger structure"
    cat "${CLOSURE_FILE}" || true
    return 1
  }

  jq '{escalation_candidate_total: .summary.escalation_candidate_total, escalation_candidates: [.escalation_candidates[] | {task_key, trigger_state, owner_ref}]}' "${CLOSURE_FILE}"
}

stage_read_only_guard() {
  set_stage_context \
    "/api/v1/ops/cockpit/daily-ops/follow-up-actions (POST)" \
    "Verify read-only user cannot apply owner-oriented follow-up actions."

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
    --arg selected_owner_ref "${SELECTED_OWNER_REF:-}" \
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
      selected_owner_ref: (if $selected_owner_ref == "" then null else $selected_owner_ref end),
      seed_ticket_id: (if $seed_ticket_id == "" then null else ($seed_ticket_id | tonumber) end),
      seed_ticket_no: (if $seed_ticket_no == "" then null else $seed_ticket_no end),
      stages: .
    }' "${TMP_STAGES}" >"${SUMMARY_JSON}"

  {
    echo "# v0.1.19 Owner Closure Loop Summary"
    echo
    echo "- overall=$(jq -r '.overall' "${SUMMARY_JSON}")"
    echo "- run_id=${RUN_ID}"
    echo "- api_base_url=${API_BASE_URL}"
    echo "- auth_user=${AUTH_USER}"
    echo "- read_only_mode=$([[ "${READ_ONLY_MODE}" == "1" ]] && echo true || echo false)"
    echo "- selected_task_key=${SELECTED_TASK_KEY:-none}"
    echo "- selected_item_type=${SELECTED_ITEM_TYPE:-none}"
    echo "- selected_owner_ref=${SELECTED_OWNER_REF:-none}"
    echo "- seed_ticket_no=${SEED_TICKET_NO:-none}"
    echo
    echo "## Stages"
    jq -r '.stages[] | "- \(.key): \(.status) | endpoint=\(.endpoint) | remediation=\(.remediation) | diagnostics=\(.diagnostics)"' "${SUMMARY_JSON}"
  } >"${SUMMARY_MD}"

  jq -n \
    --arg summary_json "${SUMMARY_JSON}" \
    --arg summary_md "${SUMMARY_MD}" \
    --arg briefing_file "${BRIEFING_FILE}" \
    --arg closure_file "${CLOSURE_FILE}" \
    --arg action_file "${ACTION_FILE}" \
    --arg seed_ticket_file "${SEED_TICKET_FILE}" \
    --argjson stages "$(jq -s '.' "${TMP_STAGES}")" \
    '{
      summary_json: $summary_json,
      summary_md: $summary_md,
      artifacts: [
        {key: "summary_json", path: $summary_json},
        {key: "summary_md", path: $summary_md},
        {key: "daily_ops_briefing", path: $briefing_file},
        {key: "daily_ops_closure_continuity", path: $closure_file},
        {key: "ownership_apply_action", path: $action_file},
        {key: "seed_ticket", path: $seed_ticket_file}
      ],
      stage_logs: ($stages | map({key, path: .log_file}))
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
        CLOSURE_FILE="${OUTPUT_DIR}/daily-ops-closure-continuity.json"
        ACTION_FILE="${OUTPUT_DIR}/daily-ops-owner-action.json"
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
  run_stage "ownership_apply" "ownership apply path" stage_ownership_apply
  run_stage "ownership_verify" "ownership verify path" stage_ownership_verify
  run_stage "handoff_digest_verify" "handoff carryover digest verify path" stage_handoff_digest_verify
  run_stage "escalation_trigger_verify" "escalation trigger semantics verify path" stage_escalation_trigger_verify
  run_stage "read_only_guard" "read-only guard path" stage_read_only_guard

  finalize_summary

  if jq -e '.overall == "fail"' "${SUMMARY_JSON}" >/dev/null; then
    cat "${SUMMARY_MD}"
    return 1
  fi

  cat "${SUMMARY_MD}"
}

main "$@"
