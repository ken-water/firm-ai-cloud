#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
DEPLOY_DIR="${ROOT_DIR}/deploy"
COMPOSE_FILE="${DEPLOY_DIR}/docker-compose.yml"
ENV_FILE="${DEPLOY_DIR}/.env"

RUN_DIR="${RUN_DIR:-.run/demo}"
MANIFEST_FILE=""
DEMO_TAG=""
DRY_RUN=0

COMPOSE_CMD=()

usage() {
  cat <<'EOF'
Usage:
  bash scripts/demo-cleanup-data.sh [options]

Options:
  --tag <demo-tag>           Demo tag to clean up
  --manifest <file>          Manifest file path
  --dry-run                  Preview cleanup with transaction rollback
  -h, --help                 Show help
EOF
}

log() {
  echo "[demo-cleanup] $*"
}

warn() {
  echo "[demo-cleanup][warn] $*" >&2
}

fail() {
  echo "[demo-cleanup][error] $*" >&2
  exit 1
}

require_tool() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "required command not found: $1"
  fi
}

detect_compose_command() {
  if docker compose version >/dev/null 2>&1; then
    COMPOSE_CMD=("docker" "compose")
    return
  fi
  if command -v docker-compose >/dev/null 2>&1; then
    COMPOSE_CMD=("docker-compose")
    return
  fi
  fail "Docker Compose is required for cleanup"
}

compose() {
  "${COMPOSE_CMD[@]}" --env-file "$ENV_FILE" -f "$COMPOSE_FILE" "$@"
}

read_env_value() {
  local key="$1"
  local line
  line="$(grep -E "^${key}=" "$ENV_FILE" | tail -n1 || true)"
  if [[ -z "$line" ]]; then
    return 1
  fi
  printf '%s' "${line#*=}"
}

latest_manifest() {
  local latest
  latest="$(ls -1t "${RUN_DIR}"/*-manifest.json 2>/dev/null | head -n1 || true)"
  if [[ -n "$latest" ]]; then
    echo "$latest"
  fi
}

sanitize_demo_tag() {
  if [[ ! "$DEMO_TAG" =~ ^[A-Za-z0-9._:-]+$ ]]; then
    fail "invalid demo tag '${DEMO_TAG}', allowed pattern: [A-Za-z0-9._:-]+"
  fi
}

sanitize_numeric_ids() {
  local value
  local -a out=()
  for value in "$@"; do
    if [[ "$value" =~ ^[0-9]+$ ]]; then
      out+=("$value")
    fi
  done
  printf '%s\n' "${out[@]:-}"
}

