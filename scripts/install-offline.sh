#!/usr/bin/env bash
set -Eeuo pipefail

NO_LOAD=0
FORCE_ENV=0

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
DEPLOY_DIR="${ROOT_DIR}/deploy"
IMAGES_DIR="${ROOT_DIR}/images"
ENV_FILE="${DEPLOY_DIR}/.env"
ENV_OFFLINE_FILE="${DEPLOY_DIR}/.env.offline"
INSTALL_SCRIPT="${ROOT_DIR}/scripts/install.sh"
DOCKER_OFFLINE_INSTALLER="${ROOT_DIR}/docker/install-docker-offline.sh"

usage() {
  cat <<'EOF'
CloudOps One offline installer

Usage:
  scripts/install-offline.sh [options]

Options:
  --no-load       Skip docker image loading
  --force-env     Overwrite deploy/.env from deploy/.env.offline
  -h, --help      Show this help message
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

parse_args() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --no-load)
        NO_LOAD=1
        shift
        ;;
      --force-env)
        FORCE_ENV=1
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

ensure_docker_available() {
  if command_exists docker; then
    return
  fi

  if [[ -x "${DOCKER_OFFLINE_INSTALLER}" ]]; then
    warn "Docker is not installed. Running bundled offline Docker installer..."
    bash "${DOCKER_OFFLINE_INSTALLER}"
  fi

  command_exists docker || fatal "Docker is not installed. Provide bundled Docker installer or install Docker manually."
}

prepare_env() {
  if [[ ! -f "${ENV_OFFLINE_FILE}" ]]; then
    fatal "Missing ${ENV_OFFLINE_FILE}. This offline package is incomplete."
  fi

  if [[ ! -f "${ENV_FILE}" || "${FORCE_ENV}" -eq 1 ]]; then
    cp "${ENV_OFFLINE_FILE}" "${ENV_FILE}"
    log "Prepared deploy/.env from deploy/.env.offline."
    return
  fi

  log "Keeping existing deploy/.env. Use --force-env to overwrite."
}

load_images() {
  if [[ "${NO_LOAD}" -eq 1 ]]; then
    warn "Skipping docker image loading as requested."
    return
  fi

  [[ -d "${IMAGES_DIR}" ]] || fatal "Missing images directory: ${IMAGES_DIR}"

  local loaded=0
  if [[ -f "${IMAGES_DIR}/cloudops-images.tar" ]]; then
    log "Loading bundled image archive: images/cloudops-images.tar"
    docker load -i "${IMAGES_DIR}/cloudops-images.tar"
    loaded=1
  fi

  while IFS= read -r -d '' tar_file; do
    if [[ "$(basename "${tar_file}")" == "cloudops-images.tar" ]]; then
      continue
    fi
    log "Loading bundled image archive: ${tar_file#${ROOT_DIR}/}"
    docker load -i "${tar_file}"
    loaded=1
  done < <(find "${IMAGES_DIR}" -maxdepth 1 -type f -name '*.tar' -print0)

  [[ "${loaded}" -eq 1 ]] || fatal "No image archive found in ${IMAGES_DIR}."
}

run_online_installer_no_pull() {
  [[ -x "${INSTALL_SCRIPT}" ]] || fatal "Missing base installer: ${INSTALL_SCRIPT}"
  log "Starting stack from local images only..."
  bash "${INSTALL_SCRIPT}" --skip-docker-install --no-pull --mirror auto
}

main() {
  parse_args "$@"
  ensure_docker_available
  prepare_env
  load_images
  run_online_installer_no_pull
}

main "$@"
