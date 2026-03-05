#!/usr/bin/env bash
set -Eeuo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
OUTPUT_DIR="${OUTPUT_DIR:-.run/qa/v0.1.1-${STAMP}}"
mkdir -p "${OUTPUT_DIR}"

SUMMARY_JSON="${OUTPUT_DIR}/summary.json"
SUMMARY_MD="${OUTPUT_DIR}/summary.md"

API_BASE_URL="${API_BASE_URL:-http://127.0.0.1:8080}"

api_reachable="false"
if curl -fsS "${API_BASE_URL}/health" >/dev/null 2>&1; then
  api_reachable="true"
fi

run_step() {
  local name="$1"
  local cmd="$2"
  local log_file="${OUTPUT_DIR}/${name}.log"

  echo "[qa-v0.1.1] running ${name}: ${cmd}" >&2
  if bash -lc "${cmd}" >"${log_file}" 2>&1; then
    echo "[qa-v0.1.1] ${name}: pass" >&2
    echo "pass"
    return 0
  fi

  echo "[qa-v0.1.1] ${name}: fail (see ${log_file})" >&2
  echo "fail"
  return 1
}

api_unit_status="$(run_step api_unit "cargo test -p api")" || true
ui_quality_status="$(run_step ui_quality "cd apps/web-console && npm run check:ui")" || true

rbac_status="skipped"
if [[ "${api_reachable}" == "true" ]]; then
  rbac_status="$(run_step rbac_matrix "bash scripts/test-rbac-policy.sh")" || true
fi

overall="pass"
if [[ "${api_unit_status}" != "pass" || "${ui_quality_status}" != "pass" ]]; then
  overall="fail"
fi
if [[ "${rbac_status}" == "fail" ]]; then
  overall="fail"
fi

cat >"${SUMMARY_JSON}" <<JSON
{
  "version": "v0.1.1",
  "generated_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "api_base_url": "${API_BASE_URL}",
  "api_reachable": ${api_reachable},
  "steps": {
    "api_unit": "${api_unit_status}",
    "ui_quality": "${ui_quality_status}",
    "rbac_matrix": "${rbac_status}"
  },
  "overall": "${overall}"
}
JSON

cat >"${SUMMARY_MD}" <<MD
# v0.1.1 Validation Summary

- Generated at: $(date -u +%Y-%m-%dT%H:%M:%SZ)
- API base URL: ${API_BASE_URL}
- API reachable for integration checks: ${api_reachable}

## Scenario Matrix

| Scenario | Coverage focus | Result |
| --- | --- | --- |
| API unit suite | Playbook dry-run/confirm flow, cockpit ranking stability, topology diagnostics contract, RBAC mapping unit coverage | ${api_unit_status} |
| Web UI quality | Type-check/build, i18n key parity, UI guardrails | ${ui_quality_status} |
| RBAC integration | Operator/viewer allow-deny paths for setup/alerts/playbooks/cockpit endpoints | ${rbac_status} |

## Residual Gaps

- Manual browser interaction flow is not fully automated (requires human walkthrough).
- If API is not reachable, integration RBAC matrix is marked as skipped and should be rerun in CI/staging.

## Artifacts

- JSON summary: ${SUMMARY_JSON}
- Markdown summary: ${SUMMARY_MD}
MD

echo "[qa-v0.1.1] summary: ${SUMMARY_MD}"

if [[ "${overall}" != "pass" ]]; then
  exit 1
fi
