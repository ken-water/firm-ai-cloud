# CloudOps One Release Governance

Version: v1.3  
Date: 2026-03-09

## 1. Scope

This document defines the mandatory release policy for CloudOps One.

It applies to:

- Git tags
- GitHub Releases
- `CHANGELOG.md`
- release notes files in `release-notes/`

## 2. Language Policy (Mandatory)

All release-facing content must be written in English, including:

- release title
- release summary
- changelog entries
- upgrade instructions
- known issues

This policy keeps release communication consistent for global open-source users.

## 3. Release Artifacts (Mandatory)

Each release must include all artifacts below:

1. SemVer tag: `vMAJOR.MINOR.PATCH`
2. `CHANGELOG.md` entry for the version
3. Detailed release note file: `release-notes/vX.Y.Z.md`
4. Published GitHub Release using the same note content
5. Release metadata includes GitHub issue list and all listed issues are closed before publish

A release is incomplete if any required artifact is missing.

## 4. Minimum Release Note Sections

Every release note must include all sections below:

- `## Release Metadata`
- `## Executive Summary`
- `## Highlights`
- `## Added`
- `## Changed`
- `## Fixed`
- `## Breaking Changes`
- `## Upgrade Steps`
- `## Validation`
- `## Known Issues`
- `## Contributors`

If a section has no content, write `None`.

## 5. Detail Standard

Release notes should be specific and actionable.

Required detail level:

- user-visible behavior changes
- API changes with endpoint/path level details
- data/migration changes with impact and rollback notes
- installation or dependency changes
- risk and compatibility notes

Avoid one-line generic summaries for public releases.

## 6. Release Workflow

1. Prepare release note file from `release-notes/TEMPLATE.md`.
2. Update `CHANGELOG.md` with the exact same version and date.
3. Run project validation (`cargo check`, frontend build, and any required smoke tests).
4. Ensure all release-scoped GitHub issues are closed and listed in release metadata.
5. Commit release artifacts.
6. Run dry-run release publish gate:
   - `make release-publish-dry VERSION=X.Y.Z`
7. Publish release through the scripted flow:
   - `make release-publish VERSION=X.Y.Z`
8. Run mandatory synchronization check:
   - `make release-check VERSION=X.Y.Z`

Notes:

- `make release-publish` is the standard path. Do not publish by manually copying notes to GitHub UI.
- The scripted flow enforces release-note structure, metadata consistency, changelog/tag alignment, and GitHub release status.

## 7. Quality Gate Checklist

Before publishing, confirm all items:

- [ ] Release notes are fully in English.
- [ ] Required sections are complete.
- [ ] `CHANGELOG.md` updated.
- [ ] Version number is consistent across tag, changelog, and release note filename.
- [ ] Release metadata includes `GitHub issues` list for the target version.
- [ ] All listed GitHub issues are closed before publish.
- [ ] Upgrade instructions were tested.
- [ ] Known issues are explicitly listed (or `None`).
- [ ] If benchmark scope changed, release note includes CI benchmark artifact references (`profile`, `gate`, `trend`, `regression` summaries).
- [ ] `make release-publish-dry VERSION=X.Y.Z` passes.
- [ ] `make release-check VERSION=X.Y.Z` passes.

## 8. Responsibility

The release owner is responsible for quality and completeness of release notes.

Pull requests that create a release tag without detailed release notes should be rejected.

## 9. Security Change Checklist (Mandatory for Auth/RBAC/Audit Scope)

If a release includes security-sensitive changes (authentication, RBAC, audit, session handling, identity mapping), add the checklist below to release preparation:

- [ ] Security-impact endpoints and role/permission changes are explicitly documented.
- [ ] OIDC environment variable changes and default behavior are documented.
- [ ] Session/token lifecycle changes (issuance, expiry, revoke) are documented.
- [ ] Audit logging behavior changes and retention impact are documented.
- [ ] Validation includes auth/RBAC regression coverage (unit/integration/CI).

Reference:

- `docs/11-security-operations-v0.0.3.md`

## 10. GitHub Actions Trigger Policy

To control CI resource consumption, repository CI workflows must not run automatically on every push or pull request by default.

Policy:

- CI is triggered manually via `workflow_dispatch` only.
- Release owner or maintainer triggers CI when verification is required.
- Local validation remains mandatory before merge/release.

Manual trigger options:

1. GitHub UI: `Actions` -> `CI` -> `Run workflow`.
2. GitHub CLI:
   - `gh workflow run ci.yml`

## 11. Sequential Release Gate (Mandatory)

Before starting a new development cycle, the immediate previous version must be fully released.

Mandatory gate:

- Previous version tag exists (`vX.Y.Z`).
- Previous version GitHub Release is published (not draft/prerelease).
- Previous version release notes and changelog are finalized.
- Previous milestone/issue set is closed or explicitly deferred with documented reason.

Definition:

- “Start a new cycle” includes creating or executing the next version backlog/milestone implementation work.

Enforcement:

- If this gate is not satisfied, new-cycle feature development should be blocked.
- Allowed exception: emergency fix required to complete or stabilize the pending previous release.
- Gate verification command: `make release-check VERSION=X.Y.Z` for the immediate previous version.

## 12. Automation Guardrail (Mandatory)

To avoid release omissions caused by manual steps, every release owner must use the scripts below:

- `scripts/release-publish.sh`:
  - handles tag creation/push and GitHub release create/edit from `release-notes/vX.Y.Z.md`
  - supports `--dry-run` for safe preflight checks
- `scripts/release-sync-check.sh`:
  - verifies changelog/release-note/tag/GitHub release synchronization
  - verifies published (non-draft/non-prerelease) status
  - verifies latest-release gate by default
- `scripts/release-github-issues-check.sh`:
  - parses `GitHub issues` from release metadata
  - verifies all listed issues are in `CLOSED` state (enforced for `>= v0.1.7`)

Minimal command set:

1. `make release-publish-dry VERSION=X.Y.Z`
2. `make release-publish VERSION=X.Y.Z`
3. `make release-check VERSION=X.Y.Z`

## 13. Product Strategy Baseline Gate (`v0.1.21+`, Mandatory)

To ensure long-term product coherence and customer-value continuity, all minor releases from `v0.1.21` onward must align with:

- `docs/62-smb-intelligent-cloud-product-strategy-baseline.md`

Release owner checklist additions:

- [ ] Release planning issue maps scope to at least one strategic pillar.
- [ ] Release scope includes explicit user-visible workflow simplification.
- [ ] Release scope defines target KPI movement (adoption, response, resolution, quality, or trust metric).
- [ ] Release notes include strategic objective summary and deferred strategic gaps.

If any item above is missing, release should be blocked until corrected.
