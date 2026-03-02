#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PKG_DIR="${SCRIPT_DIR}/packages"

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

run_root() {
  if [[ "${EUID}" -eq 0 ]]; then
    "$@"
    return
  fi

  if command_exists sudo; then
    sudo "$@"
    return
  fi

  fatal "Root privileges are required to install Docker."
}

install_deb() {
  shopt -s nullglob
  local debs=("${PKG_DIR}"/*.deb)
  shopt -u nullglob
  [[ "${#debs[@]}" -gt 0 ]] || fatal "No .deb packages found in ${PKG_DIR}."

  log "Installing Docker from local .deb packages..."
  run_root apt-get install -y "${debs[@]}"
}

install_rpm() {
  shopt -s nullglob
  local rpms=("${PKG_DIR}"/*.rpm)
  shopt -u nullglob
  [[ "${#rpms[@]}" -gt 0 ]] || fatal "No .rpm packages found in ${PKG_DIR}."

  if command_exists dnf; then
    log "Installing Docker from local .rpm packages via dnf..."
    run_root dnf install -y "${rpms[@]}"
    return
  fi

  if command_exists yum; then
    log "Installing Docker from local .rpm packages via yum..."
    run_root yum localinstall -y "${rpms[@]}"
    return
  fi

  fatal "No rpm package manager found (dnf/yum)."
}

start_docker() {
  if command_exists systemctl; then
    run_root systemctl enable --now docker
    return
  fi
  if command_exists service; then
    run_root service docker start
    return
  fi
  fatal "Cannot start Docker service automatically on this host."
}

main() {
  if command_exists docker; then
    log "Docker already installed. Skipping offline Docker installation."
    exit 0
  fi

  [[ -d "${PKG_DIR}" ]] || fatal "Missing package directory: ${PKG_DIR}"

  if command_exists apt-get; then
    install_deb
  elif command_exists dnf || command_exists yum; then
    install_rpm
  else
    fatal "Unsupported package manager for offline Docker installation."
  fi

  start_docker

  command_exists docker || fatal "Docker installation did not complete successfully."
  log "Offline Docker installation completed."
}

main "$@"
