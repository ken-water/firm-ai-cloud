# Stream SSE Baseline Contract (v0.0.5)

Date: 2026-03-03

This document defines the first-pass real-time stream contract for monitoring pages.

## 1. Endpoint

- `GET /api/v1/streams/sse`

Query parameters:

- `site` (optional)
- `department` (optional)
- `severity` (optional): `all` | `critical` | `warning` | `info`

## 2. Envelope Schema

Every SSE data frame is a JSON envelope:

```json
{
  "event_type": "stream.heartbeat",
  "scope": {
    "site": "dc-a",
    "department": "platform",
    "severity": "all"
  },
  "timestamp": "2026-03-03T12:00:00Z",
  "payload": {}
}
```

Fields:

- `event_type`: logical event kind.
- `scope`: resolved subscription scope.
- `timestamp`: RFC3339 UTC timestamp.
- `payload`: event-specific object.

## 3. Baseline Event Types

- `stream.connected`
- `stream.heartbeat`
- `stream.stale`
- `stream.recovered`
- `stream.error`
- `alert.test`
- `alert.monitoring_sync`

## 4. Reconnect and Stale Semantics

On `stream.connected`, server sends guidance:

- `reconnect_after_ms`
- `heartbeat_interval_seconds`
- `stale_after_seconds`

Client behavior recommendation:

1. Reconnect with exponential backoff starting from `reconnect_after_ms`.
2. If no non-heartbeat alert event is observed for `stale_after_seconds`, mark UI as `Delayed`.
3. Clear `Delayed` state when `stream.recovered` or a fresh alert event arrives.

## 5. Scope Enforcement Baseline

- Route is protected by RBAC (`monitoring.sources.read`).
- Non-admin subscriptions must include at least one scope filter (`site` or `department`).
- Optional auth-scope headers:
  - `x-auth-site`
  - `x-auth-department`
- If requested scope exceeds auth-scope header value, server returns `403` with explicit scope-denied message.

## 6. Manual Check

```bash
curl -N -H "x-auth-user: operator" \
  "http://127.0.0.1:8080/api/v1/streams/sse?site=dc-a&severity=all"
```

Expected initial sequence:

1. `stream.connected`
2. `alert.test`
3. periodic `stream.heartbeat`
