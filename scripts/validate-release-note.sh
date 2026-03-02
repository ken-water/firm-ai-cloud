#!/usr/bin/env bash
set -Eeuo pipefail

if [[ $# -ne 1 ]]; then
  echo "Usage: bash scripts/validate-release-note.sh <release-note-file>"
  exit 1
fi

FILE="$1"

if [[ ! -f "$FILE" ]]; then
  echo "ERROR: file not found: $FILE"
  exit 1
fi

required_sections=(
  "## Release Metadata"
  "## Executive Summary"
  "## Highlights"
  "## Added"
  "## Changed"
  "## Fixed"
  "## Breaking Changes"
  "## Upgrade Steps"
  "## Validation"
  "## Known Issues"
  "## Contributors"
)

for section in "${required_sections[@]}"; do
  if ! grep -Fq "$section" "$FILE"; then
    echo "ERROR: missing required section: $section"
    exit 1
  fi
done

if perl -ne 'if (/\p{Han}/) { exit 0 } END { exit 1 }' "$FILE"; then
  echo "ERROR: non-English characters detected in release note."
  echo "Release notes must be in English."
  exit 1
fi

echo "OK: release note validation passed for $FILE"
