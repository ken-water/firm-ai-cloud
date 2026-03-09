#!/usr/bin/env bash
set -Eeuo pipefail

VERSION=""
ENFORCE_LATEST=1
REMOTE="${REMOTE:-origin}"

log() {
  printf '[release-check] %s\n' "$*"
}

fatal() {
  printf '[release-check][ERROR] %s\n' "$*" >&2
  exit 1
}

usage() {
  cat <<'USAGE'
Release synchronization check

Usage:
  bash scripts/release-sync-check.sh [options]

Options:
  --version <x.y.z>      Check a specific version (default: latest released version in CHANGELOG.md)
  --remote <name>        Git remote name (default: origin)
  --no-enforce-latest    Skip check that this tag is the current "Latest" GitHub release
  -h, --help             Show help
USAGE
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || fatal "missing required command: $1"
}

latest_changelog_version() {
  awk '
    /^## \[[0-9]+\.[0-9]+\.[0-9]+\] - / {
      version = $2
      gsub(/^\[/, "", version)
      gsub(/\]$/, "", version)
      print version
      exit
    }
  ' CHANGELOG.md
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

latest_release_tag() {
  gh release list --limit 50 --json tagName,isLatest,publishedAt,isDraft,isPrerelease \
    | jq -r '
        (
          map(select(.isLatest == true and (.isDraft | not) and (.isPrerelease | not)))[0].tagName
        ) //
        (
          map(select((.isDraft | not) and (.isPrerelease | not)))
          | sort_by(.publishedAt)
          | reverse
          | .[0].tagName
        ) //
        ""
      '
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
      --no-enforce-latest)
        ENFORCE_LATEST=0
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

  [[ -f CHANGELOG.md ]] || fatal "CHANGELOG.md not found"

  if [[ -z "${VERSION}" ]]; then
    VERSION="$(latest_changelog_version)"
    [[ -n "${VERSION}" ]] || fatal "failed to detect latest released version from CHANGELOG.md"
  fi

  [[ "${VERSION}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || fatal "invalid version format: ${VERSION}"

  local tag="v${VERSION}"
  local escaped_version
  local changelog_date
  escaped_version="${VERSION//./\\.}"

  log "checking version=${VERSION} tag=${tag} remote=${REMOTE}"
  git remote get-url "${REMOTE}" >/dev/null 2>&1 || fatal "git remote not found: ${REMOTE}"

  grep -Eq "^## \[${escaped_version}\] - [0-9]{4}-[0-9]{2}-[0-9]{2}$" CHANGELOG.md \
    || fatal "CHANGELOG.md missing exact entry header for version ${VERSION}"

  changelog_date="$(changelog_date_for_version "${VERSION}")"
  [[ -n "${changelog_date}" ]] || fatal "failed to read changelog date for version ${VERSION}"

  local notes_file="release-notes/${tag}.md"
  [[ -f "${notes_file}" ]] || fatal "release note file not found: ${notes_file}"

  bash scripts/validate-release-note.sh "${notes_file}" >/dev/null
  grep -Eq "^- Version: \`v${escaped_version}\`$" "${notes_file}" \
    || fatal "release note metadata version mismatch in ${notes_file}"
  grep -Eq "^- Git tag: \`v${escaped_version}\`$" "${notes_file}" \
    || fatal "release note metadata git tag mismatch in ${notes_file}"
  grep -Eq "^- Release date: \`${changelog_date}\`$" "${notes_file}" \
    || fatal "release note date must match CHANGELOG.md date (${changelog_date})"
  bash scripts/release-github-issues-check.sh --version "${VERSION}" --notes-file "${notes_file}" >/dev/null

  git rev-parse -q --verify "refs/tags/${tag}" >/dev/null \
    || fatal "local git tag not found: ${tag}"

  local local_tag_commit remote_tag_commit_value
  local_tag_commit="$(git rev-list -n 1 "${tag}")"
  remote_tag_commit_value="$(remote_tag_commit "${REMOTE}" "${tag}")"
  [[ -n "${remote_tag_commit_value}" ]] || fatal "remote git tag not found on ${REMOTE}: ${tag}"
  [[ "${local_tag_commit}" == "${remote_tag_commit_value}" ]] \
    || fatal "local tag ${tag} (${local_tag_commit}) differs from remote ${REMOTE} (${remote_tag_commit_value})"

  local release_json
  release_json="$(gh release view "${tag}" --json tagName,isDraft,isPrerelease,publishedAt,url 2>/dev/null || true)"
  [[ -n "${release_json}" ]] || fatal "GitHub release not found for tag ${tag}"

  local is_draft is_prerelease published_at release_url
  is_draft="$(echo "${release_json}" | jq -r '.isDraft')"
  is_prerelease="$(echo "${release_json}" | jq -r '.isPrerelease')"
  published_at="$(echo "${release_json}" | jq -r '.publishedAt')"
  release_url="$(echo "${release_json}" | jq -r '.url')"

  [[ "${is_draft}" == "false" ]] || fatal "release ${tag} is still draft"
  [[ "${is_prerelease}" == "false" ]] || fatal "release ${tag} is prerelease"
  [[ "${published_at}" != "null" && -n "${published_at}" ]] || fatal "release ${tag} is not published"

  if (( ENFORCE_LATEST == 1 )); then
    local latest_tag
    latest_tag="$(latest_release_tag)"
    [[ -n "${latest_tag}" ]] || fatal "failed to detect latest published GitHub release"
    [[ "${latest_tag}" == "${tag}" ]] || fatal "latest GitHub release is ${latest_tag}, expected ${tag}"
  fi

  log "OK: release synchronized for ${tag}"
  log "release url: ${release_url}"
}

main "$@"
