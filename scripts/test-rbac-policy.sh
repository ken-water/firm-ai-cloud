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
assert_code 200 "$OPERATOR_USER" GET "${API_BASE_URL}/api/v1/ops/cockpit/checklists"
assert_code 200 "$OPERATOR_USER" POST "${API_BASE_URL}/api/v1/ops/cockpit/checklists/daily-alert-queue-review/complete" \
  "{\"date\":\"$(date +%F)\",\"note\":\"operator completed checklist\"}"
assert_code 200 "$OPERATOR_USER" POST "${API_BASE_URL}/api/v1/ops/cockpit/checklists/daily-alert-queue-review/exception" \
  "{\"date\":\"$(date +%F)\",\"note\":\"operator deferred checklist by policy\",\"mark_skipped\":true}"
assert_code 200 "$OPERATOR_USER" GET "${API_BASE_URL}/api/v1/ops/cockpit/backup/policies"
assert_code 200 "$OPERATOR_USER" POST "${API_BASE_URL}/api/v1/ops/cockpit/backup/policies" \
  "{\"policy_key\":\"rbac-op-backup-${STAMP}\",\"name\":\"RBAC OP BACKUP ${STAMP}\",\"frequency\":\"daily\",\"schedule_time_utc\":\"01:30\",\"retention_days\":7,\"destination_type\":\"local\",\"destination_uri\":\"file:///tmp/rbac-op-backup-${STAMP}\",\"drill_enabled\":true,\"drill_frequency\":\"weekly\",\"drill_weekday\":3,\"drill_time_utc\":\"02:30\"}"
OPERATOR_BACKUP_POLICY_ID="$(cat "$LAST_BODY_FILE" | extract_first_id)"
if [[ -z "$OPERATOR_BACKUP_POLICY_ID" ]]; then
  echo "ERROR: failed to parse operator backup policy ID" >&2
  cat "$LAST_BODY_FILE" >&2 || true
  exit 1
fi
assert_code 200 "$OPERATOR_USER" PATCH "${API_BASE_URL}/api/v1/ops/cockpit/backup/policies/${OPERATOR_BACKUP_POLICY_ID}" \
  "{\"retention_days\":14,\"note\":\"rbac operator tuning\"}"
assert_code 200 "$OPERATOR_USER" POST "${API_BASE_URL}/api/v1/ops/cockpit/backup/policies/${OPERATOR_BACKUP_POLICY_ID}/run" \
  "{\"run_type\":\"backup\"}"
OPERATOR_BACKUP_RUN_ID="$(cat "$LAST_BODY_FILE" | extract_first_id)"
if [[ -z "$OPERATOR_BACKUP_RUN_ID" ]]; then
  echo "ERROR: failed to parse operator backup run ID" >&2
  cat "$LAST_BODY_FILE" >&2 || true
  exit 1
fi
assert_code 200 "$OPERATOR_USER" POST "${API_BASE_URL}/api/v1/ops/cockpit/backup/runs/${OPERATOR_BACKUP_RUN_ID}/restore-evidence" \
  "{\"ticket_ref\":\"TKT-RBAC-${STAMP}\",\"artifact_url\":\"https://example.invalid/rbac/${STAMP}/restore-proof\",\"note\":\"rbac restore evidence\",\"close_evidence\":false}"
OPERATOR_RESTORE_EVIDENCE_ID="$(cat "$LAST_BODY_FILE" | extract_first_id)"
if [[ -z "$OPERATOR_RESTORE_EVIDENCE_ID" ]]; then
  echo "ERROR: failed to parse operator restore evidence ID" >&2
  cat "$LAST_BODY_FILE" >&2 || true
  exit 1
fi
assert_code 200 "$OPERATOR_USER" PATCH "${API_BASE_URL}/api/v1/ops/cockpit/backup/restore-evidence/${OPERATOR_RESTORE_EVIDENCE_ID}" \
  "{\"note\":\"rbac restore evidence close\",\"close_evidence\":true}"
