#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ENV_FILE="${ROOT_DIR}/deploy/.env"
COMPOSE_FILE="${ROOT_DIR}/deploy/docker-compose.yml"
PROJECT_NAME="deploy"
RUN_ID="$(date -u +%Y%m%dT%H%M%SZ)"
OUTPUT_DIR=""
ASSUME_YES=0

BACKUP_DIR=""
LOG_DIR=""
REPORT_JSON=""
REPORT_MD=""
CHECKS_FILE=""

log() {
  printf '[dr-drill] %s\n' "$*"
}

fatal() {
  printf '[dr-drill][ERROR] %s\n' "$*" >&2
  exit 1
}

usage() {
  cat <<'USAGE'
CloudOps DR drill automation

Usage:
  bash scripts/dr-drill.sh [options]

Options:
  --env-file <path>        Env file used by backup/restore scripts (default: deploy/.env)
  --compose-file <path>    Compose file used by backup/restore scripts (default: deploy/docker-compose.yml)
  --project-name <name>    Compose project name (default: deploy)
  --output-dir <path>      Drill report output directory (default: .run/dr-drill/<run-id>)
  --yes                    Non-interactive mode for restore confirmation
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
      --yes)
        ASSUME_YES=1
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

setup_output_dir() {
  if [[ -z "${OUTPUT_DIR}" ]]; then
    OUTPUT_DIR="${ROOT_DIR}/.run/dr-drill/${RUN_ID}"
  fi
  BACKUP_DIR="${OUTPUT_DIR}/backup"
  LOG_DIR="${OUTPUT_DIR}/logs"
  REPORT_JSON="${OUTPUT_DIR}/report.json"
  REPORT_MD="${OUTPUT_DIR}/report.md"
  CHECKS_FILE="${OUTPUT_DIR}/checks.ndjson"

  mkdir -p "${OUTPUT_DIR}" "${LOG_DIR}" "${BACKUP_DIR}"
  : >"${CHECKS_FILE}"
}

record_check() {
  local check_id="$1"
  local command="$2"
  local exit_code="$3"
  local log_file="$4"
  local detail="$5"
  local pass_json="false"
  if [[ "${exit_code}" -eq 0 ]]; then
    pass_json="true"
  fi

  jq -nc \
    --arg id "${check_id}" \
    --arg command "${command}" \
    --arg detail "${detail}" \
    --arg log_file "${log_file}" \
    --argjson exit_code "${exit_code}" \
    --argjson pass "${pass_json}" \
    '{
      id: $id,
      command: $command,
      pass: $pass,
      exit_code: $exit_code,
      detail: $detail,
      log_file: $log_file
    }' >>"${CHECKS_FILE}"
}

run_check() {
  local check_id="$1"
  shift
  local command="$*"
  local check_log="${LOG_DIR}/${check_id}.log"

  log "Check ${check_id}: ${command}"
  set +e
  bash -lc "${command}" >"${check_log}" 2>&1
  local exit_code=$?
  set -e

  local detail="ok"
  if [[ "${exit_code}" -ne 0 ]]; then
    detail="failed; inspect ${check_log}"
  fi
  record_check "${check_id}" "${command}" "${exit_code}" "${check_log}" "${detail}"
}

run_backup_and_restore() {
  local backup_cmd restore_cmd
  backup_cmd="bash scripts/backup-stack.sh --env-file '${ENV_FILE}' --compose-file '${COMPOSE_FILE}' --project-name '${PROJECT_NAME}' --output-dir '${BACKUP_DIR}'"
  restore_cmd="bash scripts/restore-stack.sh --input-dir '${BACKUP_DIR}' --env-file '${ENV_FILE}' --compose-file '${COMPOSE_FILE}' --project-name '${PROJECT_NAME}'"
  if [[ "${ASSUME_YES}" -eq 1 ]]; then
    restore_cmd="${restore_cmd} --yes"
  fi

  run_check "backup_stack" "${backup_cmd}"
  run_check "restore_stack" "${restore_cmd}"
}

