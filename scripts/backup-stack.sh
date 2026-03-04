#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMPOSE_FILE="${ROOT_DIR}/deploy/docker-compose.yml"
ENV_FILE="${ROOT_DIR}/deploy/.env"
PROJECT_NAME="deploy"
OUTPUT_DIR=""
BACKUP_POSTGRES=1
BACKUP_VOLUMES=1
TOOL_IMAGE=""
TIMESTAMP="$(date -u +%Y%m%dT%H%M%SZ)"
COMPOSE_CMD=()
COMPOSE_CMD_LABEL=""

VOLUME_KEYS=(
  "postgres-data"
  "minio-data"
  "opensearch-data"
  "zabbix-db-data"
  "zabbix-proxy-data"
)

log() {
  printf '[backup-stack] %s\n' "$*"
}

warn() {
  printf '[backup-stack][WARN] %s\n' "$*" >&2
}

fatal() {
  printf '[backup-stack][ERROR] %s\n' "$*" >&2
  exit 1
}

usage() {
  cat <<'USAGE'
CloudOps stack backup helper

Usage:
  bash scripts/backup-stack.sh [options]

Options:
  --env-file <path>        Env file for docker compose (default: deploy/.env)
  --compose-file <path>    Compose file (default: deploy/docker-compose.yml)
  --project-name <name>    Compose project name (default: deploy)
  --output-dir <path>      Output directory (default: backups/stack-backup-<timestamp>)
  --tool-image <image>     Image used for volume tar operations (default: POSTGRES_IMAGE from env)
  --skip-postgres          Skip PostgreSQL logical dump
  --skip-volumes           Skip named-volume archive backup
  -h, --help               Show this help
USAGE
}

parse_args() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
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
      --output-dir)
        [[ $# -ge 2 ]] || fatal "--output-dir requires a value"
        OUTPUT_DIR="$2"
        shift 2
        ;;
      --tool-image)
        [[ $# -ge 2 ]] || fatal "--tool-image requires a value"
        TOOL_IMAGE="$2"
        shift 2
        ;;
      --skip-postgres)
        BACKUP_POSTGRES=0
        shift
        ;;
      --skip-volumes)
        BACKUP_VOLUMES=0
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

prepare_output_dir() {
  if [[ -z "${OUTPUT_DIR}" ]]; then
    OUTPUT_DIR="${ROOT_DIR}/backups/stack-backup-${TIMESTAMP}"
  fi
  mkdir -p "${OUTPUT_DIR}"
  mkdir -p "${OUTPUT_DIR}/postgres" "${OUTPUT_DIR}/volumes"
}

detect_tool_image() {
  if [[ -n "${TOOL_IMAGE}" ]]; then
    return
  fi
  TOOL_IMAGE="${POSTGRES_IMAGE:-postgres:16-alpine}"
}

backup_postgres_dump() {
  if [[ "${BACKUP_POSTGRES}" -ne 1 ]]; then
    log "Skipping PostgreSQL dump (--skip-postgres)."
    return
  fi

  local pg_user pg_db dump_file
  pg_user="${POSTGRES_USER:-cloudops}"
  pg_db="${POSTGRES_DB:-cloudops}"
  dump_file="${OUTPUT_DIR}/postgres/pg_dump.sql.gz"

  log "Ensuring PostgreSQL service is running..."
  compose up -d postgres >/dev/null

  log "Creating PostgreSQL logical dump..."
  compose exec -T postgres \
    pg_dump -U "${pg_user}" --clean --if-exists --no-owner --no-privileges "${pg_db}" \
    | gzip -9 >"${dump_file}"

  [[ -s "${dump_file}" ]] || fatal "PostgreSQL dump file is empty: ${dump_file}"
  log "PostgreSQL dump saved: ${dump_file}"
}

backup_named_volume() {
  local key volume_name archive_path
  key="$1"
  volume_name="${PROJECT_NAME}_${key}"
  archive_path="${OUTPUT_DIR}/volumes/${key}.tar.gz"

  if ! docker volume inspect "${volume_name}" >/dev/null 2>&1; then
    warn "Volume not found, skipping: ${volume_name}"
    return
  fi

  log "Archiving volume ${volume_name}..."
  docker run --rm \
    -v "${volume_name}:/volume:ro" \
    -v "${OUTPUT_DIR}/volumes:/backup" \
    "${TOOL_IMAGE}" \
    sh -c "tar -C /volume -czf /backup/${key}.tar.gz ."

  [[ -s "${archive_path}" ]] || fatal "Volume archive is empty: ${archive_path}"
}

backup_volumes() {
  if [[ "${BACKUP_VOLUMES}" -ne 1 ]]; then
    log "Skipping named-volume backup (--skip-volumes)."
    return
  fi

  for key in "${VOLUME_KEYS[@]}"; do
    backup_named_volume "${key}"
  done
}

write_metadata() {
  local metadata_file
  metadata_file="${OUTPUT_DIR}/metadata.env"

  {
    echo "backup_timestamp=${TIMESTAMP}"
    echo "project_name=${PROJECT_NAME}"
    echo "compose_file=${COMPOSE_FILE}"
    echo "env_file=${ENV_FILE}"
    echo "backup_postgres=${BACKUP_POSTGRES}"
    echo "backup_volumes=${BACKUP_VOLUMES}"
    echo "tool_image=${TOOL_IMAGE}"
    echo "postgres_user=${POSTGRES_USER:-cloudops}"
    echo "postgres_db=${POSTGRES_DB:-cloudops}"
  } >"${metadata_file}"

  (
    cd "${OUTPUT_DIR}"
    find . -type f ! -name "SHA256SUMS" -print0 | sort -z | xargs -0 sha256sum >SHA256SUMS
  )
}

print_summary() {
  log "Backup completed."
  log "Output directory: ${OUTPUT_DIR}"
  log "Metadata file: ${OUTPUT_DIR}/metadata.env"
  log "Checksums file: ${OUTPUT_DIR}/SHA256SUMS"
  log "Restore command example:"
  log "  bash scripts/restore-stack.sh --input-dir ${OUTPUT_DIR} --yes"
}

main() {
  parse_args "$@"
  require_cmd docker
  require_cmd gzip
  require_cmd tar
  require_cmd sha256sum
  detect_compose_cmd

  [[ -f "${COMPOSE_FILE}" ]] || fatal "compose file not found: ${COMPOSE_FILE}"
  load_env_file
  prepare_output_dir
  detect_tool_image

  log "Using compose command: ${COMPOSE_CMD_LABEL}"
  log "Using compose file: ${COMPOSE_FILE}"
  log "Using env file: ${ENV_FILE}"
  log "Using project name: ${PROJECT_NAME}"
  log "Using tool image: ${TOOL_IMAGE}"

  backup_postgres_dump
  backup_volumes
  write_metadata
  print_summary
}

main "$@"