assert_code 200 "$OPERATOR_USER" POST "${API_BASE_URL}/api/v1/ops/cockpit/backup/scheduler/tick" \
  "{\"note\":\"rbac scheduler tick\"}"
assert_code 200 "$OPERATOR_USER" GET "${API_BASE_URL}/api/v1/ops/cockpit/backup/runs"
assert_code 200 "$OPERATOR_USER" GET "${API_BASE_URL}/api/v1/ops/cockpit/backup/restore-evidence"
assert_code 200 "$OPERATOR_USER" GET "${API_BASE_URL}/api/v1/ops/cockpit/weekly-digest"
assert_code 200 "$OPERATOR_USER" GET "${API_BASE_URL}/api/v1/ops/cockpit/weekly-digest/export?format=csv"
assert_code 200 "$OPERATOR_USER" GET "${API_BASE_URL}/api/v1/ops/cockpit/change-calendar?days=7"
assert_code 200 "$OPERATOR_USER" POST "${API_BASE_URL}/api/v1/ops/cockpit/change-calendar/conflicts" \
  "{\"start_at\":\"$(date -u +%Y-%m-%dT%H:%M:%SZ)\",\"end_at\":\"$(date -u -d '+30 minutes' +%Y-%m-%dT%H:%M:%SZ)\",\"operation_kind\":\"playbook.execute.restart-service-safe\",\"risk_level\":\"high\"}"
assert_code 200 "$OPERATOR_USER" POST "${API_BASE_URL}/api/v1/tickets" \
  "{\"title\":\"rbac-op-ticket-${STAMP}\",\"priority\":\"high\",\"category\":\"incident\",\"assignee\":\"oncall-a\"}"
OPERATOR_TICKET_ID="$(cat "$LAST_BODY_FILE" | extract_first_id)"
if [[ -z "$OPERATOR_TICKET_ID" ]]; then
  echo "ERROR: failed to parse operator ticket ID" >&2
  cat "$LAST_BODY_FILE" >&2 || true
  exit 1
fi
assert_code 200 "$OPERATOR_USER" GET "${API_BASE_URL}/api/v1/ops/cockpit/handover-digest"
assert_code 200 "$OPERATOR_USER" GET "${API_BASE_URL}/api/v1/ops/cockpit/handover-digest/export?format=csv"
assert_code 200 "$OPERATOR_USER" POST "${API_BASE_URL}/api/v1/ops/cockpit/handover-digest/items/ticket:${OPERATOR_TICKET_ID}/close" \
  "{\"shift_date\":\"$(date +%F)\",\"source_type\":\"ticket_backlog\",\"source_id\":${OPERATOR_TICKET_ID},\"next_owner\":\"${OPERATOR_USER}\",\"next_action\":\"handover close for rbac\"}"
assert_code 200 "$OPERATOR_USER" GET "${API_BASE_URL}/api/v1/cmdb/assets"
assert_code 200 "$OPERATOR_USER" POST "${API_BASE_URL}/api/v1/cmdb/assets" \
  "{\"asset_class\":\"server\",\"name\":\"rbac-op-asset-${STAMP}\",\"status\":\"active\"}"
assert_code 200 "$OPERATOR_USER" GET "${API_BASE_URL}/api/v1/tickets/escalation/policy"
assert_code 200 "$OPERATOR_USER" PUT "${API_BASE_URL}/api/v1/tickets/escalation/policy" \
  "{\"is_enabled\":true,\"near_high_minutes\":10,\"breach_high_minutes\":20,\"escalate_to_assignee\":\"ops-escalation-${STAMP}\",\"note\":\"rbac escalation policy update\"}"
assert_code 200 "$OPERATOR_USER" POST "${API_BASE_URL}/api/v1/tickets/escalation/policy/preview" \
  "{\"priority\":\"high\",\"status\":\"open\",\"ticket_age_minutes\":25,\"current_assignee\":\"oncall-a\"}"
