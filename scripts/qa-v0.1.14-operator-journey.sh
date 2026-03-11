#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

RUN_ID="${RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)}"
OUTPUT_DIR="${OUTPUT_DIR:-.run/qa/v0.1.14-journey-${RUN_ID}}"
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
REPAIR_TEMPLATE_KEY="dependency-check"
REPAIR_OWNER_KEY="repair_owner_${RUN_ID//[^a-zA-Z0-9]/}"
REPAIR_OWNER_TARGET="runbook-risk-${RUN_ID//[^a-zA-Z0-9]/}@example.com"
REPAIR_ACTION_KEY=""
READINESS_STATUS=""
READINESS_TEMPLATE_KEY=""
GUARD_USER=""
GUARD_STATUS=""

log() {
  printf '[qa-v0.1.14] %s\n' "$*"
}

fatal() {
  printf '[qa-v0.1.14][ERROR] %s\n' "$*" >&2
  exit 1
}

usage() {
  cat <<'USAGE'
Owner readiness repair validation (v0.1.14)

Usage:
  bash scripts/qa-v0.1.14-operator-journey.sh [options]

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

ensure_repair_seed() {
  request_api GET "/api/v1/ops/cockpit/runbook-templates/analytics/owners"
  status_is_success "${LAST_STATUS}" || {
    echo "failed to read owners for repair seed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  OWNER_COUNT="$(jq -r '.total // 0' "${LAST_BODY_FILE}")"
  local owners_payload
  owners_payload="$(jq -c \
    --arg owner_key "${REPAIR_OWNER_KEY}" \
    --arg target "${REPAIR_OWNER_TARGET}" \
    '
      .items as $items
      | if any($items[]?; .owner_key == $owner_key) then
          {items: $items}
        else
          {items: ($items + [{
            owner_key: $owner_key,
            display_name: "Repair Owner",
            owner_type: "team",
            owner_ref: "qa-repair-owner",
            notification_target: $target,
            note: "qa v0.1.14 seeded repair owner",
            is_enabled: true
          }])}
        end
    ' "${LAST_BODY_FILE}")"
  request_api PUT "/api/v1/ops/cockpit/runbook-templates/analytics/owners" "${owners_payload}"
  status_is_success "${LAST_STATUS}" || {
    echo "failed to seed repair owner, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  request_api GET "/api/v1/ops/cockpit/runbook-templates/analytics/owner-routing-rules"
  status_is_success "${LAST_STATUS}" || {
    echo "failed to read routing rules for repair seed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  RULE_COUNT="$(jq -r '.total // 0' "${LAST_BODY_FILE}")"
  local rules_payload
  rules_payload="$(jq -c \
    --arg owner_key "${REPAIR_OWNER_KEY}" \
    --arg template_key "${REPAIR_TEMPLATE_KEY}" \
    '
      .items as $items
      | if any($items[]?; .template_key == $template_key and .owner_key == $owner_key and (.execution_mode == null) and (.severity == null)) then
          {items: $items}
        else
          {items: ($items + [{
            template_key: $template_key,
            execution_mode: null,
            severity: null,
            owner_key: $owner_key,
            priority: 100,
            note: "qa v0.1.14 seeded repair rule",
            is_enabled: true
          }])}
        end
    ' "${LAST_BODY_FILE}")"
  request_api PUT "/api/v1/ops/cockpit/runbook-templates/analytics/owner-routing-rules" "${rules_payload}"
  status_is_success "${LAST_STATUS}" || {
    echo "failed to seed repair routing rule, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
}

stage_repair_plan_read() {
  set_stage_context \
    "/api/v1/ops/cockpit/runbook-templates/analytics/owner-readiness/repair-plan" \
    "Verify owner readiness repair plan is readable and returns a deterministic repair action."

  if (( READ_ONLY_MODE == 0 )); then
    ensure_repair_seed || return 1
  fi

  request_api GET "/api/v1/ops/cockpit/runbook-templates/analytics/owner-readiness/repair-plan?template_key=${REPAIR_TEMPLATE_KEY}"
  status_is_success "${LAST_STATUS}" || {
    echo "repair plan read failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  OWNER_COUNT="$(jq -r '.total // 0' "${LAST_BODY_FILE}")"
  if (( READ_ONLY_MODE == 1 )); then
    local readable_count
    readable_count="$(jq -r '[.items[]? | select(.action.action_key != "notification.none")] | length' "${LAST_BODY_FILE}")"
    [[ "${readable_count}" != "0" ]] || {
      echo "no repairable plan item visible in read-only mode"
      return 2
    }
    echo "repair plan readable count=${readable_count}"
    return 0
  fi

  REPAIR_ACTION_KEY="$(jq -r \
    --arg template_key "${REPAIR_TEMPLATE_KEY}" \
    --arg owner_key "${REPAIR_OWNER_KEY}" \
    '.items[]? | select(.template_key == $template_key and .owner_key == $owner_key) | .action.action_key // empty' \
    "${LAST_BODY_FILE}" | head -n 1)"
  READINESS_STATUS="$(jq -r \
    --arg template_key "${REPAIR_TEMPLATE_KEY}" \
    --arg owner_key "${REPAIR_OWNER_KEY}" \
    '.items[]? | select(.template_key == $template_key and .owner_key == $owner_key) | .readiness_status // empty' \
    "${LAST_BODY_FILE}" | head -n 1)"

  [[ -n "${REPAIR_ACTION_KEY}" ]] || {
    echo "missing repair action for seeded owner"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  case "${REPAIR_ACTION_KEY}" in
    notification.bootstrap_full|notification.channel_bootstrap|notification.subscription_bootstrap)
      ;;
    *)
      echo "unexpected repair action key=${REPAIR_ACTION_KEY}"
      cat "${LAST_BODY_FILE}" || true
      return 1
      ;;
  esac

  echo "repair plan verified owner=${REPAIR_OWNER_KEY} status=${READINESS_STATUS} action=${REPAIR_ACTION_KEY}"
}

stage_repair_apply() {
  set_stage_context \
    "/api/v1/ops/cockpit/runbook-templates/analytics/owner-readiness/repair-actions" \
    "Verify one-click readiness repair can bootstrap the required notification resources."

  if (( READ_ONLY_MODE == 1 )); then
    echo "read-only mode skips repair apply"
    return 2
  fi

  local payload
  payload="$(jq -nc \
    --arg template_key "${REPAIR_TEMPLATE_KEY}" \
    --arg owner_key "${REPAIR_OWNER_KEY}" \
    '{template_key: $template_key, owner_key: $owner_key}')"
  request_api POST "/api/v1/ops/cockpit/runbook-templates/analytics/owner-readiness/repair-actions" "${payload}"
  status_is_success "${LAST_STATUS}" || {
    echo "repair apply failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  READINESS_STATUS="$(jq -r '.readiness_after.readiness_status // empty' "${LAST_BODY_FILE}")"
  [[ "${READINESS_STATUS}" == "ready" ]] || {
    echo "repair apply did not reach ready status: ${READINESS_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  local result_count
  result_count="$(jq -r '.results | length' "${LAST_BODY_FILE}")"
  [[ "${result_count}" != "0" ]] || {
    echo "repair apply returned no resource results"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  echo "repair apply verified owner=${REPAIR_OWNER_KEY} results=${result_count}"
}

stage_readiness_verify() {
  set_stage_context \
    "/api/v1/ops/cockpit/runbook-templates/analytics/owner-readiness" \
    "Verify owner readiness becomes ready after repair."

  request_api GET "/api/v1/ops/cockpit/runbook-templates/analytics/owner-readiness?template_key=${REPAIR_TEMPLATE_KEY}"
  status_is_success "${LAST_STATUS}" || {
    echo "owner readiness read failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  READINESS_TEMPLATE_KEY="$(jq -r '.items[0].template_key // empty' "${LAST_BODY_FILE}")"
  READINESS_STATUS="$(jq -r \
    --arg template_key "${REPAIR_TEMPLATE_KEY}" \
    --arg owner_key "${REPAIR_OWNER_KEY}" \
    '.items[]? | select(.template_key == $template_key and .owner_key == $owner_key) | .readiness_status // empty' \
    "${LAST_BODY_FILE}" | head -n 1)"
  [[ -n "${READINESS_TEMPLATE_KEY}" ]] || {
    echo "readiness payload is empty"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  [[ -n "${READINESS_STATUS}" ]] || {
    echo "readiness status missing for template=${REPAIR_TEMPLATE_KEY}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  [[ "${READINESS_STATUS}" == "ready" ]] || {
    echo "expected readiness status=ready, got=${READINESS_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  echo "readiness verified template=${READINESS_TEMPLATE_KEY} status=${READINESS_STATUS}"
}

stage_cockpit_visibility_read() {
  set_stage_context \
    "/api/v1/ops/cockpit/runbook-templates/analytics/owner-readiness/repair-plan" \
    "Verify cockpit-facing readiness payload exposes the post-repair no-action state."

  request_api GET "/api/v1/ops/cockpit/runbook-templates/analytics/owner-readiness/repair-plan?template_key=${REPAIR_TEMPLATE_KEY}"
  status_is_success "${LAST_STATUS}" || {
    echo "cockpit visibility read failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  local action_key owner_label
  action_key="$(jq -r \
    --arg template_key "${REPAIR_TEMPLATE_KEY}" \
    --arg owner_key "${REPAIR_OWNER_KEY}" \
    '.items[]? | select(.template_key == $template_key and .owner_key == $owner_key) | .action.action_key // empty' \
    "${LAST_BODY_FILE}" | head -n 1)"
  owner_label="$(jq -r \
    --arg template_key "${REPAIR_TEMPLATE_KEY}" \
    --arg owner_key "${REPAIR_OWNER_KEY}" \
    '.items[]? | select(.template_key == $template_key and .owner_key == $owner_key) | .owner_label // empty' \
    "${LAST_BODY_FILE}" | head -n 1)"

  [[ -n "${owner_label}" ]] || {
    echo "owner label missing from repair-plan payload"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  [[ "${action_key}" == "notification.none" ]] || {
    echo "expected post-repair action key=notification.none, got=${action_key}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  echo "cockpit visibility verified owner_label=${owner_label} action=${action_key}"
}

stage_repair_idempotency_verify() {
  set_stage_context \
    "/api/v1/ops/cockpit/runbook-templates/analytics/owner-readiness/repair-actions" \
    "Verify applying repair again is a safe no-op once readiness is already ready."

  if (( READ_ONLY_MODE == 1 )); then
    echo "read-only mode skips repair idempotency verify"
    return 2
  fi

  local payload
  payload="$(jq -nc \
    --arg template_key "${REPAIR_TEMPLATE_KEY}" \
    --arg owner_key "${REPAIR_OWNER_KEY}" \
    '{template_key: $template_key, owner_key: $owner_key}')"
  request_api POST "/api/v1/ops/cockpit/runbook-templates/analytics/owner-readiness/repair-actions" "${payload}"
  status_is_success "${LAST_STATUS}" || {
    echo "repair idempotency call failed, status=${LAST_STATUS}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  local action_key result_count after_status
  action_key="$(jq -r '.action.action_key // empty' "${LAST_BODY_FILE}")"
  result_count="$(jq -r '.results | length' "${LAST_BODY_FILE}")"
  after_status="$(jq -r '.readiness_after.readiness_status // empty' "${LAST_BODY_FILE}")"
  [[ "${action_key}" == "notification.none" ]] || {
    echo "expected idempotent action key=notification.none, got=${action_key}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  [[ "${result_count}" == "0" ]] || {
    echo "expected idempotent result count=0, got=${result_count}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }
  [[ "${after_status}" == "ready" ]] || {
    echo "expected idempotent readiness_after=ready, got=${after_status}"
    cat "${LAST_BODY_FILE}" || true
    return 1
  }

  echo "repair idempotency verified owner=${REPAIR_OWNER_KEY}"
}

stage_read_only_guard() {
  set_stage_context \
    "/api/v1/ops/cockpit/runbook-templates/analytics/owner-readiness/repair-actions (POST)" \
    "Verify read-only user is denied by RBAC when trying to apply readiness repair."

  local payload
  payload="$(jq -nc \
    --arg template_key "${REPAIR_TEMPLATE_KEY}" \
    --arg owner_key "${REPAIR_OWNER_KEY}" \
    '{template_key: $template_key, owner_key: $owner_key}')"

  if (( READ_ONLY_MODE == 1 )); then
    GUARD_USER="${AUTH_USER}"
  else
    GUARD_USER="${READ_ONLY_AUTH_USER}"
  fi

  request_api_as_user "${GUARD_USER}" POST "/api/v1/ops/cockpit/runbook-templates/analytics/owner-readiness/repair-actions" "${payload}"
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
    --arg version "v0.1.14" \
    --arg run_id "${RUN_ID}" \
    --arg generated_at "${generated_at}" \
    --arg api_base_url "${API_BASE_URL}" \
    --arg auth_user "${AUTH_USER}" \
    --arg read_only_auth_user "${READ_ONLY_AUTH_USER}" \
    --argjson read_only "$( (( READ_ONLY_MODE == 1 )) && echo true || echo false )" \
    --arg overall "${overall}" \
    --arg owner_count "${OWNER_COUNT}" \
    --arg rule_count "${RULE_COUNT}" \
    --arg repair_template_key "${REPAIR_TEMPLATE_KEY}" \
    --arg repair_owner_key "${REPAIR_OWNER_KEY}" \
    --arg repair_owner_target "${REPAIR_OWNER_TARGET}" \
    --arg repair_action_key "${REPAIR_ACTION_KEY}" \
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
        repair_template_key: $repair_template_key,
        repair_owner_key: $repair_owner_key,
        repair_owner_target: $repair_owner_target,
        repair_action_key: $repair_action_key,
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
    --arg version "v0.1.14" \
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
    echo "# v0.1.14 Operator Journey Validation Summary"
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
    echo "- repair_template_key=${REPAIR_TEMPLATE_KEY:-none}"
    echo "- repair_owner_key=${REPAIR_OWNER_KEY:-none}"
    echo "- repair_owner_target=${REPAIR_OWNER_TARGET:-none}"
    echo "- repair_action_key=${REPAIR_ACTION_KEY:-none}"
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

  run_stage "repair_plan_read" "owner readiness repair plan read path" stage_repair_plan_read
  run_stage "repair_apply" "owner readiness repair apply path" stage_repair_apply
  run_stage "readiness_verify" "owner readiness verify path" stage_readiness_verify
  run_stage "cockpit_visibility_read" "cockpit owner visibility read path" stage_cockpit_visibility_read
  run_stage "repair_idempotency_verify" "owner readiness repair idempotency path" stage_repair_idempotency_verify
  run_stage "read_only_guard" "owner config read-only guard path" stage_read_only_guard

  write_summary
}

main "$@"
