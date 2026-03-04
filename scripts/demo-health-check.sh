#!/usr/bin/env bash
set -Eeuo pipefail

API_BASE_URL="${API_BASE_URL:-http://127.0.0.1:8080}"
AUTH_USER="${AUTH_USER:-admin}"
RUN_DIR="${RUN_DIR:-.run/demo}"

MANIFEST_FILE=""
DEMO_TAG=""

declare -a FAILURES=()
declare -a WARNINGS=()

usage() {
  cat <<'EOF'
Usage:
  bash scripts/demo-health-check.sh [options]

Options:
  --tag <demo-tag>           Demo tag (default: from latest manifest)
  --manifest <file>          Explicit manifest file path
  -h, --help                 Show help
EOF
}

log() {
  echo "[demo-health] $*"
}

warn() {
  echo "[demo-health][warn] $*" >&2
}

fail() {
  echo "[demo-health][error] $*" >&2
  exit 1
}

require_tool() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "required command not found: $1"
  fi
}

add_failure() {
  FAILURES+=("$1")
}

add_warning() {
  WARNINGS+=("$1")
}

api_curl() {
  curl -fsS -H "x-auth-user: ${AUTH_USER}" "$@"
}

api_get() {
  api_curl "$1"
}

latest_manifest() {
  local latest
  latest="$(ls -1t "${RUN_DIR}"/*-manifest.json 2>/dev/null | head -n1 || true)"
  if [[ -n "$latest" ]]; then
    echo "$latest"
  fi
}