assert_code 200 "$OPERATOR_USER" GET "${API_BASE_URL}/api/v1/tickets/escalation/queue"
assert_code 200 "$OPERATOR_USER" POST "${API_BASE_URL}/api/v1/tickets/escalation/run" \
  "{\"dry_run\":true,\"note\":\"rbac escalation run dry\"}"
assert_code 200 "$OPERATOR_USER" GET "${API_BASE_URL}/api/v1/tickets/escalation/actions?ticket_id=${OPERATOR_TICKET_ID}"
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
OPERATOR_ALERT_ID="$(cat "$LAST_BODY_FILE" | extract_first_id)"
assert_code 200 "$OPERATOR_USER" GET "${API_BASE_URL}/api/v1/alerts/policies"
assert_code 200 "$OPERATOR_USER" POST "${API_BASE_URL}/api/v1/alerts/policies/preview" \
  "{\"match_source\":\"monitoring_sync\",\"match_severity\":\"warning\",\"match_status\":\"open\",\"dedup_window_seconds\":1800}"
assert_code 200 "$OPERATOR_USER" GET "${API_BASE_URL}/api/v1/ops/cockpit/incidents"
if [[ -n "${OPERATOR_ALERT_ID}" ]]; then
  assert_code 200 "$OPERATOR_USER" GET "${API_BASE_URL}/api/v1/ops/cockpit/incidents/${OPERATOR_ALERT_ID}"
  assert_code 200 "$OPERATOR_USER" POST "${API_BASE_URL}/api/v1/ops/cockpit/incidents/${OPERATOR_ALERT_ID}/command" \
    "{\"status\":\"in_progress\",\"owner\":\"${OPERATOR_USER}\",\"eta_at\":\"$(date -u -d '+2 hour' +%Y-%m-%dT%H:%M:%SZ)\",\"summary\":\"rbac incident command update\"}"
else
  log "No alert available for operator incident command write-path check; skipped command mutation assertion"
fi
assert_code 200 "$OPERATOR_USER" GET "${API_BASE_URL}/api/v1/workflow/playbooks"
assert_code 200 "$OPERATOR_USER" GET "${API_BASE_URL}/api/v1/workflow/playbooks/policy"
assert_code 200 "$OPERATOR_USER" PUT "${API_BASE_URL}/api/v1/workflow/playbooks/policy" \
  "{\"timezone_name\":\"UTC\",\"change_freeze_enabled\":false,\"override_requires_reason\":true,\"maintenance_windows\":[{\"day_of_week\":1,\"start\":\"00:00\",\"end\":\"23:59\"},{\"day_of_week\":2,\"start\":\"00:00\",\"end\":\"23:59\"},{\"day_of_week\":3,\"start\":\"00:00\",\"end\":\"23:59\"},{\"day_of_week\":4,\"start\":\"00:00\",\"end\":\"23:59\"},{\"day_of_week\":5,\"start\":\"00:00\",\"end\":\"23:59\"},{\"day_of_week\":6,\"start\":\"00:00\",\"end\":\"23:59\"},{\"day_of_week\":7,\"start\":\"00:00\",\"end\":\"23:59\"}],\"note\":\"rbac policy update\"}"
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
assert_code 200 "$VIEWER_USER" GET "${API_BASE_URL}/api/v1/ops/cockpit/checklists"
assert_code 200 "$VIEWER_USER" GET "${API_BASE_URL}/api/v1/ops/cockpit/backup/policies"
assert_code 200 "$VIEWER_USER" GET "${API_BASE_URL}/api/v1/ops/cockpit/backup/runs"
assert_code 200 "$VIEWER_USER" GET "${API_BASE_URL}/api/v1/ops/cockpit/backup/restore-evidence"
assert_code 200 "$VIEWER_USER" GET "${API_BASE_URL}/api/v1/ops/cockpit/weekly-digest"
assert_code 200 "$VIEWER_USER" GET "${API_BASE_URL}/api/v1/ops/cockpit/weekly-digest/export?format=json"
assert_code 200 "$VIEWER_USER" GET "${API_BASE_URL}/api/v1/ops/cockpit/change-calendar?days=7"
assert_code 200 "$VIEWER_USER" GET "${API_BASE_URL}/api/v1/ops/cockpit/handover-digest"
assert_code 200 "$VIEWER_USER" GET "${API_BASE_URL}/api/v1/ops/cockpit/handover-digest/export?format=json"
assert_code 403 "$VIEWER_USER" POST "${API_BASE_URL}/api/v1/ops/cockpit/checklists/daily-alert-queue-review/complete" \
  "{\"date\":\"$(date +%F)\",\"note\":\"viewer should not update checklist\"}"
