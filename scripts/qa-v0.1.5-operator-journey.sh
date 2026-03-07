#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

RUN_ID="${RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)}"
OUTPUT_DIR="${OUTPUT_DIR:-.run/qa/v0.1.5-journey-${RUN_ID}}"
SUMMARY_JSON="${OUTPUT_DIR}/summary.json"
SUMMARY_MD="${OUTPUT_DIR}/summary.md"
API_BASE_URL="${API_BASE_URL:-http://127.0.0.1:8080}"
AUTH_USER="${AUTH_USER:-admin}"
READ_ONLY_MODE=0

TMP_STAGES=""
LAST_STATUS=""
LAST_BODY_FILE=""

TICKET_ID=""
ALERT_ID=""
BACKUP_POLICY_ID=""
BACKUP_RUN_ID=""
RESTORE_EVIDENCE_ID=""

log() {
  printf '[qa-v0.1.5] %s\n' "$*"
}

fatal() {
  printf '[qa-v0.1.5][ERROR] %s\n' "$*" >&2
  exit 1
}

usage() {
  cat <<'USAGE'
No-code operator journey validation (v0.1.5)

Usage:
  bash scripts/qa-v0.1.5-operator-journey.sh [options]

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
  diag="$(tail -n 4 "${log_file}" | tr '\n' ' ' | sed 's/[[:space:]]\+/ /g' | sed 's/^ //;s/ $//')"

  jq -nc \
    --arg key "${stage_key}" \
    --arg name "${stage_name}" \
    --arg status "${status}" \
    --arg log_file "${log_file}" \
    --arg diagnostics "${diag}" \
    '{
      key: $key,
      name: $name,
      status: $status,
      log_file: $log_file,
      diagnostics: $diagnostics
    }' >>"${TMP_STAGES}"
}

