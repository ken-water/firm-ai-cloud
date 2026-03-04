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
ZABBIX_BOOTSTRAP_SCRIPT="${ROOT_DIR}/scripts/bootstrap-zabbix.sh"
API_DOCKERFILE="${ROOT_DIR}/services/api/Dockerfile"
WEB_DOCKERFILE="${ROOT_DIR}/apps/web-console/Dockerfile"
DEFAULT_API_IMAGE="cloudops/api:0.0.8"
DEFAULT_WEB_IMAGE="cloudops/web-console:0.0.8"

COMPOSE_CMD=()

usage() {
  cat <<'EOF'
CloudOps One stack upgrader

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

get_env_value() {
  local key="$1"
  local fallback="$2"
  local line
  line="$(grep -E "^${key}=" "${ENV_FILE}" | tail -n 1 || true)"
  if [[ -z "${line}" ]]; then
    echo "${fallback}"
    return
  fi
  echo "${line#*=}"
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

ensure_app_images() {
  local api_image web_image
  api_image="$(get_env_value "CLOUDOPS_API_IMAGE" "${DEFAULT_API_IMAGE}")"
  web_image="$(get_env_value "CLOUDOPS_WEB_IMAGE" "${DEFAULT_WEB_IMAGE}")"

  if ! docker image inspect "${api_image}" >/dev/null 2>&1; then
    [[ -f "${API_DOCKERFILE}" ]] || fatal "Missing API Dockerfile: ${API_DOCKERFILE}"
    log "Building API image ${api_image}..."
    docker build -t "${api_image}" -f "${API_DOCKERFILE}" "${ROOT_DIR}"
  else
    log "Using existing API image ${api_image}."
  fi

  if ! docker image inspect "${web_image}" >/dev/null 2>&1; then
    [[ -f "${WEB_DOCKERFILE}" ]] || fatal "Missing web Dockerfile: ${WEB_DOCKERFILE}"
    log "Building web image ${web_image}..."
    docker build -t "${web_image}" -f "${WEB_DOCKERFILE}" "${ROOT_DIR}/apps/web-console"
  else
    log "Using existing web image ${web_image}."
  fi
}

run_upgrade() {
  local infra_services=(
    postgres
    redis
    opensearch
    minio
    zabbix-db
    zabbix-server
    zabbix-web
    zabbix-proxy
    zabbix-agent-local
  )

  if [[ "${NO_PULL}" -eq 0 ]]; then
    log "Pulling dependency images..."
    compose pull "${infra_services[@]}"
  fi

  ensure_app_images

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
  wait_for_service api 240
  wait_for_service web 180
}

run_zabbix_bootstrap() {
  if [[ ! -f "${ZABBIX_BOOTSTRAP_SCRIPT}" ]]; then
    warn "Zabbix bootstrap script is missing: ${ZABBIX_BOOTSTRAP_SCRIPT}"
    return
  fi

  log "Bootstrapping Zabbix proxy and local agent host..."
  if ! bash "${ZABBIX_BOOTSTRAP_SCRIPT}" --env-file "${ENV_FILE}" --timeout 180; then
    warn "Zabbix bootstrap failed. You can rerun it manually:"
    warn "  bash scripts/bootstrap-zabbix.sh --env-file deploy/.env"
  fi
}

print_summary() {
  cat <<'EOF'

CloudOps One stack upgrade complete.

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
  run_zabbix_bootstrap
  print_summary
}

main "$@"
