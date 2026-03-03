#!/usr/bin/env bash
set -Eeuo pipefail

ENV_FILE="deploy/.env"
API_URL=""
WAIT_TIMEOUT=180
SKIP_WAIT=0

log() {
  printf '[INFO] %s\n' "$*"
}

warn() {
  printf '[WARN] %s\n' "$*" >&2
}

fatal() {
  printf '[ERROR] %s\n' "$*" >&2
  exit 1
}

command_exists() {
  command -v "$1" >/dev/null 2>&1
}

load_env_file() {
  local raw line key value
  while IFS= read -r raw || [[ -n "${raw}" ]]; do
    line="${raw%$'\r'}"
    [[ -z "${line}" ]] && continue
    [[ "${line}" =~ ^[[:space:]]*# ]] && continue
    [[ "${line}" =~ ^[A-Za-z_][A-Za-z0-9_]*= ]] || continue

    key="${line%%=*}"
    value="${line#*=}"

    if [[ "${value}" =~ ^\".*\"$ ]]; then
      value="${value:1:${#value}-2}"
    elif [[ "${value}" =~ ^\'.*\'$ ]]; then
      value="${value:1:${#value}-2}"
    fi

    export "${key}=${value}"
  done < "${ENV_FILE}"
}

usage() {
  cat <<'EOF'
Bootstrap Zabbix proxy and local agent host

Usage:
  scripts/bootstrap-zabbix.sh [options]

Options:
  --env-file <path>   Use custom env file (default: deploy/.env)
  --api-url <url>     Zabbix API URL (default: http://127.0.0.1:${ZABBIX_WEB_PORT}/api_jsonrpc.php)
  --timeout <sec>     Wait timeout for local agent availability (default: 180)
  --skip-wait         Do not wait for local agent availability
  -h, --help          Show this help message
EOF
}

json_escape() {
  printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'
}

extract_json_string() {
  local json="$1"
  local key="$2"
  local match
  match="$(printf '%s' "$json" | grep -o "\"${key}\":\"[^\"]*\"" | head -n 1 || true)"
  if [[ -n "${match}" ]]; then
    printf '%s' "${match}" | cut -d '"' -f 4
  fi
}

api_call() {
  local method="$1"
  local params_json="$2"
  local auth_token="${3:-}"
  local payload
  local response

  if [[ -n "${auth_token}" ]]; then
    payload="$(printf '{"jsonrpc":"2.0","method":"%s","params":%s,"auth":"%s","id":1}' "${method}" "${params_json}" "${auth_token}")"
  else
    payload="$(printf '{"jsonrpc":"2.0","method":"%s","params":%s,"id":1}' "${method}" "${params_json}")"
  fi

  response="$(curl -fsS -H 'Content-Type: application/json-rpc' -d "${payload}" "${API_URL}" || true)"
  [[ -n "${response}" ]] || fatal "Empty response from Zabbix API (${method})."

  if printf '%s' "${response}" | grep -q '"error"'; then
    local error_data
    error_data="$(extract_json_string "${response}" "data")"
    if [[ -n "${error_data}" ]]; then
      fatal "Zabbix API ${method} failed: ${error_data}"
    fi
    fatal "Zabbix API ${method} failed: ${response}"
  fi

  printf '%s' "${response}"
}

api_call_allow_error() {
  local method="$1"
  local params_json="$2"
  local auth_token="${3:-}"
  local payload

  if [[ -n "${auth_token}" ]]; then
    payload="$(printf '{"jsonrpc":"2.0","method":"%s","params":%s,"auth":"%s","id":1}' "${method}" "${params_json}" "${auth_token}")"
  else
    payload="$(printf '{"jsonrpc":"2.0","method":"%s","params":%s,"id":1}' "${method}" "${params_json}")"
  fi

  curl -fsS -H 'Content-Type: application/json-rpc' -d "${payload}" "${API_URL}" || true
}

wait_for_api() {
  local timeout="${1}"
  local start_ts now elapsed
  start_ts="$(date +%s)"

  while true; do
    if curl -fsS -H 'Content-Type: application/json-rpc' \
      -d '{"jsonrpc":"2.0","method":"apiinfo.version","params":[],"id":1}' \
      "${API_URL}" >/dev/null 2>&1; then
      return 0
    fi

    now="$(date +%s)"
    elapsed=$((now - start_ts))
    if (( elapsed >= timeout )); then
      fatal "Timed out waiting for Zabbix API at ${API_URL}."
    fi

    sleep 3
  done
}

login_with_retry() {
  local timeout="$1"
  local start_ts now elapsed
  local admin_user_escaped admin_password_escaped response token

  start_ts="$(date +%s)"
  admin_user_escaped="$(json_escape "${ZABBIX_ADMIN_USER}")"
  admin_password_escaped="$(json_escape "${ZABBIX_ADMIN_PASSWORD}")"

  while true; do
    response="$(api_call_allow_error "user.login" "{\"username\":\"${admin_user_escaped}\",\"password\":\"${admin_password_escaped}\"}")"
    token="$(extract_json_string "${response}" "result")"
    if [[ -n "${token}" ]]; then
      AUTH_TOKEN="${token}"
      return 0
    fi

    now="$(date +%s)"
    elapsed=$((now - start_ts))
    if (( elapsed >= timeout )); then
      local error_data
      error_data="$(extract_json_string "${response}" "data")"
      if [[ -n "${error_data}" ]]; then
        fatal "Timed out waiting for Zabbix login readiness: ${error_data}"
      fi
      fatal "Timed out waiting for Zabbix login readiness."
    fi

    sleep 3
  done
}

get_proxy_id() {
  local proxy_name_escaped response
  proxy_name_escaped="$(json_escape "${ZABBIX_PROXY_NAME}")"
  response="$(api_call "proxy.get" "{\"output\":[\"proxyid\"],\"filter\":{\"name\":[\"${proxy_name_escaped}\"]}}" "${AUTH_TOKEN}")"
  extract_json_string "${response}" "proxyid"
}

ensure_proxy() {
  PROXY_ID="$(get_proxy_id)"
  if [[ -n "${PROXY_ID}" ]]; then
    log "Zabbix proxy already exists: ${ZABBIX_PROXY_NAME} (id=${PROXY_ID})."
    return
  fi

  local proxy_name_escaped
  proxy_name_escaped="$(json_escape "${ZABBIX_PROXY_NAME}")"
  api_call "proxy.create" "{\"name\":\"${proxy_name_escaped}\",\"operating_mode\":0}" "${AUTH_TOKEN}" >/dev/null
  PROXY_ID="$(get_proxy_id)"
  [[ -n "${PROXY_ID}" ]] || fatal "Failed to create or lookup proxy: ${ZABBIX_PROXY_NAME}."
  log "Created Zabbix proxy: ${ZABBIX_PROXY_NAME} (id=${PROXY_ID})."
}

get_host_group_id() {
  local group_name_escaped response
  group_name_escaped="$(json_escape "${ZABBIX_LOCAL_AGENT_GROUP}")"
  response="$(api_call "hostgroup.get" "{\"output\":[\"groupid\"],\"filter\":{\"name\":[\"${group_name_escaped}\"]}}" "${AUTH_TOKEN}")"
  extract_json_string "${response}" "groupid"
}

ensure_host_group() {
  HOST_GROUP_ID="$(get_host_group_id)"
  if [[ -n "${HOST_GROUP_ID}" ]]; then
    log "Host group already exists: ${ZABBIX_LOCAL_AGENT_GROUP} (id=${HOST_GROUP_ID})."
    return
  fi

  local group_name_escaped
  group_name_escaped="$(json_escape "${ZABBIX_LOCAL_AGENT_GROUP}")"
  api_call "hostgroup.create" "{\"name\":\"${group_name_escaped}\"}" "${AUTH_TOKEN}" >/dev/null
  HOST_GROUP_ID="$(get_host_group_id)"
  [[ -n "${HOST_GROUP_ID}" ]] || fatal "Failed to create or lookup host group: ${ZABBIX_LOCAL_AGENT_GROUP}."
  log "Created host group: ${ZABBIX_LOCAL_AGENT_GROUP} (id=${HOST_GROUP_ID})."
}

get_template_id_by_host() {
  local template_host_escaped response
  template_host_escaped="$(json_escape "${ZABBIX_LOCAL_AGENT_TEMPLATE}")"
  response="$(api_call "template.get" "{\"output\":[\"templateid\"],\"filter\":{\"host\":[\"${template_host_escaped}\"]}}" "${AUTH_TOKEN}")"
  extract_json_string "${response}" "templateid"
}

get_template_id_by_name() {
  local template_name_escaped response
  template_name_escaped="$(json_escape "${ZABBIX_LOCAL_AGENT_TEMPLATE}")"
  response="$(api_call "template.get" "{\"output\":[\"templateid\"],\"filter\":{\"name\":[\"${template_name_escaped}\"]}}" "${AUTH_TOKEN}")"
  extract_json_string "${response}" "templateid"
}

ensure_template() {
  TEMPLATE_ID="$(get_template_id_by_host)"
  if [[ -z "${TEMPLATE_ID}" ]]; then
    TEMPLATE_ID="$(get_template_id_by_name)"
  fi
  [[ -n "${TEMPLATE_ID}" ]] || fatal "Cannot find template: ${ZABBIX_LOCAL_AGENT_TEMPLATE}."
  log "Using template: ${ZABBIX_LOCAL_AGENT_TEMPLATE} (id=${TEMPLATE_ID})."
}

get_local_host_id() {
  local host_name_escaped response
  host_name_escaped="$(json_escape "${ZABBIX_LOCAL_AGENT_HOSTNAME}")"
  response="$(api_call "host.get" "{\"output\":[\"hostid\"],\"filter\":{\"host\":[\"${host_name_escaped}\"]}}" "${AUTH_TOKEN}")"
  extract_json_string "${response}" "hostid"
}

create_local_host() {
  local host_name_escaped interface_dns_escaped params
  host_name_escaped="$(json_escape "${ZABBIX_LOCAL_AGENT_HOSTNAME}")"
  interface_dns_escaped="$(json_escape "${ZABBIX_LOCAL_AGENT_DNS}")"

  params="$(cat <<EOF
{"host":"${host_name_escaped}","monitored_by":1,"proxyid":"${PROXY_ID}","interfaces":[{"type":1,"main":1,"useip":0,"ip":"","dns":"${interface_dns_escaped}","port":"10050"}],"groups":[{"groupid":"${HOST_GROUP_ID}"}],"templates":[{"templateid":"${TEMPLATE_ID}"}]}
EOF
)"

  api_call "host.create" "${params}" "${AUTH_TOKEN}" >/dev/null
}

ensure_local_host() {
  LOCAL_HOST_ID="$(get_local_host_id)"
  if [[ -n "${LOCAL_HOST_ID}" ]]; then
    log "Local Zabbix host already exists: ${ZABBIX_LOCAL_AGENT_HOSTNAME} (id=${LOCAL_HOST_ID})."
    return
  fi

  create_local_host
  LOCAL_HOST_ID="$(get_local_host_id)"
  [[ -n "${LOCAL_HOST_ID}" ]] || fatal "Failed to create local host: ${ZABBIX_LOCAL_AGENT_HOSTNAME}."
  log "Created local Zabbix host: ${ZABBIX_LOCAL_AGENT_HOSTNAME} (id=${LOCAL_HOST_ID})."
}

wait_for_local_agent_available() {
  local start_ts now elapsed response active_available interface_available host_name_escaped
  start_ts="$(date +%s)"
  host_name_escaped="$(json_escape "${ZABBIX_LOCAL_AGENT_HOSTNAME}")"

  while true; do
    response="$(api_call "host.get" "{\"output\":[\"hostid\",\"active_available\"],\"selectInterfaces\":[\"available\"],\"filter\":{\"host\":[\"${host_name_escaped}\"]}}" "${AUTH_TOKEN}")"
    active_available="$(extract_json_string "${response}" "active_available")"
    interface_available="$(extract_json_string "${response}" "available")"

    if [[ "${active_available}" == "1" || "${interface_available}" == "1" ]]; then
      log "Local agent host is available in Zabbix."
      return 0
    fi

    now="$(date +%s)"
    elapsed=$((now - start_ts))
    if (( elapsed >= WAIT_TIMEOUT )); then
      warn "Local agent host is not yet available after ${WAIT_TIMEOUT}s."
      return 1
    fi

    sleep 5
  done
}

parse_args() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --env-file)
        [[ $# -ge 2 ]] || fatal "--env-file requires a value"
        ENV_FILE="$2"
        shift 2
        ;;
      --api-url)
        [[ $# -ge 2 ]] || fatal "--api-url requires a value"
        API_URL="$2"
        shift 2
        ;;
      --timeout)
        [[ $# -ge 2 ]] || fatal "--timeout requires a value"
        WAIT_TIMEOUT="$2"
        shift 2
        ;;
      --skip-wait)
        SKIP_WAIT=1
        shift
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      *)
        fatal "Unknown argument: $1"
        ;;
    esac
  done
}

main() {
  parse_args "$@"

  command_exists curl || fatal "curl is required."
  [[ -f "${ENV_FILE}" ]] || fatal "Missing env file: ${ENV_FILE}"

  load_env_file

  ZABBIX_WEB_PORT="${ZABBIX_WEB_PORT:-8082}"
  ZABBIX_API_URL_DEFAULT="http://127.0.0.1:${ZABBIX_WEB_PORT}/api_jsonrpc.php"
  API_URL="${API_URL:-${ZABBIX_API_URL:-${ZABBIX_API_URL_DEFAULT}}}"
  ZABBIX_ADMIN_USER="${ZABBIX_WEB_ADMIN_USER:-Admin}"
  ZABBIX_ADMIN_PASSWORD="${ZABBIX_WEB_ADMIN_PASSWORD:-zabbix}"
  ZABBIX_PROXY_NAME="${ZABBIX_PROXY_HOSTNAME:-cloudops-proxy}"
  ZABBIX_LOCAL_AGENT_HOSTNAME="${ZABBIX_LOCAL_AGENT_HOSTNAME:-cloudops-local-agent}"
  ZABBIX_LOCAL_AGENT_DNS="${ZABBIX_LOCAL_AGENT_DNS:-zabbix-agent-local}"
  ZABBIX_LOCAL_AGENT_GROUP="${ZABBIX_LOCAL_AGENT_GROUP:-Linux servers}"
  ZABBIX_LOCAL_AGENT_TEMPLATE="${ZABBIX_LOCAL_AGENT_TEMPLATE:-Linux by Zabbix agent}"

  log "Waiting for Zabbix API: ${API_URL}"
  wait_for_api 180

  login_with_retry 180

  ensure_proxy
  ensure_host_group
  ensure_template
  ensure_local_host

  if [[ "${SKIP_WAIT}" -eq 0 ]]; then
    wait_for_local_agent_available || true
  fi

  log "Zabbix bootstrap completed."
}

main "$@"
