#!/usr/bin/env bash
set -Eeuo pipefail

MIRROR="cn"
SKIP_PULL=0
OUTPUT_DIR=""
BUNDLE_NAME=""
NO_ARCHIVE=0

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
DIST_DIR="${ROOT_DIR}/dist"

usage() {
  cat <<'EOF'
CloudOps One offline bundle builder

Usage:
  scripts/build-offline-bundle.sh [options]

Options:
  --mirror default|cn    Image source profile for bundle images (default: cn)
  --skip-pull            Do not pull images before saving
  --output-dir <dir>     Output directory (default: ./dist)
  --bundle-name <name>   Bundle folder name (default: cloudops-one-offline-<timestamp>)
  --no-archive           Do not create .tar.gz archive
  -h, --help             Show this help message
EOF
}

log() {
  printf '[INFO] %s\n' "$*"
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
      --mirror)
        [[ $# -ge 2 ]] || fatal "--mirror requires a value"
        MIRROR="$2"
        shift 2
        ;;
      --skip-pull)
        SKIP_PULL=1
        shift
        ;;
      --output-dir)
        [[ $# -ge 2 ]] || fatal "--output-dir requires a value"
        OUTPUT_DIR="$2"
        shift 2
        ;;
      --bundle-name)
        [[ $# -ge 2 ]] || fatal "--bundle-name requires a value"
        BUNDLE_NAME="$2"
        shift 2
        ;;
      --no-archive)
        NO_ARCHIVE=1
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

  case "${MIRROR}" in
    default|cn)
      ;;
    *)
      fatal "Unsupported mirror: ${MIRROR}. Use default or cn."
      ;;
  esac
}

resolve_settings() {
  if [[ -z "${OUTPUT_DIR}" ]]; then
    OUTPUT_DIR="${DIST_DIR}"
  fi

  if [[ -z "${BUNDLE_NAME}" ]]; then
    BUNDLE_NAME="cloudops-one-offline-$(date +%Y%m%d-%H%M%S)"
  fi

  case "${MIRROR}" in
    default)
      POSTGRES_IMAGE="postgres:16-alpine"
      REDIS_IMAGE="redis:7-alpine"
      OPENSEARCH_IMAGE="opensearchproject/opensearch:2.15.0"
      MINIO_IMAGE="minio/minio:RELEASE.2025-01-20T14-49-07Z"
      ;;
    cn)
      POSTGRES_IMAGE="docker.1ms.run/library/postgres:16-alpine"
      REDIS_IMAGE="docker.1ms.run/library/redis:7-alpine"
      OPENSEARCH_IMAGE="docker.1ms.run/opensearchproject/opensearch:2.15.0"
      MINIO_IMAGE="docker.1ms.run/minio/minio:RELEASE.2025-01-20T14-49-07Z"
      ;;
  esac

  IMAGES=("${POSTGRES_IMAGE}" "${REDIS_IMAGE}" "${OPENSEARCH_IMAGE}" "${MINIO_IMAGE}")
  BUNDLE_DIR="${OUTPUT_DIR}/${BUNDLE_NAME}"
  ARCHIVE_PATH="${OUTPUT_DIR}/${BUNDLE_NAME}.tar.gz"
}

ensure_tools() {
  command_exists docker || fatal "Docker is required to build offline bundle."
  command_exists tar || fatal "tar is required to build offline bundle."
}

pull_images_if_needed() {
  if [[ "${SKIP_PULL}" -eq 1 ]]; then
    log "Skipping image pull as requested."
    return
  fi

  for img in "${IMAGES[@]}"; do
    log "Pulling ${img}"
    docker pull "${img}"
  done
}

