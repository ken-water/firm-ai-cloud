# Web UI Quality Guardrails

Date: 2026-03-03

This document defines the minimum repeatable checks for web-console UI iterations.

## 1. Automated Checks (Local and CI Manual Trigger)

Run from repository root:

```bash
cargo check --workspace
cd apps/web-console
npm ci
npm run check:ui
```

`npm run check:ui` includes:

1. TypeScript + production build validation (`npm run build`)
2. Guardrail static checks (`npm run check:guardrails`) for:
   - RBAC write/admin role expressions in `App.tsx`
   - Presence of major section anchors (assets/relations)
   - Presence of core read-only and loading locale keys
   - Minimum count of write-action permission checks

## 2. Manual UX Verification Checklist

### 2.1 Login and Session

1. Header mode login works with a valid username.
2. Bearer mode login works with a valid token.
3. Sign-out clears UI session and returns to login gate.

### 2.2 RBAC Visibility

1. Admin role:
   - System Admin section visible.
   - CMDB/discovery/notification write actions visible.
2. Operator role:
   - System Admin section hidden.
   - CMDB/discovery/notification write actions visible.
3. Viewer role:
   - Write actions hidden.
   - Read-only guidance is visible in relations/discovery/notifications.

### 2.3 CMDB UX

1. Asset filters (search/status/class/site/sort) change result list correctly.
2. Reset filters returns full asset list.
3. Relation create rejects source=target.
4. Relation delete requires confirmation and success feedback appears.

### 2.4 Discovery and Notification UX

1. Discovery run/review actions show success feedback.
2. Discovery tables show loading/empty states correctly.
3. Notification create actions show success feedback.
4. Notification tables show loading/empty states correctly.

## 3. GitHub Actions Usage Policy

This repository keeps CI as `workflow_dispatch` only.

Guideline:

1. Do not rely on auto-trigger builds.
2. Trigger CI manually when preparing release tags, milestone completion, or high-risk refactors.
3. Use this checklist together with `docs/08-release-governance.md` before publishing release notes.
