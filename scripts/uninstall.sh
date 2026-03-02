#!/usr/bin/env bash
set -Eeuo pipefail

PURGE_DATA=0
REMOVE_ENV=0
YES=0

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
DEPLOY_DIR="${ROOT_DIR}/deploy"
COMPOSE_FILE="${DEPLOY_DIR}/docker-compose.yml"
ENV_EXAMPLE_FILE="${DEPLOY_DIR}/.env.example"
ENV_FILE="${DEPLOY_DIR}/.env"
ACTIVE_ENV_FILE=""

COMPOSE_CMD=()

usage() {
  cat <<'EOF'
CloudOps One dependency stack uninstaller

Usage:
  scripts/uninstall.sh [options]

Options:
  --purge-data      Remove containers, network, and named volumes
  --remove-env      Remove deploy/.env after uninstall
  -y, --yes         Skip confirmation prompt
  -h, --help        Show this help message
EOF
}

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

detect_compose_command() {
  if command_exists docker && docker compose version >/dev/null 2>&1; then
    COMPOSE_CMD=("docker" "compose")
    return
  fi

  if command_exists docker-compose; then
    COMPOSE_CMD=("docker-compose")
    return
  fi

  fatal "Docker Compose not found. Install Docker Compose plugin or docker-compose binary."
}

detect_env_file() {
  if [[ -f "${ENV_FILE}" ]]; then
    ACTIVE_ENV_FILE="${ENV_FILE}"
    return
  fi

  if [[ -f "${ENV_EXAMPLE_FILE}" ]]; then
    ACTIVE_ENV_FILE="${ENV_EXAMPLE_FILE}"
    warn "deploy/.env not found; using deploy/.env.example for compose variable resolution."
    return
  fi

  fatal "No env file found. Expected ${ENV_FILE} or ${ENV_EXAMPLE_FILE}."
}

compose() {
  "${COMPOSE_CMD[@]}" --env-file "${ACTIVE_ENV_FILE}" -f "${COMPOSE_FILE}" "$@"
}

confirm_if_needed() {
  if [[ "${YES}" -eq 1 ]]; then
    return
  fi

  local message
  if [[ "${PURGE_DATA}" -eq 1 ]]; then
    message="This will stop services and DELETE all persisted data volumes. Continue? [y/N] "
  else
    message="This will stop and remove services but keep persisted volumes. Continue? [y/N] "
  fi

  read -r -p "${message}" answer
  case "${answer}" in
    y|Y|yes|YES)
      ;;
    *)
      fatal "Uninstall cancelled by user."
      ;;
  esac
}

run_uninstall() {
  local down_args=("down" "--remove-orphans")
  if [[ "${PURGE_DATA}" -eq 1 ]]; then
    down_args+=("-v")
  fi

  log "Stopping and removing stack..."
  compose "${down_args[@]}"

  if [[ "${REMOVE_ENV}" -eq 1 && -f "${ENV_FILE}" ]]; then
    rm -f "${ENV_FILE}"
    log "Removed ${ENV_FILE}."
  fi
}

print_summary() {
  if [[ "${PURGE_DATA}" -eq 1 ]]; then
    log "Uninstall complete. Containers and named volumes have been removed."
  else
    log "Uninstall complete. Data volumes were preserved."
  fi
}

parse_args() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --purge-data)
        PURGE_DATA=1
        shift
        ;;
      --remove-env)
        REMOVE_ENV=1
        shift
        ;;
      -y|--yes)
        YES=1
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

  [[ -f "${COMPOSE_FILE}" ]] || fatal "Missing compose file: ${COMPOSE_FILE}"
  detect_compose_command
  detect_env_file
  confirm_if_needed
  run_uninstall
  print_summary
}

main "$@"
