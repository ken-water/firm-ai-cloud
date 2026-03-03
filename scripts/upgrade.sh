#!/usr/bin/env bash
set -Eeuo pipefail

NO_PULL=0
SKIP_HEALTHCHECK=0

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
DEPLOY_DIR="${ROOT_DIR}/deploy"
COMPOSE_FILE="${DEPLOY_DIR}/docker-compose.yml"
ENV_EXAMPLE_FILE="${DEPLOY_DIR}/.env.example"
ENV_FILE="${DEPLOY_DIR}/.env"

COMPOSE_CMD=()

usage() {
  cat <<'EOF'
CloudOps One dependency stack upgrader

Usage:
  scripts/upgrade.sh [options]

Options:
  --no-pull            Skip image pull and reuse local images
  --skip-healthcheck   Skip post-upgrade health waiting
  -h, --help           Show this help message
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

run_root() {
  if [[ "${EUID}" -eq 0 ]]; then
    "$@"
    return
  fi

  if command_exists sudo; then
    sudo "$@"
    return
  fi

  fatal "Root privileges are required for this action. Install sudo or run as root."
}

ensure_docker_running() {
  if docker info >/dev/null 2>&1; then
    return
  fi

  case "$(uname -s)" in
    Linux)
      if command_exists systemctl; then
        run_root systemctl start docker || true
      elif command_exists service; then
        run_root service docker start || true
      fi
      ;;
    Darwin)
      warn "Docker Desktop appears installed but not running. Please launch Docker Desktop."
      ;;
  esac

  if ! docker info >/dev/null 2>&1; then
    fatal "Docker daemon is not running. Start it and rerun this script."
  fi
}

detect_compose_command() {
  if docker compose version >/dev/null 2>&1; then
    COMPOSE_CMD=("docker" "compose")
    return
  fi

  if command_exists docker-compose; then
    COMPOSE_CMD=("docker-compose")
    return
  fi

  fatal "Docker Compose not found. Install Docker Compose plugin or docker-compose binary."
}

bootstrap_env_file() {
  if [[ -f "${ENV_FILE}" ]]; then
    return
  fi

  cp "${ENV_EXAMPLE_FILE}" "${ENV_FILE}"
  warn "Created ${ENV_FILE} from template. Change default passwords before production use."
}

compose() {
  "${COMPOSE_CMD[@]}" --env-file "${ENV_FILE}" -f "${COMPOSE_FILE}" "$@"
}

wait_for_service() {
  local service="$1"
  local timeout="${2:-240}"
  local start_ts
  start_ts="$(date +%s)"
  local container_id

  container_id="$(compose ps -q "${service}")"
  if [[ -z "${container_id}" ]]; then
    fatal "Could not find container ID for service ${service}."
  fi

  while true; do
    local status now elapsed
    status="$(docker inspect --format '{{if .State.Health}}{{.State.Health.Status}}{{else}}{{.State.Status}}{{end}}' "${container_id}" 2>/dev/null || true)"
    if [[ "${status}" == "healthy" || "${status}" == "running" ]]; then
      log "Service ${service} is ${status}."
      return
    fi

    now="$(date +%s)"
    elapsed=$((now - start_ts))
    if (( elapsed >= timeout )); then
      fatal "Timed out waiting for ${service} to become healthy."
    fi
    sleep 3
  done
}

run_upgrade() {
  if [[ "${NO_PULL}" -eq 0 ]]; then
    log "Pulling latest images..."
    compose pull
  fi

  log "Recreating stack with latest definitions..."
  compose up -d --remove-orphans
}

run_health_checks() {
  if [[ "${SKIP_HEALTHCHECK}" -eq 1 ]]; then
    warn "Skipping health checks as requested."
    return
  fi

  wait_for_service postgres 180
  wait_for_service redis 120
  wait_for_service opensearch 300
  wait_for_service minio 180
  wait_for_service zabbix-db 240
  wait_for_service zabbix-server 240
  wait_for_service zabbix-web 240
  wait_for_service zabbix-proxy 240
  wait_for_service zabbix-agent-local 180
}

print_summary() {
  cat <<'EOF'

Dependency stack upgrade complete.

Useful commands:
  docker compose --env-file deploy/.env -f deploy/docker-compose.yml ps
  docker compose --env-file deploy/.env -f deploy/docker-compose.yml logs -f
EOF
}

parse_args() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --no-pull)
        NO_PULL=1
        shift
        ;;
      --skip-healthcheck)
        SKIP_HEALTHCHECK=1
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
  [[ -f "${ENV_EXAMPLE_FILE}" ]] || fatal "Missing env template: ${ENV_EXAMPLE_FILE}"
  command_exists docker || fatal "Docker is not installed. Run scripts/install.sh first."

  ensure_docker_running
  detect_compose_command
  bootstrap_env_file
  run_upgrade
  run_health_checks
  print_summary
}

main "$@"
