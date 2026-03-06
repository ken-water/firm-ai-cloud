#!/usr/bin/env bash
set -Eeuo pipefail

API_BASE_URL="${API_BASE_URL:-http://127.0.0.1:8080}"
AUTH_USER="${AUTH_USER:-admin}"
AUTH_BEARER_TOKEN="${AUTH_BEARER_TOKEN:-}"
USERNAME="${USERNAME:-admin}"
PASSWORD="${PASSWORD:-ChangeMe_12345}"
RESET_REASON="${RESET_REASON:-helpdesk mfa recovery reset}"
MFA_CODE_COUNT="${MFA_CODE_COUNT:-8}"
TMP_BODY_ONE=""
TMP_BODY_TWO=""

log() {
  printf '[local-mfa-recovery-test] %s\n' "$*"
}

fatal() {
  printf '[local-mfa-recovery-test][ERROR] %s\n' "$*" >&2
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
  [[ -n "${TMP_BODY_ONE}" && -f "${TMP_BODY_ONE}" ]] && rm -f "${TMP_BODY_ONE}"
  [[ -n "${TMP_BODY_TWO}" && -f "${TMP_BODY_TWO}" ]] && rm -f "${TMP_BODY_TWO}"
}

main() {
  require_cmd curl
  require_cmd jq
  require_cmd mktemp

  [[ "${MFA_CODE_COUNT}" =~ ^[0-9]+$ ]] || fatal "MFA_CODE_COUNT must be a positive integer"
  (( MFA_CODE_COUNT > 0 )) || fatal "MFA_CODE_COUNT must be >= 1"

  TMP_BODY_ONE="$(mktemp)"
  TMP_BODY_TWO="$(mktemp)"
  trap cleanup EXIT

  log "Health check: ${API_BASE_URL}/health"
  curl -fsS "${API_BASE_URL}/health" >/dev/null || fatal "api health check failed"

  if [[ -n "${AUTH_BEARER_TOKEN}" ]]; then
    log "Using bearer-token auth mode"
  else
    log "Using x-auth-user auth mode: ${AUTH_USER}"
  fi

  log "Setting local password for authenticated actor"
  password_payload="$(jq -cn --arg new_password "${PASSWORD}" '{new_password: $new_password}')"
  api_auth_json -X POST \
    -H 'Content-Type: application/json' \
    -d "${password_payload}" \
    "${API_BASE_URL}/api/v1/auth/local/password" \
    | jq -e '.password_updated == true' >/dev/null || fatal "failed to set local password"

  log "Enrolling local MFA and capturing recovery codes"
  enroll_json="$(api_auth_json -X POST "${API_BASE_URL}/api/v1/auth/local/mfa/enroll")"
  echo "${enroll_json}" | jq -e --argjson expected "${MFA_CODE_COUNT}" '
    .mfa_enabled == true and
    ((.recovery_codes | length) == $expected)
  ' >/dev/null || fatal "MFA enrollment response is invalid"

  first_code="$(echo "${enroll_json}" | jq -r '.recovery_codes[0] // empty')"
  second_code="$(echo "${enroll_json}" | jq -r '.recovery_codes[1] // empty')"
  [[ -n "${first_code}" && -n "${second_code}" ]] || fatal "missing initial recovery codes"

  log "Validating one-time recovery code login success"
  login_payload="$(jq -cn --arg username "${USERNAME}" --arg password "${PASSWORD}" --arg recovery_code "${first_code}" '{username: $username, password: $password, recovery_code: $recovery_code}')"
  status="$(curl -sS -o "${TMP_BODY_ONE}" -w '%{http_code}' \
    -X POST \
    -H 'Content-Type: application/json' \
    -d "${login_payload}" \
    "${API_BASE_URL}/api/v1/auth/local/login")"
  [[ "${status}" == "200" ]] || fatal "expected 200 for recovery login success, got ${status}"

  log "Validating replay denial for consumed recovery code"
  status="$(curl -sS -o "${TMP_BODY_TWO}" -w '%{http_code}' \
    -X POST \
    -H 'Content-Type: application/json' \
    -d "${login_payload}" \
    "${API_BASE_URL}/api/v1/auth/local/login")"
  [[ "${status}" == "403" ]] || fatal "expected 403 for replayed recovery code, got ${status}"

  log "Checking recovery status counters"
  status_json="$(api_auth_json "${API_BASE_URL}/api/v1/auth/local/mfa/recovery/status")"
  echo "${status_json}" | jq -e '
    (.consumed_codes >= 1) and
    (.remaining_codes >= 0) and
    (.revoked_codes >= 0)
  ' >/dev/null || fatal "unexpected recovery status counters"

  log "Rotating recovery codes and verifying old code invalidation"
  rotate_json="$(api_auth_json -X POST "${API_BASE_URL}/api/v1/auth/local/mfa/recovery/rotate")"
  echo "${rotate_json}" | jq -e --argjson expected "${MFA_CODE_COUNT}" '
    (.generated_codes == $expected) and
    ((.recovery_codes | length) == $expected)
  ' >/dev/null || fatal "recovery rotation response is invalid"

  old_code_payload="$(jq -cn --arg username "${USERNAME}" --arg password "${PASSWORD}" --arg recovery_code "${second_code}" '{username: $username, password: $password, recovery_code: $recovery_code}')"
  status="$(curl -sS -o "${TMP_BODY_ONE}" -w '%{http_code}' \
    -X POST \
    -H 'Content-Type: application/json' \
    -d "${old_code_payload}" \
    "${API_BASE_URL}/api/v1/auth/local/login")"
  [[ "${status}" == "403" ]] || fatal "expected 403 for rotated (revoked) old recovery code, got ${status}"

  log "Running admin reset flow and validating governance audit"
  reset_payload="$(jq -cn --arg username "${USERNAME}" --arg reason "${RESET_REASON}" '{username: $username, reason: $reason}')"
  reset_json="$(api_auth_json -X POST \
    -H 'Content-Type: application/json' \
    -d "${reset_payload}" \
    "${API_BASE_URL}/api/v1/auth/local/mfa/recovery/admin-reset")"

  echo "${reset_json}" | jq -e --arg username "${USERNAME}" '
    (.target_username == $username) and
    (.mfa_enabled == false) and
    (.revoked_codes >= 0)
  ' >/dev/null || fatal "admin reset response is invalid"

  audit_json="$(api_auth_json "${API_BASE_URL}/api/v1/audit/logs?action=auth.local.mfa_recovery.admin_reset&limit=20")"
  echo "${audit_json}" | jq -e --arg reason "${RESET_REASON}" --arg username "${USERNAME}" '
    any(.items[]?;
      .action == "auth.local.mfa_recovery.admin_reset" and
      (.message // "") == $reason and
      (.metadata.target_username // "") == $username
    )
  ' >/dev/null || fatal "admin reset audit record not found"

  log "Local MFA recovery lifecycle validation passed."
}

main "$@"
