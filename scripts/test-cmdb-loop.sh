#!/usr/bin/env bash
set -Eeuo pipefail

API_BASE_URL="${API_BASE_URL:-http://127.0.0.1:8080}"
AUTH_USER="${AUTH_USER:-admin}"
STAMP="$(date +%s)"

log() {
  echo "[cmdb-loop] $*"
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

require_tool curl

api_curl() {
  curl -fsS -H "x-auth-user: ${AUTH_USER}" "$@"
}

log "Health check"
curl -fsS "${API_BASE_URL}/health" >/dev/null

log "Create two CMDB assets"
ASSET_A_JSON="$(api_curl -X POST "${API_BASE_URL}/api/v1/cmdb/assets" \
  -H 'Content-Type: application/json' \
  -d "{\"asset_class\":\"server\",\"name\":\"it-a-${STAMP}\",\"hostname\":\"it-a-${STAMP}.local\",\"ip\":\"10.80.0.11\",\"status\":\"active\"}")"
ASSET_B_JSON="$(api_curl -X POST "${API_BASE_URL}/api/v1/cmdb/assets" \
  -H 'Content-Type: application/json' \
  -d "{\"asset_class\":\"server\",\"name\":\"it-b-${STAMP}\",\"hostname\":\"it-b-${STAMP}.local\",\"ip\":\"10.80.0.12\",\"status\":\"active\"}")"

ASSET_A_ID="$(echo "$ASSET_A_JSON" | extract_first_id)"
ASSET_B_ID="$(echo "$ASSET_B_JSON" | extract_first_id)"

if [[ -z "$ASSET_A_ID" || -z "$ASSET_B_ID" ]]; then
  echo "ERROR: failed to parse created asset IDs" >&2
  exit 1
fi

log "Create and validate relation"
REL_JSON="$(api_curl -X POST "${API_BASE_URL}/api/v1/cmdb/relations" \
  -H 'Content-Type: application/json' \
  -d "{\"src_asset_id\":${ASSET_A_ID},\"dst_asset_id\":${ASSET_B_ID},\"relation_type\":\"depends_on\",\"source\":\"manual\"}")"
REL_ID="$(echo "$REL_JSON" | extract_first_id)"
if [[ -z "$REL_ID" ]]; then
  echo "ERROR: failed to create relation" >&2
  exit 1
fi

REL_LIST="$(api_curl "${API_BASE_URL}/api/v1/cmdb/relations?asset_id=${ASSET_A_ID}")"
echo "$REL_LIST" | grep -q '"relation_type":"depends_on"' || {
  echo "ERROR: relation list does not contain expected relation" >&2
  exit 1
}

log "Create discovery job for approve:create path"
JOB_CREATE_JSON="$(api_curl -X POST "${API_BASE_URL}/api/v1/cmdb/discovery/jobs" \
  -H 'Content-Type: application/json' \
  -d "{\"name\":\"it-loop-${STAMP}\",\"source_type\":\"zabbix_hosts\",\"scope\":{\"offboarded_threshold\":2,\"mock_hosts\":[{\"name\":\"it-a-${STAMP}\",\"hostname\":\"it-a-${STAMP}.local\",\"ip\":\"10.80.0.11\",\"asset_class\":\"server\"},{\"name\":\"new-candidate-${STAMP}\",\"hostname\":\"new-candidate-${STAMP}.local\",\"ip\":\"10.80.0.21\",\"asset_class\":\"server\"}]}}")"
JOB_CREATE_ID="$(echo "$JOB_CREATE_JSON" | extract_first_id)"
if [[ -z "$JOB_CREATE_ID" ]]; then
  echo "ERROR: failed to create discovery job" >&2
  exit 1
fi

log "Run discovery job for approve:create"
RUN_JSON="$(api_curl -X POST "${API_BASE_URL}/api/v1/cmdb/discovery/jobs/${JOB_CREATE_ID}/run")"
echo "$RUN_JSON" | grep -q '"stats"' || {
  echo "ERROR: run response missing stats" >&2
  exit 1
}

log "Approve one pending candidate with strategy=create"
CANDIDATES_JSON="$(api_curl "${API_BASE_URL}/api/v1/cmdb/discovery/candidates?review_status=pending&limit=1")"
CANDIDATE_CREATE_ID="$(echo "$CANDIDATES_JSON" | extract_first_id)"
if [[ -z "$CANDIDATE_CREATE_ID" ]]; then
  echo "ERROR: no pending candidate available for approve:create" >&2
  exit 1
fi
APPROVE_CREATE_JSON="$(api_curl -X POST "${API_BASE_URL}/api/v1/cmdb/discovery/candidates/${CANDIDATE_CREATE_ID}/approve" \
  -H 'Content-Type: application/json' \
  -d '{"reviewed_by":"smoke-test","strategy":"create","reason":"integration-create"}')"
echo "$APPROVE_CREATE_JSON" | grep -q '"action":"approve:create"' || {
  echo "ERROR: candidate approve:create action is invalid" >&2
  exit 1
}

log "Create discovery job for approve:merge path"
JOB_MERGE_JSON="$(api_curl -X POST "${API_BASE_URL}/api/v1/cmdb/discovery/jobs" \
  -H 'Content-Type: application/json' \
  -d "{\"name\":\"it-loop-merge-${STAMP}\",\"source_type\":\"zabbix_hosts\",\"scope\":{\"offboarded_threshold\":2,\"mock_hosts\":[{\"name\":\"merge-candidate-${STAMP}\",\"hostname\":\"merge-candidate-${STAMP}.local\",\"ip\":\"10.80.0.31\",\"asset_class\":\"server\"}]}}")"
JOB_MERGE_ID="$(echo "$JOB_MERGE_JSON" | extract_first_id)"
if [[ -z "$JOB_MERGE_ID" ]]; then
  echo "ERROR: failed to create merge discovery job" >&2
  exit 1
fi

log "Run discovery job for approve:merge"
api_curl -X POST "${API_BASE_URL}/api/v1/cmdb/discovery/jobs/${JOB_MERGE_ID}/run" >/dev/null

log "Approve one pending candidate with strategy=merge"
CANDIDATES_JSON="$(api_curl "${API_BASE_URL}/api/v1/cmdb/discovery/candidates?review_status=pending&limit=1")"
CANDIDATE_MERGE_ID="$(echo "$CANDIDATES_JSON" | extract_first_id)"
if [[ -z "$CANDIDATE_MERGE_ID" ]]; then
  echo "ERROR: no pending candidate available for approve:merge" >&2
  exit 1
fi
APPROVE_MERGE_JSON="$(api_curl -X POST "${API_BASE_URL}/api/v1/cmdb/discovery/candidates/${CANDIDATE_MERGE_ID}/approve" \
  -H 'Content-Type: application/json' \
  -d "{\"reviewed_by\":\"smoke-test\",\"strategy\":\"merge\",\"target_asset_id\":${ASSET_B_ID},\"reason\":\"integration-merge\"}")"
echo "$APPROVE_MERGE_JSON" | grep -q '"action":"approve:merge"' || {
  echo "ERROR: candidate approve:merge action is invalid" >&2
  exit 1
}
echo "$APPROVE_MERGE_JSON" | grep -q "\"asset_id\":${ASSET_B_ID}" || {
  echo "ERROR: candidate approve:merge did not return expected target asset id" >&2
  exit 1
}

log "Query discovery events"
EVENTS_JSON="$(api_curl "${API_BASE_URL}/api/v1/cmdb/discovery/events?limit=20")"
echo "$EVENTS_JSON" | grep -q '"items"' || {
  echo "ERROR: event query failed" >&2
  exit 1
}

log "Prepare notification config and subscription"
CHANNEL_JSON="$(api_curl -X POST "${API_BASE_URL}/api/v1/cmdb/discovery/notification-channels" \
  -H 'Content-Type: application/json' \
  -d '{"name":"smoke-webhook","channel_type":"webhook","target":"http://127.0.0.1:65535/hook","config":{"max_attempts":2,"base_delay_ms":100}}')"
CHANNEL_ID="$(echo "$CHANNEL_JSON" | extract_first_id)"
if [[ -z "$CHANNEL_ID" ]]; then
  echo "ERROR: failed to create notification channel" >&2
  exit 1
fi

api_curl -X POST "${API_BASE_URL}/api/v1/cmdb/discovery/notification-templates" \
  -H 'Content-Type: application/json' \
  -d '{"event_type":"asset.new_detected","title_template":"Asset discovered","body_template":"Fingerprint {{fingerprint}}"}' >/dev/null

api_curl -X POST "${API_BASE_URL}/api/v1/cmdb/discovery/notification-subscriptions" \
  -H 'Content-Type: application/json' \
  -d "{\"channel_id\":${CHANNEL_ID},\"event_type\":\"asset.new_detected\"}" >/dev/null

log "Trigger discovery again to generate and dispatch event"
api_curl -X POST "${API_BASE_URL}/api/v1/cmdb/discovery/jobs/${JOB_CREATE_ID}/run" >/dev/null

log "Check delivery logs"
DELIVERIES_JSON="$(api_curl "${API_BASE_URL}/api/v1/cmdb/discovery/notification-deliveries?limit=20")"
echo "$DELIVERIES_JSON" | grep -q '"items"' || {
  echo "ERROR: delivery query failed" >&2
  exit 1
}

log "CMDB discovery integration smoke test passed"
