#!/usr/bin/env bash
set -Eeuo pipefail

API_BASE_URL="${API_BASE_URL:-http://127.0.0.1:8080}"
AUTH_USER="${AUTH_USER:-admin}"
USERNAME="${USERNAME:-admin}"
PASSWORD="${PASSWORD:-ChangeMe_12345}"
LOCKOUT_THRESHOLD="${LOCKOUT_THRESHOLD:-5}"

log() {
  printf '[local-auth-test] %s\n' "$*"
}

fatal() {
  printf '[local-auth-test][ERROR] %s\n' "$*" >&2
  exit 1
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || fatal "missing required command: $1"
}

api_auth_json() {
  curl -sS -H "x-auth-user: ${AUTH_USER}" "$@"
}

main() {
  require_cmd curl
  require_cmd jq
  [[ "${LOCKOUT_THRESHOLD}" =~ ^[0-9]+$ ]] || fatal "LOCKOUT_THRESHOLD must be a positive integer"
  (( LOCKOUT_THRESHOLD > 0 )) || fatal "LOCKOUT_THRESHOLD must be >= 1"

  log "Health check: ${API_BASE_URL}/health"
  curl -fsS "${API_BASE_URL}/health" >/dev/null || fatal "api health check failed"

  log "Setting local password for ${AUTH_USER}"
  api_auth_json -X POST \
    -H 'Content-Type: application/json' \
    -d "{\"new_password\":\"${PASSWORD}\"}" \
    "${API_BASE_URL}/api/v1/auth/local/password" \
    | jq -e '.password_updated == true' >/dev/null || fatal "failed to set local password"

  log "Triggering failed local logins to hit lockout threshold=${LOCKOUT_THRESHOLD}"
  for ((i = 1; i <= LOCKOUT_THRESHOLD; i++)); do
    status="$(curl -sS -o /tmp/local-auth-login-fail.json -w '%{http_code}' \
      -X POST \
      -H 'Content-Type: application/json' \
      -d "{\"username\":\"${USERNAME}\",\"password\":\"wrong-pass-${i}\"}" \
      "${API_BASE_URL}/api/v1/auth/local/login")"
    if [[ "${status}" != "403" ]]; then
      fatal "expected 403 on failed login attempt ${i}, got ${status}"
    fi
  done

  log "Verifying blocked login after lockout with correct password"
  status="$(curl -sS -o /tmp/local-auth-login-locked.json -w '%{http_code}' \
    -X POST \
    -H 'Content-Type: application/json' \
    -d "{\"username\":\"${USERNAME}\",\"password\":\"${PASSWORD}\"}" \
    "${API_BASE_URL}/api/v1/auth/local/login")"
  if [[ "${status}" != "403" ]]; then
    fatal "expected 403 for lockout-blocked login, got ${status}"
  fi

  message="$(jq -r '.error // ""' /tmp/local-auth-login-locked.json)"
  if [[ "${message}" != *"locked until"* ]]; then
    fatal "expected lockout message to include 'locked until', got: ${message}"
  fi

  log "Local auth hardening lockout validation passed."
}

main "$@"
