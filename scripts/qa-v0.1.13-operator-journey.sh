#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

RUN_ID="${RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)}"
OUTPUT_DIR="${OUTPUT_DIR:-.run/qa/v0.1.13-journey-${RUN_ID}}"
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

OWNER_COUNT=""
RULE_COUNT=""
ALERT_TEMPLATE_KEY="dependency-check"
CREATED_TICKET_NO=""
ROUTED_OWNER_KEY=""
ROUTED_OWNER_LABEL=""
READINESS_STATUS=""
READINESS_TEMPLATE_KEY=""
GUARD_USER=""
GUARD_STATUS=""

log() {
  printf '[qa-v0.1.13] %s\n' "$*"
}

fatal() {
  printf '[qa-v0.1.13][ERROR] %s\n' "$*" >&2
  exit 1
}

usage() {
  cat <<'USAGE'
Owner directory and notification readiness validation (v0.1.13)

Usage:
  bash scripts/qa-v0.1.13-operator-journey.sh [options]

Options:
  --api-base-url <url>       API base URL
  --auth-user <username>     Auth user header for normal stages (default: admin)
  --read-only-user <user>    Auth user used for read-only guard probe (default: viewer)
  --output-dir <path>        Output directory
  --run-id <id>              Run id
  --alert-days <days>        Alert window days (1-90, default: 14)
  --read-only                Skip write-path stages
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

seed_owner_directory_if_needed() {
  local payload
  payload="$(jq -nc '{
    items: [
      {
        owner_key: "dependency_owner",
        display_name: "Dependency Owner",
        owner_type: "team",
        owner_ref: "dependency-owner",
        notification_target: "ops@example.com",
        note: "qa v0.1.13 seeded owner",
        is_enabled: true
      }
    ]
  }')"
  request_api PUT "/api/v1/ops/cockpit/runbook-templates/analytics/owners" "${payload}"
  status_is_success "${LAST_STATUS}" || {
    echo "failed to seed owner directory, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
}

seed_routing_rules_if_needed() {
  local payload
  payload="$(jq -nc '{
    items: [
      {
        template_key: "dependency-check",
        execution_mode: null,
        severity: null,
        owner_key: "dependency_owner",
        priority: 100,
        note: "qa v0.1.13 seeded rule",
        is_enabled: true
      }
    ]
  }')"
  request_api PUT "/api/v1/ops/cockpit/runbook-templates/analytics/owner-routing-rules" "${payload}"
  status_is_success "${LAST_STATUS}" || {
    echo "failed to seed routing rules, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
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
        summary: "qa v0.1.13 seeded failure",
        ticket_ref: ("QA-RISK-" + $run_id),
        artifact_url: ("https://example.invalid/qa/" + $run_id + "/risk-failure")
      },
      note: "qa v0.1.13 seed risk alert"
    }')"
  request_api POST "/api/v1/ops/cockpit/runbook-templates/dependency-check/execute" "${payload}"
  if [[ "${LAST_STATUS}" == "403" ]]; then
    echo "seed execution forbidden"
    cat "${LAST_BODY_FILE}" || true
    return 2
  fi
  status_is_success "${LAST_STATUS}" || {
    echo "seed execution failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
}

