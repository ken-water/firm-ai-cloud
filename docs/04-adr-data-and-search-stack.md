# ADR-0001: Data, Cache, Search, and Object Storage Baseline

Status: Accepted  
Date: 2026-03-02  
Owners: CloudOps One core team

## Context

CloudOps One needs a practical MVP foundation that supports:

- CMDB transactional consistency
- low-latency runtime cache/session/rate-limit patterns
- alert/audit/ticket search and filtering
- attachment and export file storage

We previously discussed whether PostgreSQL plugins/extensions alone could replace Redis, OpenSearch, and MinIO.

## Decision

For MVP and initial open-source release, we standardize on:

- `PostgreSQL` as primary transactional datastore
- `Redis` as runtime cache and fast ephemeral data layer
- `OpenSearch` as search/index backend for alerts/audit/tickets
- `MinIO` as S3-compatible object storage for files and artifacts

## Rationale

- PostgreSQL is ideal for CMDB and workflow consistency but is not a full replacement for all cache/search/object use cases at scale.
- Redis provides predictable low-latency operations for counters, short TTL cache, and stream/session patterns.
- OpenSearch supports large log/alert query workloads and flexible aggregation/search experience.
- MinIO provides object-storage semantics (bucket lifecycle, large-object handling, S3 compatibility) that are not equivalent to storing files directly in PostgreSQL.

## Consequences

- Pros:
  - clear separation of concerns
  - better performance envelope for real-time monitoring UI
  - easier horizontal scaling for search and object storage
- Cons:
  - additional operational components
  - more deployment complexity for local environments

## MVP Guardrails

- Keep Redis and OpenSearch integration modular behind internal interfaces.
- Allow reduced local mode for contributors if needed:
  - required: PostgreSQL + MinIO
  - optional in local-dev profile: Redis/OpenSearch with feature flags
- Production profile still uses all four components.

## Revisit Criteria

Re-evaluate this ADR if any of the following happens:

- active data volume remains very small and operational simplicity becomes top priority
- contributor onboarding cost from multi-service setup blocks open-source adoption
- managed cloud services are introduced and architecture constraints change