run_stage() {
  local stage_key="$1"
  local stage_name="$2"
  local stage_fn="$3"
  local log_file="${OUTPUT_DIR}/${stage_key}.log"

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

stage_setup() {
  request_api GET "/api/v1/setup/preflight"
  status_is_success "${LAST_STATUS}" || {
    echo "setup preflight failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  request_api GET "/api/v1/setup/checklist"
  status_is_success "${LAST_STATUS}" || {
    echo "setup checklist failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  echo "setup preflight/checklist OK"
  return 0
}

stage_alert() {
  request_api GET "/api/v1/alerts?limit=20&offset=0"
  status_is_success "${LAST_STATUS}" || {
    echo "alerts list failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  ALERT_ID="$(jq -r '.items[0].id // empty' "${LAST_BODY_FILE}")"
  if [[ -n "${ALERT_ID}" ]]; then
    echo "selected alert_id=${ALERT_ID}"
  else
    echo "no alert available from /alerts; incident stage may be skipped"
  fi
  return 0
}

stage_incident_command() {
  request_api GET "/api/v1/ops/cockpit/incidents?limit=20&offset=0"
  status_is_success "${LAST_STATUS}" || {
    echo "incident list failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  if [[ -z "${ALERT_ID}" ]]; then
    ALERT_ID="$(jq -r '.items[0].alert_id // empty' "${LAST_BODY_FILE}")"
  fi

  if [[ -z "${ALERT_ID}" ]]; then
    echo "no alert available for incident command write-path check"
    return 2
  fi

  if (( READ_ONLY_MODE == 1 )); then
    echo "read-only mode enabled, skipping incident command write"
    return 2
  fi

  request_api POST "/api/v1/ops/cockpit/incidents/${ALERT_ID}/command" \
    "{\"status\":\"in_progress\",\"owner\":\"${AUTH_USER}\",\"summary\":\"qa v0.1.5 incident command\"}"

  if status_is_success "${LAST_STATUS}"; then
    echo "incident command updated for alert_id=${ALERT_ID}"
    return 0
  fi

  if [[ "${LAST_STATUS}" == "403" ]]; then
    echo "incident command write forbidden under current RBAC; skipped"
    cat "${LAST_BODY_FILE}" || true
    return 2
  fi

  echo "incident command write failed, status=${LAST_STATUS}"
  cat "${LAST_BODY_FILE}" || true
  return 1
}

stage_escalation() {
  if (( READ_ONLY_MODE == 1 )); then
    echo "read-only mode enabled, skipping ticket/escalation write-path"
    return 2
  fi

  request_api POST "/api/v1/tickets" \
    "{\"title\":\"qa-v0.1.5-ticket-${RUN_ID}\",\"priority\":\"high\",\"category\":\"incident\",\"assignee\":\"${AUTH_USER}\"}"

  if [[ "${LAST_STATUS}" == "403" ]]; then
    echo "ticket write forbidden under current RBAC; skipped"
    cat "${LAST_BODY_FILE}" || true
    return 2
  fi
  status_is_success "${LAST_STATUS}" || {
    echo "ticket create failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  TICKET_ID="$(jq -r '.id // empty' "${LAST_BODY_FILE}")"
  [[ -n "${TICKET_ID}" ]] || {
    echo "ticket create returned no id"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  request_api GET "/api/v1/tickets/escalation/policy"
  status_is_success "${LAST_STATUS}" || {
    echo "load escalation policy failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  request_api POST "/api/v1/tickets/escalation/policy/preview" \
    "{\"priority\":\"high\",\"status\":\"open\",\"ticket_age_minutes\":35,\"current_assignee\":\"${AUTH_USER}\"}"
  status_is_success "${LAST_STATUS}" || {
    echo "escalation preview failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  request_api POST "/api/v1/tickets/escalation/run" \
    "{\"dry_run\":true,\"note\":\"qa v0.1.5 dry run\"}"
  status_is_success "${LAST_STATUS}" || {
    echo "escalation run failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  echo "ticket escalation flow validated ticket_id=${TICKET_ID}"
  return 0
}

stage_handover() {
  local shift_date
  shift_date="$(date +%F)"

  request_api GET "/api/v1/ops/cockpit/handover-digest?shift_date=${shift_date}"
  status_is_success "${LAST_STATUS}" || {
    echo "handover digest load failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  if (( READ_ONLY_MODE == 1 )); then
    echo "read-only mode enabled, skipping handover close write-path"
    return 2
  fi

  if [[ -z "${TICKET_ID}" ]]; then
    TICKET_ID="$(jq -r '.items[]? | select(.source_type == "ticket_backlog") | .source_id' "${LAST_BODY_FILE}" | head -n1)"
  fi

  if [[ -z "${TICKET_ID}" ]]; then
    echo "no ticket id available to validate handover close"
    return 2
  fi

  request_api POST "/api/v1/ops/cockpit/handover-digest/items/ticket:${TICKET_ID}/close" \
    "{\"shift_date\":\"${shift_date}\",\"source_type\":\"ticket_backlog\",\"source_id\":${TICKET_ID},\"next_owner\":\"${AUTH_USER}\",\"next_action\":\"qa handover close\"}"

  if status_is_success "${LAST_STATUS}"; then
    echo "handover item closed for ticket_id=${TICKET_ID}"
    return 0
  fi

  if [[ "${LAST_STATUS}" == "403" ]]; then
    echo "handover close forbidden under current RBAC; skipped"
    cat "${LAST_BODY_FILE}" || true
    return 2
  fi

  echo "handover close failed, status=${LAST_STATUS}"
  cat "${LAST_BODY_FILE}" || true
  return 1
}

stage_backup_evidence() {
  if (( READ_ONLY_MODE == 1 )); then
    echo "read-only mode enabled, skipping backup evidence write-path"
    return 2
  fi

  request_api POST "/api/v1/ops/cockpit/backup/policies" \
    "{\"policy_key\":\"qa-v015-${RUN_ID}\",\"name\":\"QA v0.1.5 ${RUN_ID}\",\"frequency\":\"daily\",\"schedule_time_utc\":\"01:30\",\"retention_days\":7,\"destination_type\":\"local\",\"destination_uri\":\"file:///tmp/qa-v015-${RUN_ID}\",\"drill_enabled\":true,\"drill_frequency\":\"weekly\",\"drill_weekday\":3,\"drill_time_utc\":\"02:30\"}"

  if [[ "${LAST_STATUS}" == "403" ]]; then
    echo "backup policy write forbidden under current RBAC; skipped"
    cat "${LAST_BODY_FILE}" || true
    return 2
  fi
  status_is_success "${LAST_STATUS}" || {
    echo "backup policy create failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  BACKUP_POLICY_ID="$(jq -r '.id // empty' "${LAST_BODY_FILE}")"
  [[ -n "${BACKUP_POLICY_ID}" ]] || {
    echo "backup policy create returned no id"
    return 1
  }

  request_api POST "/api/v1/ops/cockpit/backup/policies/${BACKUP_POLICY_ID}/run" "{\"run_type\":\"backup\"}"
  status_is_success "${LAST_STATUS}" || {
    echo "backup run failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  BACKUP_RUN_ID="$(jq -r '.run.id // empty' "${LAST_BODY_FILE}")"
  [[ -n "${BACKUP_RUN_ID}" ]] || {
    echo "backup run response missing run.id"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  request_api POST "/api/v1/ops/cockpit/backup/runs/${BACKUP_RUN_ID}/restore-evidence" \
    "{\"ticket_ref\":\"QA-${RUN_ID}\",\"artifact_url\":\"https://example.invalid/qa/${RUN_ID}/restore-proof\",\"note\":\"qa v0.1.5 evidence\",\"close_evidence\":false}"
  status_is_success "${LAST_STATUS}" || {
    echo "restore evidence create failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  RESTORE_EVIDENCE_ID="$(jq -r '.id // empty' "${LAST_BODY_FILE}")"
  [[ -n "${RESTORE_EVIDENCE_ID}" ]] || {
    echo "restore evidence response missing id"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  request_api PATCH "/api/v1/ops/cockpit/backup/restore-evidence/${RESTORE_EVIDENCE_ID}" \
    "{\"note\":\"qa close evidence\",\"close_evidence\":true}"
  status_is_success "${LAST_STATUS}" || {
    echo "restore evidence close failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  echo "backup evidence flow validated policy_id=${BACKUP_POLICY_ID} run_id=${BACKUP_RUN_ID} evidence_id=${RESTORE_EVIDENCE_ID}"
  return 0
}

write_summary() {
  local generated_at total fail_count skip_count pass_count overall
  generated_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

  total="$(jq -s 'length' "${TMP_STAGES}")"
  fail_count="$(jq -s 'map(select(.status == "fail")) | length' "${TMP_STAGES}")"
  skip_count="$(jq -s 'map(select(.status == "skipped")) | length' "${TMP_STAGES}")"
  pass_count="$(jq -s 'map(select(.status == "pass")) | length' "${TMP_STAGES}")"

  overall="pass"
  if [[ "${fail_count}" != "0" ]]; then
    overall="fail"
  fi

  jq -n \
    --arg version "v0.1.5" \
    --arg run_id "${RUN_ID}" \
    --arg generated_at "${generated_at}" \
    --arg api_base_url "${API_BASE_URL}" \
    --arg auth_user "${AUTH_USER}" \
    --argjson read_only "$( (( READ_ONLY_MODE == 1 )) && echo true || echo false )" \
    --arg overall "${overall}" \
    --arg ticket_id "${TICKET_ID}" \
    --arg alert_id "${ALERT_ID}" \
    --arg backup_policy_id "${BACKUP_POLICY_ID}" \
    --arg backup_run_id "${BACKUP_RUN_ID}" \
    --arg restore_evidence_id "${RESTORE_EVIDENCE_ID}" \
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
        alert_id: $alert_id,
        ticket_id: $ticket_id,
        backup_policy_id: $backup_policy_id,
        backup_run_id: $backup_run_id,
        restore_evidence_id: $restore_evidence_id
      },
      stages: $stages,
      totals: {
        total: ($stages | length),
        pass: ($stages | map(select(.status == "pass")) | length),
        fail: ($stages | map(select(.status == "fail")) | length),
        skipped: ($stages | map(select(.status == "skipped")) | length)
      }
    }' >"${SUMMARY_JSON}"

  {
    echo "# v0.1.5 Operator Journey Validation Summary"
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
    echo "| Stage | Result | Diagnostics | Log |"
    echo "| --- | --- | --- | --- |"
    jq -r '.stages[] | "| `\(.key)` | \(.status) | \(.diagnostics) | `\(.log_file)` |"' "${SUMMARY_JSON}"

    echo
    echo "## Entity References"
    echo
    echo "- alert_id=${ALERT_ID:-none}"
    echo "- ticket_id=${TICKET_ID:-none}"
    echo "- backup_policy_id=${BACKUP_POLICY_ID:-none}"
    echo "- backup_run_id=${BACKUP_RUN_ID:-none}"
    echo "- restore_evidence_id=${RESTORE_EVIDENCE_ID:-none}"

    if [[ "${fail_count}" != "0" ]]; then
      echo
      echo "## Failure Localization"
      jq -r '.stages[] | select(.status == "fail") | "- stage=\(.key) diagnostics=\(.diagnostics)"' "${SUMMARY_JSON}"
    fi

    echo
    echo "## Artifacts"
    echo
    echo "- JSON summary: ${SUMMARY_JSON}"
    echo "- Markdown summary: ${SUMMARY_MD}"
  } >"${SUMMARY_MD}"

  log "summary JSON: ${SUMMARY_JSON}"
  log "summary Markdown: ${SUMMARY_MD}"

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

  TMP_STAGES="$(mktemp)"
  trap 'rm -f "${TMP_STAGES}" "${LAST_BODY_FILE:-}"' EXIT

  run_stage "setup" "setup preflight and checklist" stage_setup
  run_stage "alert" "alert queue visibility" stage_alert
  run_stage "incident" "incident command update" stage_incident_command
  run_stage "escalation" "ticket escalation dry-run" stage_escalation
  run_stage "handover" "handover digest close flow" stage_handover
  run_stage "backup_evidence" "backup run and restore evidence" stage_backup_evidence

  write_summary
}

main "$@"