run_post_restore_verification() {
  run_check "compose_ps" \
    "docker compose --project-name '${PROJECT_NAME}' --env-file '${ENV_FILE}' -f '${COMPOSE_FILE}' ps || docker-compose --project-name '${PROJECT_NAME}' --env-file '${ENV_FILE}' -f '${COMPOSE_FILE}' ps"
  run_check "api_health" "curl -fsS http://127.0.0.1:8080/health"
  run_check "web_console" "curl -fsSI http://127.0.0.1:8081"
  run_check "zabbix_web" "curl -fsSI http://127.0.0.1:8082"
  run_check "cmdb_assets" "curl -fsS -H 'x-auth-user: admin' http://127.0.0.1:8080/api/v1/cmdb/assets?limit=1"
  run_check "monitoring_jobs" "curl -fsS -H 'x-auth-user: admin' http://127.0.0.1:8080/api/v1/cmdb/monitoring-sync/jobs?limit=1"
}

write_report() {
  local started_at="$1"
  local finished_at="$2"
  local duration_seconds="$3"
  local pass_json="true"
  if jq -s -e 'any(.[]; .pass == false)' "${CHECKS_FILE}" >/dev/null; then
    pass_json="false"
  fi

  jq -n \
    --arg run_id "${RUN_ID}" \
    --arg started_at "${started_at}" \
    --arg finished_at "${finished_at}" \
    --arg env_file "${ENV_FILE}" \
    --arg compose_file "${COMPOSE_FILE}" \
    --arg project_name "${PROJECT_NAME}" \
    --arg output_dir "${OUTPUT_DIR}" \
    --arg backup_dir "${BACKUP_DIR}" \
    --argjson duration_seconds "${duration_seconds}" \
    --argjson pass "${pass_json}" \
    --slurpfile checks "${CHECKS_FILE}" \
    '{
      run_id: $run_id,
      started_at: $started_at,
      finished_at: $finished_at,
      duration_seconds: $duration_seconds,
      env_file: $env_file,
      compose_file: $compose_file,
      project_name: $project_name,
      output_dir: $output_dir,
      backup_dir: $backup_dir,
      pass: $pass,
      totals: {
        checks: ($checks | length),
        failed: ($checks | map(select(.pass == false)) | length)
      },
      checks: $checks
    }' >"${REPORT_JSON}"

  local result_text="PASS"
  if [[ "${pass_json}" != "true" ]]; then
    result_text="FAIL"
  fi

  {
    echo "# DR Drill Report"
    echo
    echo "- Run ID: ${RUN_ID}"
    echo "- Started at: ${started_at}"
    echo "- Finished at: ${finished_at}"
    echo "- Duration (seconds): ${duration_seconds}"
    echo "- Project: ${PROJECT_NAME}"
    echo "- Backup directory: \`${BACKUP_DIR}\`"
    echo "- Result: **${result_text}**"
    echo
    echo "| Check | Result | Exit Code | Detail | Log |"
    echo "| --- | --- | --- | --- | --- |"
    jq -s -r '.[] | "| `\(.id)` | \((if .pass then "PASS" else "FAIL" end)) | \(.exit_code) | \(.detail) | `\(.log_file)` |"' "${CHECKS_FILE}"
  } >"${REPORT_MD}"

  log "Report JSON: ${REPORT_JSON}"
  log "Report Markdown: ${REPORT_MD}"

  if [[ "${pass_json}" != "true" ]]; then
    fatal "DR drill failed; inspect report and logs under ${OUTPUT_DIR}"
  fi
}

main() {
  parse_args "$@"
  setup_output_dir

  local started_epoch finished_epoch started_at finished_at duration_seconds
  started_epoch="$(date -u +%s)"
  started_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

  run_backup_and_restore
  run_post_restore_verification

  finished_epoch="$(date -u +%s)"
  finished_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  duration_seconds=$((finished_epoch - started_epoch))
  write_report "${started_at}" "${finished_at}" "${duration_seconds}"
}

main "$@"
