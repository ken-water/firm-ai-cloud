#!/usr/bin/env bash
set -Eeuo pipefail

PROFILE_LABEL="${BENCHMARK_PROFILE:-scale-1k}"
API_BASE_URL="${API_BASE_URL:-http://127.0.0.1:8080}"
AUTH_USER="${AUTH_USER:-admin}"
RUN_ID="${RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)}"
OUTPUT_DIR="${OUTPUT_DIR:-.run/benchmarks/profile-${PROFILE_LABEL}-${RUN_ID}}"
SKIP_GATE=0
BASELINE_API_SUMMARY=""
BASELINE_SSE_SUMMARY=""
SUMMARY_MD="${OUTPUT_DIR}/summary.md"
SUMMARY_JSON="${OUTPUT_DIR}/summary.json"

log() {
  printf '[benchmark-profile] %s\n' "$*"
}

fatal() {
  printf '[benchmark-profile][ERROR] %s\n' "$*" >&2
  exit 1
}

usage() {
  cat <<'USAGE'
Scale benchmark profile runner

Usage:
  bash scripts/benchmark-scale-profiles.sh [options]

Options:
  --profile <label>        Profile label: smoke, scale-1k, scale-5k, scale-10k (default: scale-1k)
  --api-base-url <url>     API base URL
  --auth-user <username>   Auth user header value
  --output-dir <path>      Output directory root
  --run-id <id>            Run id used by child scripts
  --baseline-api-summary <path>   Baseline API summary csv (enables trend delta report)
  --baseline-sse-summary <path>   Baseline SSE summary json (optional; use with baseline API)
  --skip-gate              Skip threshold gate step
  -h, --help               Show this help
USAGE
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || fatal "missing required command: $1"
}

validate_profile() {
  case "${PROFILE_LABEL}" in
    smoke|scale-1k|scale-5k|scale-10k) ;;
    *)
      fatal "unsupported profile: ${PROFILE_LABEL} (supported: smoke, scale-1k, scale-5k, scale-10k)"
      ;;
  esac
}

require_file() {
  local file_path="$1"
  local label="$2"
  [[ -f "${file_path}" ]] || fatal "missing required artifact (${label}): ${file_path}"
}