stage_owner_directory_read() {
  set_stage_context \
    "/api/v1/ops/cockpit/runbook-templates/analytics/owners" \
    "Verify owner directory is readable and can be bootstrapped with minimal operator-facing owner config."

  request_api GET "/api/v1/ops/cockpit/runbook-templates/analytics/owners"
  status_is_success "${LAST_STATUS}" || {
    echo "owner directory read failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  OWNER_COUNT="$(jq -r '.total // 0' "${LAST_BODY_FILE}")"
  if [[ "${OWNER_COUNT}" == "0" && "${READ_ONLY_MODE}" == "0" ]]; then
    seed_owner_directory_if_needed || return 1
    request_api GET "/api/v1/ops/cockpit/runbook-templates/analytics/owners"
    status_is_success "${LAST_STATUS}" || return 1
    OWNER_COUNT="$(jq -r '.total // 0' "${LAST_BODY_FILE}")"
  fi

  [[ "${OWNER_COUNT}" =~ ^[0-9]+$ ]] || {
    echo "invalid owner count: ${OWNER_COUNT}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  [[ "${OWNER_COUNT}" != "0" ]] || {
    echo "owner directory is empty"
    return $(( READ_ONLY_MODE == 1 ? 2 : 1 ))
  }

  echo "owner directory validated count=${OWNER_COUNT}"
}

stage_routing_rules_read() {
  set_stage_context \
    "/api/v1/ops/cockpit/runbook-templates/analytics/owner-routing-rules" \
    "Verify routing rules are readable and can be bootstrapped for dependency-check."

  request_api GET "/api/v1/ops/cockpit/runbook-templates/analytics/owner-routing-rules"
  status_is_success "${LAST_STATUS}" || {
    echo "routing rules read failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  RULE_COUNT="$(jq -r '.total // 0' "${LAST_BODY_FILE}")"
  if [[ "${RULE_COUNT}" == "0" && "${READ_ONLY_MODE}" == "0" ]]; then
    seed_routing_rules_if_needed || return 1
    request_api GET "/api/v1/ops/cockpit/runbook-templates/analytics/owner-routing-rules"
    status_is_success "${LAST_STATUS}" || return 1
    RULE_COUNT="$(jq -r '.total // 0' "${LAST_BODY_FILE}")"
  fi

  [[ "${RULE_COUNT}" =~ ^[0-9]+$ ]] || {
    echo "invalid rule count: ${RULE_COUNT}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  [[ "${RULE_COUNT}" != "0" ]] || {
    echo "routing rules are empty"
    return $(( READ_ONLY_MODE == 1 ? 2 : 1 ))
  }

  echo "routing rules validated count=${RULE_COUNT}"
}

stage_routing_verify() {
  set_stage_context \
    "/api/v1/ops/cockpit/runbook-templates/analytics/alerts + /alerts/tickets" \
    "Verify configured owner route is returned by the risk-alert ticket action."

  request_api GET "/api/v1/ops/cockpit/runbook-templates/analytics/alerts?days=${ALERT_WINDOW_DAYS}&limit=20"
  status_is_success "${LAST_STATUS}" || {
    echo "risk alerts read failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  local alert_total
  alert_total="$(jq -r '.total // 0' "${LAST_BODY_FILE}")"
  if [[ "${alert_total}" == "0" && "${READ_ONLY_MODE}" == "0" ]]; then
    seed_dependency_check_failure_alert || return $?
    request_api GET "/api/v1/ops/cockpit/runbook-templates/analytics/alerts?days=${ALERT_WINDOW_DAYS}&limit=20"
    status_is_success "${LAST_STATUS}" || return 1
    alert_total="$(jq -r '.total // 0' "${LAST_BODY_FILE}")"
  fi
  [[ "${alert_total}" != "0" ]] || {
    echo "no risk alert available for routing verify"
    return $(( READ_ONLY_MODE == 1 ? 2 : 1 ))
  }

  ALERT_TEMPLATE_KEY="$(jq -r '.items[]? | select(.template_key == "dependency-check") | .template_key' "${LAST_BODY_FILE}" | head -n 1)"
  if [[ -z "${ALERT_TEMPLATE_KEY}" ]]; then
    ALERT_TEMPLATE_KEY="$(jq -r '.items[0].template_key // empty' "${LAST_BODY_FILE}")"
  fi
  [[ -n "${ALERT_TEMPLATE_KEY}" ]] || {
    echo "failed to select alert template"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  if (( READ_ONLY_MODE == 1 )); then
    local owner_key
    owner_key="$(jq -r --arg key "${ALERT_TEMPLATE_KEY}" '.items[]? | select(.template_key == $key) | .ticket_link.owner_route.owner_key // empty' "${LAST_BODY_FILE}" | head -n 1)"
    [[ -n "${owner_key}" ]] || {
      echo "read-only mode cannot verify owner route because no ticket_link.owner_route.owner_key is visible"
      return 2
    }
    ROUTED_OWNER_KEY="${owner_key}"
    echo "routing verify reused visible ticket owner_key=${ROUTED_OWNER_KEY}"
    return 0
  fi

  local payload
  payload="$(jq -nc --arg key "${ALERT_TEMPLATE_KEY}" --argjson days "${ALERT_WINDOW_DAYS}" '{
    template_key: $key,
    execution_mode: null,
    days: $days,
    note: "qa v0.1.13 routing verify"
  }')"
  request_api POST "/api/v1/ops/cockpit/runbook-templates/analytics/alerts/tickets" "${payload}"
  status_is_success "${LAST_STATUS}" || {
    echo "ticket action failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  CREATED_TICKET_NO="$(jq -r '.ticket_link.ticket_no // empty' "${LAST_BODY_FILE}")"
  ROUTED_OWNER_KEY="$(jq -r '.ticket_link.owner_route.owner_key // empty' "${LAST_BODY_FILE}")"
  ROUTED_OWNER_LABEL="$(jq -r '.ticket_link.owner_route.owner_label // empty' "${LAST_BODY_FILE}")"

  [[ "${ROUTED_OWNER_KEY}" == "dependency_owner" ]] || {
    echo "unexpected routed owner key: ${ROUTED_OWNER_KEY}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  echo "routing verified ticket=${CREATED_TICKET_NO} owner_key=${ROUTED_OWNER_KEY} owner_label=${ROUTED_OWNER_LABEL}"
}

stage_readiness_verify() {
  set_stage_context \
    "/api/v1/ops/cockpit/runbook-templates/analytics/owner-readiness" \
    "Verify owner readiness view exposes a deterministic state for the selected template."

  request_api GET "/api/v1/ops/cockpit/runbook-templates/analytics/owner-readiness?template_key=${ALERT_TEMPLATE_KEY}"
  status_is_success "${LAST_STATUS}" || {
    echo "owner readiness read failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  READINESS_TEMPLATE_KEY="$(jq -r '.items[0].template_key // empty' "${LAST_BODY_FILE}")"
  READINESS_STATUS="$(jq -r '.items[]? | select(.template_key == "'"${ALERT_TEMPLATE_KEY}"'") | .readiness_status // empty' "${LAST_BODY_FILE}" | head -n 1)"
  [[ -n "${READINESS_TEMPLATE_KEY}" ]] || {
    echo "readiness payload is empty"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  [[ -n "${READINESS_STATUS}" ]] || {
    echo "readiness status missing for template=${ALERT_TEMPLATE_KEY}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  case "${READINESS_STATUS}" in
    missing_routing_rule|missing_notification_template|missing_owner_directory|owner_disabled|missing_notification_target|missing_notification_channel|missing_notification_subscription|ready)
      ;;
    *)
      echo "unexpected readiness status=${READINESS_STATUS}"
      cat "${LAST_BODY_FILE}" || true
      return 1
      ;;
  esac

  echo "readiness verified template=${READINESS_TEMPLATE_KEY} status=${READINESS_STATUS}"
}

stage_cockpit_visibility_read() {
  set_stage_context \
    "/api/v1/ops/cockpit/runbook-templates/analytics/alerts" \
    "Verify cockpit-facing alert payload exposes configured owner labels and dispatch state."

  request_api GET "/api/v1/ops/cockpit/runbook-templates/analytics/alerts?days=${ALERT_WINDOW_DAYS}&template_key=${ALERT_TEMPLATE_KEY}&limit=20"
  status_is_success "${LAST_STATUS}" || {
    echo "cockpit visibility read failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  local owner_label dispatch_status
  owner_label="$(jq -r '.items[0].ticket_link.owner_route.owner_label // empty' "${LAST_BODY_FILE}")"
  dispatch_status="$(jq -r '.items[0].notification_summary.latest_status // empty' "${LAST_BODY_FILE}")"

  if [[ -z "${owner_label}" && "${READ_ONLY_MODE}" == "1" ]]; then
    echo "owner label not visible in read-only mode; skip cockpit visibility assertion"
    return 2
  fi
  [[ -n "${owner_label}" ]] || {
    echo "owner label missing from cockpit-facing alert payload"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  echo "cockpit visibility verified owner_label=${owner_label} notify_status=${dispatch_status:-none}"
}

stage_read_only_guard() {
  set_stage_context \
    "/api/v1/ops/cockpit/runbook-templates/analytics/owners (PUT)" \
    "Verify read-only user is denied by RBAC when trying to change owner config."

  local payload
  payload="$(jq -nc '{
    items: [
      {
        owner_key: "guard_owner",
        display_name: "Guard Owner",
        owner_type: "team",
        owner_ref: "guard-owner",
        notification_target: "guard@example.com",
        note: "qa guard",
        is_enabled: true
      }
    ]
  }')"

  if (( READ_ONLY_MODE == 1 )); then
    GUARD_USER="${AUTH_USER}"
  else
    GUARD_USER="${READ_ONLY_AUTH_USER}"
  fi

  request_api_as_user "${GUARD_USER}" PUT "/api/v1/ops/cockpit/runbook-templates/analytics/owners" "${payload}"
  GUARD_STATUS="${LAST_STATUS}"
  [[ "${GUARD_STATUS}" == "403" ]] || {
    echo "expected guard status=403, got=${GUARD_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  echo "read-only guard validated user=${GUARD_USER} status=${GUARD_STATUS}"
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
    --arg version "v0.1.13" \
    --arg run_id "${RUN_ID}" \
    --arg generated_at "${generated_at}" \
    --arg api_base_url "${API_BASE_URL}" \
    --arg auth_user "${AUTH_USER}" \
    --arg read_only_auth_user "${READ_ONLY_AUTH_USER}" \
    --argjson read_only "$( (( READ_ONLY_MODE == 1 )) && echo true || echo false )" \
    --arg overall "${overall}" \
    --arg owner_count "${OWNER_COUNT}" \
    --arg rule_count "${RULE_COUNT}" \
    --arg alert_template_key "${ALERT_TEMPLATE_KEY}" \
    --arg created_ticket_no "${CREATED_TICKET_NO}" \
    --arg routed_owner_key "${ROUTED_OWNER_KEY}" \
    --arg routed_owner_label "${ROUTED_OWNER_LABEL}" \
    --arg readiness_status "${READINESS_STATUS}" \
    --arg readiness_template_key "${READINESS_TEMPLATE_KEY}" \
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
        owner_count: $owner_count,
        rule_count: $rule_count,
        alert_template_key: $alert_template_key,
        created_ticket_no: $created_ticket_no,
        routed_owner_key: $routed_owner_key,
        routed_owner_label: $routed_owner_label,
        readiness_status: $readiness_status,
        readiness_template_key: $readiness_template_key,
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
    --arg version "v0.1.13" \
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
    echo "# v0.1.13 Operator Journey Validation Summary"
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
    echo "- owner_count=${OWNER_COUNT:-none}"
    echo "- rule_count=${RULE_COUNT:-none}"
    echo "- alert_template_key=${ALERT_TEMPLATE_KEY:-none}"
    echo "- created_ticket_no=${CREATED_TICKET_NO:-none}"
    echo "- routed_owner_key=${ROUTED_OWNER_KEY:-none}"
    echo "- routed_owner_label=${ROUTED_OWNER_LABEL:-none}"
    echo "- readiness_status=${READINESS_STATUS:-none}"
    echo "- readiness_template_key=${READINESS_TEMPLATE_KEY:-none}"
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
  TMP_STAGES="$(mktemp)"
  trap 'rm -f "${TMP_STAGES}" "${LAST_BODY_FILE:-}"' EXIT

  run_stage "owner_directory_read" "owner directory read path" stage_owner_directory_read
  run_stage "routing_rules_read" "owner routing rules read path" stage_routing_rules_read
  run_stage "routing_verify" "configured owner routing verify path" stage_routing_verify
  run_stage "readiness_verify" "owner readiness verify path" stage_readiness_verify
  run_stage "cockpit_visibility_read" "cockpit owner visibility read path" stage_cockpit_visibility_read
  run_stage "read_only_guard" "owner config read-only guard path" stage_read_only_guard

  write_summary
}

main "$@"
