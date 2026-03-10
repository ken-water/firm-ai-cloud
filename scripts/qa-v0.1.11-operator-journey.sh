#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

RUN_ID="${RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)}"
OUTPUT_DIR="${OUTPUT_DIR:-.run/qa/v0.1.11-journey-${RUN_ID}}"
SUMMARY_JSON="${OUTPUT_DIR}/summary.json"
SUMMARY_MD="${OUTPUT_DIR}/summary.md"
ARTIFACT_INDEX_JSON="${OUTPUT_DIR}/artifact-index.json"
API_BASE_URL="${API_BASE_URL:-http://127.0.0.1:8080}"
AUTH_USER="${AUTH_USER:-admin}"
READ_ONLY_AUTH_USER="${READ_ONLY_AUTH_USER:-viewer}"
READ_ONLY_MODE=0
ALERT_WINDOW_DAYS="${ALERT_WINDOW_DAYS:-14}"

TMP_STAGES=""
LAST_STATUS=""
LAST_BODY_FILE=""

STAGE_ENDPOINT=""
STAGE_REMEDIATION=""

ANALYTICS_POLICY_KEY=""
ANALYTICS_THRESHOLD=""
ANALYTICS_SAMPLE=""
ALERT_TOTAL=""
ALERT_TEMPLATE_KEY=""
ALERT_EXPECT_CREATE=0
CREATED_FLAG=""
CREATED_TICKET_ID=""
CREATED_TICKET_NO=""
REUSED_TICKET_ID=""
REUSED_TICKET_NO=""
TICKET_SOURCE_KEY=""
GUARD_USER=""
GUARD_STATUS=""

log() {
  printf '[qa-v0.1.11] %s\n' "$*"
}

fatal() {
  printf '[qa-v0.1.11][ERROR] %s\n' "$*" >&2
  exit 1
}

usage() {
  cat <<'USAGE'
Runbook risk alert to ticket closure validation (v0.1.11)

Usage:
  bash scripts/qa-v0.1.11-operator-journey.sh [options]

Options:
  --api-base-url <url>       API base URL
  --auth-user <username>     Auth user header for normal stages (default: admin)
  --read-only-user <user>    Auth user used for read-only guard probe (default: viewer)
  --output-dir <path>        Output directory
  --run-id <id>              Run id
  --alert-days <days>        Alert window days (1-90, default: 14)
  --read-only                Skip write-path stages (for read-only validation)
  -h, --help                 Show help
USAGE
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || fatal "missing required command: $1"
}

normalize_days() {
  local raw="$1"
  local parsed

  if [[ ! "${raw}" =~ ^[0-9]+$ ]]; then
    echo 14
    return
  fi

  parsed="${raw}"
  if (( parsed < 1 )); then
    parsed=1
  elif (( parsed > 90 )); then
    parsed=90
  fi
  echo "${parsed}"
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
    args+=( -H "Content-Type: application/json" -d "${body}" )
  fi

  if ! LAST_STATUS="$(curl "${args[@]}")"; then
    LAST_STATUS="000"
  fi
}

