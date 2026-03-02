#!/usr/bin/env bash
set -Eeuo pipefail

MODE="auto"
SKIP_DOCKER_INSTALL=0
NO_PULL=0
MIRROR="auto"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
DEPLOY_DIR="${ROOT_DIR}/deploy"
COMPOSE_FILE="${DEPLOY_DIR}/docker-compose.yml"
ENV_EXAMPLE_FILE="${DEPLOY_DIR}/.env.example"
ENV_CN_EXAMPLE_FILE="${DEPLOY_DIR}/.env.cn.example"
ENV_FILE="${DEPLOY_DIR}/.env"

COMPOSE_CMD=()

usage() {
  cat <<'EOF'
CloudOps One one-click installer

Usage:
  scripts/install.sh [options]

Options:
  --mode auto|container      Installation mode (default: auto)
  --mirror auto|default|cn   Image mirror profile for deploy/.env (default: auto)
  --skip-docker-install      Do not auto-install Docker if missing
  --no-pull                  Skip image pulling before startup
  -h, --help                 Show this help message
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

detect_pkg_manager() {
  if command_exists apt-get; then
    echo "apt-get"
    return
  fi
  if command_exists dnf; then
    echo "dnf"
    return
  fi
  if command_exists yum; then
    echo "yum"
    return
  fi
  if command_exists zypper; then
    echo "zypper"
    return
  fi
  if command_exists pacman; then
    echo "pacman"
    return
  fi
  echo "unknown"
}

install_curl_if_needed() {
  if command_exists curl; then
    return
  fi

  local pm
  pm="$(detect_pkg_manager)"
  case "${pm}" in
    apt-get)
      run_root apt-get update
      run_root apt-get install -y curl
      ;;
    dnf)
      run_root dnf install -y curl
      ;;
    yum)
      run_root yum install -y curl
      ;;
    zypper)
      run_root zypper --non-interactive install curl
      ;;
    pacman)
      run_root pacman -Sy --noconfirm curl
      ;;
    *)
      fatal "curl is required, and your package manager is unsupported. Install curl and retry."
      ;;
  esac
}

install_docker_linux() {
  log "Installing Docker on Linux..."
  install_curl_if_needed

  local installer
  installer="$(mktemp)"
  curl -fsSL https://get.docker.com -o "${installer}"
  run_root sh "${installer}"
  rm -f "${installer}"

  if command_exists systemctl; then
    run_root systemctl enable --now docker || true
  elif command_exists service; then
    run_root service docker start || true
  fi
}

install_docker_macos() {
  log "Installing Docker Desktop on macOS..."
  if ! command_exists brew; then
    fatal "Homebrew is required to auto-install Docker on macOS. Install Homebrew first."
  fi

  brew install --cask docker
  open -a Docker || true
}

install_docker_if_missing() {
  if command_exists docker; then
    return
  fi

  if [[ "${SKIP_DOCKER_INSTALL}" -eq 1 ]]; then
    fatal "Docker is missing and --skip-docker-install was set."
  fi

  case "$(uname -s)" in
    Linux)
      install_docker_linux
      ;;
    Darwin)
      install_docker_macos
      ;;
    *)
      fatal "Unsupported OS for automatic Docker installation: $(uname -s)"
      ;;
  esac
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
    fatal "Docker daemon is not running. Start it and rerun this installer."
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

set_env_key() {
  local key="$1"
  local value="$2"
  local tmp_file

  tmp_file="$(mktemp)"
  awk -v k="${key}" -v v="${value}" '
    BEGIN { found = 0 }
    $0 ~ "^" k "=" {
      print k "=" v
      found = 1
      next
    }
    { print }
    END {
      if (found == 0) {
        print k "=" v
      }
    }
  ' "${ENV_FILE}" > "${tmp_file}"

  mv "${tmp_file}" "${ENV_FILE}"
}