main() {
  require_cmd bash
  require_cmd jq

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --profile)
        [[ $# -ge 2 ]] || fatal "--profile requires a label"
        PROFILE_LABEL="$2"
        shift 2
        ;;
      --api-base-url)
        [[ $# -ge 2 ]] || fatal "--api-base-url requires a URL"
        API_BASE_URL="$2"
        shift 2
        ;;
      --auth-user)
        [[ $# -ge 2 ]] || fatal "--auth-user requires a username"
        AUTH_USER="$2"
        shift 2
        ;;
      --output-dir)
        [[ $# -ge 2 ]] || fatal "--output-dir requires a path"
        OUTPUT_DIR="$2"
        shift 2
        ;;
      --run-id)
        [[ $# -ge 2 ]] || fatal "--run-id requires a value"
        RUN_ID="$2"
        shift 2
        ;;
      --baseline-api-summary)
        [[ $# -ge 2 ]] || fatal "--baseline-api-summary requires a file path"
        BASELINE_API_SUMMARY="$2"
        shift 2
        ;;
      --baseline-sse-summary)
        [[ $# -ge 2 ]] || fatal "--baseline-sse-summary requires a file path"
        BASELINE_SSE_SUMMARY="$2"
        shift 2
        ;;
      --skip-gate)
        SKIP_GATE=1
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

  validate_profile
  if [[ -n "${BASELINE_SSE_SUMMARY}" && -z "${BASELINE_API_SUMMARY}" ]]; then
    fatal "--baseline-sse-summary requires --baseline-api-summary"
  fi
  if [[ -n "${BASELINE_API_SUMMARY}" ]]; then
    [[ -f "${BASELINE_API_SUMMARY}" ]] || fatal "baseline API summary not found: ${BASELINE_API_SUMMARY}"
  fi
  if [[ -n "${BASELINE_SSE_SUMMARY}" ]]; then
    [[ -f "${BASELINE_SSE_SUMMARY}" ]] || fatal "baseline SSE summary not found: ${BASELINE_SSE_SUMMARY}"
  fi

  local api_dir="${OUTPUT_DIR}/api"
  local sse_dir="${OUTPUT_DIR}/sse"
  local gate_dir="${OUTPUT_DIR}/gate"
  local trend_dir="${OUTPUT_DIR}/trend-delta"
  local trend_enabled=0

  mkdir -p "${OUTPUT_DIR}"

  log "Running API benchmark profile=${PROFILE_LABEL}"
  API_BASE_URL="${API_BASE_URL}" \
    AUTH_USER="${AUTH_USER}" \
    RUN_ID="${RUN_ID}" \
    OUTPUT_DIR="${api_dir}" \
    BENCHMARK_PROFILE="${PROFILE_LABEL}" \
    bash scripts/benchmark-api-load.sh --profile "${PROFILE_LABEL}"
  require_file "${api_dir}/summary.csv" "api_summary_csv"
  require_file "${api_dir}/summary.md" "api_summary_md"
  require_file "${api_dir}/profile.json" "api_profile_metadata"

  log "Running SSE benchmark profile=${PROFILE_LABEL}"
  API_BASE_URL="${API_BASE_URL}" \
    AUTH_USER="${AUTH_USER}" \
    RUN_ID="${RUN_ID}" \
    OUTPUT_DIR="${sse_dir}" \
    BENCHMARK_PROFILE="${PROFILE_LABEL}" \
    bash scripts/benchmark-sse-burst-smoke.sh --profile "${PROFILE_LABEL}"
  require_file "${sse_dir}/summary.json" "sse_summary_json"
  require_file "${sse_dir}/summary.md" "sse_summary_md"
  require_file "${sse_dir}/profile.json" "sse_profile_metadata"

  if (( SKIP_GATE == 0 )); then
    log "Running benchmark threshold gate profile=${PROFILE_LABEL}"
    RUN_ID="${RUN_ID}" \
      bash scripts/benchmark-threshold-gate.sh \
        --profile "${PROFILE_LABEL}" \
        --api-summary "${api_dir}/summary.csv" \
        --sse-summary "${sse_dir}/summary.json" \
        --output-dir "${gate_dir}"
    require_file "${gate_dir}/gate-summary.json" "gate_summary_json"
    require_file "${gate_dir}/gate-summary.md" "gate_summary_md"
  else
    log "Skipping threshold gate by request (--skip-gate)"
  fi

  if [[ -n "${BASELINE_API_SUMMARY}" ]]; then
    log "Running trend delta report profile=${PROFILE_LABEL}"
    if [[ -n "${BASELINE_SSE_SUMMARY}" ]]; then
      RUN_ID="${RUN_ID}" \
        bash scripts/benchmark-trend-delta.sh \
          --profile "${PROFILE_LABEL}" \
          --current-api-summary "${api_dir}/summary.csv" \
          --baseline-api-summary "${BASELINE_API_SUMMARY}" \
          --current-sse-summary "${sse_dir}/summary.json" \
          --baseline-sse-summary "${BASELINE_SSE_SUMMARY}" \
          --output-dir "${trend_dir}"
    else
      RUN_ID="${RUN_ID}" \
        bash scripts/benchmark-trend-delta.sh \
          --profile "${PROFILE_LABEL}" \
          --current-api-summary "${api_dir}/summary.csv" \
          --baseline-api-summary "${BASELINE_API_SUMMARY}" \
          --output-dir "${trend_dir}"
    fi
    require_file "${trend_dir}/summary.json" "trend_summary_json"
    require_file "${trend_dir}/summary.md" "trend_summary_md"
    trend_enabled=1
  fi

  jq -n \
    --arg run_id "${RUN_ID}" \
    --arg profile "${PROFILE_LABEL}" \
    --arg api_base_url "${API_BASE_URL}" \
    --arg auth_user "${AUTH_USER}" \
    --arg output_dir "${OUTPUT_DIR}" \
    --arg api_summary "${api_dir}/summary.csv" \
    --arg api_markdown "${api_dir}/summary.md" \
    --arg sse_summary "${sse_dir}/summary.json" \
    --arg sse_markdown "${sse_dir}/summary.md" \
    --arg gate_summary_json "${gate_dir}/gate-summary.json" \
    --arg gate_summary_md "${gate_dir}/gate-summary.md" \
    --arg trend_summary_json "${trend_dir}/summary.json" \
    --arg trend_summary_md "${trend_dir}/summary.md" \
    --argjson gate_skipped "$SKIP_GATE" \
    --argjson trend_enabled "$trend_enabled" \
    '{
      run_id: $run_id,
      profile: $profile,
      api_base_url: $api_base_url,
      auth_user: $auth_user,
      output_dir: $output_dir,
      artifacts: {
        api_summary_csv: $api_summary,
        api_summary_md: $api_markdown,
        sse_summary_json: $sse_summary,
        sse_summary_md: $sse_markdown,
        gate_summary_json: (if $gate_skipped == 1 then null else $gate_summary_json end),
        gate_summary_md: (if $gate_skipped == 1 then null else $gate_summary_md end),
        trend_summary_json: (if $trend_enabled == 1 then $trend_summary_json else null end),
        trend_summary_md: (if $trend_enabled == 1 then $trend_summary_md else null end)
      },
      gate_skipped: ($gate_skipped == 1),
      trend_enabled: ($trend_enabled == 1)
    }' >"${SUMMARY_JSON}"

  {
    echo "# Scale Benchmark Profile Summary"
    echo
    echo "- Run ID: ${RUN_ID}"
    echo "- Profile: ${PROFILE_LABEL}"
    echo "- API Base URL: ${API_BASE_URL}"
    echo "- Auth User: ${AUTH_USER}"
    echo "- Output Directory: \`${OUTPUT_DIR}\`"
    echo
    echo "Artifacts:"
    echo "- API summary: \`${api_dir}/summary.md\`"
    echo "- SSE summary: \`${sse_dir}/summary.md\`"
    if (( SKIP_GATE == 0 )); then
      echo "- Gate summary: \`${gate_dir}/gate-summary.md\`"
    else
      echo "- Gate summary: skipped"
    fi
    if (( trend_enabled == 1 )); then
      echo "- Trend delta summary: \`${trend_dir}/summary.md\`"
    fi
    echo "- Combined summary json: \`${SUMMARY_JSON}\`"
  } >"${SUMMARY_MD}"

  log "Scale profile benchmark completed."
  log "Summary Markdown: ${SUMMARY_MD}"
  log "Summary JSON: ${SUMMARY_JSON}"
}

main "$@"
