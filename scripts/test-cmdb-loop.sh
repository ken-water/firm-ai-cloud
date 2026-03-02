#!/usr/bin/env bash
set -Eeuo pipefail

API_BASE_URL="${API_BASE_URL:-http://127.0.0.1:8080}"
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

log "Health check"
curl -fsS "${API_BASE_URL}/health" >/dev/null

log "Create two CMDB assets"
ASSET_A_JSON="$(curl -fsS -X POST "${API_BASE_URL}/api/v1/cmdb/assets" \
  -H 'Content-Type: application/json' \
  -d "{\"asset_class\":\"server\",\"name\":\"it-a-${STAMP}\",\"hostname\":\"it-a-${STAMP}.local\",\"ip\":\"10.80.0.11\",\"status\":\"active\"}")"
ASSET_B_JSON="$(curl -fsS -X POST "${API_BASE_URL}/api/v1/cmdb/assets" \
  -H 'Content-Type: application/json' \
  -d "{\"asset_class\":\"server\",\"name\":\"it-b-${STAMP}\",\"hostname\":\"it-b-${STAMP}.local\",\"ip\":\"10.80.0.12\",\"status\":\"active\"}")"

ASSET_A_ID="$(echo "$ASSET_A_JSON" | extract_first_id)"
ASSET_B_ID="$(echo "$ASSET_B_JSON" | extract_first_id)"

if [[ -z "$ASSET_A_ID" || -z "$ASSET_B_ID" ]]; then
  echo "ERROR: failed to parse created asset IDs" >&2
  exit 1
fi

log "Create and validate relation"
REL_JSON="$(curl -fsS -X POST "${API_BASE_URL}/api/v1/cmdb/relations" \
  -H 'Content-Type: application/json' \
  -d "{\"src_asset_id\":${ASSET_A_ID},\"dst_asset_id\":${ASSET_B_ID},\"relation_type\":\"depends_on\",\"source\":\"manual\"}")"
REL_ID="$(echo "$REL_JSON" | extract_first_id)"
if [[ -z "$REL_ID" ]]; then
  echo "ERROR: failed to create relation" >&2
  exit 1
fi

REL_LIST="$(curl -fsS "${API_BASE_URL}/api/v1/cmdb/relations?asset_id=${ASSET_A_ID}")"
echo "$REL_LIST" | grep -q '"relation_type":"depends_on"' || {
  echo "ERROR: relation list does not contain expected relation" >&2
  exit 1
}

log "Create discovery job with mock hosts"
JOB_JSON="$(curl -fsS -X POST "${API_BASE_URL}/api/v1/cmdb/discovery/jobs" \
  -H 'Content-Type: application/json' \
  -d "{\"name\":\"it-loop-${STAMP}\",\"source_type\":\"zabbix_hosts\",\"scope\":{\"offboarded_threshold\":2,\"mock_hosts\":[{\"name\":\"it-a-${STAMP}\",\"hostname\":\"it-a-${STAMP}.local\",\"ip\":\"10.80.0.11\",\"asset_class\":\"server\"},{\"name\":\"new-candidate-${STAMP}\",\"hostname\":\"new-candidate-${STAMP}.local\",\"ip\":\"10.80.0.21\",\"asset_class\":\"server\"}]}}")"
JOB_ID="$(echo "$JOB_JSON" | extract_first_id)"
if [[ -z "$JOB_ID" ]]; then
  echo "ERROR: failed to create discovery job" >&2
  exit 1
fi

log "Run discovery job"
RUN_JSON="$(curl -fsS -X POST "${API_BASE_URL}/api/v1/cmdb/discovery/jobs/${JOB_ID}/run")"
echo "$RUN_JSON" | grep -q '"stats"' || {
  echo "ERROR: run response missing stats" >&2
  exit 1
}

log "Review one pending candidate"
CANDIDATES_JSON="$(curl -fsS "${API_BASE_URL}/api/v1/cmdb/discovery/candidates?review_status=pending&limit=20")"
CANDIDATE_ID="$(echo "$CANDIDATES_JSON" | extract_first_id)"
if [[ -n "$CANDIDATE_ID" ]]; then
  APPROVE_JSON="$(curl -fsS -X POST "${API_BASE_URL}/api/v1/cmdb/discovery/candidates/${CANDIDATE_ID}/approve" \
    -H 'Content-Type: application/json' \
    -d '{"reviewed_by":"smoke-test"}')"
  echo "$APPROVE_JSON" | grep -Eq '"action":"(created|merged)"' || {
    echo "ERROR: candidate approve action is invalid" >&2
    exit 1
  }
fi

log "Query discovery events"
EVENTS_JSON="$(curl -fsS "${API_BASE_URL}/api/v1/cmdb/discovery/events?limit=20")"
echo "$EVENTS_JSON" | grep -q '"items"' || {
  echo "ERROR: event query failed" >&2
  exit 1
}

log "Prepare notification config and subscription"
CHANNEL_JSON="$(curl -fsS -X POST "${API_BASE_URL}/api/v1/cmdb/discovery/notification-channels" \
  -H 'Content-Type: application/json' \
  -d '{"name":"smoke-webhook","channel_type":"webhook","target":"http://127.0.0.1:65535/hook","config":{"max_attempts":2,"base_delay_ms":100}}')"
CHANNEL_ID="$(echo "$CHANNEL_JSON" | extract_first_id)"
if [[ -z "$CHANNEL_ID" ]]; then
  echo "ERROR: failed to create notification channel" >&2
  exit 1
fi

curl -fsS -X POST "${API_BASE_URL}/api/v1/cmdb/discovery/notification-templates" \
  -H 'Content-Type: application/json' \
  -d '{"event_type":"asset.new_detected","title_template":"Asset discovered","body_template":"Fingerprint {{fingerprint}}"}' >/dev/null

curl -fsS -X POST "${API_BASE_URL}/api/v1/cmdb/discovery/notification-subscriptions" \
  -H 'Content-Type: application/json' \
  -d "{\"channel_id\":${CHANNEL_ID},\"event_type\":\"asset.new_detected\"}" >/dev/null

log "Trigger discovery again to generate and dispatch event"
curl -fsS -X POST "${API_BASE_URL}/api/v1/cmdb/discovery/jobs/${JOB_ID}/run" >/dev/null

log "Check delivery logs"
DELIVERIES_JSON="$(curl -fsS "${API_BASE_URL}/api/v1/cmdb/discovery/notification-deliveries?limit=20")"
echo "$DELIVERIES_JSON" | grep -q '"items"' || {
  echo "ERROR: delivery query failed" >&2
  exit 1
}

log "CMDB discovery integration smoke test passed"
