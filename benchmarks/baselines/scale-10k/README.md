# scale-10k Baseline Artifacts

This folder stores the static baseline artifact set used by CI trend comparison for the `scale-10k` benchmark profile.

Files:

- `api-summary.csv`: baseline API endpoint summary (schema matches `scripts/benchmark-api-load.sh` output).
- `sse-summary.json`: baseline SSE burst summary (schema matches `scripts/benchmark-sse-burst-smoke.sh` output).

Usage:

- Trend delta: compare current run against these files via `scripts/benchmark-trend-delta.sh`.
- Regression guard: evaluate deltas with `scripts/benchmark-regression-guard.sh`.

When updating baselines, refresh both files together and include rationale in release notes.
