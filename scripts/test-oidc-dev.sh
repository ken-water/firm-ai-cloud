#!/usr/bin/env bash
set -Eeuo pipefail

API_BASE_URL="${API_BASE_URL:-http://127.0.0.1:8080}"
STAMP="$(date +%s)"
LAST_BODY_FILE=""
LAST_STATUS=""

log() {
  echo "[oidc-dev] $*"
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
  local method="$1"
  local url="$2"
  local body="${3-}"
  local auth_header="${4-}"
  LAST_BODY_FILE="$(mktemp)"
  local -a args=(-sS -o "$LAST_BODY_FILE" -w "%{http_code}" -X "$method")

  if [[ -n "$auth_header" ]]; then
    args+=(-H "$auth_header")
  fi
  if [[ -n "$body" ]]; then
    args+=(-H 'Content-Type: application/json' -d "$body")
  fi
  LAST_STATUS="$(curl "${args[@]}" "$url")"
}

require_tool curl
require_tool sed

log "Health check"
curl -fsS "${API_BASE_URL}/health" >/dev/null

EMAIL="oidc-dev-${STAMP}@example.local"
SUB="oidc-sub-${STAMP}"

log "Create local user mapped by email"
USER_JSON="$(curl -fsS -X POST "${API_BASE_URL}/api/v1/iam/users" \
  -H 'x-auth-user: admin' \
  -H 'Content-Type: application/json' \
  -d "{\"username\":\"oidc-dev-${STAMP}\",\"display_name\":\"OIDC Dev ${STAMP}\",\"email\":\"${EMAIL}\",\"auth_source\":\"local\"}")"
USER_ID="$(echo "$USER_JSON" | extract_first_id)"
if [[ -z "$USER_ID" ]]; then
  echo "ERROR: failed to parse user id from create user response" >&2
  echo "$USER_JSON" >&2
  exit 1
fi

VIEWER_ROLE_ID="$(curl -fsS -H 'x-auth-user: admin' "${API_BASE_URL}/api/v1/iam/roles?role_key=viewer" | extract_first_id)"
if [[ -z "$VIEWER_ROLE_ID" ]]; then
  echo "ERROR: failed to parse viewer role id" >&2
  exit 1
fi

curl -fsS -X POST -H 'x-auth-user: admin' \
  "${API_BASE_URL}/api/v1/iam/users/${USER_ID}/roles/${VIEWER_ROLE_ID}" >/dev/null

log "Start OIDC login flow"
START_JSON="$(curl -fsS "${API_BASE_URL}/api/v1/auth/oidc/start?return_to=%2Fconsole")"
STATE_TOKEN="$(echo "$START_JSON" | sed -n 's/.*"state"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n1)"
if [[ -z "$STATE_TOKEN" ]]; then
  echo "ERROR: missing state in oidc start response (check AUTH_OIDC_* env settings)" >&2
  echo "$START_JSON" >&2
  exit 1
fi

log "Complete OIDC callback in dev mode"
CALLBACK_URL="${API_BASE_URL}/api/v1/auth/oidc/callback?code=dev::${SUB}::${EMAIL}::OIDC%20Dev%20User&state=${STATE_TOKEN}"
CALLBACK_JSON="$(curl -fsS "$CALLBACK_URL")"
ACCESS_TOKEN="$(echo "$CALLBACK_JSON" | sed -n 's/.*"access_token"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n1)"
if [[ -z "$ACCESS_TOKEN" ]]; then
  echo "ERROR: missing access_token in oidc callback response" >&2
  echo "$CALLBACK_JSON" >&2
  exit 1
fi

AUTHZ="Authorization: Bearer ${ACCESS_TOKEN}"

log "Invalid bearer token should be denied"
request_code GET "${API_BASE_URL}/api/v1/auth/me" "" "Authorization: Bearer invalid-token"
if [[ "$LAST_STATUS" != "403" ]]; then
  echo "ERROR: expected 403 for invalid bearer token, got ${LAST_STATUS}" >&2
  cat "$LAST_BODY_FILE" >&2 || true
  exit 1
fi

log "Bearer token should access protected read APIs"
request_code GET "${API_BASE_URL}/api/v1/cmdb/assets" "" "$AUTHZ"
if [[ "$LAST_STATUS" != "200" ]]; then
  echo "ERROR: expected 200 for cmdb assets read, got ${LAST_STATUS}" >&2
  cat "$LAST_BODY_FILE" >&2 || true
  exit 1
fi

log "Viewer role should still be denied on write APIs"
request_code POST "${API_BASE_URL}/api/v1/cmdb/assets" "{\"asset_class\":\"server\",\"name\":\"oidc-dev-write-${STAMP}\",\"status\":\"active\"}" "$AUTHZ"
if [[ "$LAST_STATUS" != "403" ]]; then
  echo "ERROR: expected 403 for cmdb assets write with viewer role, got ${LAST_STATUS}" >&2
  cat "$LAST_BODY_FILE" >&2 || true
  exit 1
fi

log "Logout should revoke current session token"
request_code POST "${API_BASE_URL}/api/v1/auth/logout" "" "$AUTHZ"
if [[ "$LAST_STATUS" != "200" ]]; then
  echo "ERROR: expected 200 on logout, got ${LAST_STATUS}" >&2
  cat "$LAST_BODY_FILE" >&2 || true
  exit 1
fi
grep -q '"revoked"[[:space:]]*:[[:space:]]*true' "$LAST_BODY_FILE" || {
  echo "ERROR: logout response does not confirm revoked session" >&2
  cat "$LAST_BODY_FILE" >&2 || true
  exit 1
}

log "Revoked token should no longer authenticate"
request_code GET "${API_BASE_URL}/api/v1/auth/me" "" "$AUTHZ"
if [[ "$LAST_STATUS" != "403" ]]; then
  echo "ERROR: expected 403 for revoked bearer token, got ${LAST_STATUS}" >&2
  cat "$LAST_BODY_FILE" >&2 || true
  exit 1
fi

log "OIDC dev-mode flow, bearer RBAC checks, and revocation checks passed"
