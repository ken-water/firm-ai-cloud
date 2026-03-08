#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

RUN_ID="${RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)}"
OUTPUT_DIR="${OUTPUT_DIR:-.run/qa/v0.1.6-journey-${RUN_ID}}"
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

TICKET_ID=""
RESERVATION_ID=""
BACKUP_POLICY_ID=""
BACKUP_RUN_ID=""
RESTORE_EVIDENCE_ID=""
RUNBOOK_EXECUTION_ID=""

log() {
  printf '[qa-v0.1.6] %s\n' "$*"
}

fatal() {
  printf '[qa-v0.1.6][ERROR] %s\n' "$*" >&2
  exit 1
}

usage() {
  cat <<'USAGE'
No-code operator journey validation (v0.1.6)

Usage:
  bash scripts/qa-v0.1.6-operator-journey.sh [options]

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

stage_setup_profile() {
  set_stage_context \
    "/api/v1/setup/profiles and /api/v1/setup/profiles/{key}/preview" \
    "Verify setup profile key and payload shape, then retry with valid profile note/params."

  request_api GET "/api/v1/setup/preflight"
  status_is_success "${LAST_STATUS}" || {
    echo "setup preflight failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  request_api GET "/api/v1/setup/profiles"
  status_is_success "${LAST_STATUS}" || {
    echo "setup profiles list failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  request_api POST "/api/v1/setup/profiles/smb-small-office/preview" \
    "{\"note\":\"qa v0.1.6 setup profile preview\"}"
  status_is_success "${LAST_STATUS}" || {
    echo "setup profile preview failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  if (( READ_ONLY_MODE == 1 )); then
    echo "read-only mode enabled, skipping setup profile apply write-path"
    return 2
  fi

  request_api POST "/api/v1/setup/profiles/smb-small-office/apply" \
    "{\"note\":\"qa v0.1.6 setup profile apply\"}"
  if status_is_success "${LAST_STATUS}"; then
    echo "setup profile apply completed"
    return 0
  fi

  if [[ "${LAST_STATUS}" == "403" ]]; then
    echo "setup profile apply forbidden under current RBAC; skipped"
    cat "${LAST_BODY_FILE}" || true
    return 2
  fi

  echo "setup profile apply failed, status=${LAST_STATUS}"
  cat "${LAST_BODY_FILE}" || true
  return 1
}

stage_next_actions() {
  set_stage_context \
    "/api/v1/ops/cockpit/next-actions" \
    "Check cockpit queue prerequisites (alerts/tickets/incidents) and rerun snapshot loaders."

  request_api GET "/api/v1/ops/cockpit/queue"
  status_is_success "${LAST_STATUS}" || {
    echo "cockpit queue load failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  request_api GET "/api/v1/ops/cockpit/next-actions?limit=5"
  status_is_success "${LAST_STATUS}" || {
    echo "next actions load failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  echo "next action assistant API path validated"
  return 0
}

stage_calendar_reservation() {
  set_stage_context \
    "/api/v1/ops/cockpit/change-calendar/slot-recommendations and /reservations" \
    "Verify operation_kind/risk_level/duration input and retry reservation with conflict-free slot."

  request_api GET "/api/v1/ops/cockpit/change-calendar/slot-recommendations?operation_kind=playbook.execute.restart-service-safe&risk_level=high&duration_minutes=30&limit=3"
  status_is_success "${LAST_STATUS}" || {
    echo "slot recommendations failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  local recommendation_body_file="${LAST_BODY_FILE}"

  request_api GET "/api/v1/ops/cockpit/change-calendar/reservations?days=7"
  status_is_success "${LAST_STATUS}" || {
    echo "reservation list failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  if (( READ_ONLY_MODE == 1 )); then
    echo "read-only mode enabled, skipping reservation write-path"
    return 2
  fi

  local fallback_start_at fallback_end_at
  fallback_start_at="$(date -u -d '+3 hours' +%Y-%m-%dT%H:%M:%SZ)"
  fallback_end_at="$(date -u -d '+4 hours' +%Y-%m-%dT%H:%M:%SZ)"

  mapfile -t slot_pairs < <(jq -r '.items[]? | select(.start_at and .end_at) | "\(.start_at)|\(.end_at)"' "${recommendation_body_file}")
  if (( ${#slot_pairs[@]} == 0 )); then
    slot_pairs=("${fallback_start_at}|${fallback_end_at}")
  else
    slot_pairs+=("${fallback_start_at}|${fallback_end_at}")
  fi

  local slot_pair start_at end_at payload
  for slot_pair in "${slot_pairs[@]}"; do
    start_at="${slot_pair%%|*}"
    end_at="${slot_pair##*|}"
    payload="$(jq -nc \
      --arg start_at "${start_at}" \
      --arg end_at "${end_at}" \
      --arg owner "${AUTH_USER}" \
      --arg note "qa v0.1.6 reservation ${RUN_ID}" \
      '{
        start_at: $start_at,
        end_at: $end_at,
        operation_kind: "playbook.execute.restart-service-safe",
        risk_level: "high",
        owner: $owner,
        site: "dc-a",
        department: "platform",
        note: $note
      }')"

    request_api POST "/api/v1/ops/cockpit/change-calendar/reservations" "${payload}"

    if [[ "${LAST_STATUS}" == "403" ]]; then
      echo "reservation write forbidden under current RBAC; skipped"
      cat "${LAST_BODY_FILE}" || true
      return 2
    fi

    if status_is_success "${LAST_STATUS}"; then
      RESERVATION_ID="$(jq -r '.reservation.id // .id // empty' "${LAST_BODY_FILE}")"
      [[ -n "${RESERVATION_ID}" ]] || {
        echo "reservation response missing id"
        cat "${LAST_BODY_FILE}" || true
        return 1
      }

      echo "calendar reservation flow validated reservation_id=${RESERVATION_ID}"
      return 0
    fi

    if [[ "${LAST_STATUS}" == "400" ]] && grep -qi "conflict" "${LAST_BODY_FILE}"; then
      echo "reservation slot conflict for ${start_at}~${end_at}, retrying next slot"
      continue
    fi

    echo "reservation create failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  done

  echo "reservation create failed after exhausting recommended slots"
  return 1
}

stage_evidence_compliance() {
  set_stage_context \
    "/api/v1/ops/cockpit/backup/evidence-compliance/scorecard" \
    "Validate policy mode/sla selector fields and rerun scorecard with explicit week_start/as_of."

  request_api GET "/api/v1/ops/cockpit/backup/evidence-compliance/policy"
  status_is_success "${LAST_STATUS}" || {
    echo "evidence policy load failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  request_api GET "/api/v1/ops/cockpit/backup/evidence-compliance/scorecard"
  status_is_success "${LAST_STATUS}" || {
    echo "evidence scorecard load failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  request_api GET "/api/v1/ops/cockpit/backup/evidence-compliance/scorecard/export?format=json"
  status_is_success "${LAST_STATUS}" || {
    echo "evidence scorecard export failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  if (( READ_ONLY_MODE == 1 )); then
    echo "read-only mode enabled, skipping evidence policy update write-path"
    return 2
  fi

  request_api PUT "/api/v1/ops/cockpit/backup/evidence-compliance/policy" \
    "{\"mode\":\"advisory\",\"sla_hours\":24,\"require_failed_runs\":true,\"require_drill_runs\":true,\"note\":\"qa v0.1.6 evidence policy update\"}"

  if [[ "${LAST_STATUS}" == "403" ]]; then
    echo "evidence policy update forbidden under current RBAC; skipped"
    cat "${LAST_BODY_FILE}" || true
    return 2
  fi

  status_is_success "${LAST_STATUS}" || {
    echo "evidence policy update failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  echo "evidence compliance policy + scorecard flow validated"
  return 0
}

stage_runbook_template() {
  set_stage_context \
    "/api/v1/ops/cockpit/runbook-templates/{key}/execute" \
    "Check required params/preflight confirmations/evidence payload and retry with template defaults."

  request_api GET "/api/v1/ops/cockpit/runbook-templates"
  status_is_success "${LAST_STATUS}" || {
    echo "runbook template catalog load failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  request_api GET "/api/v1/ops/cockpit/runbook-templates/executions?limit=10"
  status_is_success "${LAST_STATUS}" || {
    echo "runbook execution list load failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  if (( READ_ONLY_MODE == 1 )); then
    echo "read-only mode enabled, skipping runbook execute write-path"
    return 2
  fi

  request_api POST "/api/v1/ops/cockpit/runbook-templates/service-restart-safe/execute" \
    "{\"params\":{\"asset_ref\":\"qa-asset-${RUN_ID}\",\"service_name\":\"nginx\",\"restart_scope\":\"rolling\"},\"preflight_confirmations\":[\"confirm_change_window\",\"confirm_owner_ack\",\"confirm_rollback_ready\"],\"evidence\":{\"summary\":\"qa v0.1.6 runbook execution\",\"ticket_ref\":\"QA-RUNBOOK-${RUN_ID}\",\"artifact_url\":\"https://example.invalid/qa/${RUN_ID}/runbook\"},\"note\":\"qa runbook execute\"}"

  if [[ "${LAST_STATUS}" == "403" ]]; then
    echo "runbook execute forbidden under current RBAC; skipped"
    cat "${LAST_BODY_FILE}" || true
    return 2
  fi

  status_is_success "${LAST_STATUS}" || {
    echo "runbook execute failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  RUNBOOK_EXECUTION_ID="$(jq -r '.execution.id // .id // empty' "${LAST_BODY_FILE}")"
  [[ -n "${RUNBOOK_EXECUTION_ID}" ]] || {
    echo "runbook execute response missing execution id"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  request_api GET "/api/v1/ops/cockpit/runbook-templates/executions/${RUNBOOK_EXECUTION_ID}"
  status_is_success "${LAST_STATUS}" || {
    echo "runbook execution detail failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  echo "runbook template execution flow validated execution_id=${RUNBOOK_EXECUTION_ID}"
  return 0
}

stage_handover_quality() {
  set_stage_context \
    "/api/v1/ops/cockpit/handover-digest/reminders" \
    "Generate digest for valid shift_date and inspect reminder export payload for ownership/overdue fields."

  local shift_date
  shift_date="$(date +%F)"

  request_api GET "/api/v1/ops/cockpit/handover-digest?shift_date=${shift_date}"
  status_is_success "${LAST_STATUS}" || {
    echo "handover digest load failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  request_api GET "/api/v1/ops/cockpit/handover-digest/reminders?shift_date=${shift_date}"
  status_is_success "${LAST_STATUS}" || {
    echo "handover reminder load failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  request_api GET "/api/v1/ops/cockpit/handover-digest/reminders/export?shift_date=${shift_date}&format=csv"
  status_is_success "${LAST_STATUS}" || {
    echo "handover reminder export failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  echo "handover quality gate path validated"
  return 0
}

stage_backup_evidence_writepath() {
  set_stage_context \
    "/api/v1/ops/cockpit/backup/runs/{id}/restore-evidence" \
    "Validate run_id and evidence payload, then close evidence before SLA window expiration."

  if (( READ_ONLY_MODE == 1 )); then
    echo "read-only mode enabled, skipping backup evidence write-path"
    return 2
  fi

  request_api POST "/api/v1/ops/cockpit/backup/policies" \
    "{\"policy_key\":\"qa-v016-${RUN_ID}\",\"name\":\"QA v0.1.6 ${RUN_ID}\",\"frequency\":\"daily\",\"schedule_time_utc\":\"01:30\",\"retention_days\":7,\"destination_type\":\"local\",\"destination_uri\":\"file:///tmp/qa-v016-${RUN_ID}\",\"drill_enabled\":true,\"drill_frequency\":\"weekly\",\"drill_weekday\":3,\"drill_time_utc\":\"02:30\"}"
  status_is_success "${LAST_STATUS}" || {
    echo "backup policy create failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  BACKUP_POLICY_ID="$(jq -r '.id // empty' "${LAST_BODY_FILE}")"
  [[ -n "${BACKUP_POLICY_ID}" ]] || {
    echo "backup policy create returned no id"
    cat "${LAST_BODY_FILE}" || true
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
    "{\"ticket_ref\":\"QA-${RUN_ID}\",\"artifact_url\":\"https://example.invalid/qa/${RUN_ID}/restore-proof\",\"note\":\"qa v0.1.6 evidence\",\"close_evidence\":false}"
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

  echo "backup evidence closure flow validated policy_id=${BACKUP_POLICY_ID} run_id=${BACKUP_RUN_ID} evidence_id=${RESTORE_EVIDENCE_ID}"
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
    --arg version "v0.1.6" \
    --arg run_id "${RUN_ID}" \
    --arg generated_at "${generated_at}" \
    --arg api_base_url "${API_BASE_URL}" \
    --arg auth_user "${AUTH_USER}" \
    --argjson read_only "$( (( READ_ONLY_MODE == 1 )) && echo true || echo false )" \
    --arg overall "${overall}" \
    --arg reservation_id "${RESERVATION_ID}" \
    --arg backup_policy_id "${BACKUP_POLICY_ID}" \
    --arg backup_run_id "${BACKUP_RUN_ID}" \
    --arg restore_evidence_id "${RESTORE_EVIDENCE_ID}" \
    --arg runbook_execution_id "${RUNBOOK_EXECUTION_ID}" \
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
        reservation_id: $reservation_id,
        backup_policy_id: $backup_policy_id,
        backup_run_id: $backup_run_id,
        restore_evidence_id: $restore_evidence_id,
        runbook_execution_id: $runbook_execution_id
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
    --arg version "v0.1.6" \
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
    echo "# v0.1.6 Operator Journey Validation Summary"
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
    echo "- reservation_id=${RESERVATION_ID:-none}"
    echo "- backup_policy_id=${BACKUP_POLICY_ID:-none}"
    echo "- backup_run_id=${BACKUP_RUN_ID:-none}"
    echo "- restore_evidence_id=${RESTORE_EVIDENCE_ID:-none}"
    echo "- runbook_execution_id=${RUNBOOK_EXECUTION_ID:-none}"

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

  run_stage "setup_profile" "setup profile preview/apply path" stage_setup_profile
  run_stage "next_actions" "cockpit next-best-action path" stage_next_actions
  run_stage "calendar_reservation" "change calendar reservation guidance path" stage_calendar_reservation
  run_stage "evidence_compliance" "restore evidence compliance scorecard path" stage_evidence_compliance
  run_stage "runbook_template" "one-click runbook template execution path" stage_runbook_template
  run_stage "handover_quality" "handover reminder quality gate path" stage_handover_quality
  run_stage "backup_evidence_writepath" "backup evidence write-path closure" stage_backup_evidence_writepath

  write_summary
}

main "$@"
