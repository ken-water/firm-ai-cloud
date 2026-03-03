#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
RUN_DIR="${ROOT_DIR}/.run"
API_PID_FILE="${RUN_DIR}/api.pid"
WEB_PID_FILE="${RUN_DIR}/web.pid"
API_LOG_FILE="${RUN_DIR}/api.log"
WEB_LOG_FILE="${RUN_DIR}/web.log"

log() {
  echo "[dev-lan-up] $*"
}

require_tool() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "ERROR: required command not found: $1" >&2
    exit 1
  fi
}

pid_from_file() {
  local file="$1"
  if [[ ! -f "$file" ]]; then
    return 1
  fi

  local pid
  pid="$(tr -d '[:space:]' <"$file")"
  if [[ -z "$pid" ]]; then
    return 1
  fi
  echo "$pid"
}

pid_is_running() {
  local pid="$1"
  kill -0 "$pid" >/dev/null 2>&1
}

cleanup_stale_pid_file() {
  local file="$1"
  local pid
  if ! pid="$(pid_from_file "$file")"; then
    rm -f "$file"
    return
  fi

  if ! pid_is_running "$pid"; then
    rm -f "$file"
  fi
}

detect_lan_ip() {
  if [[ -n "${LAN_IP:-}" ]]; then
    echo "${LAN_IP}"
    return
  fi

  local candidates
  candidates="$(hostname -I 2>/dev/null || true)"
  for ip in $candidates; do
    if [[ "$ip" =~ ^127\. ]]; then
      continue
    fi
    if [[ "$ip" == *:* ]]; then
      continue
    fi
    echo "$ip"
    return
  done

  echo "127.0.0.1"
}

wait_http_ready() {
  local url="$1"
  local name="$2"
  local max_attempts="${3:-60}"

  for _ in $(seq 1 "$max_attempts"); do
    if curl -fsS "$url" >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done

  echo "ERROR: ${name} failed to become ready at ${url}" >&2
  return 1
}

start_api() {
  cleanup_stale_pid_file "$API_PID_FILE"
  local pid
  if pid="$(pid_from_file "$API_PID_FILE")" && pid_is_running "$pid"; then
    log "API is already running (pid=${pid})"
    return
  fi

  log "Starting API on 0.0.0.0:8080"
  (
    cd "$ROOT_DIR"
    nohup env API_HOST=0.0.0.0 cargo run -p api >"$API_LOG_FILE" 2>&1 &
    echo $! >"$API_PID_FILE"
  )

  if ! wait_http_ready "http://127.0.0.1:8080/health" "API" 90; then
    tail -n 80 "$API_LOG_FILE" >&2 || true
    exit 1
  fi
}

start_web() {
  local lan_ip="$1"

  cleanup_stale_pid_file "$WEB_PID_FILE"
  local pid
  if pid="$(pid_from_file "$WEB_PID_FILE")" && pid_is_running "$pid"; then
    log "Web console is already running (pid=${pid})"
    return
  fi

  log "Starting web console on 0.0.0.0:5173 (API=${lan_ip}:8080)"
  (
    cd "${ROOT_DIR}/apps/web-console"
    nohup env VITE_API_BASE_URL="http://${lan_ip}:8080" npm run dev >"$WEB_LOG_FILE" 2>&1 &
    echo $! >"$WEB_PID_FILE"
  )

  if ! wait_http_ready "http://127.0.0.1:5173" "Web console" 90; then
    tail -n 80 "$WEB_LOG_FILE" >&2 || true
    exit 1
  fi
}

main() {
  require_tool curl
  require_tool npm
  require_tool cargo

  mkdir -p "$RUN_DIR"

  local lan_ip
  lan_ip="$(detect_lan_ip)"
  log "Detected LAN IP: ${lan_ip}"

  start_api
  start_web "$lan_ip"

  log "LAN access URL: http://${lan_ip}:5173"
  log "API endpoint URL: http://${lan_ip}:8080"
  log "Log files: ${API_LOG_FILE}, ${WEB_LOG_FILE}"
}

main "$@"
