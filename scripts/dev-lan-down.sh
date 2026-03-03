#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
RUN_DIR="${ROOT_DIR}/.run"
API_PID_FILE="${RUN_DIR}/api.pid"
WEB_PID_FILE="${RUN_DIR}/web.pid"

log() {
  echo "[dev-lan-down] $*"
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

stop_process() {
  local name="$1"
  local file="$2"

  local pid
  if ! pid="$(pid_from_file "$file")"; then
    log "${name} is not running (no pid file)"
    rm -f "$file"
    return
  fi

  if ! pid_is_running "$pid"; then
    log "${name} pid ${pid} is stale"
    rm -f "$file"
    return
  fi

  log "Stopping ${name} (pid=${pid})"
  kill "$pid" >/dev/null 2>&1 || true

  for _ in $(seq 1 20); do
    if ! pid_is_running "$pid"; then
      rm -f "$file"
      log "${name} stopped"
      return
    fi
    sleep 1
  done

  log "${name} did not stop gracefully; forcing kill"
  kill -9 "$pid" >/dev/null 2>&1 || true
  rm -f "$file"
}

main() {
  stop_process "web console" "$WEB_PID_FILE"
  stop_process "api" "$API_PID_FILE"
}

main "$@"