bootstrap_env_file() {
  if [[ -f "${ENV_FILE}" ]]; then
    return
  fi

  if [[ "${MIRROR}" == "cn" && -f "${ENV_CN_EXAMPLE_FILE}" ]]; then
    cp "${ENV_CN_EXAMPLE_FILE}" "${ENV_FILE}"
  else
    cp "${ENV_EXAMPLE_FILE}" "${ENV_FILE}"
  fi
  warn "Created ${ENV_FILE} from template. Change default passwords before production use."
}

apply_mirror_profile() {
  case "${MIRROR}" in
    auto)
      return
      ;;
    default)
      set_env_key "POSTGRES_IMAGE" "postgres:16-alpine"
      set_env_key "REDIS_IMAGE" "redis:7-alpine"
      set_env_key "OPENSEARCH_IMAGE" "opensearchproject/opensearch:2.15.0"
      set_env_key "MINIO_IMAGE" "minio/minio:RELEASE.2025-01-20T14-49-07Z"
      log "Applied default image mirror profile."
      ;;
    cn)
      set_env_key "POSTGRES_IMAGE" "docker.1ms.run/library/postgres:16-alpine"
      set_env_key "REDIS_IMAGE" "docker.1ms.run/library/redis:7-alpine"
      set_env_key "OPENSEARCH_IMAGE" "docker.1ms.run/opensearchproject/opensearch:2.15.0"
      set_env_key "MINIO_IMAGE" "docker.1ms.run/minio/minio:RELEASE.2025-01-20T14-49-07Z"
      log "Applied China image mirror profile (docker.1ms.run)."
      ;;
  esac
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

start_stack() {
  if [[ "${NO_PULL}" -eq 0 ]]; then
    log "Pulling latest images..."
    compose pull
  fi

  log "Starting dependency stack..."
  compose up -d

  wait_for_service postgres 180
  wait_for_service redis 120
  wait_for_service opensearch 300
  wait_for_service minio 180
}

print_summary() {
  cat <<'EOF'

CloudOps One dependency stack is ready.

Endpoints:
  PostgreSQL:       127.0.0.1:5432
  Redis:            127.0.0.1:6379
  OpenSearch API:   http://127.0.0.1:9200
  MinIO API:        http://127.0.0.1:9000
  MinIO Console:    http://127.0.0.1:9001

Useful commands:
  docker compose --env-file deploy/.env -f deploy/docker-compose.yml ps
  docker compose --env-file deploy/.env -f deploy/docker-compose.yml logs -f
  docker compose --env-file deploy/.env -f deploy/docker-compose.yml down
EOF
}

parse_args() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --mode)
        [[ $# -ge 2 ]] || fatal "--mode requires a value"
        MODE="$2"
        shift 2
        ;;
      --mirror)
        [[ $# -ge 2 ]] || fatal "--mirror requires a value"
        MIRROR="$2"
        shift 2
        ;;
      --skip-docker-install)
        SKIP_DOCKER_INSTALL=1
        shift
        ;;
      --no-pull)
        NO_PULL=1
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

  case "${MODE}" in
    auto|container)
      ;;
    *)
      fatal "Unsupported mode: ${MODE}. Use auto or container."
      ;;
  esac

  case "${MIRROR}" in
    auto|default|cn)
      ;;
    *)
      fatal "Unsupported mirror: ${MIRROR}. Use auto, default, or cn."
      ;;
  esac
}

main() {
  parse_args "$@"

  [[ -f "${COMPOSE_FILE}" ]] || fatal "Missing compose file: ${COMPOSE_FILE}"
  [[ -f "${ENV_EXAMPLE_FILE}" ]] || fatal "Missing env template: ${ENV_EXAMPLE_FILE}"
  if [[ "${MIRROR}" == "cn" && ! -f "${ENV_CN_EXAMPLE_FILE}" ]]; then
    fatal "Missing China env template: ${ENV_CN_EXAMPLE_FILE}"
  fi

  install_docker_if_missing
  ensure_docker_running
  detect_compose_command
  bootstrap_env_file
  apply_mirror_profile
  start_stack
  print_summary
}

main "$@"
