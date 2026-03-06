#!/usr/bin/env bash
set -Eeuo pipefail

API_BASE_URL="${API_BASE_URL:-http://127.0.0.1:8080}"
STAMP="$(date +%s)"
LAST_BODY_FILE=""
LAST_STATUS=""

log() {
  echo "[rbac-policy] $*"
}

require_tool() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "ERROR: required command not found: $1" >&2
    exit 1
  fi
}

extract_first_id() {
  sed -n 's/.*"id"[[:space:]]*:[[:space:]]*\([0-9][0-9]*\).*/\1/p' | head -n1
}

request_code() {
  local user="$1"
  local method="$2"
  local url="$3"
  local body="${4-}"

  LAST_BODY_FILE="$(mktemp)"
  local -a args=(-sS -o "$LAST_BODY_FILE" -w "%{http_code}" -X "$method")

  if [[ -n "$user" ]]; then
    args+=(-H "x-auth-user: $user")
  fi

  if [[ -n "$body" ]]; then
    args+=(-H "Content-Type: application/json" -d "$body")
  fi

  LAST_STATUS="$(curl "${args[@]}" "$url")"
}

assert_code() {
  local expected="$1"
  local user="$2"
  local method="$3"
  local url="$4"
  local body="${5-}"

  request_code "$user" "$method" "$url" "$body"
  if [[ "$LAST_STATUS" != "$expected" ]]; then
    echo "ERROR: expected HTTP $expected but got $LAST_STATUS for $method $url (user=$user)" >&2
    cat "$LAST_BODY_FILE" >&2 || true
    exit 1
  fi
}

require_tool curl

log "Health check"
curl -fsS "${API_BASE_URL}/health" >/dev/null

log "Public endpoint should remain accessible"
assert_code 200 "" GET "${API_BASE_URL}/health"

log "Protected endpoints should deny missing auth header"
assert_code 403 "" GET "${API_BASE_URL}/api/v1/cmdb/assets"
grep -q "header or bearer token is required" "$LAST_BODY_FILE" || {
  echo "ERROR: missing-header response is not consistent English message" >&2
  cat "$LAST_BODY_FILE" >&2 || true
  exit 1
}

assert_code 403 "" GET "${API_BASE_URL}/api/v1/iam/users"
assert_code 403 "" GET "${API_BASE_URL}/api/v1/audit/logs"

log "Create operator and viewer users"
OPERATOR_JSON="$(curl -fsS -X POST "${API_BASE_URL}/api/v1/iam/users" \
  -H "x-auth-user: admin" \
  -H 'Content-Type: application/json' \
  -d "{\"username\":\"rbac-operator-${STAMP}\",\"auth_source\":\"local\"}")"
VIEWER_JSON="$(curl -fsS -X POST "${API_BASE_URL}/api/v1/iam/users" \
  -H "x-auth-user: admin" \
  -H 'Content-Type: application/json' \
  -d "{\"username\":\"rbac-viewer-${STAMP}\",\"auth_source\":\"local\"}")"

OPERATOR_USER_ID="$(echo "$OPERATOR_JSON" | extract_first_id)"
VIEWER_USER_ID="$(echo "$VIEWER_JSON" | extract_first_id)"
if [[ -z "$OPERATOR_USER_ID" || -z "$VIEWER_USER_ID" ]]; then
  echo "ERROR: failed to parse operator/viewer user IDs" >&2
  exit 1
fi

OPERATOR_ROLE_ID="$(curl -fsS -H "x-auth-user: admin" "${API_BASE_URL}/api/v1/iam/roles?role_key=operator" | extract_first_id)"
VIEWER_ROLE_ID="$(curl -fsS -H "x-auth-user: admin" "${API_BASE_URL}/api/v1/iam/roles?role_key=viewer" | extract_first_id)"
if [[ -z "$OPERATOR_ROLE_ID" || -z "$VIEWER_ROLE_ID" ]]; then
  echo "ERROR: failed to parse operator/viewer role IDs" >&2
  exit 1
fi

log "Bind role matrix for test users"
curl -fsS -X POST -H "x-auth-user: admin" \
  "${API_BASE_URL}/api/v1/iam/users/${OPERATOR_USER_ID}/roles/${OPERATOR_ROLE_ID}" >/dev/null
curl -fsS -X POST -H "x-auth-user: admin" \
  "${API_BASE_URL}/api/v1/iam/users/${VIEWER_USER_ID}/roles/${VIEWER_ROLE_ID}" >/dev/null