request_api() {
  local method="$1"
  local path="$2"
  local body="${3-}"
  request_api_as_user "${AUTH_USER}" "${method}" "${path}" "${body}"
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

seed_dependency_check_failure_alert() {
  local payload
  payload="$(jq -nc \
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
        summary: "qa v0.1.11 seeded failure",
        ticket_ref: ("QA-RISK-" + $run_id),
        artifact_url: ("https://example.invalid/qa/" + $run_id + "/risk-failure")
      },
      note: "qa v0.1.11 seed risk alert"
    }')"

  request_api POST "/api/v1/ops/cockpit/runbook-templates/dependency-check/execute" "${payload}"

  if [[ "${LAST_STATUS}" == "403" ]]; then
    echo "seed execution forbidden under current RBAC"
    cat "${LAST_BODY_FILE}" || true
    return 2
  fi

  status_is_success "${LAST_STATUS}" || {
    echo "seed execution failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  local seeded_status
  seeded_status="$(jq -r '.execution.status // empty' "${LAST_BODY_FILE}")"
  [[ "${seeded_status}" == "failed" ]] || {
    echo "expected failed seeded execution, got=${seeded_status}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  echo "seeded one failed dependency-check execution"
  return 0
}

stage_policy_read() {
  set_stage_context \
    "/api/v1/ops/cockpit/runbook-templates, /analytics/policy" \
    "Verify runbook template catalog and analytics policy envelope for risk-ticket flow."

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

  request_api GET "/api/v1/ops/cockpit/runbook-templates/analytics/policy"
  status_is_success "${LAST_STATUS}" || {
    echo "analytics policy load failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  ANALYTICS_POLICY_KEY="$(jq -r '.policy.policy_key // empty' "${LAST_BODY_FILE}")"
  ANALYTICS_THRESHOLD="$(jq -r '.policy.failure_rate_threshold_percent // empty' "${LAST_BODY_FILE}")"
  ANALYTICS_SAMPLE="$(jq -r '.policy.minimum_sample_size // empty' "${LAST_BODY_FILE}")"

  [[ -n "${ANALYTICS_POLICY_KEY}" ]] || {
    echo "analytics policy key is missing"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  [[ "${ANALYTICS_THRESHOLD}" =~ ^[0-9]+$ ]] || {
    echo "analytics threshold is invalid: ${ANALYTICS_THRESHOLD}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  [[ "${ANALYTICS_SAMPLE}" =~ ^[0-9]+$ ]] || {
    echo "analytics minimum sample is invalid: ${ANALYTICS_SAMPLE}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  echo "policy read validated templates=${template_total} policy=${ANALYTICS_POLICY_KEY} threshold=${ANALYTICS_THRESHOLD}% sample=${ANALYTICS_SAMPLE}"
  return 0
}

stage_alerts_read() {
  set_stage_context \
    "/api/v1/ops/cockpit/runbook-templates/analytics/alerts" \
    "Verify risk alerts payload and pick one active alert for ticket create/reuse validation."

  ALERT_WINDOW_DAYS="$(normalize_days "${ALERT_WINDOW_DAYS}")"

  request_api GET "/api/v1/ops/cockpit/runbook-templates/analytics/alerts?days=${ALERT_WINDOW_DAYS}&limit=20"
  status_is_success "${LAST_STATUS}" || {
    echo "risk alerts read failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  ALERT_TOTAL="$(jq -r '.total // 0' "${LAST_BODY_FILE}")"
  [[ "${ALERT_TOTAL}" =~ ^[0-9]+$ ]] || {
    echo "risk alerts total is invalid: ${ALERT_TOTAL}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  if [[ "${ALERT_TOTAL}" == "0" ]]; then
    if (( READ_ONLY_MODE == 1 )); then
      echo "no active risk alert in read-only mode; skip downstream ticket stages"
      return 2
    fi

    echo "no active risk alert found; seeding one failed dependency-check execution"
    seed_dependency_check_failure_alert || return $?

    request_api GET "/api/v1/ops/cockpit/runbook-templates/analytics/alerts?days=${ALERT_WINDOW_DAYS}&limit=20"
    status_is_success "${LAST_STATUS}" || {
      echo "risk alerts re-read failed after seed, status=${LAST_STATUS}"
      cat "${LAST_BODY_FILE}" || true
      return 1
    }

    ALERT_TOTAL="$(jq -r '.total // 0' "${LAST_BODY_FILE}")"
    [[ "${ALERT_TOTAL}" =~ ^[0-9]+$ ]] || {
      echo "risk alerts total is invalid after seed: ${ALERT_TOTAL}"
      cat "${LAST_BODY_FILE}" || true
      return 1
    }
    [[ "${ALERT_TOTAL}" != "0" ]] || {
      echo "risk alerts still empty after seed"
      cat "${LAST_BODY_FILE}" || true
      return 1
    }
  fi

  local window_days
  window_days="$(jq -r '.window.days // empty' "${LAST_BODY_FILE}")"
  if [[ "${window_days}" =~ ^[0-9]+$ ]]; then
    ALERT_WINDOW_DAYS="${window_days}"
  fi

  ALERT_TEMPLATE_KEY="$(jq -r '.items[]? | select((.ticket_link == null) and (.template_key|type == "string") and (.template_key|length > 0)) | .template_key' "${LAST_BODY_FILE}" | head -n 1)"
  if [[ -z "${ALERT_TEMPLATE_KEY}" ]]; then
    ALERT_TEMPLATE_KEY="$(jq -r '.items[0].template_key // empty' "${LAST_BODY_FILE}")"
  fi

  [[ -n "${ALERT_TEMPLATE_KEY}" ]] || {
    echo "failed to select target alert template"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  if jq -e --arg key "${ALERT_TEMPLATE_KEY}" '.items[]? | select(.template_key == $key and .ticket_link == null)' "${LAST_BODY_FILE}" >/dev/null 2>&1; then
    ALERT_EXPECT_CREATE=1
  else
    ALERT_EXPECT_CREATE=0
  fi

  local existing_ticket
  existing_ticket="$(jq -r --arg key "${ALERT_TEMPLATE_KEY}" '.items[]? | select(.template_key == $key) | .ticket_link.ticket_no // empty' "${LAST_BODY_FILE}" | head -n 1)"

  echo "alerts read validated total=${ALERT_TOTAL} selected_template=${ALERT_TEMPLATE_KEY} days=${ALERT_WINDOW_DAYS} expect_create=${ALERT_EXPECT_CREATE} existing_ticket=${existing_ticket:-none}"
  return 0
}

build_ticket_action_payload() {
  local note="$1"
  jq -nc \
    --arg key "${ALERT_TEMPLATE_KEY}" \
    --argjson days "${ALERT_WINDOW_DAYS}" \
    --arg note "${note}" \
    '{
      template_key: $key,
      execution_mode: null,
      days: $days,
      note: $note
    }'
}

stage_alert_ticket_create() {
  set_stage_context \
    "/api/v1/ops/cockpit/runbook-templates/analytics/alerts/tickets (POST)" \
    "Create (or deterministic reuse) a runbook-risk ticket from selected alert template."

  if (( READ_ONLY_MODE == 1 )); then
    echo "read-only mode enabled, skipping alert ticket create write-path"
    return 2
  fi

  [[ -n "${ALERT_TEMPLATE_KEY}" ]] || {
    echo "alert template key is required before create stage"
    return 1
  }

  local payload
  payload="$(build_ticket_action_payload "qa v0.1.11 alert ticket create")"

  request_api POST "/api/v1/ops/cockpit/runbook-templates/analytics/alerts/tickets" "${payload}"

  if [[ "${LAST_STATUS}" == "403" ]]; then
    echo "alert ticket create forbidden for user=${AUTH_USER}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  fi

  status_is_success "${LAST_STATUS}" || {
    echo "alert ticket create failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  CREATED_FLAG="$(jq -r '.created // empty' "${LAST_BODY_FILE}")"
  CREATED_TICKET_ID="$(jq -r '.ticket_link.ticket_id // empty' "${LAST_BODY_FILE}")"
  CREATED_TICKET_NO="$(jq -r '.ticket_link.ticket_no // empty' "${LAST_BODY_FILE}")"
  TICKET_SOURCE_KEY="$(jq -r '.source_key // empty' "${LAST_BODY_FILE}")"

  [[ "${CREATED_FLAG}" == "true" || "${CREATED_FLAG}" == "false" ]] || {
    echo "create response missing boolean field created"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  [[ "${CREATED_TICKET_ID}" =~ ^[0-9]+$ ]] || {
    echo "create response missing ticket_id: ${CREATED_TICKET_ID}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  [[ -n "${CREATED_TICKET_NO}" ]] || {
    echo "create response missing ticket_no"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  if [[ "${ALERT_EXPECT_CREATE}" == "1" && "${CREATED_FLAG}" != "true" ]]; then
    echo "warning: selected alert had no ticket_link but endpoint returned created=false (possible concurrent run)"
  fi

  echo "alert ticket action validated created=${CREATED_FLAG} ticket=${CREATED_TICKET_NO} source_key=${TICKET_SOURCE_KEY}"
  return 0
}

stage_alert_ticket_reuse() {
  set_stage_context \
    "/api/v1/ops/cockpit/runbook-templates/analytics/alerts/tickets (POST repeated)" \
    "Call ticket action again and verify dedup returns created=false with same open ticket."

  if (( READ_ONLY_MODE == 1 )); then
    echo "read-only mode enabled, skipping alert ticket reuse write-path"
    return 2
  fi

  [[ -n "${ALERT_TEMPLATE_KEY}" ]] || {
    echo "alert template key is required before reuse stage"
    return 1
  }

  local payload
  payload="$(build_ticket_action_payload "qa v0.1.11 alert ticket reuse")"

  request_api POST "/api/v1/ops/cockpit/runbook-templates/analytics/alerts/tickets" "${payload}"

  if [[ "${LAST_STATUS}" == "403" ]]; then
    echo "alert ticket reuse forbidden for user=${AUTH_USER}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  fi

  status_is_success "${LAST_STATUS}" || {
    echo "alert ticket reuse call failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  local reuse_created
  reuse_created="$(jq -r '.created // empty' "${LAST_BODY_FILE}")"
  REUSED_TICKET_ID="$(jq -r '.ticket_link.ticket_id // empty' "${LAST_BODY_FILE}")"
  REUSED_TICKET_NO="$(jq -r '.ticket_link.ticket_no // empty' "${LAST_BODY_FILE}")"

  [[ "${reuse_created}" == "false" ]] || {
    echo "expected created=false on repeated call, got=${reuse_created}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  [[ "${REUSED_TICKET_ID}" =~ ^[0-9]+$ ]] || {
    echo "reuse response missing ticket_id: ${REUSED_TICKET_ID}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  [[ -n "${REUSED_TICKET_NO}" ]] || {
    echo "reuse response missing ticket_no"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  if [[ -n "${CREATED_TICKET_NO}" && "${REUSED_TICKET_NO}" != "${CREATED_TICKET_NO}" ]]; then
    echo "reused ticket mismatch: expected=${CREATED_TICKET_NO} actual=${REUSED_TICKET_NO}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  fi

  echo "alert ticket reuse validated ticket=${REUSED_TICKET_NO}"
  return 0
}

stage_ticket_records_verify() {
  set_stage_context \
    "/api/v1/ops/cockpit/runbook-templates/analytics/alerts + /api/v1/tickets" \
    "Verify alert-linked ticket is visible in alert feed and persisted in ticket records."

  if [[ -z "${ALERT_TEMPLATE_KEY}" ]]; then
    if (( READ_ONLY_MODE == 1 )); then
      echo "no alert template key available in read-only mode; skip record verification"
      return 2
    fi
    echo "alert template key is required before record verification"
    return 1
  fi

  request_api GET "/api/v1/ops/cockpit/runbook-templates/analytics/alerts?days=${ALERT_WINDOW_DAYS}&template_key=${ALERT_TEMPLATE_KEY}&limit=20"
  status_is_success "${LAST_STATUS}" || {
    echo "alerts verify read failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  local linked_ticket_no linked_ticket_id
  linked_ticket_no="$(jq -r --arg key "${ALERT_TEMPLATE_KEY}" '.items[]? | select(.template_key == $key) | .ticket_link.ticket_no // empty' "${LAST_BODY_FILE}" | head -n 1)"
  linked_ticket_id="$(jq -r --arg key "${ALERT_TEMPLATE_KEY}" '.items[]? | select(.template_key == $key) | .ticket_link.ticket_id // empty' "${LAST_BODY_FILE}" | head -n 1)"

  if [[ -z "${linked_ticket_no}" || -z "${linked_ticket_id}" ]]; then
    if (( READ_ONLY_MODE == 1 )); then
      echo "selected alert has no linked ticket in read-only mode; skip"
      return 2
    fi
    echo "selected alert has no linked ticket in alert feed"
    cat "${LAST_BODY_FILE}" || true
    return 1
  fi

  local expected_ticket_no
  expected_ticket_no="${REUSED_TICKET_NO:-${CREATED_TICKET_NO:-${linked_ticket_no}}}"

  [[ "${linked_ticket_no}" == "${expected_ticket_no}" ]] || {
    echo "alert feed ticket mismatch: expected=${expected_ticket_no} actual=${linked_ticket_no}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  request_api GET "/api/v1/tickets?query=${expected_ticket_no}&limit=20"
  status_is_success "${LAST_STATUS}" || {
    echo "ticket list lookup failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  local list_ticket_id list_ticket_category
  list_ticket_id="$(jq -r --arg no "${expected_ticket_no}" '.items[]? | select(.ticket_no == $no) | .id' "${LAST_BODY_FILE}" | head -n 1)"
  list_ticket_category="$(jq -r --arg no "${expected_ticket_no}" '.items[]? | select(.ticket_no == $no) | .category // empty' "${LAST_BODY_FILE}" | head -n 1)"

  [[ "${list_ticket_id}" =~ ^[0-9]+$ ]] || {
    echo "ticket ${expected_ticket_no} not found in /api/v1/tickets"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  [[ "${list_ticket_category}" == "runbook-risk" ]] || {
    echo "ticket category mismatch: expected=runbook-risk actual=${list_ticket_category}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  request_api GET "/api/v1/tickets/${list_ticket_id}"
  status_is_success "${LAST_STATUS}" || {
    echo "ticket detail lookup failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  local metadata_source metadata_source_key
  metadata_source="$(jq -r '.ticket.metadata.source // empty' "${LAST_BODY_FILE}")"
  metadata_source_key="$(jq -r '.ticket.metadata.source_key // empty' "${LAST_BODY_FILE}")"

  [[ "${metadata_source}" == "runbook_risk_alert" ]] || {
    echo "ticket metadata.source mismatch: ${metadata_source}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  if [[ -n "${TICKET_SOURCE_KEY}" && "${metadata_source_key}" != "${TICKET_SOURCE_KEY}" ]]; then
    echo "ticket metadata.source_key mismatch: expected=${TICKET_SOURCE_KEY} actual=${metadata_source_key}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  fi

  echo "ticket records verify passed ticket=${expected_ticket_no} id=${list_ticket_id}"
  return 0
}

stage_read_only_guard() {
  set_stage_context \
    "/api/v1/ops/cockpit/runbook-templates/analytics/alerts/tickets (POST)" \
    "Verify read-only user is denied by RBAC with HTTP 403 on ticket action endpoint."

  local guard_template guard_days payload
  guard_template="${ALERT_TEMPLATE_KEY:-dependency-check}"
  guard_days="$(normalize_days "${ALERT_WINDOW_DAYS}")"

  payload="$(jq -nc \
    --arg key "${guard_template}" \
    --argjson days "${guard_days}" \
    '{
      template_key: $key,
      execution_mode: null,
      days: $days,
      note: "qa read-only guard"
    }')"

  if (( READ_ONLY_MODE == 1 )); then
    GUARD_USER="${AUTH_USER}"
  else
    GUARD_USER="${READ_ONLY_AUTH_USER}"
  fi

  request_api_as_user "${GUARD_USER}" POST "/api/v1/ops/cockpit/runbook-templates/analytics/alerts/tickets" "${payload}"
  GUARD_STATUS="${LAST_STATUS}"

  [[ "${GUARD_STATUS}" == "403" ]] || {
    echo "expected read-only guard status=403, got=${GUARD_STATUS} (user=${GUARD_USER})"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  echo "read-only guard validated user=${GUARD_USER} status=${GUARD_STATUS}"
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
    --arg version "v0.1.11" \
    --arg run_id "${RUN_ID}" \
    --arg generated_at "${generated_at}" \
    --arg api_base_url "${API_BASE_URL}" \
    --arg auth_user "${AUTH_USER}" \
    --arg read_only_auth_user "${READ_ONLY_AUTH_USER}" \
    --argjson read_only "$( (( READ_ONLY_MODE == 1 )) && echo true || echo false )" \
    --arg overall "${overall}" \
    --arg analytics_policy_key "${ANALYTICS_POLICY_KEY}" \
    --arg analytics_threshold "${ANALYTICS_THRESHOLD}" \
    --arg analytics_sample "${ANALYTICS_SAMPLE}" \
    --arg alert_total "${ALERT_TOTAL}" \
    --arg alert_template_key "${ALERT_TEMPLATE_KEY}" \
    --arg alert_window_days "${ALERT_WINDOW_DAYS}" \
    --arg create_flag "${CREATED_FLAG}" \
    --arg created_ticket_id "${CREATED_TICKET_ID}" \
    --arg created_ticket_no "${CREATED_TICKET_NO}" \
    --arg reused_ticket_id "${REUSED_TICKET_ID}" \
    --arg reused_ticket_no "${REUSED_TICKET_NO}" \
    --arg ticket_source_key "${TICKET_SOURCE_KEY}" \
    --arg guard_user "${GUARD_USER}" \
    --arg guard_status "${GUARD_STATUS}" \
    --slurpfile stages "${TMP_STAGES}" \
    '{
      version: $version,
      run_id: $run_id,
      generated_at: $generated_at,
      api_base_url: $api_base_url,
      auth_user: $auth_user,
      read_only_auth_user: $read_only_auth_user,
      read_only_mode: $read_only,
      overall: $overall,
      entities: {
        analytics_policy_key: $analytics_policy_key,
        analytics_failure_rate_threshold_percent: $analytics_threshold,
        analytics_minimum_sample_size: $analytics_sample,
        alert_total: $alert_total,
        alert_template_key: $alert_template_key,
        alert_window_days: $alert_window_days,
        create_call_created: $create_flag,
        created_ticket_id: $created_ticket_id,
        created_ticket_no: $created_ticket_no,
        reused_ticket_id: $reused_ticket_id,
        reused_ticket_no: $reused_ticket_no,
        ticket_source_key: $ticket_source_key,
        read_only_guard_user: $guard_user,
        read_only_guard_status: $guard_status
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
    --arg version "v0.1.11" \
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
    echo "# v0.1.11 Operator Journey Validation Summary"
    echo
    echo "- Run ID: ${RUN_ID}"
    echo "- Generated at: ${generated_at}"
    echo "- API base URL: ${API_BASE_URL}"
    echo "- Auth user: ${AUTH_USER}"
    echo "- Read-only guard user: ${READ_ONLY_AUTH_USER}"
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
    echo "- analytics_policy_key=${ANALYTICS_POLICY_KEY:-none}"
    echo "- analytics_failure_rate_threshold_percent=${ANALYTICS_THRESHOLD:-none}"
    echo "- analytics_minimum_sample_size=${ANALYTICS_SAMPLE:-none}"
    echo "- alert_total=${ALERT_TOTAL:-none}"
    echo "- alert_template_key=${ALERT_TEMPLATE_KEY:-none}"
    echo "- alert_window_days=${ALERT_WINDOW_DAYS:-none}"
    echo "- create_call_created=${CREATED_FLAG:-none}"
    echo "- created_ticket_id=${CREATED_TICKET_ID:-none}"
    echo "- created_ticket_no=${CREATED_TICKET_NO:-none}"
    echo "- reused_ticket_id=${REUSED_TICKET_ID:-none}"
    echo "- reused_ticket_no=${REUSED_TICKET_NO:-none}"
    echo "- ticket_source_key=${TICKET_SOURCE_KEY:-none}"
    echo "- read_only_guard_user=${GUARD_USER:-none}"
    echo "- read_only_guard_status=${GUARD_STATUS:-none}"

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
      --read-only-user)
        READ_ONLY_AUTH_USER="$2"
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
      --alert-days)
        ALERT_WINDOW_DAYS="$2"
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

  ALERT_WINDOW_DAYS="$(normalize_days "${ALERT_WINDOW_DAYS}")"

  mkdir -p "${OUTPUT_DIR}"
  SUMMARY_JSON="${OUTPUT_DIR}/summary.json"
  SUMMARY_MD="${OUTPUT_DIR}/summary.md"
  ARTIFACT_INDEX_JSON="${OUTPUT_DIR}/artifact-index.json"

  TMP_STAGES="$(mktemp)"
  trap 'rm -f "${TMP_STAGES}" "${LAST_BODY_FILE:-}"' EXIT

  run_stage "policy_read" "runbook analytics policy read path" stage_policy_read
  run_stage "alerts_read" "runbook risk alerts read path" stage_alerts_read
  run_stage "alert_ticket_create" "risk alert ticket create path" stage_alert_ticket_create
  run_stage "alert_ticket_reuse" "risk alert ticket reuse dedup path" stage_alert_ticket_reuse
  run_stage "ticket_records_verify" "risk alert ticket records verify path" stage_ticket_records_verify
  run_stage "read_only_guard" "risk alert ticket read-only guard path" stage_read_only_guard

  write_summary
}

main "$@"