copy_bundle_files() {
  rm -rf "${BUNDLE_DIR}"
  mkdir -p "${BUNDLE_DIR}/deploy" "${BUNDLE_DIR}/scripts" "${BUNDLE_DIR}/docs" "${BUNDLE_DIR}/images" "${BUNDLE_DIR}/docker"

  cp "${ROOT_DIR}/deploy/docker-compose.yml" "${BUNDLE_DIR}/deploy/docker-compose.yml"
  cp "${ROOT_DIR}/deploy/.env.example" "${BUNDLE_DIR}/deploy/.env.example"
  cp "${ROOT_DIR}/deploy/.env.cn.example" "${BUNDLE_DIR}/deploy/.env.cn.example"
  cp "${ROOT_DIR}/scripts/install.sh" "${BUNDLE_DIR}/scripts/install.sh"
  cp "${ROOT_DIR}/scripts/install-offline.sh" "${BUNDLE_DIR}/scripts/install-offline.sh"
  cp "${ROOT_DIR}/scripts/upgrade.sh" "${BUNDLE_DIR}/scripts/upgrade.sh"
  cp "${ROOT_DIR}/scripts/uninstall.sh" "${BUNDLE_DIR}/scripts/uninstall.sh"
  cp "${ROOT_DIR}/docs/05-installation.md" "${BUNDLE_DIR}/docs/05-installation.md"
  cp "${ROOT_DIR}/docs/06-offline-installation.md" "${BUNDLE_DIR}/docs/06-offline-installation.md"
  cp "${ROOT_DIR}/README.md" "${BUNDLE_DIR}/README.md"

  if [[ -d "${ROOT_DIR}/docker" ]]; then
    cp -R "${ROOT_DIR}/docker/." "${BUNDLE_DIR}/docker/"
  fi

  chmod +x "${BUNDLE_DIR}/scripts/"*.sh
  if [[ -x "${BUNDLE_DIR}/docker/install-docker-offline.sh" ]]; then
    chmod +x "${BUNDLE_DIR}/docker/install-docker-offline.sh"
  fi
}

write_offline_env() {
  local env_src="${ROOT_DIR}/deploy/.env.example"
  if [[ "${MIRROR}" == "cn" ]]; then
    env_src="${ROOT_DIR}/deploy/.env.cn.example"
  fi

  cp "${env_src}" "${BUNDLE_DIR}/deploy/.env.offline"
  cp "${BUNDLE_DIR}/deploy/.env.offline" "${BUNDLE_DIR}/deploy/.env"
}

save_images() {
  local image_tar="${BUNDLE_DIR}/images/cloudops-images.tar"
  log "Saving images to ${image_tar}"
  docker save -o "${image_tar}" "${IMAGES[@]}"

  if command_exists sha256sum; then
    (
      cd "${BUNDLE_DIR}"
      sha256sum images/cloudops-images.tar > SHA256SUMS
    )
  fi
}

write_manifest() {
  cat > "${BUNDLE_DIR}/BUNDLE_MANIFEST.txt" <<EOF
bundle_name=${BUNDLE_NAME}
created_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)
mirror_profile=${MIRROR}
postgres_image=${POSTGRES_IMAGE}
redis_image=${REDIS_IMAGE}
opensearch_image=${OPENSEARCH_IMAGE}
minio_image=${MINIO_IMAGE}
install_command=bash scripts/install-offline.sh
EOF
}

archive_bundle() {
  if [[ "${NO_ARCHIVE}" -eq 1 ]]; then
    return
  fi

  rm -f "${ARCHIVE_PATH}"
  (
    cd "${OUTPUT_DIR}"
    tar -czf "${ARCHIVE_PATH}" "${BUNDLE_NAME}"
  )
}

print_summary() {
  cat <<EOF

Offline bundle build complete.

Bundle directory:
  ${BUNDLE_DIR}
EOF

  if [[ "${NO_ARCHIVE}" -eq 0 ]]; then
    cat <<EOF
Bundle archive:
  ${ARCHIVE_PATH}
EOF
  fi

  cat <<'EOF'

Customer-side install command (after extracting bundle):
  bash scripts/install-offline.sh
EOF
}

main() {
  parse_args "$@"
  resolve_settings
  ensure_tools
  mkdir -p "${OUTPUT_DIR}"
  pull_images_if_needed
  copy_bundle_files
  write_offline_env
  save_images
  write_manifest
  archive_bundle
  print_summary
}

main "$@"
