#!/usr/bin/env bash
set -Eeuo pipefail

API_BASE_URL="${API_BASE_URL:-http://127.0.0.1:8080}"
AUTH_USER="${AUTH_USER:-admin}"
AUTH_BEARER_TOKEN="${AUTH_BEARER_TOKEN:-}"
USERNAME="${USERNAME:-admin}"
PASSWORD="${PASSWORD:-ChangeMe_12345}"
LOCKOUT_THRESHOLD="${LOCKOUT_THRESHOLD:-5}"
TMP_FAIL_BODY=""
TMP_LOCKED_BODY=""

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

auth_header() {
  if [[ -n "${AUTH_BEARER_TOKEN}" ]]; then
    printf 'Authorization: Bearer %s' "${AUTH_BEARER_TOKEN}"
  else
    printf 'x-auth-user: %s' "${AUTH_USER}"
  fi
}

api_auth_json() {
  curl -sS -H "$(auth_header)" "$@"
}

cleanup() {
  [[ -n "${TMP_FAIL_BODY}" && -f "${TMP_FAIL_BODY}" ]] && rm -f "${TMP_FAIL_BODY}"
  [[ -n "${TMP_LOCKED_BODY}" && -f "${TMP_LOCKED_BODY}" ]] && rm -f "${TMP_LOCKED_BODY}"
}

main() {
  require_cmd curl
  require_cmd jq
  require_cmd mktemp
  [[ "${LOCKOUT_THRESHOLD}" =~ ^[0-9]+$ ]] || fatal "LOCKOUT_THRESHOLD must be a positive integer"
  (( LOCKOUT_THRESHOLD > 0 )) || fatal "LOCKOUT_THRESHOLD must be >= 1"
  TMP_FAIL_BODY="$(mktemp)"
  TMP_LOCKED_BODY="$(mktemp)"
  trap cleanup EXIT

  log "Health check: ${API_BASE_URL}/health"
  curl -fsS "${API_BASE_URL}/health" >/dev/null || fatal "api health check failed"

  if [[ -n "${AUTH_BEARER_TOKEN}" ]]; then
    log "Using bearer-token auth mode"
  else
    log "Using x-auth-user auth mode: ${AUTH_USER}"
  fi

  password_payload="$(jq -cn --arg new_password "${PASSWORD}" '{new_password: $new_password}')"
  log "Setting local password for ${AUTH_USER}"
  api_auth_json -X POST \
    -H 'Content-Type: application/json' \
    -d "${password_payload}" \
    "${API_BASE_URL}/api/v1/auth/local/password" \
    | jq -e '.password_updated == true' >/dev/null || fatal "failed to set local password"

  log "Triggering failed local logins to hit lockout threshold=${LOCKOUT_THRESHOLD}"
  for ((i = 1; i <= LOCKOUT_THRESHOLD; i++)); do
    failed_payload="$(jq -cn \
      --arg username "${USERNAME}" \
      --arg password "wrong-pass-${i}" \
      '{username: $username, password: $password}')"
    status="$(curl -sS -o "${TMP_FAIL_BODY}" -w '%{http_code}' \
      -X POST \
      -H 'Content-Type: application/json' \
      -d "${failed_payload}" \
      "${API_BASE_URL}/api/v1/auth/local/login")"
    if [[ "${status}" != "403" ]]; then
      fatal "expected 403 on failed login attempt ${i}, got ${status}"
    fi
  done

  log "Verifying blocked login after lockout with correct password"
  locked_payload="$(jq -cn \
    --arg username "${USERNAME}" \
    --arg password "${PASSWORD}" \
    '{username: $username, password: $password}')"
  status="$(curl -sS -o "${TMP_LOCKED_BODY}" -w '%{http_code}' \
    -X POST \
    -H 'Content-Type: application/json' \
    -d "${locked_payload}" \
    "${API_BASE_URL}/api/v1/auth/local/login")"
  if [[ "${status}" != "403" ]]; then
    fatal "expected 403 for lockout-blocked login, got ${status}"
  fi

  message="$(jq -r '.error // ""' "${TMP_LOCKED_BODY}")"
  if [[ "${message}" != *"locked until"* ]]; then
    fatal "expected lockout message to include 'locked until', got: ${message}"
  fi

  log "Local auth hardening lockout validation passed."
}

main "$@"