assert_code 403 "$VIEWER_USER" POST "${API_BASE_URL}/api/v1/ops/cockpit/checklists/daily-alert-queue-review/exception" \
  "{\"date\":\"$(date +%F)\",\"note\":\"viewer should not write checklist exception\",\"mark_skipped\":true}"
assert_code 403 "$VIEWER_USER" POST "${API_BASE_URL}/api/v1/ops/cockpit/backup/policies" \
  "{\"policy_key\":\"rbac-viewer-backup-${STAMP}\",\"name\":\"RBAC VIEWER BACKUP ${STAMP}\",\"frequency\":\"daily\",\"schedule_time_utc\":\"01:30\",\"retention_days\":7,\"destination_type\":\"local\",\"destination_uri\":\"file:///tmp/rbac-viewer-backup-${STAMP}\"}"
assert_code 403 "$VIEWER_USER" PATCH "${API_BASE_URL}/api/v1/ops/cockpit/backup/policies/${OPERATOR_BACKUP_POLICY_ID}" \
  "{\"retention_days\":21}"
assert_code 403 "$VIEWER_USER" POST "${API_BASE_URL}/api/v1/ops/cockpit/backup/policies/${OPERATOR_BACKUP_POLICY_ID}/run" \
  "{\"run_type\":\"backup\"}"
assert_code 403 "$VIEWER_USER" POST "${API_BASE_URL}/api/v1/ops/cockpit/backup/scheduler/tick" \
  "{\"note\":\"viewer should not trigger scheduler\"}"
assert_code 403 "$VIEWER_USER" POST "${API_BASE_URL}/api/v1/ops/cockpit/backup/runs/${OPERATOR_BACKUP_RUN_ID}/restore-evidence" \
  "{\"ticket_ref\":\"TKT-DENY-${STAMP}\",\"artifact_url\":\"https://example.invalid/deny\",\"note\":\"viewer should not write evidence\"}"
assert_code 403 "$VIEWER_USER" PATCH "${API_BASE_URL}/api/v1/ops/cockpit/backup/restore-evidence/${OPERATOR_RESTORE_EVIDENCE_ID}" \
  "{\"note\":\"viewer should not patch evidence\",\"close_evidence\":true}"
assert_code 403 "$VIEWER_USER" POST "${API_BASE_URL}/api/v1/ops/cockpit/change-calendar/conflicts" \
  "{\"start_at\":\"$(date -u +%Y-%m-%dT%H:%M:%SZ)\",\"end_at\":\"$(date -u -d '+30 minutes' +%Y-%m-%dT%H:%M:%SZ)\",\"operation_kind\":\"playbook.execute.restart-service-safe\",\"risk_level\":\"high\"}"
assert_code 403 "$VIEWER_USER" POST "${API_BASE_URL}/api/v1/ops/cockpit/handover-digest/items/ticket:${OPERATOR_TICKET_ID}/close" \
  "{\"shift_date\":\"$(date +%F)\",\"source_type\":\"ticket_backlog\",\"source_id\":${OPERATOR_TICKET_ID},\"next_owner\":\"${VIEWER_USER}\",\"next_action\":\"viewer should not close handover\"}"
assert_code 200 "$VIEWER_USER" GET "${API_BASE_URL}/api/v1/cmdb/assets"
assert_code 403 "$VIEWER_USER" POST "${API_BASE_URL}/api/v1/cmdb/assets" \
  "{\"asset_class\":\"server\",\"name\":\"rbac-viewer-asset-${STAMP}\",\"status\":\"active\"}"