OPERATOR_USER="rbac-operator-${STAMP}"
VIEWER_USER="rbac-viewer-${STAMP}"

log "Validate operator permission matrix"
assert_code 200 "$OPERATOR_USER" GET "${API_BASE_URL}/api/v1/setup/preflight"
assert_code 200 "$OPERATOR_USER" GET "${API_BASE_URL}/api/v1/setup/checklist"
assert_code 200 "$OPERATOR_USER" GET "${API_BASE_URL}/api/v1/setup/templates"
assert_code 200 "$OPERATOR_USER" POST "${API_BASE_URL}/api/v1/setup/templates/identity-safe-baseline/preview" \
  "{\"params\":{\"identity_mode\":\"break_glass_only\",\"break_glass_users\":\"${OPERATOR_USER}\"}}"
assert_code 200 "$OPERATOR_USER" POST "${API_BASE_URL}/api/v1/setup/templates/identity-safe-baseline/apply" \
  "{\"params\":{\"identity_mode\":\"break_glass_only\",\"break_glass_users\":\"${OPERATOR_USER}\"},\"note\":\"rbac apply\"}"
assert_code 200 "$OPERATOR_USER" GET "${API_BASE_URL}/api/v1/ops/cockpit/queue"
assert_code 200 "$OPERATOR_USER" GET "${API_BASE_URL}/api/v1/cmdb/assets"
assert_code 200 "$OPERATOR_USER" POST "${API_BASE_URL}/api/v1/cmdb/assets" \
  "{\"asset_class\":\"server\",\"name\":\"rbac-op-asset-${STAMP}\",\"status\":\"active\"}"
assert_code 200 "$OPERATOR_USER" GET "${API_BASE_URL}/api/v1/monitoring/sources"
assert_code 200 "$OPERATOR_USER" GET "${API_BASE_URL}/api/v1/monitoring/overview"
assert_code 200 "$OPERATOR_USER" GET "${API_BASE_URL}/api/v1/monitoring/layers/hardware"
assert_code 200 "$OPERATOR_USER" GET "${API_BASE_URL}/api/v1/topology/maps/site:dc-a"
assert_code 200 "$OPERATOR_USER" POST "${API_BASE_URL}/api/v1/monitoring/sources" \
  "{\"name\":\"rbac-op-monitor-${STAMP}\",\"source_type\":\"zabbix\",\"endpoint\":\"http://127.0.0.1:8080/health\",\"secret_ref\":\"dev/rbac-op-monitor-${STAMP}\"}"
OPERATOR_MONITOR_SOURCE_ID="$(cat "$LAST_BODY_FILE" | extract_first_id)"
if [[ -z "$OPERATOR_MONITOR_SOURCE_ID" ]]; then
  echo "ERROR: failed to parse operator monitoring source ID" >&2
  cat "$LAST_BODY_FILE" >&2 || true
  exit 1
fi
assert_code 200 "$OPERATOR_USER" POST "${API_BASE_URL}/api/v1/monitoring/sources/${OPERATOR_MONITOR_SOURCE_ID}/probe"
assert_code 200 "$OPERATOR_USER" GET "${API_BASE_URL}/api/v1/alerts"
assert_code 200 "$OPERATOR_USER" GET "${API_BASE_URL}/api/v1/alerts/policies"
assert_code 200 "$OPERATOR_USER" GET "${API_BASE_URL}/api/v1/workflow/playbooks"
assert_code 200 "$OPERATOR_USER" POST "${API_BASE_URL}/api/v1/workflow/playbooks/restart-service-safe/dry-run" \
  "{\"asset_ref\":\"rbac-op-asset-${STAMP}\",\"params\":{\"asset_ref\":\"rbac-op-asset-${STAMP}\",\"service_name\":\"nginx\",\"grace_seconds\":30}}"
assert_code 200 "$OPERATOR_USER" POST "${API_BASE_URL}/api/v1/alerts/policies" \
  "{\"policy_key\":\"rbac-op-policy-${STAMP}\",\"name\":\"RBAC OP ${STAMP}\",\"is_enabled\":true,\"match_source\":\"monitoring_sync\",\"match_severity\":\"warning\",\"dedup_window_seconds\":1800,\"ticket_priority\":\"high\",\"ticket_category\":\"incident\"}"
