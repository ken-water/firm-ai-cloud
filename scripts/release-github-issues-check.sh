#!/usr/bin/env bash
set -Eeuo pipefail

VERSION=""
NOTES_FILE=""
MIN_ENFORCED_VERSION="${MIN_ENFORCED_VERSION:-0.1.7}"
MIN_DEMO_ACCEPTANCE_VERSION="${MIN_DEMO_ACCEPTANCE_VERSION:-0.1.25}"

log() {
  printf '[release-issues-gate] %s\n' "$*"
}

fatal() {
  printf '[release-issues-gate][ERROR] %s\n' "$*" >&2
  exit 1
}

usage() {
  cat <<'USAGE'
GitHub issue closure gate for release notes.

Usage:
  bash scripts/release-github-issues-check.sh --version <x.y.z> --notes-file <path>

Options:
  --version <x.y.z>    Release version without leading v (required)
  --notes-file <path>  Release note file path (required)
  -h, --help           Show help
USAGE
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || fatal "missing required command: $1"
}

semver_ge() {
  local left="$1"
  local right="$2"
  [[ "$(printf '%s\n%s\n' "${right}" "${left}" | sort -V | head -n 1)" == "${right}" ]]
}

extract_issues_line() {
  local notes_file="$1"
  sed -n 's/^- GitHub issues: `\(.*\)`$/\1/p' "${notes_file}" | head -n 1
}

extract_demo_acceptance_line() {
  local notes_file="$1"
  sed -n 's/^- Demo acceptance artifacts: `\(.*\)`$/\1/p' "${notes_file}" | head -n 1
}

main() {
  require_cmd gh
  require_cmd sed
  require_cmd sort
  require_cmd head
  require_cmd tr

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --version)
        [[ $# -ge 2 ]] || fatal "--version requires a value"
        VERSION="$2"
        shift 2
        ;;
      --notes-file)
        [[ $# -ge 2 ]] || fatal "--notes-file requires a value"
        NOTES_FILE="$2"
        shift 2
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

  [[ -n "${VERSION}" ]] || fatal "--version is required"
  [[ "${VERSION}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || fatal "invalid version format: ${VERSION}"
  [[ -n "${NOTES_FILE}" ]] || fatal "--notes-file is required"
  [[ -f "${NOTES_FILE}" ]] || fatal "release note file not found: ${NOTES_FILE}"

  if ! semver_ge "${VERSION}" "${MIN_ENFORCED_VERSION}"; then
    log "skip issue closure gate for ${VERSION} (< ${MIN_ENFORCED_VERSION})"
    exit 0
  fi

  local raw
  raw="$(extract_issues_line "${NOTES_FILE}")"
  [[ -n "${raw}" ]] || fatal "missing release metadata field: - GitHub issues: \`#123, #124\`"

  local normalized
  normalized="$(echo "${raw}" | tr ',' '\n' | sed 's/^[[:space:]]*//;s/[[:space:]]*$//' | sed '/^$/d')"
  [[ -n "${normalized}" ]] || fatal "GitHub issues list is empty in ${NOTES_FILE}"

  local -a issue_numbers=()
  local token
  while IFS= read -r token; do
    [[ "${token}" =~ ^#[0-9]+$ ]] || fatal "invalid issue token '${token}' in ${NOTES_FILE}; use format #123"
    issue_numbers+=("${token#\#}")
  done <<< "${normalized}"

  local issue_number state
  for issue_number in "${issue_numbers[@]}"; do
    state="$(gh issue view "${issue_number}" --json state --jq '.state' 2>/dev/null || true)"
    [[ -n "${state}" ]] || fatal "failed to query GitHub issue #${issue_number}"
    [[ "${state}" == "CLOSED" ]] || fatal "GitHub issue #${issue_number} is ${state}, expected CLOSED"
  done

  if semver_ge "${VERSION}" "${MIN_DEMO_ACCEPTANCE_VERSION}"; then
    local demo_raw demo_paths path_token
    demo_raw="$(extract_demo_acceptance_line "${NOTES_FILE}")"
    [[ -n "${demo_raw}" ]] || fatal "missing release metadata field: - Demo acceptance artifacts: \`path1, path2\`"

    demo_paths="$(echo "${demo_raw}" | tr ',' '\n' | sed 's/^[[:space:]]*//;s/[[:space:]]*$//' | sed '/^$/d')"
    [[ -n "${demo_paths}" ]] || fatal "demo acceptance artifacts list is empty in ${NOTES_FILE}"

    while IFS= read -r path_token; do
      [[ -n "${path_token}" ]] || continue
      [[ -f "${path_token}" ]] || fatal "demo acceptance artifact not found: ${path_token}"
    done <<< "${demo_paths}"
  fi

  log "OK: all listed GitHub issues are CLOSED for v${VERSION}"
}

main "$@"