to_bigint_array_sql() {
  if [[ $# -eq 0 ]]; then
    printf 'ARRAY[]::BIGINT[]'
    return
  fi
  local joined
  joined="$(IFS=,; echo "$*")"
  printf 'ARRAY[%s]::BIGINT[]' "$joined"
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
    --dry-run)
      DRY_RUN=1
      shift
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
  require_tool jq
  require_tool docker
  detect_compose_command

  if [[ ! -f "$COMPOSE_FILE" ]]; then
    fail "compose file not found: $COMPOSE_FILE"
  fi
  if [[ ! -f "$ENV_FILE" ]]; then
    fail "env file not found: $ENV_FILE"
  fi

  if [[ -z "$MANIFEST_FILE" && -n "$DEMO_TAG" && -f "${RUN_DIR}/${DEMO_TAG}-manifest.json" ]]; then
    MANIFEST_FILE="${RUN_DIR}/${DEMO_TAG}-manifest.json"
  fi
  if [[ -z "$MANIFEST_FILE" ]]; then
    MANIFEST_FILE="$(latest_manifest)"
  fi
  if [[ -n "$MANIFEST_FILE" && ! -f "$MANIFEST_FILE" ]]; then
    fail "manifest file not found: ${MANIFEST_FILE}"
  fi

  local manifest_tag
  manifest_tag=""
  if [[ -n "$MANIFEST_FILE" ]]; then
    manifest_tag="$(jq -r '.demo_tag // empty' "$MANIFEST_FILE")"
  fi

  if [[ -z "$DEMO_TAG" ]]; then
    DEMO_TAG="$manifest_tag"
  fi
  if [[ -z "$DEMO_TAG" ]]; then
    fail "demo tag is required (pass --tag or --manifest)"
  fi
  sanitize_demo_tag

  local -a asset_ids=() source_ids=() discovery_job_ids=() channel_ids=()
  local channel_id=""
  if [[ -n "$MANIFEST_FILE" ]]; then
    mapfile -t asset_ids < <(jq -r '.asset_ids[]? | tostring' "$MANIFEST_FILE")
    mapfile -t source_ids < <(jq -r '.monitoring_source_ids[]? | tostring' "$MANIFEST_FILE")
    mapfile -t discovery_job_ids < <(jq -r '.discovery_job_ids[]? | tostring' "$MANIFEST_FILE")
    channel_id="$(jq -r '.notification_channel_id // empty' "$MANIFEST_FILE")"
    if [[ "$channel_id" =~ ^[0-9]+$ ]]; then
      channel_ids+=("$channel_id")
    fi
  fi

  mapfile -t asset_ids < <(sanitize_numeric_ids "${asset_ids[@]:-}")
  mapfile -t source_ids < <(sanitize_numeric_ids "${source_ids[@]:-}")
  mapfile -t discovery_job_ids < <(sanitize_numeric_ids "${discovery_job_ids[@]:-}")
  mapfile -t channel_ids < <(sanitize_numeric_ids "${channel_ids[@]:-}")

  local asset_array_sql source_array_sql discovery_job_array_sql channel_array_sql tx_final
  asset_array_sql="$(to_bigint_array_sql "${asset_ids[@]:-}")"
  source_array_sql="$(to_bigint_array_sql "${source_ids[@]:-}")"
  discovery_job_array_sql="$(to_bigint_array_sql "${discovery_job_ids[@]:-}")"
  channel_array_sql="$(to_bigint_array_sql "${channel_ids[@]:-}")"
  tx_final="COMMIT"
  if [[ "$DRY_RUN" -eq 1 ]]; then
    tx_final="ROLLBACK"
  fi

  local postgres_db postgres_user postgres_password
  postgres_db="${POSTGRES_DB:-$(read_env_value POSTGRES_DB || true)}"
  postgres_user="${POSTGRES_USER:-$(read_env_value POSTGRES_USER || true)}"
  postgres_password="${POSTGRES_PASSWORD:-$(read_env_value POSTGRES_PASSWORD || true)}"

  [[ -n "$postgres_db" ]] || fail "POSTGRES_DB is missing in ${ENV_FILE}"
  [[ -n "$postgres_user" ]] || fail "POSTGRES_USER is missing in ${ENV_FILE}"
  [[ -n "$postgres_password" ]] || fail "POSTGRES_PASSWORD is missing in ${ENV_FILE}"

  log "Starting cleanup for demo tag: ${DEMO_TAG}"
  if [[ -n "$MANIFEST_FILE" ]]; then
    log "Using manifest: ${MANIFEST_FILE}"
  else
    warn "No manifest file found, cleanup will rely on tag matching only"
  fi
  if [[ "$DRY_RUN" -eq 1 ]]; then
    warn "Dry-run mode enabled, transaction will be rolled back"
  fi

  compose exec -T postgres env PGPASSWORD="${postgres_password}" \
    psql -X -v ON_ERROR_STOP=1 -U "${postgres_user}" -d "${postgres_db}" <<SQL
BEGIN;

CREATE TEMP TABLE tmp_demo_assets (id BIGINT PRIMARY KEY) ON COMMIT DROP;
CREATE TEMP TABLE tmp_demo_sources (id BIGINT PRIMARY KEY) ON COMMIT DROP;
CREATE TEMP TABLE tmp_demo_jobs (id BIGINT PRIMARY KEY) ON COMMIT DROP;
CREATE TEMP TABLE tmp_demo_channels (id BIGINT PRIMARY KEY) ON COMMIT DROP;

INSERT INTO tmp_demo_assets (id)
SELECT DISTINCT x FROM unnest(${asset_array_sql}) AS x
ON CONFLICT DO NOTHING;

INSERT INTO tmp_demo_sources (id)
SELECT DISTINCT x FROM unnest(${source_array_sql}) AS x
ON CONFLICT DO NOTHING;

INSERT INTO tmp_demo_jobs (id)
SELECT DISTINCT x FROM unnest(${discovery_job_array_sql}) AS x
ON CONFLICT DO NOTHING;

INSERT INTO tmp_demo_channels (id)
SELECT DISTINCT x FROM unnest(${channel_array_sql}) AS x
ON CONFLICT DO NOTHING;

INSERT INTO tmp_demo_assets (id)
SELECT id
FROM assets
WHERE name ILIKE '%${DEMO_TAG}%'
   OR COALESCE(hostname, '') ILIKE '%${DEMO_TAG}%'
   OR COALESCE(qr_code, '') ILIKE '%${DEMO_TAG}%'
   OR COALESCE(barcode, '') ILIKE '%${DEMO_TAG}%'
ON CONFLICT DO NOTHING;

INSERT INTO tmp_demo_sources (id)
SELECT id
FROM monitoring_sources
WHERE name ILIKE '%${DEMO_TAG}%'
   OR endpoint ILIKE '%${DEMO_TAG}%'
ON CONFLICT DO NOTHING;

INSERT INTO tmp_demo_jobs (id)
SELECT id
FROM discovery_jobs
WHERE name ILIKE '%${DEMO_TAG}%'
   OR scope::text ILIKE '%${DEMO_TAG}%'
ON CONFLICT DO NOTHING;

INSERT INTO tmp_demo_channels (id)
SELECT id
FROM discovery_notification_channels
WHERE name ILIKE '%${DEMO_TAG}%'
   OR target ILIKE '%${DEMO_TAG}%'
ON CONFLICT DO NOTHING;

WITH deleted AS (
  DELETE FROM discovery_notification_deliveries
  WHERE channel_id IN (SELECT id FROM tmp_demo_channels)
     OR target ILIKE '%${DEMO_TAG}%'
     OR payload::text ILIKE '%${DEMO_TAG}%'
  RETURNING 1
)
SELECT 'discovery_notification_deliveries' AS table_name, COUNT(*) AS deleted_count FROM deleted;

WITH deleted AS (
  DELETE FROM discovery_notification_subscriptions
  WHERE channel_id IN (SELECT id FROM tmp_demo_channels)
     OR event_type ILIKE '%${DEMO_TAG}%'
     OR COALESCE(site, '') ILIKE '%${DEMO_TAG}%'
     OR COALESCE(department, '') ILIKE '%${DEMO_TAG}%'
  RETURNING 1
)
SELECT 'discovery_notification_subscriptions' AS table_name, COUNT(*) AS deleted_count FROM deleted;

WITH deleted AS (
  DELETE FROM discovery_notification_templates
  WHERE title_template ILIKE '%${DEMO_TAG}%'
     OR body_template ILIKE '%${DEMO_TAG}%'
     OR event_type ILIKE '%${DEMO_TAG}%'
  RETURNING 1
)
SELECT 'discovery_notification_templates' AS table_name, COUNT(*) AS deleted_count FROM deleted;

WITH deleted AS (
  DELETE FROM discovery_notification_channels
  WHERE id IN (SELECT id FROM tmp_demo_channels)
     OR name ILIKE '%${DEMO_TAG}%'
     OR target ILIKE '%${DEMO_TAG}%'
  RETURNING 1
)
SELECT 'discovery_notification_channels' AS table_name, COUNT(*) AS deleted_count FROM deleted;

WITH deleted AS (
  DELETE FROM discovery_candidates
  WHERE job_id IN (SELECT id FROM tmp_demo_jobs)
     OR fingerprint ILIKE '%${DEMO_TAG}%'
     OR payload::text ILIKE '%${DEMO_TAG}%'
     OR COALESCE(review_reason, '') ILIKE '%${DEMO_TAG}%'
  RETURNING 1
)
SELECT 'discovery_candidates' AS table_name, COUNT(*) AS deleted_count FROM deleted;

WITH deleted AS (
  DELETE FROM discovery_asset_states
  WHERE job_id IN (SELECT id FROM tmp_demo_jobs)
     OR asset_id IN (SELECT id FROM tmp_demo_assets)
     OR profile::text ILIKE '%${DEMO_TAG}%'
  RETURNING 1
)
SELECT 'discovery_asset_states' AS table_name, COUNT(*) AS deleted_count FROM deleted;

WITH deleted AS (
  DELETE FROM discovery_events
  WHERE job_id IN (SELECT id FROM tmp_demo_jobs)
     OR asset_id IN (SELECT id FROM tmp_demo_assets)
     OR COALESCE(fingerprint, '') ILIKE '%${DEMO_TAG}%'
     OR payload::text ILIKE '%${DEMO_TAG}%'
  RETURNING 1
)
SELECT 'discovery_events' AS table_name, COUNT(*) AS deleted_count FROM deleted;

WITH deleted AS (
  DELETE FROM discovery_jobs
  WHERE id IN (SELECT id FROM tmp_demo_jobs)
     OR name ILIKE '%${DEMO_TAG}%'
     OR scope::text ILIKE '%${DEMO_TAG}%'
  RETURNING 1
)
SELECT 'discovery_jobs' AS table_name, COUNT(*) AS deleted_count FROM deleted;

WITH deleted AS (
  DELETE FROM cmdb_monitoring_sync_jobs
  WHERE asset_id IN (SELECT id FROM tmp_demo_assets)
     OR payload::text ILIKE '%${DEMO_TAG}%'
  RETURNING 1
)
SELECT 'cmdb_monitoring_sync_jobs' AS table_name, COUNT(*) AS deleted_count FROM deleted;

WITH deleted AS (
  DELETE FROM cmdb_monitoring_bindings
  WHERE asset_id IN (SELECT id FROM tmp_demo_assets)
     OR source_id IN (SELECT id FROM tmp_demo_sources)
  RETURNING 1
)
SELECT 'cmdb_monitoring_bindings' AS table_name, COUNT(*) AS deleted_count FROM deleted;

WITH deleted AS (
  DELETE FROM asset_relations
  WHERE src_asset_id IN (SELECT id FROM tmp_demo_assets)
     OR dst_asset_id IN (SELECT id FROM tmp_demo_assets)
  RETURNING 1
)
SELECT 'asset_relations' AS table_name, COUNT(*) AS deleted_count FROM deleted;

WITH deleted AS (
  DELETE FROM asset_owner_bindings
  WHERE asset_id IN (SELECT id FROM tmp_demo_assets)
  RETURNING 1
)
SELECT 'asset_owner_bindings' AS table_name, COUNT(*) AS deleted_count FROM deleted;

WITH deleted AS (
  DELETE FROM asset_business_service_bindings
  WHERE asset_id IN (SELECT id FROM tmp_demo_assets)
  RETURNING 1
)
SELECT 'asset_business_service_bindings' AS table_name, COUNT(*) AS deleted_count FROM deleted;

WITH deleted AS (
  DELETE FROM asset_department_bindings
  WHERE asset_id IN (SELECT id FROM tmp_demo_assets)
  RETURNING 1
)
SELECT 'asset_department_bindings' AS table_name, COUNT(*) AS deleted_count FROM deleted;

WITH deleted AS (
  DELETE FROM assets
  WHERE id IN (SELECT id FROM tmp_demo_assets)
     OR name ILIKE '%${DEMO_TAG}%'
     OR COALESCE(hostname, '') ILIKE '%${DEMO_TAG}%'
     OR COALESCE(qr_code, '') ILIKE '%${DEMO_TAG}%'
     OR COALESCE(barcode, '') ILIKE '%${DEMO_TAG}%'
  RETURNING 1
)
SELECT 'assets' AS table_name, COUNT(*) AS deleted_count FROM deleted;

WITH deleted AS (
  DELETE FROM monitoring_sources
  WHERE id IN (SELECT id FROM tmp_demo_sources)
     OR name ILIKE '%${DEMO_TAG}%'
     OR endpoint ILIKE '%${DEMO_TAG}%'
  RETURNING 1
)
SELECT 'monitoring_sources' AS table_name, COUNT(*) AS deleted_count FROM deleted;

${tx_final};
SQL

  if [[ "$DRY_RUN" -eq 0 ]]; then
    rm -f "${RUN_DIR}/${DEMO_TAG}-manifest.json" "${RUN_DIR}/${DEMO_TAG}-health-check.json"
    rm -rf "${RUN_DIR}/${DEMO_TAG}-snapshots"
    log "Removed local demo artifacts for ${DEMO_TAG} under ${RUN_DIR}"
  fi

  log "Cleanup finished (${tx_final})"
  warn "Audit logs are append-only and are intentionally not removed."
}

main "$@"