assert_code 403 "$OPERATOR_USER" GET "${API_BASE_URL}/api/v1/iam/users"
grep -q "permission denied" "$LAST_BODY_FILE" || {
  echo "ERROR: operator forbidden response is not expected English message" >&2
  cat "$LAST_BODY_FILE" >&2 || true
  exit 1
}
assert_code 403 "$OPERATOR_USER" GET "${API_BASE_URL}/api/v1/audit/logs"

log "Validate viewer permission matrix"
assert_code 200 "$VIEWER_USER" GET "${API_BASE_URL}/api/v1/setup/preflight"
assert_code 200 "$VIEWER_USER" GET "${API_BASE_URL}/api/v1/setup/checklist"
assert_code 200 "$VIEWER_USER" GET "${API_BASE_URL}/api/v1/setup/templates"
assert_code 403 "$VIEWER_USER" POST "${API_BASE_URL}/api/v1/setup/templates/identity-safe-baseline/preview" \
  "{\"params\":{\"identity_mode\":\"break_glass_only\",\"break_glass_users\":\"${VIEWER_USER}\"}}"
assert_code 200 "$VIEWER_USER" GET "${API_BASE_URL}/api/v1/ops/cockpit/queue"
assert_code 200 "$VIEWER_USER" GET "${API_BASE_URL}/api/v1/cmdb/assets"
assert_code 403 "$VIEWER_USER" POST "${API_BASE_URL}/api/v1/cmdb/assets" \
  "{\"asset_class\":\"server\",\"name\":\"rbac-viewer-asset-${STAMP}\",\"status\":\"active\"}"
assert_code 200 "$VIEWER_USER" GET "${API_BASE_URL}/api/v1/monitoring/sources"
assert_code 200 "$VIEWER_USER" GET "${API_BASE_URL}/api/v1/monitoring/overview"
assert_code 200 "$VIEWER_USER" GET "${API_BASE_URL}/api/v1/monitoring/layers/service"
assert_code 200 "$VIEWER_USER" GET "${API_BASE_URL}/api/v1/topology/maps/site:dc-a"
assert_code 403 "$VIEWER_USER" GET "${API_BASE_URL}/api/v1/topology/maps/global"
assert_code 403 "$VIEWER_USER" POST "${API_BASE_URL}/api/v1/monitoring/sources" \
  "{\"name\":\"rbac-viewer-monitor-${STAMP}\",\"source_type\":\"zabbix\",\"endpoint\":\"http://127.0.0.1:8080/health\",\"secret_ref\":\"dev/rbac-viewer-monitor-${STAMP}\"}"
assert_code 200 "$VIEWER_USER" GET "${API_BASE_URL}/api/v1/cmdb/discovery/jobs"
assert_code 403 "$VIEWER_USER" POST "${API_BASE_URL}/api/v1/cmdb/discovery/jobs" \
  "{\"name\":\"rbac-viewer-job-${STAMP}\",\"source_type\":\"mock_hosts\",\"scope\":{}}"
assert_code 200 "$VIEWER_USER" GET "${API_BASE_URL}/api/v1/cmdb/discovery/notification-channels"
assert_code 403 "$VIEWER_USER" POST "${API_BASE_URL}/api/v1/cmdb/discovery/notification-channels" \
  "{\"name\":\"rbac-viewer-channel\",\"channel_type\":\"webhook\",\"target\":\"http://127.0.0.1:65535/h\"}"
assert_code 200 "$VIEWER_USER" GET "${API_BASE_URL}/api/v1/alerts"
assert_code 200 "$VIEWER_USER" GET "${API_BASE_URL}/api/v1/alerts/policies"
assert_code 200 "$VIEWER_USER" GET "${API_BASE_URL}/api/v1/workflow/playbooks"
assert_code 403 "$VIEWER_USER" POST "${API_BASE_URL}/api/v1/workflow/playbooks/restart-service-safe/dry-run" \
  "{\"asset_ref\":\"rbac-viewer-asset-${STAMP}\",\"params\":{\"asset_ref\":\"rbac-viewer-asset-${STAMP}\",\"service_name\":\"nginx\",\"grace_seconds\":30}}"
assert_code 403 "$VIEWER_USER" POST "${API_BASE_URL}/api/v1/alerts/policies" \
  "{\"policy_key\":\"rbac-viewer-policy-${STAMP}\",\"name\":\"RBAC VIEWER ${STAMP}\",\"is_enabled\":true,\"match_source\":\"monitoring_sync\",\"match_severity\":\"warning\",\"dedup_window_seconds\":1800,\"ticket_priority\":\"high\",\"ticket_category\":\"incident\"}"

log "RBAC matrix and deny-by-default checks passed"
