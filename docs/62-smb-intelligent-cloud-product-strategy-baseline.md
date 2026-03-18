# CloudOps One Product Strategy Baseline (SMB and Solo-Operator Focus)

Date: 2026-03-18  
Version: v1.0  
Scope: `v0.1.21+` and future minor releases

## 1. Purpose

This document defines the mandatory product strategy baseline for CloudOps One.

It is the long-term iteration foundation for:

- rapid SMB adoption in real customer environments,
- measurable operator value delivery in daily operations,
- 3-5 year progression toward best-in-class usability and reputation.

## 2. North Star

CloudOps One should let SMB teams and solo operators manage IT and business resources with minimal technical background.

Success means users can answer in under 30 seconds:

1. what is risky now,
2. who is handling it,
3. what the next safe action should be.

## 3. Core Strategic Pillars

### 3.1 Unified Resource Model (Physical + Virtual + Business Assets)

The product must converge on one resource graph that includes:

- physical resources: sites, racks, hosts, network, storage,
- virtual resources: cloud services, SaaS apps, accounts, licenses,
- business/legal resources: software rights, patents, copyrights, trademarks, permits/contracts.

Required baseline:

- unified IDs and lifecycle states,
- owner and department binding,
- environment classification (`production/test/dev/idle`),
- cost and compliance metadata for analysis.

### 3.2 Business-Centric Operations Model

Monitoring, CMDB, topology, alerting, and tickets must all support business service context.

Required baseline:

- business domain -> business service -> app service -> resource dependency mapping,
- alert and incident routing by business ownership,
- business-level health, risk, and resource consumption visibility.

### 3.3 Organization and Workflow Model

Complex operations require cross-department execution and approval.

Required baseline:

- department/role/on-call/escalation-chain modeling,
- serial/parallel approval paths,
- timeout escalation, delegate approval, and full audit traceability.

### 3.4 Customizable Operations Dashboard System

The product must provide configurable dashboard experience instead of fixed monolithic pages.

Required baseline:

- modular dashboard widgets,
- user/team templates,
- rotation playlist with per-screen duration,
- condition-based focus switching (for example, severe incident preemption),
- role-specific defaults (NOC, manager, network, CMDB, change control).

### 3.5 Evidence-Backed AI Value Layer

AI capability must be practical, reliable, and traceable.

Required baseline:

- natural-language analytics over live platform data,
- answers with evidence links and time range,
- diagnosis suggestions with confidence and risk tags,
- actionable output to ticket/runbook/approval flows with human confirmation.

## 4. UX Principles (Mandatory)

All new UX work should follow these principles:

1. module-first navigation: each major capability has a clear home screen,
2. one-screen readability: high-level state is visible without deep drilling,
3. progressive drill-down: summary -> risk -> owner -> action,
4. consistent scope controls: site/business/environment/time,
5. clear role safety: read-only and write paths must be explicit.

## 5. Customer Feedback Operating System

Iteration decisions must be driven by real usage signals, not only internal assumptions.

Mandatory loop:

1. collect: product telemetry + issue tickets + interviews + pilot reviews,
2. classify: onboarding friction, daily operation friction, trust/visibility gap, automation gap,
3. prioritize: impact x frequency x strategic fit,
4. ship: small release with measurable target metrics,
5. review: 7/30/90 day adoption and quality follow-up.

## 6. Release-Level Strategy Gate (Mandatory for Minor Versions)

From `v0.1.21+`, each minor release planning issue must state:

1. which strategic pillar(s) it advances,
2. which user-visible workflow gets simpler,
3. which measurable value metric is expected to improve.

Each release note should include:

- strategic objective summary,
- before/after operator workflow impact,
- known tradeoffs and deferred strategic gaps.

## 7. Suggested KPI Baseline

Track at least the following product KPIs:

1. time-to-first-value (new tenant),
2. severe-alert assignment latency,
3. mean time to acknowledge and to resolve,
4. dashboard customization adoption rate,
5. AI answer evidence coverage rate,
6. monthly active operator retention and NPS trend.

## 8. Near-Term Execution Guidance (`v0.1.21+`)

Recommended next focus sequence:

1. information architecture and dashboard studio foundation,
2. business-centric monitoring and CMDB analytics views,
3. organization-aware workflow approvals,
4. evidence-backed AI analytics and execution assistant.

This sequence balances immediate usability gains with long-term platform coherence.