assert_code 200 "$VIEWER_USER" GET "${API_BASE_URL}/api/v1/tickets/escalation/policy"
assert_code 200 "$VIEWER_USER" GET "${API_BASE_URL}/api/v1/tickets/escalation/queue"
assert_code 200 "$VIEWER_USER" GET "${API_BASE_URL}/api/v1/tickets/escalation/actions?ticket_id=${OPERATOR_TICKET_ID}"
assert_code 403 "$VIEWER_USER" PUT "${API_BASE_URL}/api/v1/tickets/escalation/policy" \
  "{\"near_high_minutes\":5,\"breach_high_minutes\":15,\"escalate_to_assignee\":\"viewer-deny\"}"
assert_code 403 "$VIEWER_USER" POST "${API_BASE_URL}/api/v1/tickets/escalation/policy/preview" \
  "{\"priority\":\"high\",\"status\":\"open\",\"ticket_age_minutes\":30}"
assert_code 403 "$VIEWER_USER" POST "${API_BASE_URL}/api/v1/tickets/escalation/run" \
  "{\"dry_run\":true,\"note\":\"viewer should not run\"}"
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
assert_code 403 "$VIEWER_USER" POST "${API_BASE_URL}/api/v1/alerts/policies/preview" \
  "{\"match_source\":\"monitoring_sync\",\"match_severity\":\"warning\",\"match_status\":\"open\",\"dedup_window_seconds\":1800}"
assert_code 200 "$VIEWER_USER" GET "${API_BASE_URL}/api/v1/ops/cockpit/incidents"
if [[ -n "${OPERATOR_ALERT_ID}" ]]; then
  assert_code 200 "$VIEWER_USER" GET "${API_BASE_URL}/api/v1/ops/cockpit/incidents/${OPERATOR_ALERT_ID}"
  assert_code 403 "$VIEWER_USER" POST "${API_BASE_URL}/api/v1/ops/cockpit/incidents/${OPERATOR_ALERT_ID}/command" \
    "{\"status\":\"blocked\",\"owner\":\"${VIEWER_USER}\",\"eta_at\":\"$(date -u -d '+2 hour' +%Y-%m-%dT%H:%M:%SZ)\",\"summary\":\"viewer should not update\"}"
else
  assert_code 403 "$VIEWER_USER" POST "${API_BASE_URL}/api/v1/ops/cockpit/incidents/999999/command" \
    "{\"status\":\"blocked\",\"owner\":\"${VIEWER_USER}\",\"eta_at\":\"$(date -u -d '+2 hour' +%Y-%m-%dT%H:%M:%SZ)\",\"summary\":\"viewer should not update\"}"
fi
assert_code 200 "$VIEWER_USER" GET "${API_BASE_URL}/api/v1/workflow/playbooks"
assert_code 200 "$VIEWER_USER" GET "${API_BASE_URL}/api/v1/workflow/playbooks/policy"
assert_code 403 "$VIEWER_USER" PUT "${API_BASE_URL}/api/v1/workflow/playbooks/policy" \
  "{\"timezone_name\":\"UTC\"}"
assert_code 403 "$VIEWER_USER" POST "${API_BASE_URL}/api/v1/workflow/playbooks/restart-service-safe/dry-run" \
  "{\"asset_ref\":\"rbac-viewer-asset-${STAMP}\",\"params\":{\"asset_ref\":\"rbac-viewer-asset-${STAMP}\",\"service_name\":\"nginx\",\"grace_seconds\":30}}"
assert_code 403 "$VIEWER_USER" POST "${API_BASE_URL}/api/v1/alerts/policies" \
  "{\"policy_key\":\"rbac-viewer-policy-${STAMP}\",\"name\":\"RBAC VIEWER ${STAMP}\",\"is_enabled\":true,\"match_source\":\"monitoring_sync\",\"match_severity\":\"warning\",\"dedup_window_seconds\":1800,\"ticket_priority\":\"high\",\"ticket_category\":\"incident\"}"

log "RBAC matrix and deny-by-default checks passed"
