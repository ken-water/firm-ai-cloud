# CloudOps One Release Governance

Version: v1.0  
Date: 2026-03-02

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
3. Run release note validation script:
   - `bash scripts/validate-release-note.sh release-notes/vX.Y.Z.md`
4. Run project validation (`cargo check`, frontend build, and any required smoke tests).
5. Commit release artifacts.
6. Create tag and push:
   - `git tag vX.Y.Z`
   - `git push origin main --tags`
7. Publish GitHub Release by copying content from `release-notes/vX.Y.Z.md`.

## 7. Quality Gate Checklist

Before publishing, confirm all items:

- [ ] Release notes are fully in English.
- [ ] Required sections are complete.
- [ ] `CHANGELOG.md` updated.
- [ ] Version number is consistent across tag, changelog, and release note filename.
- [ ] Upgrade instructions were tested.
- [ ] Known issues are explicitly listed (or `None`).

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
