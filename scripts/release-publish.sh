#!/usr/bin/env bash
set -Eeuo pipefail

VERSION=""
REMOTE="${REMOTE:-origin}"
MARK_LATEST=1
DRY_RUN=0

log() {
  printf '[release-publish] %s\n' "$*"
}

fatal() {
  printf '[release-publish][ERROR] %s\n' "$*" >&2
  exit 1
}

usage() {
  cat <<'USAGE'
Publish (or reconcile) a CloudOps One release from release-note artifacts.

Usage:
  bash scripts/release-publish.sh --version <x.y.z> [options]

Options:
  --version <x.y.z>   Target version to publish (required)
  --remote <name>     Git remote name for tag sync (default: origin)
  --no-latest         Do not force this release to be marked as Latest
  --dry-run           Print planned actions without changing git/GitHub state
  -h, --help          Show help
USAGE
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || fatal "missing required command: $1"
}

release_title_from_file() {
  local notes_file="$1"
  local default_title="$2"
  local first_line=""

  first_line="$(awk 'NR==1 {print; exit}' "${notes_file}")"
  if [[ "${first_line}" =~ ^#\  ]]; then
    printf '%s' "${first_line#\# }"
    return
  fi

  printf '%s' "${default_title}"
}

changelog_date_for_version() {
  local version="$1"
  awk -v version="${version}" '
    $0 ~ "^## \\[" version "\\] - [0-9]{4}-[0-9]{2}-[0-9]{2}$" {
      print $4
      exit
    }
  ' CHANGELOG.md
}

is_release_existing() {
  local tag="$1"
  gh release view "${tag}" --json tagName >/dev/null 2>&1
}

remote_tag_commit() {
  local remote="$1"
  local tag="$2"
  local sha=""

  sha="$(git ls-remote --tags "${remote}" "refs/tags/${tag}^{}" | awk 'NR==1 {print $1}')"
  if [[ -z "${sha}" ]]; then
    sha="$(git ls-remote --tags "${remote}" "refs/tags/${tag}" | awk 'NR==1 {print $1}')"
  fi

  printf '%s' "${sha}"
}

run_cmd() {
  if (( DRY_RUN == 1 )); then
    printf '[release-publish][dry-run] %s\n' "$*"
    return
  fi

  "$@"
}

main() {
  require_cmd git
  require_cmd gh
  require_cmd awk
  require_cmd grep
  require_cmd jq

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --version)
        [[ $# -ge 2 ]] || fatal "--version requires a value"
        VERSION="$2"
        shift 2
        ;;
      --remote)
        [[ $# -ge 2 ]] || fatal "--remote requires a value"
        REMOTE="$2"
        shift 2
        ;;
      --no-latest)
        MARK_LATEST=0
        shift
        ;;
      --dry-run)
        DRY_RUN=1
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

  [[ -n "${VERSION}" ]] || fatal "--version is required"
  [[ "${VERSION}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || fatal "invalid version format: ${VERSION}"
  [[ -f CHANGELOG.md ]] || fatal "CHANGELOG.md not found"
  git remote get-url "${REMOTE}" >/dev/null 2>&1 || fatal "git remote not found: ${REMOTE}"

  if ! git diff --quiet || ! git diff --cached --quiet; then
    fatal "working tree has tracked changes; commit or stash before publishing"
  fi

  local tag="v${VERSION}"
  local escaped_version
  escaped_version="${VERSION//./\\.}"
  local notes_file="release-notes/${tag}.md"
  local changelog_date
  local release_title

  grep -Eq "^## \[${escaped_version}\] - [0-9]{4}-[0-9]{2}-[0-9]{2}$" CHANGELOG.md \
    || fatal "CHANGELOG.md missing exact entry header for version ${VERSION}"

  changelog_date="$(changelog_date_for_version "${VERSION}")"
  [[ -n "${changelog_date}" ]] || fatal "failed to read changelog date for version ${VERSION}"
  [[ -f "${notes_file}" ]] || fatal "release note file not found: ${notes_file}"

  bash scripts/validate-release-note.sh "${notes_file}" >/dev/null
  grep -Eq "^- Version: \`v${escaped_version}\`$" "${notes_file}" \
    || fatal "release note metadata version mismatch in ${notes_file}"
  grep -Eq "^- Git tag: \`v${escaped_version}\`$" "${notes_file}" \
    || fatal "release note metadata git tag mismatch in ${notes_file}"
  grep -Eq "^- Release date: \`${changelog_date}\`$" "${notes_file}" \
    || fatal "release note date must match CHANGELOG.md date (${changelog_date})"

  release_title="$(release_title_from_file "${notes_file}" "CloudOps One ${tag}")"
  log "publishing version=${VERSION} tag=${tag} remote=${REMOTE}"

  if git rev-parse -q --verify "refs/tags/${tag}" >/dev/null; then
    log "local tag already exists: ${tag}"
  else
    log "local tag not found: ${tag}"
  fi

  local local_tag_exists=0
  local local_tag_commit=""
  local remote_tag_commit_value=""
  local head_commit=""

  if git rev-parse -q --verify "refs/tags/${tag}" >/dev/null; then
    local_tag_exists=1
    local_tag_commit="$(git rev-list -n 1 "${tag}")"
  fi
  remote_tag_commit_value="$(remote_tag_commit "${REMOTE}" "${tag}")"

  if (( local_tag_exists == 1 )) && [[ -n "${remote_tag_commit_value}" ]]; then
    :
  elif (( local_tag_exists == 1 )) && [[ -z "${remote_tag_commit_value}" ]]; then
    log "pushing existing local tag ${tag} to ${REMOTE}"
    if (( DRY_RUN == 1 )); then
      printf '[release-publish][dry-run] git push %s refs/tags/%s\n' "${REMOTE}" "${tag}"
      remote_tag_commit_value="${local_tag_commit}"
    else
      git push "${REMOTE}" "refs/tags/${tag}"
      remote_tag_commit_value="$(remote_tag_commit "${REMOTE}" "${tag}")"
    fi
  elif (( local_tag_exists == 0 )) && [[ -n "${remote_tag_commit_value}" ]]; then
    log "fetching remote tag ${tag} from ${REMOTE}"
    if (( DRY_RUN == 1 )); then
      printf '[release-publish][dry-run] git fetch %s refs/tags/%s:refs/tags/%s\n' "${REMOTE}" "${tag}" "${tag}"
      local_tag_commit="${remote_tag_commit_value}"
      local_tag_exists=1
    else
      git fetch "${REMOTE}" "refs/tags/${tag}:refs/tags/${tag}"
      local_tag_commit="$(git rev-list -n 1 "${tag}")"
      local_tag_exists=1
    fi
  else
    head_commit="$(git rev-parse HEAD)"
    log "creating and pushing new tag ${tag} at HEAD (${head_commit})"
    if (( DRY_RUN == 1 )); then
      printf '[release-publish][dry-run] git tag %s\n' "${tag}"
      printf '[release-publish][dry-run] git push %s refs/tags/%s\n' "${REMOTE}" "${tag}"
      local_tag_commit="${head_commit}"
      remote_tag_commit_value="${head_commit}"
      local_tag_exists=1
    else
      git tag "${tag}"
      git push "${REMOTE}" "refs/tags/${tag}"
      local_tag_commit="$(git rev-list -n 1 "${tag}")"
      remote_tag_commit_value="$(remote_tag_commit "${REMOTE}" "${tag}")"
      local_tag_exists=1
    fi
  fi

  (( local_tag_exists == 1 )) || fatal "failed to ensure local tag ${tag}"
  [[ -n "${local_tag_commit}" ]] || local_tag_commit="$(git rev-list -n 1 "${tag}")"
  [[ -n "${remote_tag_commit_value}" ]] || fatal "failed to ensure remote tag ${tag} on ${REMOTE}"

  [[ "${local_tag_commit}" == "${remote_tag_commit_value}" ]] \
    || fatal "local tag ${tag} (${local_tag_commit}) differs from remote ${REMOTE} (${remote_tag_commit_value})"

  local latest_flag=()
  local sync_latest_flag=()
  if (( MARK_LATEST == 1 )); then
    latest_flag+=(--latest)
  else
    sync_latest_flag+=(--no-enforce-latest)
  fi

  if is_release_existing "${tag}"; then
    log "release already exists, updating notes/title"
    run_cmd gh release edit "${tag}" \
      --draft=false \
      --prerelease=false \
      --title "${release_title}" \
      --notes-file "${notes_file}" \
      "${latest_flag[@]}"
  else
    log "creating GitHub release ${tag}"
    run_cmd gh release create "${tag}" \
      --verify-tag \
      --draft=false \
      --prerelease=false \
      --title "${release_title}" \
      --notes-file "${notes_file}" \
      "${latest_flag[@]}"
  fi

  if (( DRY_RUN == 0 )); then
    bash scripts/release-sync-check.sh --version "${VERSION}" --remote "${REMOTE}" "${sync_latest_flag[@]}"
    local release_url
    release_url="$(gh release view "${tag}" --json url | jq -r '.url')"
    log "DONE: ${release_url}"
  else
    log "dry-run completed"
  fi
}

main "$@"
