#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMPOSE_FILE="${ROOT_DIR}/deploy/docker-compose.yml"
ENV_FILE="${ROOT_DIR}/deploy/.env"
PROJECT_NAME="deploy"
INPUT_DIR=""
RESTORE_POSTGRES=1
RESTORE_VOLUMES=1
TOOL_IMAGE=""
ASSUME_YES=0
COMPOSE_CMD=()
COMPOSE_CMD_LABEL=""

VOLUME_KEYS=(
  "postgres-data"
  "minio-data"
  "opensearch-data"
  "zabbix-db-data"
  "zabbix-proxy-data"
)

INFRA_SERVICES=(
  "postgres"
  "redis"
  "opensearch"
  "minio"
  "zabbix-db"
  "zabbix-server"
  "zabbix-web"
  "zabbix-proxy"
  "zabbix-agent-local"
)

log() {
  printf '[restore-stack] %s\n' "$*"
}

warn() {
  printf '[restore-stack][WARN] %s\n' "$*" >&2
}

fatal() {
  printf '[restore-stack][ERROR] %s\n' "$*" >&2
  exit 1
}

usage() {
  cat <<'USAGE'
CloudOps stack restore helper

Usage:
  bash scripts/restore-stack.sh --input-dir <backup-dir> [options]

Options:
  --input-dir <path>       Backup directory created by backup-stack.sh (required)
  --env-file <path>        Env file for docker compose (default: deploy/.env)
  --compose-file <path>    Compose file (default: deploy/docker-compose.yml)
  --project-name <name>    Compose project name (default: deploy)
  --tool-image <image>     Image used for volume extraction (default: POSTGRES_IMAGE from env)
  --skip-postgres          Skip PostgreSQL restore
  --skip-volumes           Skip named-volume restore
  --yes                    Non-interactive confirmation
  -h, --help               Show this help
USAGE
}