to_number_array_json() {
  if [[ $# -eq 0 ]]; then
    echo '[]'
    return
  fi
  printf '%s\n' "$@" | jq -Rsc 'split("\n") | map(select(length > 0) | tonumber)'
}

to_string_array_json() {
  if [[ $# -eq 0 ]]; then
    echo '[]'
    return
  fi
  printf '%s\n' "$@" | jq -Rsc 'split("\n") | map(select(length > 0))'
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --tag)
      [[ $# -ge 2 ]] || fail "--tag requires a value"
      DEMO_TAG="$2"
      shift 2
      ;;
    --manifest)
      [[ $# -ge 2 ]] || fail "--manifest requires a value"
      MANIFEST_FILE="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      fail "unknown option: $1"
      ;;
  esac
done

main() {
  require_tool curl
  require_tool jq

  mkdir -p "$RUN_DIR"

  if [[ -z "$MANIFEST_FILE" && -n "$DEMO_TAG" && -f "${RUN_DIR}/${DEMO_TAG}-manifest.json" ]]; then
    MANIFEST_FILE="${RUN_DIR}/${DEMO_TAG}-manifest.json"
  fi

  if [[ -z "$MANIFEST_FILE" ]]; then
    MANIFEST_FILE="$(latest_manifest)"
  fi

  if [[ -z "$MANIFEST_FILE" ]]; then
    fail "no manifest found in ${RUN_DIR}, run scripts/demo-seed-data.sh first"
  fi

  if [[ ! -f "$MANIFEST_FILE" ]]; then
    fail "manifest not found: ${MANIFEST_FILE}"
  fi

  local manifest_tag
  manifest_tag="$(jq -r '.demo_tag // empty' "$MANIFEST_FILE")"
  if [[ -z "$DEMO_TAG" ]]; then
    DEMO_TAG="$manifest_tag"
  fi
  if [[ -z "$DEMO_TAG" ]]; then
    fail "manifest missing demo_tag: ${MANIFEST_FILE}"
  fi

  local snapshot_dir
  snapshot_dir="$(jq -r '.snapshot_dir // empty' "$MANIFEST_FILE")"
  if [[ -z "$snapshot_dir" ]]; then
    snapshot_dir="${RUN_DIR}/${DEMO_TAG}-snapshots"
  fi

  mapfile -t asset_ids < <(jq -r '.asset_ids[]? | tostring' "$MANIFEST_FILE")
  mapfile -t source_ids < <(jq -r '.monitoring_source_ids[]? | tostring' "$MANIFEST_FILE")
  mapfile -t discovery_job_ids < <(jq -r '.discovery_job_ids[]? | tostring' "$MANIFEST_FILE")
  local channel_id
  channel_id="$(jq -r '.notification_channel_id // empty' "$MANIFEST_FILE")"

  local asset_ids_json source_ids_json discovery_job_ids_json
  asset_ids_json="$(to_number_array_json "${asset_ids[@]:-}")"
  source_ids_json="$(to_number_array_json "${source_ids[@]:-}")"
  discovery_job_ids_json="$(to_number_array_json "${discovery_job_ids[@]:-}")"

  log "Checking API health"
  if ! curl -fsS "${API_BASE_URL}/health" >/dev/null; then
    add_failure "API health endpoint is not reachable: ${API_BASE_URL}/health"
  fi

  local assets_expected assets_found assets_operational
  assets_expected="${#asset_ids[@]}"
  assets_found=0
  assets_operational=0

  local asset_id asset_json status
  for asset_id in "${asset_ids[@]:-}"; do
    if [[ -z "$asset_id" ]]; then
      continue
    fi
    if asset_json="$(api_get "${API_BASE_URL}/api/v1/cmdb/assets/${asset_id}" 2>/dev/null)"; then
      assets_found=$((assets_found + 1))
      status="$(echo "$asset_json" | jq -r '.status // "unknown"')"
      if [[ "$status" == "operational" ]]; then
        assets_operational=$((assets_operational + 1))
      fi
    else
      add_failure "asset ${asset_id} is missing"
    fi
  done

  local sources_json discovery_jobs_json channels_json sync_jobs_json candidates_json deliveries_json overview_json
  sources_json="$(api_get "${API_BASE_URL}/api/v1/monitoring/sources?source_type=zabbix")"
  discovery_jobs_json="$(api_get "${API_BASE_URL}/api/v1/cmdb/discovery/jobs")"
  channels_json="$(api_get "${API_BASE_URL}/api/v1/cmdb/discovery/notification-channels")"
  sync_jobs_json="$(api_get "${API_BASE_URL}/api/v1/cmdb/monitoring-sync/jobs?limit=500")"
  candidates_json="$(api_get "${API_BASE_URL}/api/v1/cmdb/discovery/candidates?review_status=pending&limit=500")"
  deliveries_json="$(api_get "${API_BASE_URL}/api/v1/cmdb/discovery/notification-deliveries?limit=500")"
  overview_json="$(api_get "${API_BASE_URL}/api/v1/monitoring/overview?site=dc-a")"

  local sources_expected sources_found jobs_expected jobs_found channel_found
  sources_expected="${#source_ids[@]}"
  jobs_expected="${#discovery_job_ids[@]}"
  sources_found="$(echo "$sources_json" | jq --argjson ids "$source_ids_json" '[ .[] | select(.id as $id | $ids | index($id)) ] | length')"
  jobs_found="$(echo "$discovery_jobs_json" | jq --argjson ids "$discovery_job_ids_json" '[ .[] | select(.id as $id | $ids | index($id)) ] | length')"

  channel_found=0
  if [[ -n "$channel_id" ]]; then
    channel_found="$(echo "$channels_json" | jq --argjson channel_id "$channel_id" '[ .[] | select(.id == $channel_id) ] | length')"
  fi

  local sync_summary_json sync_total sync_by_status
  sync_summary_json="$(echo "$sync_jobs_json" | jq -c --argjson ids "$asset_ids_json" '
    [ .items[] | select(.asset_id as $asset_id | $ids | index($asset_id)) ] as $items
    | {
        total: ($items | length),
        by_status: (reduce $items[] as $item ({}; .[$item.status] = ((.[$item.status] // 0) + 1)))
      }')"
  sync_total="$(echo "$sync_summary_json" | jq -r '.total')"
  sync_by_status="$(echo "$sync_summary_json" | jq -c '.by_status')"

  local pending_candidates_tag delivery_tag_count overview_asset_total snapshot_count tag_lc
  tag_lc="$(echo "$DEMO_TAG" | tr '[:upper:]' '[:lower:]')"
  pending_candidates_tag="$(echo "$candidates_json" | jq --arg tag_lc "$tag_lc" '[ .items[] | select(((.payload | tostring | ascii_downcase) | contains($tag_lc))) ] | length')"
  delivery_tag_count="$(echo "$deliveries_json" | jq --arg tag_lc "$tag_lc" '[ .items[] | select((((.target | tostring | ascii_downcase) | contains($tag_lc)) or ((.payload | tostring | ascii_downcase) | contains($tag_lc)))) ] | length')"
  overview_asset_total="$(echo "$overview_json" | jq -r '.summary.asset_total // 0')"

  if [[ -d "$snapshot_dir" ]]; then
    snapshot_count="$(find "$snapshot_dir" -maxdepth 1 -type f | wc -l | tr -d ' ')"
  else
    snapshot_count=0
    add_failure "snapshot directory missing: ${snapshot_dir}"
  fi

  if [[ "$assets_expected" -eq 0 ]]; then
    add_failure "manifest has no asset ids"
  fi
  if [[ "$assets_found" -lt "$assets_expected" ]]; then
    add_failure "assets found (${assets_found}) is less than expected (${assets_expected})"
  fi
  if [[ "$sources_expected" -gt 0 && "$sources_found" -lt "$sources_expected" ]]; then
    add_failure "monitoring sources found (${sources_found}) is less than expected (${sources_expected})"
  fi
  if [[ "$jobs_expected" -gt 0 && "$jobs_found" -lt "$jobs_expected" ]]; then
    add_failure "discovery jobs found (${jobs_found}) is less than expected (${jobs_expected})"
  fi
  if [[ -n "$channel_id" && "$channel_found" -lt 1 ]]; then
    add_failure "notification channel ${channel_id} is missing"
  fi
  if [[ "$sync_total" -lt 1 ]]; then
    add_failure "no monitoring sync jobs were found for demo assets"
  fi
  if [[ "$snapshot_count" -lt 1 ]]; then
    add_failure "no snapshot files found under ${snapshot_dir}"
  fi
  if [[ "$pending_candidates_tag" -lt 1 ]]; then
    add_warning "no pending discovery candidates currently tagged by ${DEMO_TAG} (may already be reviewed)"
  fi
  if [[ "$delivery_tag_count" -lt 1 ]]; then
    add_warning "no tagged notification deliveries found yet (delivery may still be pending)"
  fi

  local failures_json warnings_json report_file result
  failures_json="$(to_string_array_json "${FAILURES[@]:-}")"
  warnings_json="$(to_string_array_json "${WARNINGS[@]:-}")"
  report_file="${RUN_DIR}/${DEMO_TAG}-health-check.json"
  result="pass"
  if [[ "${#FAILURES[@]}" -gt 0 ]]; then
    result="fail"
  fi

  jq -nc \
    --arg result "$result" \
    --arg checked_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    --arg api_base_url "$API_BASE_URL" \
    --arg auth_user "$AUTH_USER" \
    --arg demo_tag "$DEMO_TAG" \
    --arg manifest_file "$MANIFEST_FILE" \
    --arg snapshot_dir "$snapshot_dir" \
    --argjson assets_expected "$assets_expected" \
    --argjson assets_found "$assets_found" \
    --argjson assets_operational "$assets_operational" \
    --argjson sources_expected "$sources_expected" \
    --argjson sources_found "$sources_found" \
    --argjson jobs_expected "$jobs_expected" \
    --argjson jobs_found "$jobs_found" \
    --argjson channel_found "$channel_found" \
    --argjson sync_total "$sync_total" \
    --argjson sync_by_status "$sync_by_status" \
    --argjson pending_candidates_tag "$pending_candidates_tag" \
    --argjson delivery_tag_count "$delivery_tag_count" \
    --argjson overview_asset_total "$overview_asset_total" \
    --argjson snapshot_count "$snapshot_count" \
    --argjson failures "$failures_json" \
    --argjson warnings "$warnings_json" \
    '{
      result: $result,
      checked_at: $checked_at,
      context: {
        api_base_url: $api_base_url,
        auth_user: $auth_user,
        demo_tag: $demo_tag,
        manifest_file: $manifest_file,
        snapshot_dir: $snapshot_dir
      },
      metrics: {
        assets_expected: $assets_expected,
        assets_found: $assets_found,
        assets_operational: $assets_operational,
        sources_expected: $sources_expected,
        sources_found: $sources_found,
        jobs_expected: $jobs_expected,
        jobs_found: $jobs_found,
        channel_found: ($channel_found > 0),
        sync_total: $sync_total,
        sync_by_status: $sync_by_status,
        pending_candidates_tag: $pending_candidates_tag,
        delivery_tag_count: $delivery_tag_count,
        overview_asset_total: $overview_asset_total,
        snapshot_count: $snapshot_count
      },
      failures: $failures,
      warnings: $warnings
    }' >"$report_file"

  log "Result: ${result}"
  log "Demo tag: ${DEMO_TAG}"
  log "Assets: ${assets_found}/${assets_expected} (operational: ${assets_operational})"
  log "Monitoring sources: ${sources_found}/${sources_expected}"
  log "Discovery jobs: ${jobs_found}/${jobs_expected}"
  log "Monitoring sync jobs: ${sync_total}, by status: ${sync_by_status}"
  log "Pending candidates (tag): ${pending_candidates_tag}"
  log "Notification deliveries (tag): ${delivery_tag_count}"
  log "Monitoring overview asset_total (site=dc-a): ${overview_asset_total}"
  log "Snapshots: ${snapshot_count} files"
  log "Report: ${report_file}"

  if [[ "${#WARNINGS[@]}" -gt 0 ]]; then
    local item
    for item in "${WARNINGS[@]}"; do
      warn "$item"
    done
  fi

  if [[ "${#FAILURES[@]}" -gt 0 ]]; then
    local item
    for item in "${FAILURES[@]}"; do
      warn "$item"
    done
    exit 1
  fi
}

main "$@"