parse_args() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --input-dir)
        [[ $# -ge 2 ]] || fatal "--input-dir requires a value"
        INPUT_DIR="$2"
        shift 2
        ;;
      --env-file)
        [[ $# -ge 2 ]] || fatal "--env-file requires a value"
        ENV_FILE="$2"
        shift 2
        ;;
      --compose-file)
        [[ $# -ge 2 ]] || fatal "--compose-file requires a value"
        COMPOSE_FILE="$2"
        shift 2
        ;;
      --project-name)
        [[ $# -ge 2 ]] || fatal "--project-name requires a value"
        PROJECT_NAME="$2"
        shift 2
        ;;
      --tool-image)
        [[ $# -ge 2 ]] || fatal "--tool-image requires a value"
        TOOL_IMAGE="$2"
        shift 2
        ;;
      --skip-postgres)
        RESTORE_POSTGRES=0
        shift
        ;;
      --skip-volumes)
        RESTORE_VOLUMES=0
        shift
        ;;
      --yes)
        ASSUME_YES=1
        shift
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      *)
        fatal "unknown argument: $1"
        ;;
    esac
  done
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || fatal "missing required command: $1"
}

detect_compose_cmd() {
  if docker compose version >/dev/null 2>&1; then
    COMPOSE_CMD=(docker compose)
    COMPOSE_CMD_LABEL="docker compose"
    return
  fi

  if command -v docker-compose >/dev/null 2>&1; then
    COMPOSE_CMD=(docker-compose)
    COMPOSE_CMD_LABEL="docker-compose"
    return
  fi

  fatal "missing docker compose command: install Docker Compose v2 plugin or docker-compose"
}

compose() {
  "${COMPOSE_CMD[@]}" \
    --project-name "${PROJECT_NAME}" \
    --env-file "${ENV_FILE}" \
    -f "${COMPOSE_FILE}" \
    "$@"
}

load_env_file() {
  [[ -f "${ENV_FILE}" ]] || fatal "env file not found: ${ENV_FILE}"
  while IFS= read -r raw_line || [[ -n "${raw_line}" ]]; do
    local line key value
    line="${raw_line#"${raw_line%%[![:space:]]*}"}"
    [[ -z "${line}" || "${line:0:1}" == "#" ]] && continue
    [[ "${line}" == export* ]] && line="${line#export }"
    [[ "${line}" == *=* ]] || continue

    key="${line%%=*}"
    value="${line#*=}"
    key="${key//[[:space:]]/}"

    if [[ ! "${key}" =~ ^[A-Za-z_][A-Za-z0-9_]*$ ]]; then
      warn "Ignoring invalid env key from ${ENV_FILE}: ${key}"
      continue
    fi

    export "${key}=${value}"
  done <"${ENV_FILE}"
}

validate_input_dir() {
  [[ -n "${INPUT_DIR}" ]] || fatal "--input-dir is required"
  [[ -d "${INPUT_DIR}" ]] || fatal "input directory not found: ${INPUT_DIR}"
  [[ -f "${INPUT_DIR}/metadata.env" ]] || warn "metadata.env not found in backup directory"
  [[ -f "${INPUT_DIR}/SHA256SUMS" ]] || warn "SHA256SUMS not found in backup directory"
}

detect_tool_image() {
  if [[ -n "${TOOL_IMAGE}" ]]; then
    return
  fi
  TOOL_IMAGE="${POSTGRES_IMAGE:-postgres:16-alpine}"
}

confirm_restore() {
  local message
  message="This operation will overwrite running stack data for project '${PROJECT_NAME}'. Continue? [y/N] "

  if [[ "${ASSUME_YES}" -eq 1 ]]; then
    return
  fi

  read -r -p "${message}" reply
  if [[ ! "${reply}" =~ ^[Yy]$ ]]; then
    fatal "restore cancelled by user"
  fi
}

stop_stack() {
  log "Stopping stack before restore..."
  compose down >/dev/null
}

restore_named_volume() {
  local key volume_name archive_path
  key="$1"
  volume_name="${PROJECT_NAME}_${key}"
  archive_path="${INPUT_DIR}/volumes/${key}.tar.gz"

  if [[ ! -f "${archive_path}" ]]; then
    warn "Volume archive missing, skipping: ${archive_path}"
    return
  fi

  docker volume create "${volume_name}" >/dev/null

  log "Clearing volume ${volume_name}..."
  docker run --rm \
    -v "${volume_name}:/volume" \
    "${TOOL_IMAGE}" \
    sh -c "rm -rf /volume/* /volume/.[!.]* /volume/..?* 2>/dev/null || true"

  log "Restoring volume ${volume_name} from ${archive_path}..."
  docker run --rm \
    -v "${volume_name}:/volume" \
    -v "${INPUT_DIR}/volumes:/backup:ro" \
    "${TOOL_IMAGE}" \
    sh -c "tar -C /volume -xzf /backup/${key}.tar.gz"
}

restore_volumes() {
  if [[ "${RESTORE_VOLUMES}" -ne 1 ]]; then
    log "Skipping named-volume restore (--skip-volumes)."
    return
  fi

  for key in "${VOLUME_KEYS[@]}"; do
    restore_named_volume "${key}"
  done
}

wait_for_postgres() {
  local pg_user pg_db timeout elapsed
  pg_user="${POSTGRES_USER:-cloudops}"
  pg_db="${POSTGRES_DB:-cloudops}"
  timeout=120
  elapsed=0

  log "Waiting for PostgreSQL readiness..."
  until compose exec -T postgres pg_isready -U "${pg_user}" -d "${pg_db}" >/dev/null 2>&1; do
    sleep 2
    elapsed=$((elapsed + 2))
    if (( elapsed >= timeout )); then
      fatal "postgres did not become ready within ${timeout}s"
    fi
  done
}

restore_postgres_dump() {
  if [[ "${RESTORE_POSTGRES}" -ne 1 ]]; then
    log "Skipping PostgreSQL restore (--skip-postgres)."
    return
  fi

  local dump_file pg_user pg_db
  dump_file="${INPUT_DIR}/postgres/pg_dump.sql.gz"
  pg_user="${POSTGRES_USER:-cloudops}"
  pg_db="${POSTGRES_DB:-cloudops}"

  if [[ ! -f "${dump_file}" ]]; then
    warn "PostgreSQL dump not found, skipping: ${dump_file}"
    return
  fi

  log "Starting PostgreSQL service..."
  compose up -d postgres >/dev/null
  wait_for_postgres

  log "Restoring PostgreSQL dump..."
  gunzip -c "${dump_file}" | compose exec -T postgres \
    psql -v ON_ERROR_STOP=1 -U "${pg_user}" -d "${pg_db}" >/dev/null
}

start_stack() {
  log "Starting full stack..."
  if compose up -d >/dev/null; then
    return
  fi

  warn "Full stack start failed (likely missing app images or no registry access)."
  warn "Falling back to infrastructure services only."
  compose up -d "${INFRA_SERVICES[@]}" >/dev/null || fatal "failed to start infrastructure services"
}

print_summary() {
  log "Restore completed."
  log "Input directory: ${INPUT_DIR}"
  log "Suggested checks:"
  log "  ${COMPOSE_CMD_LABEL} --project-name ${PROJECT_NAME} --env-file ${ENV_FILE} -f ${COMPOSE_FILE} ps"
  log "  curl -I http://127.0.0.1:8082"
  log "  curl -fsS http://127.0.0.1:8080/health  # if api service is running"
}

main() {
  parse_args "$@"
  require_cmd docker
  require_cmd gunzip
  require_cmd tar
  detect_compose_cmd

  [[ -f "${COMPOSE_FILE}" ]] || fatal "compose file not found: ${COMPOSE_FILE}"
  validate_input_dir
  load_env_file
  detect_tool_image

  log "Using compose command: ${COMPOSE_CMD_LABEL}"
  log "Using compose file: ${COMPOSE_FILE}"
  log "Using env file: ${ENV_FILE}"
  log "Using project name: ${PROJECT_NAME}"
  log "Using tool image: ${TOOL_IMAGE}"

  confirm_restore
  stop_stack
  restore_volumes
  restore_postgres_dump
  start_stack
  print_summary
}

main "$@"
