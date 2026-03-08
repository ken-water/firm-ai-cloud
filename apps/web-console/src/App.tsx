import { useCallback, useEffect, useMemo, useState } from "react";
import type { CSSProperties } from "react";
import { useTranslation } from "react-i18next";
import { AppShell, AuthGate } from "./components/layout";
import {
  API_BASE_URL,
  AUTH_SESSION_EXPIRED_EVENT,
  DEFAULT_AUTH_TOKEN,
  DEFAULT_AUTH_USER,
  apiFetch,
  getRuntimeAuthSession,
  readErrorMessage,
  setRuntimeAuthSession,
  type AuthMode,
  type AuthSession
} from "./lib/api-client";
import {
  buildMetricPolylinePoints,
  buildParallelEdgeMeta,
  buildTopologyEdgePath,
  buildTopologyNodePositions,
  bucketBarWidth,
  createOwnerDraft,
  escapeCsvCell,
  formatMetricValue,
  formatSignedDelta,
  maxBucketAssetTotal,
  normalizeOwnerType,
  parseBindingList,
  parseDateMs,
  parseImpactDepth,
  parseImpactRelationTypesInput,
  parseMonitoringWindowMinutes,
  parseWorkflowReportRangeDays,
  readPayloadString,
  relationTypeColor,
  renderCustomFields,
  sampleValueForField,
  statusChipClass,
  topologyEdgeKey,
  topologyNodeFill,
  trimToNull,
  truncateTopologyLabel,
  workflowTemplateDisplayName,
  buildWorkflowDailyTrend,
  buildWorkflowTrendRankRows
} from "./lib/console-utils";
import {
  buildConsolePageHash,
  consolePageSections,
  defaultConsolePage,
  resolveConsolePageFromHash,
  type ConsolePage,
  type FunctionWorkspace,
  type MenuAxis
} from "./pages/console-page-routing";
import { CmdbSections } from "./pages/cmdb-sections";
import { IntegrationMonitoringSections } from "./pages/integration-monitoring-sections";
import { OverviewAdminSections } from "./pages/overview-admin-sections";
import { SetupAlertSections } from "./pages/setup-alert-sections";
import { TopologyWorkspaceSections } from "./pages/topology-workspace-sections";
import { WorkflowTicketSections } from "./pages/workflow-ticket-sections";

type Asset = {
  id: number;
  asset_class: string;
  name: string;
  hostname: string | null;
  ip: string | null;
  status: string;
  site: string | null;
  department: string | null;
  owner: string | null;
  qr_code: string | null;
  barcode: string | null;
  custom_fields: Record<string, unknown>;
  created_at: string;
  updated_at: string;
};

type AssetListResponse = {
  items: Asset[];
  total: number;
  limit: number;
  offset: number;
};

type AssetStatsBucket = {
  key: string;
  label: string;
  asset_total: number;
};

type AssetStatsUnbound = {
  department_assets: number;
  business_service_assets: number;
};

type AssetStatsScope = {
  site: string | null;
  status: string | null;
  asset_class: string | null;
};

type AssetStatsResponse = {
  generated_at: string;
  scope: AssetStatsScope;
  total_assets: number;
  status_buckets: AssetStatsBucket[];
  department_buckets: AssetStatsBucket[];
  business_service_buckets: AssetStatsBucket[];
  unbound: AssetStatsUnbound;
};

type FieldDefinition = {
  id: number;
  field_key: string;
  name: string;
  field_type: string;
  max_length: number | null;
  required: boolean;
  options: string[] | null;
  scanner_enabled: boolean;
  is_enabled: boolean;
  created_at: string;
  updated_at: string;
};

type AssetRelation = {
  id: number;
  src_asset_id: number;
  dst_asset_id: number;
  relation_type: string;
  source: string;
  created_at: string;
  updated_at: string;
};

type AssetOwnerBinding = {
  owner_type: string;
  owner_ref: string;
};

type AssetOperationalReadiness = {
  department_count: number;
  business_service_count: number;
  owner_count: number;
  can_transition_operational: boolean;
  missing: string[];
};

type AssetBindingsResponse = {
  asset_id: number;
  departments: string[];
  business_services: string[];
  owners: AssetOwnerBinding[];
  readiness: AssetOperationalReadiness;
};

type MonitoringBindingRecord = {
  id: number;
  asset_id: number;
  source_system: string;
  source_id: number | null;
  external_host_id: string | null;
  last_sync_status: string;
  last_sync_message: string | null;
  last_sync_at: string | null;
  mapping: Record<string, unknown>;
  created_at: string;
  updated_at: string;
};

type MonitoringSyncJobRecord = {
  id: number;
  asset_id: number;
  trigger_source: string;
  status: string;
  attempt: number;
  max_attempts: number;
  run_after: string;
  requested_by: string | null;
  requested_at: string;
  started_at: string | null;
  completed_at: string | null;
  last_error: string | null;
  payload: Record<string, unknown>;
  created_at: string;
  updated_at: string;
};

type AssetMonitoringBindingResponse = {
  asset_id: number;
  binding: MonitoringBindingRecord | null;
  latest_job: MonitoringSyncJobRecord | null;
};

type AssetLifecycleTransitionResponse = {
  asset_id: number;
  previous_status: string;
  status: string;
  readiness: AssetOperationalReadiness;
};

type ImpactNode = {
  id: number;
  name: string;
  asset_class: string;
  status: string;
  depth: number;
};

type ImpactEdge = {
  id: number;
  src_asset_id: number;
  dst_asset_id: number;
  relation_type: string;
  source: string;
  direction: string;
  depth: number;
};

type AssetImpactResponse = {
  root_asset_id: number;
  direction: string;
  depth_limit: number;
  relation_types: string[];
  nodes: ImpactNode[];
  edges: ImpactEdge[];
  affected_business_services: ImpactNode[];
  affected_owners: ImpactNode[];
};

type TopologyMapNode = {
  id: number;
  name: string;
  asset_class: string;
  status: string;
  site: string | null;
  department: string | null;
  monitoring_status: string | null;
  latest_job_status: string | null;
  health: string;
};

type TopologyMapEdge = {
  id: number;
  src_asset_id: number;
  dst_asset_id: number;
  relation_type: string;
  source: string;
};

type TopologyMapScope = {
  scope_key: string;
  site: string | null;
  department: string | null;
};

type TopologyMapWindow = {
  limit: number;
  offset: number;
};

type TopologyMapStats = {
  total_nodes: number;
  window_nodes: number;
  window_edges: number;
};

type TopologyMapResponse = {
  generated_at: string;
  scope: TopologyMapScope;
  window: TopologyMapWindow;
  stats: TopologyMapStats;
  nodes: TopologyMapNode[];
  edges: TopologyMapEdge[];
  empty: boolean;
};

type TopologyDiagnosticsChecklistStep = {
  key: string;
  title: string;
  done: boolean;
  hint: string;
};

type TopologyDiagnosticsQuickAction = {
  key: string;
  label: string;
  href: string | null;
  api_path: string | null;
  method: string | null;
  body: Record<string, unknown> | null;
  requires_write: boolean;
};

type TopologyDiagnosticsResponse = {
  generated_at: string;
  window_minutes: number;
  relation: {
    edge_id: number;
    src_asset_id: number;
    src_name: string;
    dst_asset_id: number;
    dst_name: string;
    relation_type: string;
    source: string;
    site: string | null;
    department: string | null;
  };
  trend: Array<{
    bucket_at: string;
    total_jobs: number;
    failed_jobs: number;
  }>;
  alerts: Array<{
    id: number;
    alert_source: string;
    alert_key: string;
    title: string;
    severity: string;
    status: string;
    asset_id: number | null;
    last_seen_at: string;
  }>;
  recent_changes: Array<{
    id: number;
    actor: string;
    action: string;
    target_type: string;
    target_id: string | null;
    result: string;
    message: string | null;
    created_at: string;
  }>;
  impacted: {
    services: Array<{
      id: number;
      name: string;
      asset_class: string;
      site: string | null;
      department: string | null;
    }>;
    owners: Array<{
      id: number;
      name: string;
      asset_class: string;
      site: string | null;
      department: string | null;
    }>;
  };
  checklist: TopologyDiagnosticsChecklistStep[];
  quick_actions: TopologyDiagnosticsQuickAction[];
};

type DiscoveryJob = {
  id: number;
  name: string;
  source_type: string;
  scope: Record<string, unknown>;
  schedule: string | null;
  status: string;
  is_enabled: boolean;
  last_run_at: string | null;
  next_run_at: string | null;
  last_run_status: string | null;
  last_error: string | null;
  created_at: string;
  updated_at: string;
};

type DiscoveryCandidate = {
  id: number;
  job_id: number | null;
  fingerprint: string;
  payload: Record<string, unknown>;
  review_status: string;
  discovered_at: string;
  reviewed_by: string | null;
  reviewed_at: string | null;
  created_at: string;
  updated_at: string;
};

type NotificationChannel = {
  id: number;
  name: string;
  channel_type: string;
  target: string;
  config: Record<string, unknown>;
  is_enabled: boolean;
  created_at: string;
  updated_at: string;
};

type NotificationTemplate = {
  id: number;
  event_type: string;
  title_template: string;
  body_template: string;
  is_enabled: boolean;
  created_at: string;
  updated_at: string;
};

type NotificationSubscription = {
  id: number;
  channel_id: number;
  event_type: string;
  site: string | null;
  department: string | null;
  is_enabled: boolean;
  created_at: string;
  updated_at: string;
};

type MonitoringSource = {
  id: number;
  name: string;
  source_type: string;
  endpoint: string;
  proxy_endpoint: string | null;
  auth_type: string;
  username: string | null;
  secret_ref: string;
  secret_storage: string;
  site: string | null;
  department: string | null;
  is_enabled: boolean;
  last_probe_at: string | null;
  last_probe_status: string | null;
  last_probe_message: string | null;
  created_at: string;
  updated_at: string;
};

type MonitoringOverviewSummary = {
  source_total: number;
  source_enabled_total: number;
  source_reachable_total: number;
  source_unreachable_total: number;
  source_unknown_probe_total: number;
  asset_total: number;
  monitored_asset_total: number;
};

type MonitoringOverviewHealthSummary = {
  healthy: number;
  warning: number;
  critical: number;
  unknown: number;
};

type MonitoringOverviewLayer = {
  layer: string;
  asset_total: number;
  monitored_asset_total: number;
  health: MonitoringOverviewHealthSummary;
};

type MonitoringOverviewResponse = {
  generated_at: string;
  scope: {
    site: string | null;
    department: string | null;
  };
  summary: MonitoringOverviewSummary;
  layers: MonitoringOverviewLayer[];
  empty: boolean;
};

type MonitoringMetricPoint = {
  timestamp: string;
  value: number;
};

type MonitoringMetricSeries = {
  metric: string;
  label: string;
  unit: string;
  item_key: string | null;
  note: string | null;
  latest: MonitoringMetricPoint | null;
  points: MonitoringMetricPoint[];
};

type MonitoringMetricsSource = {
  id: number;
  name: string;
  endpoint: string;
  auth_type: string;
};

type MonitoringMetricsResponse = {
  generated_at: string;
  asset_id: number;
  asset_name: string;
  host_id: string;
  window_minutes: number;
  source: MonitoringMetricsSource;
  series: MonitoringMetricSeries[];
};

type WorkflowStepKind = "approval" | "script" | "manual";

type WorkflowStepDefinition = {
  id: string;
  name: string;
  kind: WorkflowStepKind;
  auto_run: boolean;
  script: string | null;
  timeout_seconds: number;
  approver_group: string | null;
};

type WorkflowDefinition = {
  steps: WorkflowStepDefinition[];
};

type WorkflowTemplate = {
  id: number;
  name: string;
  description: string | null;
  definition: WorkflowDefinition;
  is_enabled: boolean;
  created_at: string;
  updated_at: string;
};

type WorkflowRequest = {
  id: number;
  template_id: number;
  template_name: string;
  title: string;
  requester: string;
  status: string;
  current_step_index: number;
  payload: Record<string, unknown>;
  last_error: string | null;
  approved_by: string | null;
  approved_at: string | null;
  executed_by: string | null;
  executed_at: string | null;
  completed_at: string | null;
  created_at: string;
  updated_at: string;
};

type WorkflowExecutionLog = {
  id: number;
  request_id: number;
  step_index: number;
  step_id: string;
  step_name: string;
  step_kind: string;
  status: string;
  executor: string | null;
  started_at: string | null;
  finished_at: string | null;
  duration_ms: number | null;
  exit_code: number | null;
  output: string | null;
  error: string | null;
  metadata: Record<string, unknown>;
  created_at: string;
  updated_at: string;
};

type PlaybookParameterField = {
  key: string;
  type: "string" | "integer" | "number" | "boolean" | "enum";
  required?: boolean;
  min?: number;
  max?: number;
  max_length?: number;
  options?: string[];
  default?: unknown;
};

type PlaybookParameterSchema = {
  fields: PlaybookParameterField[];
};

type PlaybookCatalogItem = {
  id: number;
  key: string;
  name: string;
  category: string;
  risk_level: "low" | "medium" | "high" | "critical";
  params: PlaybookParameterSchema;
  description: string | null;
  requires_confirmation: boolean;
  rbac_hint: Record<string, unknown>;
  is_enabled: boolean;
  is_system: boolean;
  updated_at: string;
};

type PlaybookListResponse = {
  items: PlaybookCatalogItem[];
  total: number;
  limit: number;
  offset: number;
};

type PlaybookExecutionDetail = {
  id: number;
  playbook_id: number;
  playbook_key: string;
  playbook_name: string;
  category: string;
  risk_level: string;
  actor: string;
  asset_ref: string | null;
  mode: "dry_run" | "execute";
  status: string;
  confirmation_required: boolean;
  confirmation_verified: boolean;
  confirmed_at: string | null;
  params: Record<string, unknown>;
  planned_steps: string[];
  result: Record<string, unknown>;
  related_ticket_id: number | null;
  related_alert_id: number | null;
  replay_of_execution_id: number | null;
  expires_at: string | null;
  finished_at: string | null;
  created_at: string;
  updated_at: string;
};

type PlaybookExecutionListItem = {
  id: number;
  playbook_key: string;
  playbook_name: string;
  category: string;
  risk_level: string;
  actor: string;
  asset_ref: string | null;
  mode: string;
  status: string;
  confirmation_required: boolean;
  confirmation_verified: boolean;
  related_ticket_id: number | null;
  related_alert_id: number | null;
  replay_of_execution_id: number | null;
  created_at: string;
  finished_at: string | null;
};

type PlaybookExecutionListResponse = {
  items: PlaybookExecutionListItem[];
  total: number;
  limit: number;
  offset: number;
};

type PlaybookDryRunResponse = {
  execution: PlaybookExecutionDetail;
  risk_summary: {
    risk_level: string;
    requires_confirmation: boolean;
    ttl_minutes: number;
    summary: string;
  };
  confirmation: {
    token: string;
    expires_at: string;
    instruction: string;
  } | null;
  reservation_context: ChangeCalendarReservationRecord | null;
};

type PlaybookMaintenanceWindow = {
  day_of_week: number;
  start: string;
  end: string;
  label: string | null;
};

type PlaybookExecutionPolicyRuntime = {
  timezone_now: string;
  in_maintenance_window: boolean;
  next_allowed_at: string | null;
  blocked_reason: string | null;
};

type PlaybookExecutionPolicyResponse = {
  policy: {
    policy_key: string;
    timezone_name: string;
    maintenance_windows: PlaybookMaintenanceWindow[];
    change_freeze_enabled: boolean;
    override_requires_reason: boolean;
    updated_by: string;
    updated_at: string;
  };
  runtime: PlaybookExecutionPolicyRuntime;
};

type PlaybookApprovalRequestStatus = "pending" | "approved" | "rejected" | "expired" | "used";

type PlaybookApprovalRequestRecord = {
  id: number;
  dry_run_execution_id: number;
  playbook_id: number;
  playbook_key: string;
  requester: string;
  request_note: string | null;
  status: PlaybookApprovalRequestStatus;
  approver: string | null;
  approver_note: string | null;
  approval_token: string | null;
  approved_at: string | null;
  expires_at: string;
  used_at: string | null;
  created_at: string;
  updated_at: string;
};

type PlaybookApprovalRequestListResponse = {
  items: PlaybookApprovalRequestRecord[];
  total: number;
  limit: number;
  offset: number;
};

type DailyCockpitAction = {
  key: string;
  label: string;
  href: string | null;
  api_path: string | null;
  method: string | null;
  body: Record<string, unknown> | null;
  requires_write: boolean;
};

type DailyCockpitQueueItem = {
  queue_key: string;
  item_type: string;
  priority_score: number;
  priority_level: string;
  rationale: string;
  rationale_details: string[];
  observed_at: string;
  site: string | null;
  department: string | null;
  entity: Record<string, unknown>;
  actions: DailyCockpitAction[];
};

type DailyCockpitQueueResponse = {
  generated_at: string;
  scope: {
    site: string | null;
    department: string | null;
  };
  window: {
    limit: number;
    offset: number;
    total: number;
  };
  items: DailyCockpitQueueItem[];
};

type NextBestActionItem = {
  suggestion_key: string;
  domain: string;
  priority_score: number;
  risk_level: string;
  reason: string;
  source_signal: string;
  observed_at: string;
  entity: Record<string, unknown>;
  action: DailyCockpitAction;
};

type NextBestActionResponse = {
  generated_at: string;
  scope: {
    site: string | null;
    department: string | null;
  };
  shift_date: string;
  total: number;
  items: NextBestActionItem[];
};

type OpsChecklistItem = {
  template_key: string;
  title: string;
  description: string | null;
  frequency: string;
  due_weekday: number | null;
  status: "pending" | "completed" | "skipped";
  overdue: boolean;
  exception_note: string | null;
  completed_at: string | null;
  updated_at: string | null;
  guidance: string | null;
};

type OpsChecklistResponse = {
  generated_at: string;
  checklist_date: string;
  operator: string;
  scope: {
    site: string | null;
    department: string | null;
  };
  summary: {
    total: number;
    completed: number;
    pending: number;
    skipped: number;
    overdue: number;
  };
  items: OpsChecklistItem[];
};

type OpsChecklistUpdateResponse = {
  checklist_date: string;
  template_key: string;
  status: string;
  operator: string;
  scope: {
    site: string | null;
    department: string | null;
  };
  completed_at: string | null;
  exception_note: string | null;
};

type BackupPolicyRecord = {
  id: number;
  policy_key: string;
  name: string;
  frequency: "daily" | "weekly";
  schedule_time_utc: string;
  schedule_weekday: number | null;
  retention_days: number;
  destination_type: "s3" | "nfs" | "local";
  destination_uri: string;
  destination_validated: boolean;
  drill_enabled: boolean;
  drill_frequency: "weekly" | "monthly" | "quarterly";
  drill_weekday: number | null;
  drill_time_utc: string;
  last_backup_status: string;
  last_backup_at: string | null;
  last_backup_error: string | null;
  last_drill_status: string;
  last_drill_at: string | null;
  last_drill_error: string | null;
  next_backup_at: string | null;
  next_drill_at: string | null;
  updated_by: string;
  created_at: string;
  updated_at: string;
};

type BackupPolicyListResponse = {
  generated_at: string;
  total: number;
  items: BackupPolicyRecord[];
};

type BackupPolicyRunRecord = {
  id: number;
  policy_id: number;
  run_type: "backup" | "drill";
  status: "succeeded" | "failed";
  triggered_by: string;
  triggered_by_scheduler: boolean;
  note: string | null;
  remediation_hint: string | null;
  error_message: string | null;
  started_at: string;
  finished_at: string;
  restore_evidence_count: number;
  latest_restore_verified_at: string | null;
  latest_restore_closure_status: string | null;
  created_at: string;
};

type BackupPolicyRunsResponse = {
  generated_at: string;
  total: number;
  limit: number;
  offset: number;
  items: BackupPolicyRunRecord[];
};

type BackupPolicyRunResult = {
  policy: BackupPolicyRecord;
  run: BackupPolicyRunRecord;
  remediation_hints: string[];
};

type BackupSchedulerTickResponse = {
  generated_at: string;
  backup_runs: number;
  drill_runs: number;
  runs: BackupPolicyRunRecord[];
};

type BackupRestoreEvidenceRecord = {
  id: number;
  run_id: number;
  policy_id: number;
  run_type: "backup" | "drill";
  run_status: "succeeded" | "failed";
  ticket_ref: string | null;
  artifact_url: string;
  note: string | null;
  verifier: string;
  closure_status: "open" | "closed";
  closed_at: string | null;
  closed_by: string | null;
  created_at: string;
  updated_at: string;
};

type BackupRestoreEvidenceListResponse = {
  generated_at: string;
  total: number;
  limit: number;
  offset: number;
  coverage: {
    required_runs: number;
    covered_runs: number;
    missing_runs: number;
  };
  missing_run_ids: number[];
  items: BackupRestoreEvidenceRecord[];
};

type BackupEvidenceCompliancePolicy = {
  policy_key: string;
  mode: "advisory" | "enforced";
  sla_hours: number;
  require_failed_runs: boolean;
  require_drill_runs: boolean;
  updated_by: string;
  updated_at: string;
};

type BackupEvidenceCompliancePolicyResponse = {
  generated_at: string;
  policy: BackupEvidenceCompliancePolicy;
};

type BackupEvidenceCompliancePolicyDraft = {
  mode: "advisory" | "enforced";
  sla_hours: string;
  require_failed_runs: boolean;
  require_drill_runs: boolean;
  note: string;
};

type BackupEvidenceComplianceScorecardItem = {
  run_id: number;
  policy_id: number;
  run_type: "backup" | "drill";
  run_status: "succeeded" | "failed";
  started_at: string;
  deadline_at: string;
  evidence_total: number;
  closed_evidence_count: number;
  closure_state: "open_within_sla" | "overdue_open" | "closed_within_sla" | "closed_late";
  closed_at: string | null;
  latest_evidence_id: number | null;
  latest_evidence_at: string | null;
  latest_closure_status: "open" | "closed" | null;
  overdue_hours: number;
  run_ref: string;
};

type BackupEvidenceComplianceScorecardResponse = {
  generated_at: string;
  scorecard_key: string;
  week_start: string;
  week_end: string;
  as_of: string;
  policy: BackupEvidenceCompliancePolicy;
  metrics: {
    required_runs: number;
    closed_runs: number;
    closed_within_sla_runs: number;
    open_runs: number;
    overdue_runs: number;
    overdue_open_runs: number;
  };
  timeline: Array<{
    date: string;
    required_runs: number;
    closed_runs: number;
    overdue_runs: number;
  }>;
  overdue_items: BackupEvidenceComplianceScorecardItem[];
};

type BackupEvidenceComplianceScorecardExportResponse = {
  generated_at: string;
  scorecard_key: string;
  format: "csv" | "json";
  content: string;
};

type BackupRestoreEvidenceForm = {
  run_id: string;
  ticket_ref: string;
  artifact_url: string;
  note: string;
  verifier: string;
  close_evidence: boolean;
};

type ChangeCalendarEvent = {
  event_key: string;
  event_type: string;
  severity: string;
  title: string;
  starts_at: string;
  ends_at: string;
  source_type: string;
  source_id: string;
  details: string;
};

type ChangeCalendarResponse = {
  generated_at: string;
  range: {
    start_date: string;
    end_date: string;
  };
  total: number;
  items: ChangeCalendarEvent[];
};

type ChangeCalendarConflictItem = {
  code: string;
  title: string;
  detail: string;
  severity: string;
  source: string;
};

type ChangeCalendarConflictResponse = {
  generated_at: string;
  slot: {
    start_at: string;
    end_at: string;
    operation_kind: string;
    risk_level: string;
  };
  has_conflict: boolean;
  decision_reason: string;
  conflicts: ChangeCalendarConflictItem[];
  recommended_slot: string | null;
};

type ChangeCalendarConflictDraft = {
  start_at_local: string;
  end_at_local: string;
  operation_kind: string;
  risk_level: "low" | "medium" | "high" | "critical";
};

type ChangeCalendarReservationRecord = {
  id: number;
  operation_kind: string;
  risk_level: "low" | "medium" | "high" | "critical";
  start_at: string;
  end_at: string;
  site: string | null;
  department: string | null;
  owner: string;
  note: string | null;
  status: "reserved" | "cancelled";
  created_at: string;
  updated_at: string;
};

type ChangeCalendarReservationListResponse = {
  generated_at: string;
  range: {
    start_date: string;
    end_date: string;
  };
  total: number;
  items: ChangeCalendarReservationRecord[];
};

type ChangeCalendarReservationDraft = {
  start_at_local: string;
  end_at_local: string;
  operation_kind: string;
  risk_level: "low" | "medium" | "high" | "critical";
  owner: string;
  site: string;
  department: string;
  note: string;
};

type ChangeCalendarSlotRecommendationItem = {
  rank: number;
  start_at: string;
  end_at: string;
  score: number;
  rationale: string[];
};

type ChangeCalendarSlotRecommendationResponse = {
  generated_at: string;
  operation_kind: string;
  risk_level: "low" | "medium" | "high" | "critical";
  duration_minutes: number;
  scope: {
    site: string | null;
    department: string | null;
  };
  pending_risky_workload: {
    unresolved_incidents: number;
    high_priority_tickets: number;
    pending_approvals: number;
  };
  total: number;
  items: ChangeCalendarSlotRecommendationItem[];
};

type WeeklyDigestResponse = {
  generated_at: string;
  digest_key: string;
  week_start: string;
  week_end: string;
  metrics: {
    open_critical_alerts: number;
    open_warning_alerts: number;
    suppressed_alert_threads: number;
    stale_open_tickets: number;
    workflow_approval_backlog: number;
    playbook_approval_backlog: number;
    backup_failed_policies: number;
    drill_failed_policies: number;
    continuity_runs_requiring_evidence: number;
    continuity_runs_with_evidence: number;
    continuity_runs_missing_evidence: number;
    locked_local_accounts: number;
    local_accounts_without_mfa: number;
  };
  top_risks: string[];
  unresolved_items: string[];
  recommended_actions: string[];
};

type WeeklyDigestExportResponse = {
  generated_at: string;
  digest_key: string;
  format: "csv" | "json";
  content: string;
};

type HandoverCarryoverItem = {
  item_key: string;
  source_type: string;
  source_id: number;
  title: string;
  owner: string;
  next_owner: string;
  next_action: string;
  status: "open" | "closed";
  note: string | null;
  risk_level: string;
  observed_at: string;
  source_ref: string;
  overdue: boolean;
  overdue_days: number;
  ownership_violations: string[];
};

type HandoverDigestResponse = {
  generated_at: string;
  digest_key: string;
  shift_date: string;
  metrics: {
    unresolved_incidents: number;
    escalation_backlog: number;
    failed_continuity_runs: number;
    pending_approvals: number;
    restore_evidence_missing_runs: number;
    closed_items: number;
    overdue_open_items: number;
    ownership_gap_items: number;
  };
  overdue_trend: Array<{
    shift_date: string;
    open_items: number;
    overdue_items: number;
  }>;
  items: HandoverCarryoverItem[];
};

type HandoverDigestExportResponse = {
  generated_at: string;
  digest_key: string;
  format: "csv" | "json";
  content: string;
};

type HandoverReminderResponse = {
  generated_at: string;
  digest_key: string;
  shift_date: string;
  total: number;
  items: HandoverCarryoverItem[];
};

type HandoverReminderExportResponse = {
  generated_at: string;
  digest_key: string;
  format: "csv" | "json";
  content: string;
};

type RunbookTemplateParamField = {
  key: string;
  label: string;
  field_type: "string" | "number" | "enum";
  required: boolean;
  options: string[];
  min_value: number | null;
  max_value: number | null;
  default_value: string | null;
  placeholder: string | null;
};

type RunbookTemplateChecklistItem = {
  key: string;
  label: string;
  detail: string;
};

type RunbookTemplateStepItem = {
  step_id: string;
  name: string;
  detail: string;
};

type RunbookTemplateCatalogItem = {
  key: string;
  name: string;
  description: string;
  category: string;
  execution_modes: ("simulate" | "live")[];
  params: RunbookTemplateParamField[];
  preflight: RunbookTemplateChecklistItem[];
  steps: RunbookTemplateStepItem[];
};

type RunbookTemplateCatalogResponse = {
  generated_at: string;
  total: number;
  items: RunbookTemplateCatalogItem[];
};

type RunbookExecutionTimelineEvent = {
  step_id: string;
  name: string;
  detail: string;
  status: "succeeded" | "failed";
  started_at: string;
  finished_at: string;
  output: string;
  remediation_hint: string | null;
};

type RunbookExecutionEvidenceRecord = {
  summary: string;
  ticket_ref: string | null;
  artifact_url: string | null;
  captured_at: string;
  execution_status: "succeeded" | "failed";
  operator: string;
};

type RunbookTemplateExecutionItem = {
  id: number;
  template_key: string;
  template_name: string;
  status: "succeeded" | "failed";
  execution_mode: "simulate" | "live";
  actor: string;
  params: Record<string, unknown>;
  preflight: {
    confirmed: string[];
    total_required: number;
  };
  timeline: RunbookExecutionTimelineEvent[];
  evidence: RunbookExecutionEvidenceRecord;
  runtime_summary: Record<string, unknown>;
  remediation_hints: string[];
  note: string | null;
  created_at: string;
  updated_at: string;
};

type RunbookTemplateExecutionListResponse = {
  generated_at: string;
  total: number;
  limit: number;
  offset: number;
  items: RunbookTemplateExecutionItem[];
};

type RunbookTemplateExecuteResponse = {
  generated_at: string;
  template: RunbookTemplateCatalogItem;
  execution: RunbookTemplateExecutionItem;
};

type RunbookExecutionPolicyItem = {
  policy_key: string;
  mode: "simulate_only" | "hybrid_live";
  live_templates: string[];
  max_live_step_timeout_seconds: number;
  allow_simulate_failure: boolean;
  note: string | null;
  updated_by: string;
  created_at: string;
  updated_at: string;
};

type RunbookExecutionPolicyResponse = {
  generated_at: string;
  policy: RunbookExecutionPolicyItem;
};

type RunbookExecutionPolicyDraft = {
  mode: "simulate_only" | "hybrid_live";
  live_templates_csv: string;
  max_live_step_timeout_seconds: string;
  allow_simulate_failure: boolean;
  note: string;
};

type RunbookEvidenceDraft = {
  summary: string;
  ticket_ref: string;
  artifact_url: string;
  note: string;
};

type IncidentCommandStatus = "triage" | "in_progress" | "blocked" | "mitigated" | "postmortem";

type IncidentCommandRecord = {
  alert_id: number;
  alert_source: string;
  alert_key: string;
  title: string;
  severity: string;
  alert_status: string;
  site: string | null;
  department: string | null;
  command_status: IncidentCommandStatus;
  command_owner: string;
  eta_at: string | null;
  blocker: string | null;
  summary: string | null;
  updated_by: string;
  updated_at: string;
};

type IncidentCommandEvent = {
  id: number;
  alert_id: number;
  event_type: "created" | "status_transition" | "command_updated";
  from_status: IncidentCommandStatus | null;
  to_status: IncidentCommandStatus;
  command_owner: string;
  eta_at: string | null;
  blocker: string | null;
  summary: string | null;
  note: string | null;
  actor: string;
  created_at: string;
};

type IncidentCommandListResponse = {
  generated_at: string;
  total: number;
  limit: number;
  offset: number;
  items: IncidentCommandRecord[];
};

type IncidentCommandDetailResponse = {
  generated_at: string;
  item: IncidentCommandRecord;
  timeline: IncidentCommandEvent[];
};

type IncidentCommandDraft = {
  alert_id: string;
  status: IncidentCommandStatus;
  owner: string;
  eta_at: string;
  blocker: string;
  summary: string;
  note: string;
};

type BackupPolicyForm = {
  policy_id: string;
  policy_key: string;
  name: string;
  frequency: "daily" | "weekly";
  schedule_time_utc: string;
  schedule_weekday: string;
  retention_days: string;
  destination_type: "s3" | "nfs" | "local";
  destination_uri: string;
  drill_enabled: boolean;
  drill_frequency: "weekly" | "monthly" | "quarterly";
  drill_weekday: string;
  drill_time_utc: string;
  note: string;
};

type TicketListItem = {
  id: number;
  ticket_no: string;
  title: string;
  status: string;
  priority: string;
  category: string;
  requester: string;
  assignee: string | null;
  workflow_template_id: number | null;
  workflow_request_id: number | null;
  closed_at: string | null;
  created_at: string;
  updated_at: string;
  asset_link_count: number;
  alert_link_count: number;
  escalation_state: "normal" | "near_breach" | "breached";
  escalation_age_minutes: number;
  escalation_due_at: string | null;
  escalation_last_action_at: string | null;
  escalation_last_action_kind: string | null;
};

type TicketAssetLink = {
  asset_id: number;
  asset_name: string | null;
  asset_class: string | null;
  asset_status: string | null;
};

type TicketAlertLink = {
  alert_source: string;
  alert_key: string;
  alert_title: string | null;
  severity: string | null;
};

type TicketRecord = {
  id: number;
  ticket_no: string;
  title: string;
  description: string | null;
  status: string;
  priority: string;
  category: string;
  requester: string;
  assignee: string | null;
  workflow_template_id: number | null;
  workflow_request_id: number | null;
  metadata: Record<string, unknown>;
  last_status_note: string | null;
  closed_at: string | null;
  created_at: string;
  updated_at: string;
};

type TicketEscalationPolicyRecord = {
  id: number;
  policy_key: string;
  name: string;
  is_enabled: boolean;
  near_critical_minutes: number;
  breach_critical_minutes: number;
  near_high_minutes: number;
  breach_high_minutes: number;
  near_medium_minutes: number;
  breach_medium_minutes: number;
  near_low_minutes: number;
  breach_low_minutes: number;
  escalate_to_assignee: string;
  updated_by: string;
  created_at: string;
  updated_at: string;
};

type TicketEscalationActionRecord = {
  id: number;
  ticket_id: number;
  action_kind: string;
  state_before: string;
  state_after: string;
  from_assignee: string | null;
  to_assignee: string | null;
  actor: string;
  reason: string | null;
  created_at: string;
};

type TicketEscalationDetail = {
  policy_key: string;
  policy_name: string;
  policy_enabled: boolean;
  state: string;
  age_minutes: number;
  near_breach_minutes: number;
  breach_minutes: number;
  due_at: string | null;
  escalate_to_assignee: string;
  latest_action: TicketEscalationActionRecord | null;
};

type TicketEscalationPreviewResponse = {
  priority: string;
  status: string;
  ticket_age_minutes: number;
  state: string;
  near_breach_minutes: number;
  breach_minutes: number;
  should_escalate: boolean;
  escalate_to_assignee: string;
};

type TicketEscalationActionsResponse = {
  generated_at: string;
  total: number;
  limit: number;
  offset: number;
  items: TicketEscalationActionRecord[];
};

type TicketEscalationRunResponse = {
  generated_at: string;
  dry_run: boolean;
  policy_key: string;
  processed: number;
  escalated: number;
  skipped: number;
  actions: TicketEscalationActionRecord[];
};

type TicketEscalationPolicyDraft = {
  name: string;
  is_enabled: boolean;
  near_critical_minutes: string;
  breach_critical_minutes: string;
  near_high_minutes: string;
  breach_high_minutes: string;
  near_medium_minutes: string;
  breach_medium_minutes: string;
  near_low_minutes: string;
  breach_low_minutes: string;
  escalate_to_assignee: string;
  note: string;
};

type TicketEscalationPreviewDraft = {
  priority: "low" | "medium" | "high" | "critical";
  status: "open" | "in_progress" | "resolved" | "closed" | "cancelled";
  ticket_age_minutes: string;
  current_assignee: string;
};

type TicketDetailResponse = {
  ticket: TicketRecord;
  asset_links: TicketAssetLink[];
  alert_links: TicketAlertLink[];
  escalation: TicketEscalationDetail;
};

type TicketListResponse = {
  items: TicketListItem[];
  total: number;
  limit: number;
  offset: number;
};

type SetupCheckStatus = "pass" | "warn" | "fail";

type SetupCheckItem = {
  key: string;
  title: string;
  status: SetupCheckStatus;
  critical: boolean;
  message: string;
  remediation: string;
};

type SetupChecklistSummary = {
  total: number;
  passed: number;
  warned: number;
  failed: number;
  critical_failed: number;
  ready: boolean;
};

type SetupChecklistResponse = {
  generated_at: string;
  category: string;
  summary: SetupChecklistSummary;
  checks: SetupCheckItem[];
};

type SetupTemplateSchemaField = {
  key: string;
  label: string;
  type: "string" | "enum";
  required?: boolean;
  options?: string[];
  default?: string;
  placeholder?: string;
  max_length?: number;
};

type SetupTemplateCatalogItem = {
  key: string;
  name: string;
  category: string;
  description: string | null;
  param_schema: {
    fields: SetupTemplateSchemaField[];
  };
  apply_plan: {
    actions?: string[];
    [key: string]: unknown;
  };
  rollback_hints: string[];
  is_enabled: boolean;
  is_system: boolean;
  updated_at: string;
};

type SetupTemplateCatalogResponse = {
  items: SetupTemplateCatalogItem[];
  total: number;
};

type SetupTemplateValidationError = {
  field: string;
  message: string;
};

type SetupTemplatePreviewAction = {
  action_key: string;
  summary: string;
  outcome: string;
  detail: string;
};

type SetupTemplatePreviewResponse = {
  template: SetupTemplateCatalogItem;
  ready: boolean;
  validation_errors: SetupTemplateValidationError[];
  actions: SetupTemplatePreviewAction[];
  rollback_hints: string[];
};

type SetupTemplateApplyAction = {
  action_key: string;
  outcome: string;
  target_id: string | null;
  detail: string;
};

type SetupTemplateApplyResponse = {
  actor: string;
  template_key: string;
  status: string;
  applied_actions: SetupTemplateApplyAction[];
  rollback_hints: string[];
};

type SetupProfileCatalogItem = {
  key: string;
  name: string;
  description: string;
  target_scale: string;
  defaults: Record<string, unknown>;
};

type SetupProfileCatalogResponse = {
  items: SetupProfileCatalogItem[];
  total: number;
};

type SetupProfileChangeSummary = {
  domain: string;
  before: string;
  after: string;
  changed: boolean;
};

type SetupProfilePreviewResponse = {
  profile: SetupProfileCatalogItem;
  ready: boolean;
  summary: SetupProfileChangeSummary[];
};

type SetupProfileApplyAction = {
  action_key: string;
  outcome: string;
  detail: string;
};

type SetupProfileApplyResponse = {
  run_id: number;
  actor: string;
  profile_key: string;
  status: string;
  actions: SetupProfileApplyAction[];
  history_hint: string;
};

type SetupProfileHistoryRecord = {
  id: number;
  profile_key: string;
  profile_name: string;
  actor: string;
  status: string;
  note: string | null;
  reverted_by: string | null;
  reverted_at: string | null;
  created_at: string;
};

type SetupProfileHistoryResponse = {
  items: SetupProfileHistoryRecord[];
  total: number;
  limit: number;
  offset: number;
};

type SetupProfileRevertResponse = {
  run_id: number;
  status: string;
  reverted_by: string;
  reverted_at: string;
};

type AlertSeverity = "critical" | "warning" | "info";
type AlertStatus = "open" | "acknowledged" | "closed";

type AlertRecord = {
  id: number;
  alert_source: string;
  alert_key: string;
  dedup_key: string;
  title: string;
  severity: AlertSeverity;
  status: AlertStatus;
  site: string | null;
  department: string | null;
  asset_id: number | null;
  payload: Record<string, unknown>;
  first_seen_at: string;
  last_seen_at: string;
  acknowledged_by: string | null;
  acknowledged_at: string | null;
  closed_by: string | null;
  closed_at: string | null;
  created_at: string;
  updated_at: string;
};

type AlertTimelineRecord = {
  id: number;
  alert_id: number;
  event_type: string;
  actor: string;
  message: string | null;
  metadata: Record<string, unknown>;
  created_at: string;
};

type AlertLinkedTicketRecord = {
  id: number;
  ticket_no: string;
  title: string;
  status: string;
  priority: string;
  created_at: string;
};

type AlertListResponse = {
  items: AlertRecord[];
  total: number;
  limit: number;
  offset: number;
};

type AlertDetailResponse = {
  alert: AlertRecord;
  timeline: AlertTimelineRecord[];
  linked_tickets: AlertLinkedTicketRecord[];
  governance: {
    dedup_event_count: number;
    suppressed_count: number;
    latest_suppression_reason: string | null;
    latest_unsuppressed_event_at: string | null;
    latest_unsuppressed_policy_key: string | null;
  };
};

type AlertRemediationPlanResponse = {
  alert_id: number;
  playbook_key: string;
  playbook_name: string;
  risk_level: string;
  requires_confirmation: boolean;
  summary: string;
  params: Record<string, unknown>;
  execution_steps: string[];
  confirmation_flow: string;
  rollback_guidance: string[];
};

type AlertBulkActionResponse = {
  action: string;
  requested: number;
  updated: number;
  skipped: number;
  updated_ids: number[];
  skipped_ids: number[];
};

type AlertTicketPolicyRecord = {
  id: number;
  policy_key: string;
  name: string;
  description: string | null;
  is_system: boolean;
  is_enabled: boolean;
  match_source: string | null;
  match_severity: string | null;
  match_site: string | null;
  match_department: string | null;
  match_status: string | null;
  dedup_window_seconds: number;
  ticket_priority: string;
  ticket_category: string;
  workflow_template_id: number | null;
  created_by: string;
  created_at: string;
  updated_at: string;
};

type AlertTicketPolicyListResponse = {
  items: AlertTicketPolicyRecord[];
  total: number;
};

type AlertPolicyPreviewSample = {
  alert_id: number;
  title: string;
  severity: string;
  alert_source: string;
  status: string;
  last_seen_at: string;
};

type AlertPolicyPreviewResponse = {
  generated_at: string;
  dedup_window_seconds: number;
  matched_alert_count: number;
  potentially_suppressed_count: number;
  sample_alerts: AlertPolicyPreviewSample[];
  summary: string;
};

type AlertPolicyForm = {
  policy_key: string;
  name: string;
  description: string;
  is_enabled: boolean;
  match_source: string;
  match_severity: "all" | AlertSeverity;
  match_status: "all" | AlertStatus;
  dedup_window_seconds: string;
  ticket_priority: "low" | "medium" | "high" | "critical";
  ticket_category: string;
};

type NewFieldForm = {
  field_key: string;
  name: string;
  field_type: string;
  max_length: string;
  options_csv: string;
  required: boolean;
  scanner_enabled: boolean;
};

type NewRelationForm = {
  dst_asset_id: string;
  relation_type: string;
  source: string;
};

type NewNotificationChannelForm = {
  name: string;
  channel_type: "email" | "webhook";
  target: string;
  config_json: string;
};

type NewNotificationTemplateForm = {
  event_type: string;
  title_template: string;
  body_template: string;
};

type NewNotificationSubscriptionForm = {
  channel_id: string;
  event_type: string;
  site: string;
  department: string;
};

type NewMonitoringSourceForm = {
  name: string;
  source_type: "zabbix";
  endpoint: string;
  proxy_endpoint: string;
  auth_type: "token" | "basic";
  username: string;
  secret_ref: string;
  site: string;
  department: string;
  is_enabled: boolean;
};

type MonitoringSourceFilterForm = {
  source_type: string;
  site: string;
  department: string;
  is_enabled: "all" | "true" | "false";
};

type AlertFilterForm = {
  status: "all" | AlertStatus;
  severity: "all" | AlertSeverity;
  suppressed: "all" | "true" | "false";
  site: string;
  query: string;
};

type NewWorkflowTemplateStepForm = {
  id: string;
  name: string;
  kind: WorkflowStepKind;
  auto_run: boolean;
  script: string;
  timeout_seconds: string;
  approver_group: string;
};

type NewWorkflowRequestForm = {
  template_id: string;
  title: string;
  payload_json: string;
};

type NewTicketForm = {
  title: string;
  description: string;
  priority: "low" | "medium" | "high" | "critical";
  category: string;
  assignee: string;
  asset_ids_csv: string;
  alert_source: string;
  alert_key: string;
  alert_title: string;
  alert_severity: string;
  workflow_template_id: string;
  trigger_workflow: boolean;
};

type WorkflowDailyTrendPoint = {
  day_key: string;
  day_label: string;
  total: number;
  completed: number;
  failed: number;
  active: number;
};

type WorkflowTrendRankRow = {
  key: string;
  label: string;
  week_current: number;
  week_previous: number;
  week_delta: number;
  month_current: number;
  month_previous: number;
  month_delta: number;
};

type AssetSortMode = "updated_desc" | "name_asc" | "id_asc";
type LifecycleStatus = "idle" | "onboarding" | "operational" | "maintenance" | "retired";
type ImpactDirection = "downstream" | "upstream" | "both";
type OwnerType = "team" | "user" | "group" | "external";

type OwnerDraft = {
  key: string;
  owner_type: OwnerType;
  owner_ref: string;
};

const SUPPORTED_UI_LANGUAGES = ["en-US", "zh-CN"] as const;

type UiLanguage = (typeof SUPPORTED_UI_LANGUAGES)[number];

type AuthIdentity = {
  user: {
    id: number;
    username: string;
    display_name: string | null;
    email: string | null;
  };
  roles: string[];
};

const defaultFieldForm: NewFieldForm = {
  field_key: "",
  name: "",
  field_type: "text",
  max_length: "",
  options_csv: "",
  required: false,
  scanner_enabled: false
};

const defaultRelationForm: NewRelationForm = {
  dst_asset_id: "",
  relation_type: "depends_on",
  source: "manual"
};

const defaultNotificationChannelForm: NewNotificationChannelForm = {
  name: "",
  channel_type: "webhook",
  target: "",
  config_json: "{}"
};

const defaultNotificationTemplateForm: NewNotificationTemplateForm = {
  event_type: "asset.new_detected",
  title_template: "Discovery Event: {{event_type}}",
  body_template: "{{payload}}"
};

const defaultNotificationSubscriptionForm: NewNotificationSubscriptionForm = {
  channel_id: "",
  event_type: "asset.new_detected",
  site: "",
  department: ""
};

const defaultMonitoringSourceForm: NewMonitoringSourceForm = {
  name: "",
  source_type: "zabbix",
  endpoint: "",
  proxy_endpoint: "",
  auth_type: "token",
  username: "",
  secret_ref: "",
  site: "",
  department: "",
  is_enabled: true
};

const defaultMonitoringSourceFilters: MonitoringSourceFilterForm = {
  source_type: "",
  site: "",
  department: "",
  is_enabled: "all"
};

const defaultAlertFilters: AlertFilterForm = {
  status: "open",
  severity: "all",
  suppressed: "all",
  site: "",
  query: ""
};

const defaultAlertPolicyForm: AlertPolicyForm = {
  policy_key: "",
  name: "",
  description: "",
  is_enabled: true,
  match_source: "monitoring_sync",
  match_severity: "warning",
  match_status: "open",
  dedup_window_seconds: "1800",
  ticket_priority: "high",
  ticket_category: "incident"
};

const defaultBackupPolicyForm: BackupPolicyForm = {
  policy_id: "",
  policy_key: "",
  name: "",
  frequency: "daily",
  schedule_time_utc: "01:30",
  schedule_weekday: "1",
  retention_days: "14",
  destination_type: "local",
  destination_uri: "file:///var/lib/cloudops/backups",
  drill_enabled: true,
  drill_frequency: "weekly",
  drill_weekday: "3",
  drill_time_utc: "02:30",
  note: ""
};

const defaultBackupRestoreEvidenceForm: BackupRestoreEvidenceForm = {
  run_id: "",
  ticket_ref: "",
  artifact_url: "",
  note: "",
  verifier: "",
  close_evidence: true
};

const defaultBackupEvidenceCompliancePolicyDraft: BackupEvidenceCompliancePolicyDraft = {
  mode: "advisory",
  sla_hours: "24",
  require_failed_runs: true,
  require_drill_runs: true,
  note: ""
};

const defaultChangeCalendarConflictDraft: ChangeCalendarConflictDraft = {
  start_at_local: formatLocalDateTimeInput(new Date()),
  end_at_local: formatLocalDateTimeInput(new Date(Date.now() + 30 * 60 * 1000)),
  operation_kind: "playbook.execute.restart-service-safe",
  risk_level: "high"
};

const defaultChangeCalendarReservationDraft: ChangeCalendarReservationDraft = {
  start_at_local: formatLocalDateTimeInput(new Date()),
  end_at_local: formatLocalDateTimeInput(new Date(Date.now() + 60 * 60 * 1000)),
  operation_kind: "playbook.execute.restart-service-safe",
  risk_level: "high",
  owner: "ops-oncall",
  site: "",
  department: "",
  note: ""
};

const defaultIncidentCommandDraft: IncidentCommandDraft = {
  alert_id: "",
  status: "triage",
  owner: "",
  eta_at: "",
  blocker: "",
  summary: "",
  note: ""
};

const defaultRunbookEvidenceDraft: RunbookEvidenceDraft = {
  summary: "",
  ticket_ref: "",
  artifact_url: "",
  note: ""
};

const defaultRunbookExecutionPolicyDraft: RunbookExecutionPolicyDraft = {
  mode: "simulate_only",
  live_templates_csv: "",
  max_live_step_timeout_seconds: "10",
  allow_simulate_failure: true,
  note: ""
};

const defaultWorkflowStepForm: NewWorkflowTemplateStepForm = {
  id: "",
  name: "",
  kind: "script",
  auto_run: true,
  script: "echo 'workflow step executed'",
  timeout_seconds: "300",
  approver_group: ""
};

const defaultWorkflowRequestForm: NewWorkflowRequestForm = {
  template_id: "",
  title: "",
  payload_json: "{}"
};

const defaultTicketForm: NewTicketForm = {
  title: "",
  description: "",
  priority: "medium",
  category: "incident",
  assignee: "",
  asset_ids_csv: "",
  alert_source: "zabbix",
  alert_key: "",
  alert_title: "",
  alert_severity: "warning",
  workflow_template_id: "",
  trigger_workflow: false
};

const defaultTicketEscalationPolicyDraft: TicketEscalationPolicyDraft = {
  name: "Default Ticket SLA Policy",
  is_enabled: true,
  near_critical_minutes: "30",
  breach_critical_minutes: "60",
  near_high_minutes: "60",
  breach_high_minutes: "120",
  near_medium_minutes: "120",
  breach_medium_minutes: "240",
  near_low_minutes: "240",
  breach_low_minutes: "480",
  escalate_to_assignee: "ops-escalation",
  note: ""
};

const defaultTicketEscalationPreviewDraft: TicketEscalationPreviewDraft = {
  priority: "high",
  status: "open",
  ticket_age_minutes: "90",
  current_assignee: "ops-oncall"
};

function buildTicketEscalationPolicyDraft(policy: TicketEscalationPolicyRecord): TicketEscalationPolicyDraft {
  return {
    name: policy.name,
    is_enabled: policy.is_enabled,
    near_critical_minutes: String(policy.near_critical_minutes),
    breach_critical_minutes: String(policy.breach_critical_minutes),
    near_high_minutes: String(policy.near_high_minutes),
    breach_high_minutes: String(policy.breach_high_minutes),
    near_medium_minutes: String(policy.near_medium_minutes),
    breach_medium_minutes: String(policy.breach_medium_minutes),
    near_low_minutes: String(policy.near_low_minutes),
    breach_low_minutes: String(policy.breach_low_minutes),
    escalate_to_assignee: policy.escalate_to_assignee,
    note: ""
  };
}

const lifecycleStatuses: LifecycleStatus[] = [
  "idle",
  "onboarding",
  "operational",
  "maintenance",
  "retired"
];

const defaultImpactRelationTypes = ["contains", "depends_on", "runs_service", "owned_by"];

function buildRunbookParamDraft(template: RunbookTemplateCatalogItem | null): Record<string, string> {
  if (!template) {
    return {};
  }
  const draft: Record<string, string> = {};
  for (const field of template.params ?? []) {
    draft[field.key] = field.default_value ?? "";
  }
  return draft;
}

function buildRunbookPreflightDraft(template: RunbookTemplateCatalogItem | null): Record<string, boolean> {
  if (!template) {
    return {};
  }
  const draft: Record<string, boolean> = {};
  for (const item of template.preflight ?? []) {
    draft[item.key] = false;
  }
  return draft;
}

function buildRunbookExecutionPolicyDraft(policy: RunbookExecutionPolicyItem): RunbookExecutionPolicyDraft {
  return {
    mode: policy.mode,
    live_templates_csv: (policy.live_templates ?? []).join(","),
    max_live_step_timeout_seconds: String(policy.max_live_step_timeout_seconds),
    allow_simulate_failure: policy.allow_simulate_failure,
    note: policy.note ?? ""
  };
}

export function App() {
  const { t, i18n } = useTranslation();
  const currentLanguage = normalizeUiLanguage(i18n.resolvedLanguage ?? i18n.language);
  const initialRuntimeAuthSession = getRuntimeAuthSession();
  const [authSession, setAuthSession] = useState<AuthSession | null>(initialRuntimeAuthSession);
  const [authIdentity, setAuthIdentity] = useState<AuthIdentity | null>(null);
  const [authLoading, setAuthLoading] = useState(false);
  const [authError, setAuthError] = useState<string | null>(null);
  const [authNotice, setAuthNotice] = useState<string | null>(null);
  const [loginMode, setLoginMode] = useState<AuthMode>(initialRuntimeAuthSession?.mode ?? "header");
  const [loginPrincipal, setLoginPrincipal] = useState<string>(
    initialRuntimeAuthSession?.mode === "header" ? initialRuntimeAuthSession.principal : DEFAULT_AUTH_USER
  );
  const [loginToken, setLoginToken] = useState<string>(
    initialRuntimeAuthSession?.mode === "bearer" ? initialRuntimeAuthSession.token ?? "" : DEFAULT_AUTH_TOKEN
  );
  const [assets, setAssets] = useState<Asset[]>([]);
  const [assetStats, setAssetStats] = useState<AssetStatsResponse | null>(null);
  const [fieldDefinitions, setFieldDefinitions] = useState<FieldDefinition[]>([]);
  const [loadingAssets, setLoadingAssets] = useState(false);
  const [loadingAssetStats, setLoadingAssetStats] = useState(false);
  const [loadingFields, setLoadingFields] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [creatingSample, setCreatingSample] = useState(false);
  const [creatingField, setCreatingField] = useState(false);
  const [newField, setNewField] = useState<NewFieldForm>(defaultFieldForm);
  const [scanCode, setScanCode] = useState("");
  const [scanMode, setScanMode] = useState<"auto" | "qr" | "barcode">("auto");
  const [scanning, setScanning] = useState(false);
  const [scanResult, setScanResult] = useState<Asset | null>(null);
  const [relations, setRelations] = useState<AssetRelation[]>([]);
  const [loadingRelations, setLoadingRelations] = useState(false);
  const [selectedAssetId, setSelectedAssetId] = useState<string>("");
  const [assetSearch, setAssetSearch] = useState("");
  const [assetStatusFilter, setAssetStatusFilter] = useState("");
  const [assetClassFilter, setAssetClassFilter] = useState("");
  const [assetSiteFilter, setAssetSiteFilter] = useState("");
  const [assetSortMode, setAssetSortMode] = useState<AssetSortMode>("updated_desc");
  const [relationNotice, setRelationNotice] = useState<string | null>(null);
  const [creatingRelation, setCreatingRelation] = useState(false);
  const [deletingRelationId, setDeletingRelationId] = useState<number | null>(null);
  const [newRelation, setNewRelation] = useState<NewRelationForm>(defaultRelationForm);
  const [assetBindings, setAssetBindings] = useState<AssetBindingsResponse | null>(null);
  const [loadingAssetBindings, setLoadingAssetBindings] = useState(false);
  const [updatingAssetBindings, setUpdatingAssetBindings] = useState(false);
  const [bindingNotice, setBindingNotice] = useState<string | null>(null);
  const [bindingDepartmentsInput, setBindingDepartmentsInput] = useState("");
  const [bindingBusinessServicesInput, setBindingBusinessServicesInput] = useState("");
  const [bindingOwnerDrafts, setBindingOwnerDrafts] = useState<OwnerDraft[]>([]);
  const [transitioningLifecycleStatus, setTransitioningLifecycleStatus] = useState<LifecycleStatus | null>(null);
  const [lifecycleNotice, setLifecycleNotice] = useState<string | null>(null);
  const [assetMonitoring, setAssetMonitoring] = useState<AssetMonitoringBindingResponse | null>(null);
  const [loadingAssetMonitoring, setLoadingAssetMonitoring] = useState(false);
  const [triggeringMonitoringSync, setTriggeringMonitoringSync] = useState(false);
  const [monitoringNotice, setMonitoringNotice] = useState<string | null>(null);
  const [impactDirection, setImpactDirection] = useState<ImpactDirection>("downstream");
  const [impactDepth, setImpactDepth] = useState("4");
  const [impactRelationTypesInput, setImpactRelationTypesInput] = useState(defaultImpactRelationTypes.join(","));
  const [assetImpact, setAssetImpact] = useState<AssetImpactResponse | null>(null);
  const [loadingAssetImpact, setLoadingAssetImpact] = useState(false);
  const [impactNotice, setImpactNotice] = useState<string | null>(null);
  const [selectedTopologyEdgeKey, setSelectedTopologyEdgeKey] = useState<string | null>(null);
  const [topologyMap, setTopologyMap] = useState<TopologyMapResponse | null>(null);
  const [loadingTopologyMap, setLoadingTopologyMap] = useState(false);
  const [topologyMapNotice, setTopologyMapNotice] = useState<string | null>(null);
  const [topologyScopeInput, setTopologyScopeInput] = useState("global");
  const [topologySiteFilter, setTopologySiteFilter] = useState("");
  const [topologyDepartmentFilter, setTopologyDepartmentFilter] = useState("");
  const [topologyWindowLimit, setTopologyWindowLimit] = useState("200");
  const [topologyWindowOffset, setTopologyWindowOffset] = useState("0");
  const [selectedTopologyMapNodeId, setSelectedTopologyMapNodeId] = useState<string>("");
  const [selectedTopologyMapEdgeKey, setSelectedTopologyMapEdgeKey] = useState<string | null>(
    null
  );
  const [topologyDiagnostics, setTopologyDiagnostics] = useState<TopologyDiagnosticsResponse | null>(null);
  const [loadingTopologyDiagnostics, setLoadingTopologyDiagnostics] = useState(false);
  const [topologyDiagnosticsNotice, setTopologyDiagnosticsNotice] = useState<string | null>(null);
  const [topologyDiagnosticsWindowMinutes, setTopologyDiagnosticsWindowMinutes] = useState("120");
  const [runningTopologyDiagnosticsActionKey, setRunningTopologyDiagnosticsActionKey] = useState<string | null>(null);
  const [discoveryJobs, setDiscoveryJobs] = useState<DiscoveryJob[]>([]);
  const [discoveryCandidates, setDiscoveryCandidates] = useState<DiscoveryCandidate[]>([]);
  const [loadingDiscoveryJobs, setLoadingDiscoveryJobs] = useState(false);
  const [loadingDiscoveryCandidates, setLoadingDiscoveryCandidates] = useState(false);
  const [runningDiscoveryJobId, setRunningDiscoveryJobId] = useState<number | null>(null);
  const [reviewingCandidateId, setReviewingCandidateId] = useState<number | null>(null);
  const [discoveryNotice, setDiscoveryNotice] = useState<string | null>(null);
  const [notificationChannels, setNotificationChannels] = useState<NotificationChannel[]>([]);
  const [notificationTemplates, setNotificationTemplates] = useState<NotificationTemplate[]>([]);
  const [notificationSubscriptions, setNotificationSubscriptions] = useState<NotificationSubscription[]>([]);
  const [loadingNotificationChannels, setLoadingNotificationChannels] = useState(false);
  const [loadingNotificationTemplates, setLoadingNotificationTemplates] = useState(false);
  const [loadingNotificationSubscriptions, setLoadingNotificationSubscriptions] = useState(false);
  const [creatingNotificationChannel, setCreatingNotificationChannel] = useState(false);
  const [creatingNotificationTemplate, setCreatingNotificationTemplate] = useState(false);
  const [creatingNotificationSubscription, setCreatingNotificationSubscription] = useState(false);
  const [notificationNotice, setNotificationNotice] = useState<string | null>(null);
  const [workflowTemplates, setWorkflowTemplates] = useState<WorkflowTemplate[]>([]);
  const [workflowRequests, setWorkflowRequests] = useState<WorkflowRequest[]>([]);
  const [workflowLogs, setWorkflowLogs] = useState<WorkflowExecutionLog[]>([]);
  const [loadingWorkflowTemplates, setLoadingWorkflowTemplates] = useState(false);
  const [loadingWorkflowRequests, setLoadingWorkflowRequests] = useState(false);
  const [loadingWorkflowLogs, setLoadingWorkflowLogs] = useState(false);
  const [creatingWorkflowTemplate, setCreatingWorkflowTemplate] = useState(false);
  const [creatingWorkflowRequest, setCreatingWorkflowRequest] = useState(false);
  const [executingWorkflowRequestId, setExecutingWorkflowRequestId] = useState<number | null>(null);
  const [approvingWorkflowRequestId, setApprovingWorkflowRequestId] = useState<number | null>(null);
  const [rejectingWorkflowRequestId, setRejectingWorkflowRequestId] = useState<number | null>(null);
  const [manualCompletingWorkflowRequestId, setManualCompletingWorkflowRequestId] = useState<number | null>(null);
  const [selectedWorkflowRequestId, setSelectedWorkflowRequestId] = useState<string>("");
  const [workflowNotice, setWorkflowNotice] = useState<string | null>(null);
  const [workflowReportRangeDays, setWorkflowReportRangeDays] = useState("30");
  const [workflowReportStatusFilter, setWorkflowReportStatusFilter] = useState("all");
  const [workflowReportTemplateFilter, setWorkflowReportTemplateFilter] = useState("all");
  const [workflowReportRequesterFilter, setWorkflowReportRequesterFilter] = useState("");
  const [playbookCatalog, setPlaybookCatalog] = useState<PlaybookCatalogItem[]>([]);
  const [playbookExecutions, setPlaybookExecutions] = useState<PlaybookExecutionListItem[]>([]);
  const [playbookExecutionPolicy, setPlaybookExecutionPolicy] = useState<PlaybookExecutionPolicyResponse | null>(null);
  const [playbookApprovalRequests, setPlaybookApprovalRequests] = useState<PlaybookApprovalRequestRecord[]>([]);
  const [loadingPlaybookCatalog, setLoadingPlaybookCatalog] = useState(false);
  const [loadingPlaybookExecutions, setLoadingPlaybookExecutions] = useState(false);
  const [loadingPlaybookPolicy, setLoadingPlaybookPolicy] = useState(false);
  const [loadingPlaybookApprovals, setLoadingPlaybookApprovals] = useState(false);
  const [runningPlaybookDryRun, setRunningPlaybookDryRun] = useState(false);
  const [runningPlaybookExecute, setRunningPlaybookExecute] = useState(false);
  const [requestingPlaybookApproval, setRequestingPlaybookApproval] = useState(false);
  const [approvingPlaybookApprovalId, setApprovingPlaybookApprovalId] = useState<number | null>(null);
  const [rejectingPlaybookApprovalId, setRejectingPlaybookApprovalId] = useState<number | null>(null);
  const [selectedPlaybookKey, setSelectedPlaybookKey] = useState("");
  const [playbookCategoryFilter, setPlaybookCategoryFilter] = useState("all");
  const [playbookQuery, setPlaybookQuery] = useState("");
  const [playbookAssetRef, setPlaybookAssetRef] = useState("");
  const [playbookReservationId, setPlaybookReservationId] = useState("");
  const [playbookParamsDraft, setPlaybookParamsDraft] = useState<Record<string, string>>({});
  const [playbookConfirmationToken, setPlaybookConfirmationToken] = useState("");
  const [selectedPlaybookApprovalId, setSelectedPlaybookApprovalId] = useState("");
  const [playbookApprovalToken, setPlaybookApprovalToken] = useState("");
  const [playbookApprovalRequestNote, setPlaybookApprovalRequestNote] = useState("");
  const [playbookApprovalDecisionNote, setPlaybookApprovalDecisionNote] = useState("");
  const [playbookMaintenanceOverrideReason, setPlaybookMaintenanceOverrideReason] = useState("");
  const [playbookMaintenanceOverrideConfirmed, setPlaybookMaintenanceOverrideConfirmed] = useState(false);
  const [playbookDryRunResponse, setPlaybookDryRunResponse] = useState<PlaybookDryRunResponse | null>(null);
  const [playbookExecutionResult, setPlaybookExecutionResult] = useState<PlaybookExecutionDetail | null>(null);
  const [playbookNotice, setPlaybookNotice] = useState<string | null>(null);
  const [dailyCockpitQueue, setDailyCockpitQueue] = useState<DailyCockpitQueueResponse | null>(null);
  const [nextBestActions, setNextBestActions] = useState<NextBestActionResponse | null>(null);
  const [opsChecklist, setOpsChecklist] = useState<OpsChecklistResponse | null>(null);
  const [incidentCommands, setIncidentCommands] = useState<IncidentCommandRecord[]>([]);
  const [incidentCommandDraft, setIncidentCommandDraft] = useState<IncidentCommandDraft>(defaultIncidentCommandDraft);
  const [incidentCommandDetail, setIncidentCommandDetail] = useState<IncidentCommandDetailResponse | null>(null);
  const [selectedIncidentAlertId, setSelectedIncidentAlertId] = useState("");
  const [runbookTemplates, setRunbookTemplates] = useState<RunbookTemplateCatalogItem[]>([]);
  const [runbookExecutions, setRunbookExecutions] = useState<RunbookTemplateExecutionItem[]>([]);
  const [runbookExecutionPolicy, setRunbookExecutionPolicy] = useState<RunbookExecutionPolicyItem | null>(null);
  const [runbookExecutionPolicyDraft, setRunbookExecutionPolicyDraft] =
    useState<RunbookExecutionPolicyDraft>(defaultRunbookExecutionPolicyDraft);
  const [selectedRunbookTemplateKey, setSelectedRunbookTemplateKey] = useState("");
  const [runbookExecutionMode, setRunbookExecutionMode] = useState<"simulate" | "live">("simulate");
  const [runbookParamDraft, setRunbookParamDraft] = useState<Record<string, string>>({});
  const [runbookPreflightDraft, setRunbookPreflightDraft] = useState<Record<string, boolean>>({});
  const [runbookEvidenceDraft, setRunbookEvidenceDraft] = useState<RunbookEvidenceDraft>(defaultRunbookEvidenceDraft);
  const [backupPolicies, setBackupPolicies] = useState<BackupPolicyRecord[]>([]);
  const [backupPolicyRuns, setBackupPolicyRuns] = useState<BackupPolicyRunRecord[]>([]);
  const [backupPolicyDraft, setBackupPolicyDraft] = useState<BackupPolicyForm>(defaultBackupPolicyForm);
  const [backupRestoreEvidence, setBackupRestoreEvidence] = useState<BackupRestoreEvidenceRecord[]>([]);
  const [backupRestoreEvidenceCoverage, setBackupRestoreEvidenceCoverage] = useState({
    required_runs: 0,
    covered_runs: 0,
    missing_runs: 0
  });
  const [backupRestoreEvidenceMissingRunIds, setBackupRestoreEvidenceMissingRunIds] = useState<number[]>([]);
  const [backupRestoreEvidenceDraft, setBackupRestoreEvidenceDraft] =
    useState<BackupRestoreEvidenceForm>(defaultBackupRestoreEvidenceForm);
  const [backupRestoreRunStatusFilter, setBackupRestoreRunStatusFilter] = useState("all");
  const [backupEvidenceCompliancePolicy, setBackupEvidenceCompliancePolicy] =
    useState<BackupEvidenceCompliancePolicyResponse | null>(null);
  const [backupEvidenceCompliancePolicyDraft, setBackupEvidenceCompliancePolicyDraft] =
    useState<BackupEvidenceCompliancePolicyDraft>(defaultBackupEvidenceCompliancePolicyDraft);
  const [backupEvidenceComplianceScorecard, setBackupEvidenceComplianceScorecard] =
    useState<BackupEvidenceComplianceScorecardResponse | null>(null);
  const [backupEvidenceComplianceWeekStart, setBackupEvidenceComplianceWeekStart] = useState(() => {
    const now = new Date();
    const weekday = now.getDay();
    const offset = weekday === 0 ? -6 : 1 - weekday;
    now.setDate(now.getDate() + offset);
    return formatLocalDateKey(now);
  });
  const [changeCalendar, setChangeCalendar] = useState<ChangeCalendarResponse | null>(null);
  const [changeCalendarStartDate, setChangeCalendarStartDate] = useState(() => formatLocalDateKey(new Date()));
  const [changeCalendarEndDate, setChangeCalendarEndDate] = useState(() => {
    const date = new Date();
    date.setDate(date.getDate() + 13);
    return formatLocalDateKey(date);
  });
  const [changeCalendarConflictDraft, setChangeCalendarConflictDraft] =
    useState<ChangeCalendarConflictDraft>(defaultChangeCalendarConflictDraft);
  const [changeCalendarConflictResult, setChangeCalendarConflictResult] =
    useState<ChangeCalendarConflictResponse | null>(null);
  const [changeCalendarReservationDraft, setChangeCalendarReservationDraft] =
    useState<ChangeCalendarReservationDraft>(defaultChangeCalendarReservationDraft);
  const [changeCalendarReservations, setChangeCalendarReservations] = useState<ChangeCalendarReservationRecord[]>([]);
  const [changeCalendarSlotRecommendations, setChangeCalendarSlotRecommendations] =
    useState<ChangeCalendarSlotRecommendationResponse | null>(null);
  const [loadingDailyCockpit, setLoadingDailyCockpit] = useState(false);
  const [loadingNextBestActions, setLoadingNextBestActions] = useState(false);
  const [loadingOpsChecklist, setLoadingOpsChecklist] = useState(false);
  const [loadingIncidentCommands, setLoadingIncidentCommands] = useState(false);
  const [loadingIncidentCommandDetail, setLoadingIncidentCommandDetail] = useState(false);
  const [loadingRunbookTemplates, setLoadingRunbookTemplates] = useState(false);
  const [loadingRunbookExecutions, setLoadingRunbookExecutions] = useState(false);
  const [loadingRunbookExecutionPolicy, setLoadingRunbookExecutionPolicy] = useState(false);
  const [executingRunbookTemplate, setExecutingRunbookTemplate] = useState(false);
  const [savingRunbookExecutionPolicy, setSavingRunbookExecutionPolicy] = useState(false);
  const [savingIncidentCommand, setSavingIncidentCommand] = useState(false);
  const [loadingBackupPolicies, setLoadingBackupPolicies] = useState(false);
  const [loadingBackupPolicyRuns, setLoadingBackupPolicyRuns] = useState(false);
  const [loadingBackupRestoreEvidence, setLoadingBackupRestoreEvidence] = useState(false);
  const [loadingBackupEvidenceCompliancePolicy, setLoadingBackupEvidenceCompliancePolicy] = useState(false);
  const [savingBackupEvidenceCompliancePolicy, setSavingBackupEvidenceCompliancePolicy] = useState(false);
  const [loadingBackupEvidenceComplianceScorecard, setLoadingBackupEvidenceComplianceScorecard] = useState(false);
  const [exportingBackupEvidenceComplianceScorecard, setExportingBackupEvidenceComplianceScorecard] = useState(false);
  const [loadingChangeCalendar, setLoadingChangeCalendar] = useState(false);
  const [checkingChangeCalendarConflict, setCheckingChangeCalendarConflict] = useState(false);
  const [loadingChangeCalendarReservations, setLoadingChangeCalendarReservations] = useState(false);
  const [loadingChangeCalendarRecommendations, setLoadingChangeCalendarRecommendations] = useState(false);
  const [creatingChangeCalendarReservation, setCreatingChangeCalendarReservation] = useState(false);
  const [savingBackupPolicy, setSavingBackupPolicy] = useState(false);
  const [savingBackupRestoreEvidence, setSavingBackupRestoreEvidence] = useState(false);
  const [runningBackupPolicyActionId, setRunningBackupPolicyActionId] = useState<string | null>(null);
  const [tickingBackupScheduler, setTickingBackupScheduler] = useState(false);
  const [weeklyDigest, setWeeklyDigest] = useState<WeeklyDigestResponse | null>(null);
  const [loadingWeeklyDigest, setLoadingWeeklyDigest] = useState(false);
  const [exportingWeeklyDigest, setExportingWeeklyDigest] = useState(false);
  const [weeklyDigestWeekStart, setWeeklyDigestWeekStart] = useState(() => {
    const now = new Date();
    const weekday = now.getDay();
    const offset = weekday === 0 ? -6 : 1 - weekday;
    now.setDate(now.getDate() + offset);
    return formatLocalDateKey(now);
  });
  const [handoverDigest, setHandoverDigest] = useState<HandoverDigestResponse | null>(null);
  const [handoverReminders, setHandoverReminders] = useState<HandoverReminderResponse | null>(null);
  const [loadingHandoverDigest, setLoadingHandoverDigest] = useState(false);
  const [loadingHandoverReminders, setLoadingHandoverReminders] = useState(false);
  const [exportingHandoverDigest, setExportingHandoverDigest] = useState(false);
  const [exportingHandoverReminders, setExportingHandoverReminders] = useState(false);
  const [closingHandoverItemKey, setClosingHandoverItemKey] = useState<string | null>(null);
  const [handoverDigestShiftDate, setHandoverDigestShiftDate] = useState(() => formatLocalDateKey(new Date()));
  const [runningDailyCockpitActionKey, setRunningDailyCockpitActionKey] = useState<string | null>(null);
  const [runningOpsChecklistActionKey, setRunningOpsChecklistActionKey] = useState<string | null>(null);
  const [dailyCockpitNotice, setDailyCockpitNotice] = useState<string | null>(null);
  const [opsChecklistNotice, setOpsChecklistNotice] = useState<string | null>(null);
  const [incidentCommandNotice, setIncidentCommandNotice] = useState<string | null>(null);
  const [runbookNotice, setRunbookNotice] = useState<string | null>(null);
  const [backupPolicyNotice, setBackupPolicyNotice] = useState<string | null>(null);
  const [changeCalendarNotice, setChangeCalendarNotice] = useState<string | null>(null);
  const [weeklyDigestNotice, setWeeklyDigestNotice] = useState<string | null>(null);
  const [handoverDigestNotice, setHandoverDigestNotice] = useState<string | null>(null);
  const [dailyCockpitSiteFilter, setDailyCockpitSiteFilter] = useState("");
  const [dailyCockpitDepartmentFilter, setDailyCockpitDepartmentFilter] = useState("");
  const [opsChecklistDate, setOpsChecklistDate] = useState(() => formatLocalDateKey(new Date()));
  const [tickets, setTickets] = useState<TicketListItem[]>([]);
  const [ticketDetail, setTicketDetail] = useState<TicketDetailResponse | null>(null);
  const [loadingTickets, setLoadingTickets] = useState(false);
  const [loadingTicketDetail, setLoadingTicketDetail] = useState(false);
  const [creatingTicket, setCreatingTicket] = useState(false);
  const [updatingTicketStatusId, setUpdatingTicketStatusId] = useState<number | null>(null);
  const [ticketNotice, setTicketNotice] = useState<string | null>(null);
  const [selectedTicketId, setSelectedTicketId] = useState<string>("");
  const [ticketStatusFilter, setTicketStatusFilter] = useState("all");
  const [ticketPriorityFilter, setTicketPriorityFilter] = useState("all");
  const [ticketQueryFilter, setTicketQueryFilter] = useState("");
  const [ticketStatusDraft, setTicketStatusDraft] = useState("open");
  const [newTicket, setNewTicket] = useState<NewTicketForm>(defaultTicketForm);
  const [ticketEscalationPolicy, setTicketEscalationPolicy] = useState<TicketEscalationPolicyRecord | null>(null);
  const [ticketEscalationPolicyDraft, setTicketEscalationPolicyDraft] =
    useState<TicketEscalationPolicyDraft>(defaultTicketEscalationPolicyDraft);
  const [ticketEscalationPreviewDraft, setTicketEscalationPreviewDraft] =
    useState<TicketEscalationPreviewDraft>(defaultTicketEscalationPreviewDraft);
  const [ticketEscalationPreview, setTicketEscalationPreview] = useState<TicketEscalationPreviewResponse | null>(null);
  const [ticketEscalationQueue, setTicketEscalationQueue] = useState<TicketListItem[]>([]);
  const [ticketEscalationActions, setTicketEscalationActions] = useState<TicketEscalationActionRecord[]>([]);
  const [ticketEscalationRunResponse, setTicketEscalationRunResponse] = useState<TicketEscalationRunResponse | null>(null);
  const [ticketEscalationRunNote, setTicketEscalationRunNote] = useState("");
  const [loadingTicketEscalationPolicy, setLoadingTicketEscalationPolicy] = useState(false);
  const [savingTicketEscalationPolicy, setSavingTicketEscalationPolicy] = useState(false);
  const [previewingTicketEscalationPolicy, setPreviewingTicketEscalationPolicy] = useState(false);
  const [loadingTicketEscalationQueue, setLoadingTicketEscalationQueue] = useState(false);
  const [loadingTicketEscalationActions, setLoadingTicketEscalationActions] = useState(false);
  const [runningTicketEscalation, setRunningTicketEscalation] = useState(false);
  const [monitoringSources, setMonitoringSources] = useState<MonitoringSource[]>([]);
  const [loadingMonitoringSources, setLoadingMonitoringSources] = useState(false);
  const [creatingMonitoringSource, setCreatingMonitoringSource] = useState(false);
  const [probingMonitoringSourceId, setProbingMonitoringSourceId] = useState<number | null>(null);
  const [monitoringSourceNotice, setMonitoringSourceNotice] = useState<string | null>(null);
  const [monitoringMetrics, setMonitoringMetrics] = useState<MonitoringMetricsResponse | null>(null);
  const [loadingMonitoringMetrics, setLoadingMonitoringMetrics] = useState(false);
  const [monitoringMetricsWindowMinutes, setMonitoringMetricsWindowMinutes] = useState("60");
  const [monitoringMetricsError, setMonitoringMetricsError] = useState<string | null>(null);
  const [monitoringOverview, setMonitoringOverview] = useState<MonitoringOverviewResponse | null>(null);
  const [loadingMonitoringOverview, setLoadingMonitoringOverview] = useState(false);
  const [setupPreflight, setSetupPreflight] = useState<SetupChecklistResponse | null>(null);
  const [setupChecklist, setSetupChecklist] = useState<SetupChecklistResponse | null>(null);
  const [loadingSetupPreflight, setLoadingSetupPreflight] = useState(false);
  const [loadingSetupChecklist, setLoadingSetupChecklist] = useState(false);
  const [setupTemplates, setSetupTemplates] = useState<SetupTemplateCatalogItem[]>([]);
  const [loadingSetupTemplates, setLoadingSetupTemplates] = useState(false);
  const [selectedSetupTemplateKey, setSelectedSetupTemplateKey] = useState("");
  const [setupTemplateParamsDraft, setSetupTemplateParamsDraft] = useState<Record<string, string>>({});
  const [setupTemplateNote, setSetupTemplateNote] = useState("");
  const [setupTemplatePreview, setSetupTemplatePreview] = useState<SetupTemplatePreviewResponse | null>(null);
  const [setupTemplateApplyResult, setSetupTemplateApplyResult] = useState<SetupTemplateApplyResponse | null>(null);
  const [runningSetupTemplatePreview, setRunningSetupTemplatePreview] = useState(false);
  const [runningSetupTemplateApply, setRunningSetupTemplateApply] = useState(false);
  const [setupTemplateNotice, setSetupTemplateNotice] = useState<string | null>(null);
  const [setupProfiles, setSetupProfiles] = useState<SetupProfileCatalogItem[]>([]);
  const [loadingSetupProfiles, setLoadingSetupProfiles] = useState(false);
  const [selectedSetupProfileKey, setSelectedSetupProfileKey] = useState("");
  const [setupProfileNote, setSetupProfileNote] = useState("");
  const [setupProfilePreview, setSetupProfilePreview] = useState<SetupProfilePreviewResponse | null>(null);
  const [setupProfileApplyResult, setSetupProfileApplyResult] = useState<SetupProfileApplyResponse | null>(null);
  const [runningSetupProfilePreview, setRunningSetupProfilePreview] = useState(false);
  const [runningSetupProfileApply, setRunningSetupProfileApply] = useState(false);
  const [setupProfileHistory, setSetupProfileHistory] = useState<SetupProfileHistoryRecord[]>([]);
  const [loadingSetupProfileHistory, setLoadingSetupProfileHistory] = useState(false);
  const [runningSetupProfileRevertId, setRunningSetupProfileRevertId] = useState<number | null>(null);
  const [setupProfileNotice, setSetupProfileNotice] = useState<string | null>(null);
  const [setupStep, setSetupStep] = useState(0);
  const [setupCompleted, setSetupCompleted] = useState(false);
  const [setupNotice, setSetupNotice] = useState<string | null>(null);
  const [alerts, setAlerts] = useState<AlertRecord[]>([]);
  const [alertsTotal, setAlertsTotal] = useState(0);
  const [loadingAlerts, setLoadingAlerts] = useState(false);
  const [selectedAlertIds, setSelectedAlertIds] = useState<number[]>([]);
  const [selectedAlertId, setSelectedAlertId] = useState("");
  const [alertDetail, setAlertDetail] = useState<AlertDetailResponse | null>(null);
  const [loadingAlertDetail, setLoadingAlertDetail] = useState(false);
  const [alertActionRunningId, setAlertActionRunningId] = useState<number | null>(null);
  const [alertBulkActionRunning, setAlertBulkActionRunning] = useState<"ack" | "close" | null>(null);
  const [alertNotice, setAlertNotice] = useState<string | null>(null);
  const [alertFilters, setAlertFilters] = useState<AlertFilterForm>(defaultAlertFilters);
  const [alertPolicies, setAlertPolicies] = useState<AlertTicketPolicyRecord[]>([]);
  const [loadingAlertPolicies, setLoadingAlertPolicies] = useState(false);
  const [creatingAlertPolicy, setCreatingAlertPolicy] = useState(false);
  const [updatingAlertPolicyId, setUpdatingAlertPolicyId] = useState<number | null>(null);
  const [previewingAlertPolicy, setPreviewingAlertPolicy] = useState(false);
  const [alertPolicyDraft, setAlertPolicyDraft] = useState<AlertPolicyForm>(defaultAlertPolicyForm);
  const [alertPolicyPreview, setAlertPolicyPreview] = useState<AlertPolicyPreviewResponse | null>(null);
  const [alertPolicyNotice, setAlertPolicyNotice] = useState<string | null>(null);
  const [activePage, setActivePage] = useState<ConsolePage>(() =>
    resolveConsolePageFromHash(typeof window !== "undefined" ? window.location.hash : "", true)
  );
  const [menuAxis, setMenuAxis] = useState<MenuAxis>("screen");
  const [functionWorkspace, setFunctionWorkspace] = useState<FunctionWorkspace>("cmdb");
  const [departmentWorkspace, setDepartmentWorkspace] = useState("all");
  const [businessWorkspace, setBusinessWorkspace] = useState("all");
  const [newMonitoringSource, setNewMonitoringSource] =
    useState<NewMonitoringSourceForm>(defaultMonitoringSourceForm);
  const [monitoringSourceFilters, setMonitoringSourceFilters] =
    useState<MonitoringSourceFilterForm>(defaultMonitoringSourceFilters);
  const [newNotificationChannel, setNewNotificationChannel] =
    useState<NewNotificationChannelForm>(defaultNotificationChannelForm);
  const [newNotificationTemplate, setNewNotificationTemplate] =
    useState<NewNotificationTemplateForm>(defaultNotificationTemplateForm);
  const [newNotificationSubscription, setNewNotificationSubscription] =
    useState<NewNotificationSubscriptionForm>(defaultNotificationSubscriptionForm);
  const [newWorkflowStep, setNewWorkflowStep] =
    useState<NewWorkflowTemplateStepForm>(defaultWorkflowStepForm);
  const [newWorkflowTemplateName, setNewWorkflowTemplateName] = useState("");
  const [newWorkflowTemplateDescription, setNewWorkflowTemplateDescription] = useState("");
  const [newWorkflowTemplateSteps, setNewWorkflowTemplateSteps] = useState<NewWorkflowTemplateStepForm[]>([]);
  const [newWorkflowRequest, setNewWorkflowRequest] =
    useState<NewWorkflowRequestForm>(defaultWorkflowRequestForm);
  const roleSet = useMemo(() => new Set(authIdentity?.roles ?? []), [authIdentity?.roles]);
  const canWriteCmdb = roleSet.has("admin") || roleSet.has("operator");
  const canAccessAdmin = roleSet.has("admin");
  const roleText = useMemo(() => (authIdentity?.roles.length ? authIdentity.roles.join(", ") : "-"), [authIdentity]);

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }

    const syncFromHash = () => {
      const canReadAdminPage = canAccessAdmin || !authIdentity;
      const page = resolveConsolePageFromHash(window.location.hash, canReadAdminPage);
      setActivePage(page);
      const canonicalHash = buildConsolePageHash(page);
      if (window.location.hash !== canonicalHash) {
        window.history.replaceState(window.history.state, "", canonicalHash);
      }
    };

    syncFromHash();
    window.addEventListener("hashchange", syncFromHash);
    return () => window.removeEventListener("hashchange", syncFromHash);
  }, [authIdentity, canAccessAdmin]);

  const applyAuthSession = useCallback((session: AuthSession | null) => {
    setRuntimeAuthSession(session);
    setAuthSession(session);
  }, []);

  const loadCurrentIdentity = useCallback(async () => {
    const response = await apiFetch(`${API_BASE_URL}/api/v1/auth/me`);
    if (!response.ok) {
      throw new Error(await readErrorMessage(response));
    }
    const payload: AuthIdentity = await response.json();
    return payload;
  }, []);

  const signIn = useCallback(async () => {
    const normalizedPrincipal = loginPrincipal.trim();
    const normalizedToken = loginToken.trim();
    setAuthError(null);
    setAuthNotice(null);

    if (loginMode === "header") {
      if (!normalizedPrincipal) {
        setAuthError(t("auth.messages.usernameRequired"));
        return;
      }
      applyAuthSession({
        mode: "header",
        principal: normalizedPrincipal,
        token: null
      });
      return;
    }

    if (!normalizedToken) {
      setAuthError(t("auth.messages.tokenRequired"));
      return;
    }

    applyAuthSession({
      mode: "bearer",
      principal: "oidc-session",
      token: normalizedToken
    });
  }, [applyAuthSession, loginMode, loginPrincipal, loginToken, t]);

  const signOut = useCallback(async () => {
    const currentSession = getRuntimeAuthSession();
    if (currentSession?.mode === "bearer" && currentSession.token) {
      try {
        await apiFetch(`${API_BASE_URL}/api/v1/auth/logout`, { method: "POST" });
      } catch {
        // Best-effort session revoke; local sign-out still proceeds.
      }
    }

    applyAuthSession(null);
    setAuthIdentity(null);
    setAuthError(null);
    setAuthNotice(t("auth.messages.signedOut"));
    setError(null);
  }, [applyAuthSession, t]);

  useEffect(() => {
    const onSessionExpired = () => {
      applyAuthSession(null);
      setAuthIdentity(null);
      setAuthError(null);
      setAuthNotice(t("auth.messages.sessionExpired"));
      setError(null);
    };

    window.addEventListener(AUTH_SESSION_EXPIRED_EVENT, onSessionExpired);
    return () => window.removeEventListener(AUTH_SESSION_EXPIRED_EVENT, onSessionExpired);
  }, [applyAuthSession, t]);

  useEffect(() => {
    let cancelled = false;

    if (!authSession) {
      setAuthLoading(false);
      setAuthIdentity(null);
      return;
    }

    setAuthLoading(true);
    setAuthError(null);
    void loadCurrentIdentity()
      .then((identity) => {
        if (cancelled) {
          return;
        }
        setAuthIdentity(identity);
        if (authSession.mode === "header" && authSession.principal !== identity.user.username) {
          applyAuthSession({
            ...authSession,
            principal: identity.user.username
          });
        }
      })
      .catch((err) => {
        if (cancelled) {
          return;
        }
        applyAuthSession(null);
        setAuthIdentity(null);
        setAuthError(err instanceof Error ? err.message : t("cmdb.messages.error"));
      })
      .finally(() => {
        if (!cancelled) {
          setAuthLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [applyAuthSession, authSession, loadCurrentIdentity, t]);

  const loadAssets = useCallback(async () => {
    setLoadingAssets(true);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/assets?limit=50`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: AssetListResponse = await response.json();
      setAssets(payload.items);
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setLoadingAssets(false);
    }
  }, []);

  const loadAssetStats = useCallback(async () => {
    setLoadingAssetStats(true);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/assets/stats`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: AssetStatsResponse = await response.json();
      setAssetStats(payload);
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setAssetStats(null);
    } finally {
      setLoadingAssetStats(false);
    }
  }, []);

  const loadFieldDefinitions = useCallback(async () => {
    setLoadingFields(true);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/field-definitions`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: FieldDefinition[] = await response.json();
      setFieldDefinitions(payload);
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setLoadingFields(false);
    }
  }, []);

  const loadRelations = useCallback(async (assetId: number) => {
    setLoadingRelations(true);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/relations?asset_id=${assetId}`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: AssetRelation[] = await response.json();
      setRelations(payload);
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setLoadingRelations(false);
    }
  }, []);

  const applyBindingsToForm = useCallback((payload: AssetBindingsResponse) => {
    setBindingDepartmentsInput(payload.departments.join(", "));
    setBindingBusinessServicesInput(payload.business_services.join(", "));
    setBindingOwnerDrafts(
      payload.owners.map((owner, index) =>
        createOwnerDraft(normalizeOwnerType(owner.owner_type), owner.owner_ref, `${payload.asset_id}-${index}`)
      )
    );
  }, []);

  const loadAssetBindings = useCallback(async (assetId: number) => {
    setLoadingAssetBindings(true);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/assets/${assetId}/bindings`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: AssetBindingsResponse = await response.json();
      setAssetBindings(payload);
      applyBindingsToForm(payload);
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setAssetBindings(null);
      setBindingOwnerDrafts([]);
    } finally {
      setLoadingAssetBindings(false);
    }
  }, [applyBindingsToForm]);

  const loadAssetMonitoring = useCallback(async (assetId: number) => {
    setLoadingAssetMonitoring(true);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/assets/${assetId}/monitoring-binding`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: AssetMonitoringBindingResponse = await response.json();
      setAssetMonitoring(payload);
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setAssetMonitoring(null);
    } finally {
      setLoadingAssetMonitoring(false);
    }
  }, []);

  const loadAssetImpact = useCallback(async (
    assetId: number,
    direction: ImpactDirection,
    depth: number,
    relationTypes: string[]
  ) => {
    setLoadingAssetImpact(true);
    setError(null);
    try {
      const params = new URLSearchParams({
        direction,
        depth: String(depth)
      });
      if (relationTypes.length > 0) {
        params.set("relation_types", relationTypes.join(","));
      }
      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/assets/${assetId}/impact?${params.toString()}`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: AssetImpactResponse = await response.json();
      setAssetImpact(payload);
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setAssetImpact(null);
      return null;
    } finally {
      setLoadingAssetImpact(false);
    }
  }, []);

  const loadTopologyMap = useCallback(async () => {
    setLoadingTopologyMap(true);
    setError(null);
    setTopologyMapNotice(null);

    try {
      const normalizedScope = trimToNull(topologyScopeInput) ?? "global";
      const normalizedSite = trimToNull(topologySiteFilter);
      const normalizedDepartment = trimToNull(topologyDepartmentFilter);

      const limitParsed = Number.parseInt(topologyWindowLimit.trim(), 10);
      const offsetParsed = Number.parseInt(topologyWindowOffset.trim(), 10);
      if (!Number.isFinite(limitParsed) || limitParsed < 10 || limitParsed > 500) {
        throw new Error(t("topology.workspace.messages.invalidLimit"));
      }
      if (!Number.isFinite(offsetParsed) || offsetParsed < 0) {
        throw new Error(t("topology.workspace.messages.invalidOffset"));
      }

      const params = new URLSearchParams({
        limit: String(limitParsed),
        offset: String(offsetParsed)
      });
      if (normalizedSite) {
        params.set("site", normalizedSite);
      }
      if (normalizedDepartment) {
        params.set("department", normalizedDepartment);
      }

      const response = await apiFetch(
        `${API_BASE_URL}/api/v1/topology/maps/${encodeURIComponent(normalizedScope)}?${params.toString()}`
      );
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }

      const payload: TopologyMapResponse = await response.json();
      setTopologyMap(payload);
      if (payload.nodes.length > 0) {
        setSelectedTopologyMapNodeId(String(payload.nodes[0].id));
      } else {
        setSelectedTopologyMapNodeId("");
      }
      setSelectedTopologyMapEdgeKey(null);

      if (payload.empty) {
        setTopologyMapNotice(t("topology.workspace.messages.emptyScope"));
      } else {
        setTopologyMapNotice(
          t("topology.workspace.messages.loaded", {
            nodes: payload.stats.window_nodes,
            edges: payload.stats.window_edges
          })
        );
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setTopologyMap(null);
      setSelectedTopologyMapNodeId("");
      setSelectedTopologyMapEdgeKey(null);
    } finally {
      setLoadingTopologyMap(false);
    }
  }, [
    t,
    topologyDepartmentFilter,
    topologyScopeInput,
    topologySiteFilter,
    topologyWindowLimit,
    topologyWindowOffset
  ]);

  const loadTopologyEdgeDiagnostics = useCallback(async (edgeId: number) => {
    if (!Number.isFinite(edgeId) || edgeId <= 0) {
      setTopologyDiagnostics(null);
      return null;
    }

    const parsedWindow = Number.parseInt(topologyDiagnosticsWindowMinutes.trim(), 10);
    if (!Number.isFinite(parsedWindow) || parsedWindow < 15 || parsedWindow > 1440) {
      setError(t("topology.workspace.messages.invalidDiagnosticsWindow"));
      return null;
    }

    setLoadingTopologyDiagnostics(true);
    setError(null);
    setTopologyDiagnosticsNotice(null);
    try {
      const response = await apiFetch(
        `${API_BASE_URL}/api/v1/topology/diagnostics/edges/${edgeId}?window_minutes=${parsedWindow}`
      );
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: TopologyDiagnosticsResponse = await response.json();
      setTopologyDiagnostics(payload);
      setTopologyDiagnosticsNotice(
        t("topology.workspace.messages.diagnosticsLoaded", {
          alerts: payload.alerts.length,
          changes: payload.recent_changes.length
        })
      );
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setTopologyDiagnostics(null);
      return null;
    } finally {
      setLoadingTopologyDiagnostics(false);
    }
  }, [t, topologyDiagnosticsWindowMinutes]);

  const runTopologyDiagnosticsAction = useCallback(async (action: TopologyDiagnosticsQuickAction) => {
    if (action.requires_write && !canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }

    if (action.href) {
      if (typeof window !== "undefined") {
        window.location.hash = action.href;
      }
      return;
    }

    if (!action.api_path || !action.method) {
      return;
    }

    setRunningTopologyDiagnosticsActionKey(action.key);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}${action.api_path}`, {
        method: action.method,
        headers: action.body ? { "Content-Type": "application/json" } : undefined,
        body: action.body ? JSON.stringify(action.body) : undefined
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      setTopologyDiagnosticsNotice(
        t("topology.workspace.messages.diagnosticsActionDone", {
          action: action.label
        })
      );
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setRunningTopologyDiagnosticsActionKey(null);
    }
  }, [canWriteCmdb, t]);

  const loadDiscoveryJobs = useCallback(async () => {
    setLoadingDiscoveryJobs(true);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/discovery/jobs`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: DiscoveryJob[] = await response.json();
      setDiscoveryJobs(payload);
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setLoadingDiscoveryJobs(false);
    }
  }, []);

  const loadDiscoveryCandidates = useCallback(async () => {
    setLoadingDiscoveryCandidates(true);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/discovery/candidates?review_status=pending&limit=100`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: { items: DiscoveryCandidate[] } = await response.json();
      setDiscoveryCandidates(payload.items);
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setLoadingDiscoveryCandidates(false);
    }
  }, []);

  const loadNotificationChannels = useCallback(async () => {
    setLoadingNotificationChannels(true);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/discovery/notification-channels`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: NotificationChannel[] = await response.json();
      setNotificationChannels(payload);
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setLoadingNotificationChannels(false);
    }
  }, []);

  const loadNotificationTemplates = useCallback(async () => {
    setLoadingNotificationTemplates(true);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/discovery/notification-templates`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: NotificationTemplate[] = await response.json();
      setNotificationTemplates(payload);
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setLoadingNotificationTemplates(false);
    }
  }, []);

  const loadNotificationSubscriptions = useCallback(async () => {
    setLoadingNotificationSubscriptions(true);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/discovery/notification-subscriptions`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: NotificationSubscription[] = await response.json();
      setNotificationSubscriptions(payload);
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setLoadingNotificationSubscriptions(false);
    }
  }, []);

  const loadWorkflowTemplates = useCallback(async () => {
    setLoadingWorkflowTemplates(true);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/workflow/templates`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: WorkflowTemplate[] = await response.json();
      setWorkflowTemplates(payload);
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setWorkflowTemplates([]);
      return [];
    } finally {
      setLoadingWorkflowTemplates(false);
    }
  }, []);

  const loadWorkflowRequests = useCallback(async () => {
    setLoadingWorkflowRequests(true);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/workflow/requests?limit=100`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: WorkflowRequest[] = await response.json();
      setWorkflowRequests(payload);
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setWorkflowRequests([]);
      return [];
    } finally {
      setLoadingWorkflowRequests(false);
    }
  }, []);

  const loadWorkflowLogs = useCallback(async (requestId: number) => {
    if (!Number.isFinite(requestId) || requestId <= 0) {
      setWorkflowLogs([]);
      return [];
    }

    setLoadingWorkflowLogs(true);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/workflow/requests/${requestId}/logs`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: WorkflowExecutionLog[] = await response.json();
      setWorkflowLogs(payload);
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setWorkflowLogs([]);
      return [];
    } finally {
      setLoadingWorkflowLogs(false);
    }
  }, []);

  const loadPlaybookCatalog = useCallback(async () => {
    setLoadingPlaybookCatalog(true);
    setError(null);
    try {
      const params = new URLSearchParams({
        limit: "200",
        offset: "0",
        is_enabled: "true"
      });
      if (playbookCategoryFilter !== "all") {
        params.set("category", playbookCategoryFilter);
      }
      if (playbookQuery.trim().length > 0) {
        params.set("query", playbookQuery.trim());
      }

      const response = await apiFetch(`${API_BASE_URL}/api/v1/workflow/playbooks?${params.toString()}`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: PlaybookListResponse = await response.json();
      setPlaybookCatalog(payload.items);
      return payload.items;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setPlaybookCatalog([]);
      return [];
    } finally {
      setLoadingPlaybookCatalog(false);
    }
  }, [playbookCategoryFilter, playbookQuery]);

  const loadPlaybookExecutionPolicy = useCallback(async () => {
    setLoadingPlaybookPolicy(true);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/workflow/playbooks/policy`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: PlaybookExecutionPolicyResponse = await response.json();
      setPlaybookExecutionPolicy(payload);
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setPlaybookExecutionPolicy(null);
      return null;
    } finally {
      setLoadingPlaybookPolicy(false);
    }
  }, []);

  const loadPlaybookApprovalRequests = useCallback(async () => {
    setLoadingPlaybookApprovals(true);
    setError(null);
    try {
      const params = new URLSearchParams({
        limit: "120",
        offset: "0"
      });
      if (selectedPlaybookKey.trim().length > 0) {
        params.set("playbook_key", selectedPlaybookKey.trim());
      }

      const response = await apiFetch(`${API_BASE_URL}/api/v1/workflow/playbooks/approvals?${params.toString()}`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: PlaybookApprovalRequestListResponse = await response.json();
      setPlaybookApprovalRequests(payload.items);
      return payload.items;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setPlaybookApprovalRequests([]);
      return [];
    } finally {
      setLoadingPlaybookApprovals(false);
    }
  }, [selectedPlaybookKey]);

  const loadPlaybookExecutions = useCallback(async () => {
    setLoadingPlaybookExecutions(true);
    setError(null);
    try {
      const params = new URLSearchParams({
        limit: "80",
        offset: "0"
      });
      if (selectedPlaybookKey.trim().length > 0) {
        params.set("playbook_key", selectedPlaybookKey.trim());
      }

      const response = await apiFetch(`${API_BASE_URL}/api/v1/workflow/playbooks/executions?${params.toString()}`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: PlaybookExecutionListResponse = await response.json();
      setPlaybookExecutions(payload.items);
      return payload.items;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setPlaybookExecutions([]);
      return [];
    } finally {
      setLoadingPlaybookExecutions(false);
    }
  }, [selectedPlaybookKey]);

  const loadDailyCockpitQueue = useCallback(async () => {
    setLoadingDailyCockpit(true);
    setError(null);
    setDailyCockpitNotice(null);
    try {
      const params = new URLSearchParams({
        limit: "40",
        offset: "0"
      });

      if (dailyCockpitSiteFilter.trim().length > 0) {
        params.set("site", dailyCockpitSiteFilter.trim());
      }
      if (dailyCockpitDepartmentFilter.trim().length > 0) {
        params.set("department", dailyCockpitDepartmentFilter.trim());
      }

      const response = await apiFetch(`${API_BASE_URL}/api/v1/ops/cockpit/queue?${params.toString()}`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }

      const payload: DailyCockpitQueueResponse = await response.json();
      setDailyCockpitQueue(payload);
      setDailyCockpitNotice(
        t("cmdb.dailyCockpit.messages.loaded", {
          total: payload.window.total,
          visible: payload.items.length
        })
      );
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setDailyCockpitQueue(null);
      return null;
    } finally {
      setLoadingDailyCockpit(false);
    }
  }, [dailyCockpitDepartmentFilter, dailyCockpitSiteFilter, t]);

  const loadNextBestActions = useCallback(async () => {
    setLoadingNextBestActions(true);
    setError(null);
    try {
      const params = new URLSearchParams({
        limit: "40"
      });
      if (dailyCockpitSiteFilter.trim().length > 0) {
        params.set("site", dailyCockpitSiteFilter.trim());
      }
      if (dailyCockpitDepartmentFilter.trim().length > 0) {
        params.set("department", dailyCockpitDepartmentFilter.trim());
      }
      if (handoverDigestShiftDate.trim().length > 0) {
        params.set("shift_date", handoverDigestShiftDate.trim());
      }

      const response = await apiFetch(`${API_BASE_URL}/api/v1/ops/cockpit/next-actions?${params.toString()}`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: NextBestActionResponse = await response.json();
      setNextBestActions(payload);
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setNextBestActions(null);
      return null;
    } finally {
      setLoadingNextBestActions(false);
    }
  }, [dailyCockpitDepartmentFilter, dailyCockpitSiteFilter, handoverDigestShiftDate]);

  const loadOpsChecklist = useCallback(async () => {
    setLoadingOpsChecklist(true);
    setError(null);
    setOpsChecklistNotice(null);
    try {
      const params = new URLSearchParams();
      if (opsChecklistDate.trim().length > 0) {
        params.set("date", opsChecklistDate.trim());
      }
      if (dailyCockpitSiteFilter.trim().length > 0) {
        params.set("site", dailyCockpitSiteFilter.trim());
      }
      if (dailyCockpitDepartmentFilter.trim().length > 0) {
        params.set("department", dailyCockpitDepartmentFilter.trim());
      }

      const query = params.toString();
      const response = await apiFetch(
        `${API_BASE_URL}/api/v1/ops/cockpit/checklists${query ? `?${query}` : ""}`
      );
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }

      const payload: OpsChecklistResponse = await response.json();
      setOpsChecklist(payload);
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setOpsChecklist(null);
      return null;
    } finally {
      setLoadingOpsChecklist(false);
    }
  }, [dailyCockpitDepartmentFilter, dailyCockpitSiteFilter, opsChecklistDate]);

  const loadIncidentCommands = useCallback(async () => {
    setLoadingIncidentCommands(true);
    setError(null);
    setIncidentCommandNotice(null);
    try {
      const params = new URLSearchParams({
        limit: "40",
        offset: "0"
      });
      if (dailyCockpitSiteFilter.trim().length > 0) {
        params.set("site", dailyCockpitSiteFilter.trim());
      }
      if (dailyCockpitDepartmentFilter.trim().length > 0) {
        params.set("department", dailyCockpitDepartmentFilter.trim());
      }

      const response = await apiFetch(`${API_BASE_URL}/api/v1/ops/cockpit/incidents?${params.toString()}`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }

      const payload: IncidentCommandListResponse = await response.json();
      setIncidentCommands(payload.items);
      setSelectedIncidentAlertId((current) => {
        if (current.length > 0 && payload.items.some((item) => String(item.alert_id) === current)) {
          return current;
        }
        return payload.items.length > 0 ? String(payload.items[0].alert_id) : "";
      });
      return payload.items;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setIncidentCommands([]);
      setSelectedIncidentAlertId("");
      setIncidentCommandDetail(null);
      return [];
    } finally {
      setLoadingIncidentCommands(false);
    }
  }, [dailyCockpitDepartmentFilter, dailyCockpitSiteFilter]);

  const loadIncidentCommandDetail = useCallback(async (alertIdInput?: number | string) => {
    const raw = typeof alertIdInput === "number" ? String(alertIdInput) : (alertIdInput ?? selectedIncidentAlertId);
    const alertId = Number.parseInt(raw.trim(), 10);
    if (!Number.isFinite(alertId) || alertId <= 0) {
      setIncidentCommandDetail(null);
      return null;
    }

    setLoadingIncidentCommandDetail(true);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/ops/cockpit/incidents/${alertId}`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: IncidentCommandDetailResponse = await response.json();
      setIncidentCommandDetail(payload);
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setIncidentCommandDetail(null);
      return null;
    } finally {
      setLoadingIncidentCommandDetail(false);
    }
  }, [selectedIncidentAlertId]);

  const saveIncidentCommand = useCallback(async () => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return null;
    }

    const alertId = Number.parseInt(incidentCommandDraft.alert_id.trim(), 10);
    if (!Number.isFinite(alertId) || alertId <= 0) {
      setError("Alert id is required.");
      return null;
    }
    const owner = incidentCommandDraft.owner.trim();
    if (owner.length === 0) {
      setError("Owner is required.");
      return null;
    }

    const etaRaw = incidentCommandDraft.eta_at.trim();
    if (
      (incidentCommandDraft.status === "in_progress" || incidentCommandDraft.status === "blocked")
      && etaRaw.length === 0
    ) {
      setError("ETA is required when status is in_progress or blocked.");
      return null;
    }

    let etaAt: string | null = null;
    if (etaRaw.length > 0) {
      const parsed = new Date(etaRaw);
      if (Number.isNaN(parsed.getTime())) {
        setError("ETA must use a valid RFC3339 datetime.");
        return null;
      }
      etaAt = parsed.toISOString();
    }

    const payload = {
      status: incidentCommandDraft.status,
      owner,
      eta_at: etaAt,
      blocker: trimToNull(incidentCommandDraft.blocker),
      summary: trimToNull(incidentCommandDraft.summary),
      note: trimToNull(incidentCommandDraft.note)
    };

    setSavingIncidentCommand(true);
    setError(null);
    setIncidentCommandNotice(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/ops/cockpit/incidents/${alertId}/command`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify(payload)
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }

      const detail: IncidentCommandDetailResponse = await response.json();
      setIncidentCommandDetail(detail);
      setIncidentCommandNotice(
        `Incident command for alert #${detail.item.alert_id} updated: status=${detail.item.command_status}, owner=${detail.item.command_owner}.`
      );
      await loadIncidentCommands();
      return detail;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      return null;
    } finally {
      setSavingIncidentCommand(false);
    }
  }, [canWriteCmdb, incidentCommandDraft, loadIncidentCommands, t]);

  const loadRunbookExecutionPolicy = useCallback(async () => {
    setLoadingRunbookExecutionPolicy(true);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/ops/cockpit/runbook-templates/execution-policy`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: RunbookExecutionPolicyResponse = await response.json();
      setRunbookExecutionPolicy(payload.policy);
      setRunbookExecutionPolicyDraft(buildRunbookExecutionPolicyDraft(payload.policy));
      return payload.policy;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setRunbookExecutionPolicy(null);
      setRunbookExecutionPolicyDraft(defaultRunbookExecutionPolicyDraft);
      return null;
    } finally {
      setLoadingRunbookExecutionPolicy(false);
    }
  }, []);

  const saveRunbookExecutionPolicy = useCallback(async () => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return null;
    }

    const maxTimeout = Number.parseInt(runbookExecutionPolicyDraft.max_live_step_timeout_seconds.trim(), 10);
    if (!Number.isFinite(maxTimeout) || maxTimeout < 1 || maxTimeout > 120) {
      setError("Max live step timeout must be an integer between 1 and 120.");
      return null;
    }

    const liveTemplates = runbookExecutionPolicyDraft.live_templates_csv
      .split(",")
      .map((item) => item.trim())
      .filter((item) => item.length > 0);

    if (runbookExecutionPolicyDraft.mode === "hybrid_live" && liveTemplates.length === 0) {
      setError("At least one live template is required when mode is hybrid_live.");
      return null;
    }

    setSavingRunbookExecutionPolicy(true);
    setRunbookNotice(null);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/ops/cockpit/runbook-templates/execution-policy`, {
        method: "PUT",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          mode: runbookExecutionPolicyDraft.mode,
          live_templates: liveTemplates,
          max_live_step_timeout_seconds: maxTimeout,
          allow_simulate_failure: runbookExecutionPolicyDraft.allow_simulate_failure,
          note: trimToNull(runbookExecutionPolicyDraft.note)
        })
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: RunbookExecutionPolicyResponse = await response.json();
      setRunbookExecutionPolicy(payload.policy);
      setRunbookExecutionPolicyDraft(buildRunbookExecutionPolicyDraft(payload.policy));
      setRunbookNotice(
        `Runbook execution policy updated: mode=${payload.policy.mode}, live_templates=${payload.policy.live_templates.length}.`
      );
      return payload.policy;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      return null;
    } finally {
      setSavingRunbookExecutionPolicy(false);
    }
  }, [canWriteCmdb, runbookExecutionPolicyDraft, t]);

  const loadRunbookTemplates = useCallback(async () => {
    setLoadingRunbookTemplates(true);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/ops/cockpit/runbook-templates`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: RunbookTemplateCatalogResponse = await response.json();
      const items = payload.items ?? [];
      setRunbookTemplates(items);

      const keepSelected = selectedRunbookTemplateKey.length > 0
        && items.some((item) => item.key === selectedRunbookTemplateKey);
      const nextSelectedKey = keepSelected
        ? selectedRunbookTemplateKey
        : (items[0]?.key ?? "");
      setSelectedRunbookTemplateKey(nextSelectedKey);

      const selectedTemplate = items.find((item) => item.key === nextSelectedKey) ?? null;
      if (selectedTemplate) {
        setRunbookParamDraft((prev) =>
          keepSelected && Object.keys(prev).length > 0 ? prev : buildRunbookParamDraft(selectedTemplate)
        );
        setRunbookPreflightDraft((prev) =>
          keepSelected && Object.keys(prev).length > 0 ? prev : buildRunbookPreflightDraft(selectedTemplate)
        );
      } else {
        setRunbookParamDraft({});
        setRunbookPreflightDraft({});
      }

      return items;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setRunbookTemplates([]);
      setSelectedRunbookTemplateKey("");
      setRunbookParamDraft({});
      setRunbookPreflightDraft({});
      return [];
    } finally {
      setLoadingRunbookTemplates(false);
    }
  }, [selectedRunbookTemplateKey]);

  const loadRunbookTemplateExecutions = useCallback(async () => {
    setLoadingRunbookExecutions(true);
    setError(null);
    try {
      const params = new URLSearchParams({
        limit: "40",
        offset: "0"
      });
      if (selectedRunbookTemplateKey.trim().length > 0) {
        params.set("template_key", selectedRunbookTemplateKey.trim());
      }
      const response = await apiFetch(`${API_BASE_URL}/api/v1/ops/cockpit/runbook-templates/executions?${params.toString()}`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: RunbookTemplateExecutionListResponse = await response.json();
      setRunbookExecutions(payload.items ?? []);
      return payload.items ?? [];
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setRunbookExecutions([]);
      return [];
    } finally {
      setLoadingRunbookExecutions(false);
    }
  }, [selectedRunbookTemplateKey]);

  const executeRunbookTemplate = useCallback(async () => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return null;
    }

    const template = runbookTemplates.find((item) => item.key === selectedRunbookTemplateKey) ?? null;
    if (!template) {
      setError("Runbook template is required.");
      return null;
    }
    const supportedModes = (template.execution_modes ?? []).length > 0
      ? template.execution_modes
      : ["simulate"];
    if (!supportedModes.includes(runbookExecutionMode)) {
      setError(`Runbook execution mode '${runbookExecutionMode}' is not supported by template '${template.key}'.`);
      return null;
    }
    if (runbookExecutionMode === "live" && runbookExecutionPolicy?.mode !== "hybrid_live") {
      setError("Live execution is disabled by runbook execution policy.");
      return null;
    }

    const paramsPayload: Record<string, unknown> = {};
    for (const field of template.params ?? []) {
      const rawValue = (runbookParamDraft[field.key] ?? "").trim();
      if (field.field_type === "number") {
        if (rawValue.length === 0) {
          if (field.required) {
            setError(`Runbook parameter '${field.label}' is required.`);
            return null;
          }
          continue;
        }
        const parsed = Number.parseInt(rawValue, 10);
        if (!Number.isFinite(parsed)) {
          setError(`Runbook parameter '${field.label}' must be an integer.`);
          return null;
        }
        paramsPayload[field.key] = parsed;
      } else {
        if (rawValue.length === 0) {
          if (field.required) {
            setError(`Runbook parameter '${field.label}' is required.`);
            return null;
          }
          continue;
        }
        paramsPayload[field.key] = rawValue;
      }
    }

    const preflightConfirmations = (template.preflight ?? [])
      .filter((item) => runbookPreflightDraft[item.key])
      .map((item) => item.key);

    const summary = runbookEvidenceDraft.summary.trim();
    if (summary.length === 0) {
      setError("Runbook evidence summary is required.");
      return null;
    }

    setExecutingRunbookTemplate(true);
    setRunbookNotice(null);
    setError(null);
    try {
      const response = await apiFetch(
        `${API_BASE_URL}/api/v1/ops/cockpit/runbook-templates/${template.key}/execute`,
        {
          method: "POST",
          headers: {
            "Content-Type": "application/json"
          },
          body: JSON.stringify({
            execution_mode: runbookExecutionMode,
            params: paramsPayload,
            preflight_confirmations: preflightConfirmations,
            evidence: {
              summary,
              ticket_ref: trimToNull(runbookEvidenceDraft.ticket_ref),
              artifact_url: trimToNull(runbookEvidenceDraft.artifact_url)
            },
            note: trimToNull(runbookEvidenceDraft.note)
          })
        }
      );
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: RunbookTemplateExecuteResponse = await response.json();
      await loadRunbookTemplateExecutions();
      setRunbookPreflightDraft(buildRunbookPreflightDraft(template));
      const durationMs = Number(payload.execution.runtime_summary?.duration_ms ?? 0);
      setRunbookNotice(
        `Runbook '${payload.template.name}' execution #${payload.execution.id} finished with status '${payload.execution.status}' in mode '${payload.execution.execution_mode}'${Number.isFinite(durationMs) && durationMs > 0 ? ` (duration=${durationMs}ms)` : ""}.`
      );
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      return null;
    } finally {
      setExecutingRunbookTemplate(false);
    }
  }, [
    canWriteCmdb,
    loadRunbookTemplateExecutions,
    runbookEvidenceDraft.artifact_url,
    runbookEvidenceDraft.note,
    runbookEvidenceDraft.summary,
    runbookEvidenceDraft.ticket_ref,
    runbookExecutionMode,
    runbookExecutionPolicy?.mode,
    runbookParamDraft,
    runbookPreflightDraft,
    runbookTemplates,
    selectedRunbookTemplateKey,
    t
  ]);

  useEffect(() => {
    const template = runbookTemplates.find((item) => item.key === selectedRunbookTemplateKey) ?? null;
    if (!template) {
      return;
    }

    const supportedModes = (template.execution_modes ?? []).length > 0
      ? template.execution_modes
      : ["simulate"];
    setRunbookExecutionMode((prev) => (supportedModes.includes(prev) ? prev : "simulate"));

    setRunbookParamDraft((prev) => {
      const next = buildRunbookParamDraft(template);
      for (const key of Object.keys(next)) {
        if (typeof prev[key] === "string") {
          next[key] = prev[key];
        }
      }
      return next;
    });

    setRunbookPreflightDraft((prev) => {
      const next = buildRunbookPreflightDraft(template);
      for (const key of Object.keys(next)) {
        if (typeof prev[key] === "boolean") {
          next[key] = prev[key];
        }
      }
      return next;
    });
  }, [runbookTemplates, selectedRunbookTemplateKey]);

  const loadBackupPolicies = useCallback(async () => {
    setLoadingBackupPolicies(true);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/ops/cockpit/backup/policies`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: BackupPolicyListResponse = await response.json();
      setBackupPolicies(payload.items);
      return payload.items;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setBackupPolicies([]);
      return [];
    } finally {
      setLoadingBackupPolicies(false);
    }
  }, []);

  const loadBackupPolicyRuns = useCallback(async () => {
    setLoadingBackupPolicyRuns(true);
    setError(null);
    try {
      const params = new URLSearchParams({
        limit: "40",
        offset: "0"
      });
      const response = await apiFetch(`${API_BASE_URL}/api/v1/ops/cockpit/backup/runs?${params.toString()}`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: BackupPolicyRunsResponse = await response.json();
      setBackupPolicyRuns(payload.items);
      return payload.items;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setBackupPolicyRuns([]);
      return [];
    } finally {
      setLoadingBackupPolicyRuns(false);
    }
  }, []);

  const loadBackupRestoreEvidence = useCallback(async () => {
    setLoadingBackupRestoreEvidence(true);
    setError(null);
    try {
      const params = new URLSearchParams({
        limit: "80",
        offset: "0"
      });
      const policyId = Number.parseInt(backupPolicyDraft.policy_id.trim(), 10);
      if (Number.isFinite(policyId) && policyId > 0) {
        params.set("policy_id", String(policyId));
      }
      if (backupRestoreRunStatusFilter !== "all") {
        params.set("run_status", backupRestoreRunStatusFilter);
      }
      const response = await apiFetch(`${API_BASE_URL}/api/v1/ops/cockpit/backup/restore-evidence?${params.toString()}`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: BackupRestoreEvidenceListResponse = await response.json();
      setBackupRestoreEvidence(payload.items);
      setBackupRestoreEvidenceCoverage(payload.coverage);
      setBackupRestoreEvidenceMissingRunIds(payload.missing_run_ids);
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setBackupRestoreEvidence([]);
      setBackupRestoreEvidenceCoverage({
        required_runs: 0,
        covered_runs: 0,
        missing_runs: 0
      });
      setBackupRestoreEvidenceMissingRunIds([]);
      return null;
    } finally {
      setLoadingBackupRestoreEvidence(false);
    }
  }, [backupPolicyDraft.policy_id, backupRestoreRunStatusFilter]);

  const loadBackupEvidenceCompliancePolicy = useCallback(async () => {
    setLoadingBackupEvidenceCompliancePolicy(true);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/ops/cockpit/backup/evidence-compliance/policy`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: BackupEvidenceCompliancePolicyResponse = await response.json();
      setBackupEvidenceCompliancePolicy(payload);
      setBackupEvidenceCompliancePolicyDraft({
        mode: payload.policy.mode,
        sla_hours: String(payload.policy.sla_hours),
        require_failed_runs: payload.policy.require_failed_runs,
        require_drill_runs: payload.policy.require_drill_runs,
        note: ""
      });
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setBackupEvidenceCompliancePolicy(null);
      return null;
    } finally {
      setLoadingBackupEvidenceCompliancePolicy(false);
    }
  }, []);

  const loadBackupEvidenceComplianceScorecard = useCallback(async () => {
    setLoadingBackupEvidenceComplianceScorecard(true);
    setError(null);
    try {
      const params = new URLSearchParams();
      if (backupEvidenceComplianceWeekStart.trim().length > 0) {
        params.set("week_start", backupEvidenceComplianceWeekStart.trim());
      }
      const response = await apiFetch(
        `${API_BASE_URL}/api/v1/ops/cockpit/backup/evidence-compliance/scorecard?${params.toString()}`
      );
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: BackupEvidenceComplianceScorecardResponse = await response.json();
      setBackupEvidenceComplianceScorecard(payload);
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setBackupEvidenceComplianceScorecard(null);
      return null;
    } finally {
      setLoadingBackupEvidenceComplianceScorecard(false);
    }
  }, [backupEvidenceComplianceWeekStart]);

  const saveBackupEvidenceCompliancePolicy = useCallback(async () => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return null;
    }

    const slaHours = Number.parseInt(backupEvidenceCompliancePolicyDraft.sla_hours.trim(), 10);
    if (!Number.isFinite(slaHours) || slaHours <= 0) {
      setError("Evidence SLA hours must be a positive integer.");
      return null;
    }

    if (!backupEvidenceCompliancePolicyDraft.require_failed_runs && !backupEvidenceCompliancePolicyDraft.require_drill_runs) {
      setError("At least one evidence scope is required (failed runs or drill runs).");
      return null;
    }

    setSavingBackupEvidenceCompliancePolicy(true);
    setBackupPolicyNotice(null);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/ops/cockpit/backup/evidence-compliance/policy`, {
        method: "PUT",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          mode: backupEvidenceCompliancePolicyDraft.mode,
          sla_hours: slaHours,
          require_failed_runs: backupEvidenceCompliancePolicyDraft.require_failed_runs,
          require_drill_runs: backupEvidenceCompliancePolicyDraft.require_drill_runs,
          note: trimToNull(backupEvidenceCompliancePolicyDraft.note)
        })
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: BackupEvidenceCompliancePolicyResponse = await response.json();
      setBackupEvidenceCompliancePolicy(payload);
      setBackupEvidenceCompliancePolicyDraft((prev) => ({
        ...prev,
        mode: payload.policy.mode,
        sla_hours: String(payload.policy.sla_hours),
        require_failed_runs: payload.policy.require_failed_runs,
        require_drill_runs: payload.policy.require_drill_runs,
        note: ""
      }));
      await loadBackupEvidenceComplianceScorecard();
      setBackupPolicyNotice(
        `Evidence compliance policy updated: mode=${payload.policy.mode}, sla_hours=${payload.policy.sla_hours}.`
      );
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      return null;
    } finally {
      setSavingBackupEvidenceCompliancePolicy(false);
    }
  }, [backupEvidenceCompliancePolicyDraft, canWriteCmdb, loadBackupEvidenceComplianceScorecard, t]);

  const exportBackupEvidenceComplianceScorecard = useCallback(async (format: "csv" | "json") => {
    setExportingBackupEvidenceComplianceScorecard(true);
    setBackupPolicyNotice(null);
    setError(null);
    try {
      const params = new URLSearchParams({ format });
      if (backupEvidenceComplianceWeekStart.trim().length > 0) {
        params.set("week_start", backupEvidenceComplianceWeekStart.trim());
      }
      const response = await apiFetch(
        `${API_BASE_URL}/api/v1/ops/cockpit/backup/evidence-compliance/scorecard/export?${params.toString()}`
      );
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: BackupEvidenceComplianceScorecardExportResponse = await response.json();
      if (typeof window !== "undefined") {
        const blob = new Blob(
          [payload.content],
          { type: format === "csv" ? "text/csv;charset=utf-8" : "application/json;charset=utf-8" }
        );
        const url = URL.createObjectURL(blob);
        const anchor = document.createElement("a");
        anchor.href = url;
        anchor.download = `${payload.scorecard_key}.${format}`;
        document.body.append(anchor);
        anchor.click();
        anchor.remove();
        URL.revokeObjectURL(url);
      }
      setBackupPolicyNotice(`Evidence compliance scorecard exported as ${payload.scorecard_key}.${format}.`);
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      return null;
    } finally {
      setExportingBackupEvidenceComplianceScorecard(false);
    }
  }, [backupEvidenceComplianceWeekStart]);

  const loadChangeCalendar = useCallback(async () => {
    setLoadingChangeCalendar(true);
    setError(null);
    try {
      const params = new URLSearchParams();
      const startDate = changeCalendarStartDate.trim();
      const endDate = changeCalendarEndDate.trim();
      if (startDate) {
        params.set("start_date", startDate);
      }
      if (endDate) {
        params.set("end_date", endDate);
      }
      const response = await apiFetch(`${API_BASE_URL}/api/v1/ops/cockpit/change-calendar?${params.toString()}`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: ChangeCalendarResponse = await response.json();
      setChangeCalendar(payload);
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setChangeCalendar(null);
      return null;
    } finally {
      setLoadingChangeCalendar(false);
    }
  }, [changeCalendarEndDate, changeCalendarStartDate]);

  const loadChangeCalendarReservations = useCallback(async () => {
    setLoadingChangeCalendarReservations(true);
    setError(null);
    try {
      const params = new URLSearchParams();
      const startDate = changeCalendarStartDate.trim();
      const endDate = changeCalendarEndDate.trim();
      if (startDate) {
        params.set("start_date", startDate);
      }
      if (endDate) {
        params.set("end_date", endDate);
      }
      params.set("status", "reserved");
      params.set("limit", "120");

      const response = await apiFetch(
        `${API_BASE_URL}/api/v1/ops/cockpit/change-calendar/reservations?${params.toString()}`
      );
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: ChangeCalendarReservationListResponse = await response.json();
      setChangeCalendarReservations(payload.items);
      return payload.items;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setChangeCalendarReservations([]);
      return [];
    } finally {
      setLoadingChangeCalendarReservations(false);
    }
  }, [changeCalendarEndDate, changeCalendarStartDate]);

  const loadChangeCalendarSlotRecommendations = useCallback(async () => {
    setLoadingChangeCalendarRecommendations(true);
    setError(null);
    try {
      const startAt = localDateTimeInputToUtcRfc3339(changeCalendarReservationDraft.start_at_local);
      const endAt = localDateTimeInputToUtcRfc3339(changeCalendarReservationDraft.end_at_local);
      if (!startAt || !endAt) {
        throw new Error("Recommendation requires valid draft start/end.");
      }
      const startMs = Date.parse(startAt);
      const endMs = Date.parse(endAt);
      if (!Number.isFinite(startMs) || !Number.isFinite(endMs) || endMs <= startMs) {
        throw new Error("Recommendation requires end time later than start time.");
      }
      const durationMinutes = Math.max(15, Math.round((endMs - startMs) / 60000));

      const params = new URLSearchParams({
        operation_kind: changeCalendarReservationDraft.operation_kind.trim() || "playbook.execute",
        risk_level: changeCalendarReservationDraft.risk_level,
        duration_minutes: String(durationMinutes),
        limit: "5"
      });
      if (changeCalendarReservationDraft.site.trim().length > 0) {
        params.set("site", changeCalendarReservationDraft.site.trim());
      }
      if (changeCalendarReservationDraft.department.trim().length > 0) {
        params.set("department", changeCalendarReservationDraft.department.trim());
      }

      const response = await apiFetch(
        `${API_BASE_URL}/api/v1/ops/cockpit/change-calendar/slot-recommendations?${params.toString()}`
      );
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: ChangeCalendarSlotRecommendationResponse = await response.json();
      setChangeCalendarSlotRecommendations(payload);
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setChangeCalendarSlotRecommendations(null);
      return null;
    } finally {
      setLoadingChangeCalendarRecommendations(false);
    }
  }, [changeCalendarReservationDraft]);

  const createChangeCalendarReservation = useCallback(async () => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return null;
    }

    const startAt = localDateTimeInputToUtcRfc3339(changeCalendarReservationDraft.start_at_local);
    const endAt = localDateTimeInputToUtcRfc3339(changeCalendarReservationDraft.end_at_local);
    if (!startAt || !endAt) {
      setError("Reservation requires valid start/end date-time.");
      return null;
    }
    if (Date.parse(endAt) <= Date.parse(startAt)) {
      setError("Reservation requires end time later than start time.");
      return null;
    }
    if (changeCalendarReservationDraft.operation_kind.trim().length === 0) {
      setError("Operation kind is required for reservation.");
      return null;
    }
    if (changeCalendarReservationDraft.owner.trim().length === 0) {
      setError("Owner is required for reservation.");
      return null;
    }

    setCreatingChangeCalendarReservation(true);
    setChangeCalendarNotice(null);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/ops/cockpit/change-calendar/reservations`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          start_at: startAt,
          end_at: endAt,
          operation_kind: changeCalendarReservationDraft.operation_kind.trim(),
          risk_level: changeCalendarReservationDraft.risk_level,
          owner: changeCalendarReservationDraft.owner.trim(),
          site: trimToNull(changeCalendarReservationDraft.site),
          department: trimToNull(changeCalendarReservationDraft.department),
          note: trimToNull(changeCalendarReservationDraft.note)
        })
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: { reservation: ChangeCalendarReservationRecord; decision_reason: string } = await response.json();
      setPlaybookReservationId(String(payload.reservation.id));
      setChangeCalendarNotice(
        `Reservation #${payload.reservation.id} created: ${payload.decision_reason}`
      );
      await Promise.all([loadChangeCalendar(), loadChangeCalendarReservations()]);
      return payload.reservation;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      return null;
    } finally {
      setCreatingChangeCalendarReservation(false);
    }
  }, [
    canWriteCmdb,
    changeCalendarReservationDraft,
    loadChangeCalendar,
    loadChangeCalendarReservations,
    t
  ]);

  const checkChangeCalendarConflicts = useCallback(async () => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return null;
    }

    const startAt = localDateTimeInputToUtcRfc3339(changeCalendarConflictDraft.start_at_local);
    const endAt = localDateTimeInputToUtcRfc3339(changeCalendarConflictDraft.end_at_local);
    if (!startAt || !endAt) {
      setError("Conflict check requires valid start/end date-time.");
      return null;
    }
    if (Date.parse(endAt) <= Date.parse(startAt)) {
      setError("Conflict check requires end time later than start time.");
      return null;
    }

    setCheckingChangeCalendarConflict(true);
    setChangeCalendarNotice(null);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/ops/cockpit/change-calendar/conflicts`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          start_at: startAt,
          end_at: endAt,
          operation_kind: changeCalendarConflictDraft.operation_kind.trim(),
          risk_level: changeCalendarConflictDraft.risk_level
        })
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: ChangeCalendarConflictResponse = await response.json();
      setChangeCalendarConflictResult(payload);
      setChangeCalendarNotice(
        payload.has_conflict
          ? `Calendar conflict detected: ${payload.decision_reason}`
          : `Calendar slot is clear: ${payload.decision_reason}`
      );
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      return null;
    } finally {
      setCheckingChangeCalendarConflict(false);
    }
  }, [canWriteCmdb, changeCalendarConflictDraft, t]);

  const saveBackupRestoreEvidence = useCallback(async () => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return null;
    }

    const runId = Number.parseInt(backupRestoreEvidenceDraft.run_id.trim(), 10);
    if (!Number.isFinite(runId) || runId <= 0) {
      setError("Run ID is required for restore evidence.");
      return null;
    }
    const artifactUrl = backupRestoreEvidenceDraft.artifact_url.trim();
    if (!artifactUrl) {
      setError("Artifact URL is required.");
      return null;
    }

    setSavingBackupRestoreEvidence(true);
    setBackupPolicyNotice(null);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/ops/cockpit/backup/runs/${runId}/restore-evidence`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          ticket_ref: trimToNull(backupRestoreEvidenceDraft.ticket_ref),
          artifact_url: artifactUrl,
          note: trimToNull(backupRestoreEvidenceDraft.note),
          verifier: trimToNull(backupRestoreEvidenceDraft.verifier),
          close_evidence: backupRestoreEvidenceDraft.close_evidence
        })
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: BackupRestoreEvidenceRecord = await response.json();
      await Promise.all([
        loadBackupPolicyRuns(),
        loadBackupRestoreEvidence(),
        loadBackupEvidenceComplianceScorecard()
      ]);
      setBackupPolicyNotice(
        `Restore evidence #${payload.id} attached to run #${payload.run_id} (${payload.closure_status}).`
      );
      setBackupRestoreEvidenceDraft((prev) => ({
        ...defaultBackupRestoreEvidenceForm,
        run_id: prev.run_id,
        verifier: prev.verifier
      }));
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      return null;
    } finally {
      setSavingBackupRestoreEvidence(false);
    }
  }, [
    backupRestoreEvidenceDraft,
    canWriteCmdb,
    loadBackupEvidenceComplianceScorecard,
    loadBackupPolicyRuns,
    loadBackupRestoreEvidence,
    t
  ]);

  const closeBackupRestoreEvidence = useCallback(async (evidenceId: number) => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return null;
    }
    if (!Number.isFinite(evidenceId) || evidenceId <= 0) {
      return null;
    }

    setSavingBackupRestoreEvidence(true);
    setBackupPolicyNotice(null);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/ops/cockpit/backup/restore-evidence/${evidenceId}`, {
        method: "PATCH",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          close_evidence: true
        })
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: BackupRestoreEvidenceRecord = await response.json();
      await Promise.all([
        loadBackupPolicyRuns(),
        loadBackupRestoreEvidence(),
        loadBackupEvidenceComplianceScorecard()
      ]);
      setBackupPolicyNotice(`Restore evidence #${payload.id} closed.`);
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      return null;
    } finally {
      setSavingBackupRestoreEvidence(false);
    }
  }, [canWriteCmdb, loadBackupEvidenceComplianceScorecard, loadBackupPolicyRuns, loadBackupRestoreEvidence, t]);

  const saveBackupPolicy = useCallback(async () => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return null;
    }

    const policyKey = backupPolicyDraft.policy_key.trim().toLowerCase();
    const name = backupPolicyDraft.name.trim();
    if (policyKey.length === 0 || name.length === 0) {
      setError("Policy key and name are required.");
      return null;
    }

    const retentionDays = Number.parseInt(backupPolicyDraft.retention_days.trim(), 10);
    if (!Number.isFinite(retentionDays) || retentionDays <= 0) {
      setError("Retention days must be a positive integer.");
      return null;
    }

    const scheduleWeekday = Number.parseInt(backupPolicyDraft.schedule_weekday.trim(), 10);
    const drillWeekday = Number.parseInt(backupPolicyDraft.drill_weekday.trim(), 10);
    const editingPolicyId = Number.parseInt(backupPolicyDraft.policy_id.trim(), 10);
    const payload = {
      policy_key: policyKey,
      name,
      frequency: backupPolicyDraft.frequency,
      schedule_time_utc: backupPolicyDraft.schedule_time_utc.trim(),
      schedule_weekday: backupPolicyDraft.frequency === "weekly" && Number.isFinite(scheduleWeekday)
        ? scheduleWeekday
        : null,
      retention_days: retentionDays,
      destination_type: backupPolicyDraft.destination_type,
      destination_uri: backupPolicyDraft.destination_uri.trim(),
      drill_enabled: backupPolicyDraft.drill_enabled,
      drill_frequency: backupPolicyDraft.drill_frequency,
      drill_weekday: backupPolicyDraft.drill_frequency === "weekly" && Number.isFinite(drillWeekday)
        ? drillWeekday
        : null,
      drill_time_utc: backupPolicyDraft.drill_time_utc.trim(),
      note: trimToNull(backupPolicyDraft.note)
    };

    setSavingBackupPolicy(true);
    setBackupPolicyNotice(null);
    setError(null);
    try {
      const isEdit = Number.isFinite(editingPolicyId) && editingPolicyId > 0;
      const url = isEdit
        ? `${API_BASE_URL}/api/v1/ops/cockpit/backup/policies/${editingPolicyId}`
        : `${API_BASE_URL}/api/v1/ops/cockpit/backup/policies`;
      const response = await apiFetch(url, {
        method: isEdit ? "PATCH" : "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify(payload)
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const item: BackupPolicyRecord = await response.json();
      await Promise.all([
        loadBackupPolicies(),
        loadBackupPolicyRuns(),
        loadBackupRestoreEvidence(),
        loadBackupEvidenceComplianceScorecard()
      ]);
      setBackupPolicyNotice(
        isEdit
          ? `Backup policy '${item.policy_key}' updated.`
          : `Backup policy '${item.policy_key}' created.`
      );
      setBackupPolicyDraft({
        ...defaultBackupPolicyForm,
        policy_id: String(item.id),
        policy_key: item.policy_key,
        name: item.name,
        frequency: item.frequency,
        schedule_time_utc: item.schedule_time_utc,
        schedule_weekday: String(item.schedule_weekday ?? 1),
        retention_days: String(item.retention_days),
        destination_type: item.destination_type,
        destination_uri: item.destination_uri,
        drill_enabled: item.drill_enabled,
        drill_frequency: item.drill_frequency,
        drill_weekday: String(item.drill_weekday ?? 3),
        drill_time_utc: item.drill_time_utc,
        note: ""
      });
      return item;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      return null;
    } finally {
      setSavingBackupPolicy(false);
    }
  }, [
    backupPolicyDraft,
    canWriteCmdb,
    loadBackupEvidenceComplianceScorecard,
    loadBackupPolicies,
    loadBackupPolicyRuns,
    loadBackupRestoreEvidence,
    t
  ]);

  const runBackupPolicy = useCallback(async (
    policyId: number,
    runType: "backup" | "drill",
    simulateFailure: boolean
  ) => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return null;
    }
    if (!Number.isFinite(policyId) || policyId <= 0) {
      return null;
    }

    const actionKey = `${policyId}:${runType}:${simulateFailure ? "fail" : "run"}`;
    setRunningBackupPolicyActionId(actionKey);
    setBackupPolicyNotice(null);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/ops/cockpit/backup/policies/${policyId}/run`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          run_type: runType,
          simulate_failure: simulateFailure,
          note: trimToNull(backupPolicyDraft.note)
        })
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: BackupPolicyRunResult = await response.json();
      await Promise.all([
        loadBackupPolicies(),
        loadBackupPolicyRuns(),
        loadBackupRestoreEvidence(),
        loadBackupEvidenceComplianceScorecard()
      ]);
      setBackupRestoreEvidenceDraft((prev) => ({
        ...prev,
        run_id: String(payload.run.id)
      }));
      setBackupPolicyNotice(
        `${runType} run #${payload.run.id} for policy '${payload.policy.policy_key}' finished with status '${payload.run.status}'.`
      );
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      return null;
    } finally {
      setRunningBackupPolicyActionId(null);
    }
  }, [
    backupPolicyDraft.note,
    canWriteCmdb,
    loadBackupEvidenceComplianceScorecard,
    loadBackupPolicies,
    loadBackupPolicyRuns,
    loadBackupRestoreEvidence,
    t
  ]);

  const runBackupSchedulerTick = useCallback(async () => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return null;
    }
    setTickingBackupScheduler(true);
    setBackupPolicyNotice(null);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/ops/cockpit/backup/scheduler/tick`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          note: trimToNull(backupPolicyDraft.note)
        })
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: BackupSchedulerTickResponse = await response.json();
      await Promise.all([
        loadBackupPolicies(),
        loadBackupPolicyRuns(),
        loadBackupRestoreEvidence(),
        loadBackupEvidenceComplianceScorecard()
      ]);
      setBackupPolicyNotice(
        `Scheduler tick completed: backup_runs=${payload.backup_runs}, drill_runs=${payload.drill_runs}.`
      );
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      return null;
    } finally {
      setTickingBackupScheduler(false);
    }
  }, [
    backupPolicyDraft.note,
    canWriteCmdb,
    loadBackupEvidenceComplianceScorecard,
    loadBackupPolicies,
    loadBackupPolicyRuns,
    loadBackupRestoreEvidence,
    t
  ]);

  const loadWeeklyDigest = useCallback(async () => {
    setLoadingWeeklyDigest(true);
    setError(null);
    try {
      const params = new URLSearchParams();
      if (weeklyDigestWeekStart.trim().length > 0) {
        params.set("week_start", weeklyDigestWeekStart.trim());
      }
      const response = await apiFetch(`${API_BASE_URL}/api/v1/ops/cockpit/weekly-digest?${params.toString()}`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: WeeklyDigestResponse = await response.json();
      setWeeklyDigest(payload);
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setWeeklyDigest(null);
      return null;
    } finally {
      setLoadingWeeklyDigest(false);
    }
  }, [weeklyDigestWeekStart]);

  const exportWeeklyDigest = useCallback(async (format: "csv" | "json") => {
    setExportingWeeklyDigest(true);
    setWeeklyDigestNotice(null);
    setError(null);
    try {
      const params = new URLSearchParams({
        format
      });
      if (weeklyDigestWeekStart.trim().length > 0) {
        params.set("week_start", weeklyDigestWeekStart.trim());
      }
      const response = await apiFetch(`${API_BASE_URL}/api/v1/ops/cockpit/weekly-digest/export?${params.toString()}`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: WeeklyDigestExportResponse = await response.json();

      if (typeof window !== "undefined") {
        const blob = new Blob(
          [payload.content],
          { type: format === "csv" ? "text/csv;charset=utf-8" : "application/json;charset=utf-8" }
        );
        const url = URL.createObjectURL(blob);
        const anchor = document.createElement("a");
        anchor.href = url;
        anchor.download = `${payload.digest_key}.${format}`;
        document.body.append(anchor);
        anchor.click();
        anchor.remove();
        URL.revokeObjectURL(url);
      }

      setWeeklyDigestNotice(`Weekly digest exported as ${payload.digest_key}.${format}.`);
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      return null;
    } finally {
      setExportingWeeklyDigest(false);
    }
  }, [weeklyDigestWeekStart]);

  const loadHandoverDigest = useCallback(async () => {
    setLoadingHandoverDigest(true);
    setError(null);
    try {
      const params = new URLSearchParams();
      if (handoverDigestShiftDate.trim().length > 0) {
        params.set("shift_date", handoverDigestShiftDate.trim());
      }
      const response = await apiFetch(`${API_BASE_URL}/api/v1/ops/cockpit/handover-digest?${params.toString()}`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: HandoverDigestResponse = await response.json();
      setHandoverDigest(payload);
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setHandoverDigest(null);
      return null;
    } finally {
      setLoadingHandoverDigest(false);
    }
  }, [handoverDigestShiftDate]);

  const loadHandoverReminders = useCallback(async () => {
    setLoadingHandoverReminders(true);
    setError(null);
    try {
      const params = new URLSearchParams();
      if (handoverDigestShiftDate.trim().length > 0) {
        params.set("shift_date", handoverDigestShiftDate.trim());
      }
      const response = await apiFetch(`${API_BASE_URL}/api/v1/ops/cockpit/handover-digest/reminders?${params.toString()}`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: HandoverReminderResponse = await response.json();
      setHandoverReminders(payload);
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setHandoverReminders(null);
      return null;
    } finally {
      setLoadingHandoverReminders(false);
    }
  }, [handoverDigestShiftDate]);

  const exportHandoverDigest = useCallback(async (format: "csv" | "json") => {
    setExportingHandoverDigest(true);
    setHandoverDigestNotice(null);
    setError(null);
    try {
      const params = new URLSearchParams({
        format
      });
      if (handoverDigestShiftDate.trim().length > 0) {
        params.set("shift_date", handoverDigestShiftDate.trim());
      }
      const response = await apiFetch(`${API_BASE_URL}/api/v1/ops/cockpit/handover-digest/export?${params.toString()}`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: HandoverDigestExportResponse = await response.json();
      if (typeof window !== "undefined") {
        const blob = new Blob(
          [payload.content],
          { type: format === "csv" ? "text/csv;charset=utf-8" : "application/json;charset=utf-8" }
        );
        const url = URL.createObjectURL(blob);
        const anchor = document.createElement("a");
        anchor.href = url;
        anchor.download = `${payload.digest_key}.${format}`;
        document.body.append(anchor);
        anchor.click();
        anchor.remove();
        URL.revokeObjectURL(url);
      }
      setHandoverDigestNotice(`Handover digest exported as ${payload.digest_key}.${format}.`);
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      return null;
    } finally {
      setExportingHandoverDigest(false);
    }
  }, [handoverDigestShiftDate]);

  const exportHandoverReminders = useCallback(async (format: "csv" | "json") => {
    setExportingHandoverReminders(true);
    setHandoverDigestNotice(null);
    setError(null);
    try {
      const params = new URLSearchParams({
        format
      });
      if (handoverDigestShiftDate.trim().length > 0) {
        params.set("shift_date", handoverDigestShiftDate.trim());
      }
      const response = await apiFetch(`${API_BASE_URL}/api/v1/ops/cockpit/handover-digest/reminders/export?${params.toString()}`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: HandoverReminderExportResponse = await response.json();
      if (typeof window !== "undefined") {
        const blob = new Blob(
          [payload.content],
          { type: format === "csv" ? "text/csv;charset=utf-8" : "application/json;charset=utf-8" }
        );
        const url = URL.createObjectURL(blob);
        const anchor = document.createElement("a");
        anchor.href = url;
        anchor.download = `${payload.digest_key}-reminders.${format}`;
        document.body.append(anchor);
        anchor.click();
        anchor.remove();
        URL.revokeObjectURL(url);
      }
      setHandoverDigestNotice(`Handover reminders exported as ${payload.digest_key}-reminders.${format}.`);
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      return null;
    } finally {
      setExportingHandoverReminders(false);
    }
  }, [handoverDigestShiftDate]);

  const closeHandoverCarryoverItem = useCallback(async (item: HandoverCarryoverItem) => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return null;
    }
    const nextOwner = item.next_owner.trim();
    const nextAction = item.next_action.trim();
    if (!nextOwner || !nextAction) {
      setError("next_owner and next_action are required to close handover item.");
      return null;
    }

    setClosingHandoverItemKey(item.item_key);
    setHandoverDigestNotice(null);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/ops/cockpit/handover-digest/items/${encodeURIComponent(item.item_key)}/close`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          shift_date: handoverDigestShiftDate,
          source_type: item.source_type,
          source_id: item.source_id,
          next_owner: nextOwner,
          next_action: nextAction,
          note: item.note ?? undefined
        })
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      await Promise.all([loadHandoverDigest(), loadHandoverReminders(), loadWeeklyDigest()]);
      setHandoverDigestNotice(`Handover item '${item.item_key}' closed.`);
      return true;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      return null;
    } finally {
      setClosingHandoverItemKey(null);
    }
  }, [canWriteCmdb, handoverDigestShiftDate, loadHandoverDigest, loadHandoverReminders, loadWeeklyDigest, t]);

  const loadDailyCockpitSnapshot = useCallback(async () => {
    const [
      queue,
      nextActions,
      checklist,
      incidents,
      runbooks,
      runbookPolicy,
      runbookExecutions,
      policies,
      runs,
      evidence,
      evidencePolicy,
      evidenceScorecard,
      calendar,
      reservations,
      digest,
      handover,
      reminders
    ] = await Promise.all([
      loadDailyCockpitQueue(),
      loadNextBestActions(),
      loadOpsChecklist(),
      loadIncidentCommands(),
      loadRunbookTemplates(),
      loadRunbookExecutionPolicy(),
      loadRunbookTemplateExecutions(),
      loadBackupPolicies(),
      loadBackupPolicyRuns(),
      loadBackupRestoreEvidence(),
      loadBackupEvidenceCompliancePolicy(),
      loadBackupEvidenceComplianceScorecard(),
      loadChangeCalendar(),
      loadChangeCalendarReservations(),
      loadWeeklyDigest(),
      loadHandoverDigest(),
      loadHandoverReminders()
    ]);
    return {
      queue,
      nextActions,
      checklist,
      incidents,
      runbooks,
      runbookPolicy,
      runbookExecutions,
      policies,
      runs,
      evidence,
      evidencePolicy,
      evidenceScorecard,
      calendar,
      reservations,
      digest,
      handover,
      reminders
    };
  }, [
    loadBackupEvidenceCompliancePolicy,
    loadBackupEvidenceComplianceScorecard,
    loadRunbookExecutionPolicy,
    loadRunbookTemplateExecutions,
    loadRunbookTemplates,
    loadBackupPolicies,
    loadBackupPolicyRuns,
    loadBackupRestoreEvidence,
    loadChangeCalendar,
    loadChangeCalendarReservations,
    loadDailyCockpitQueue,
    loadHandoverDigest,
    loadHandoverReminders,
    loadIncidentCommands,
    loadNextBestActions,
    loadOpsChecklist,
    loadWeeklyDigest
  ]);

  const runDailyCockpitAction = useCallback(async (
    itemKey: string,
    action: DailyCockpitAction
  ) => {
    if (!itemKey || itemKey.trim().length === 0) {
      return;
    }
    if (action.requires_write && !canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }

    if (action.href) {
      if (typeof window !== "undefined") {
        window.location.hash = action.href;
      }
      return;
    }

    if (!action.api_path || !action.method) {
      return;
    }

    const normalizedItemKey = itemKey.trim();
    setRunningDailyCockpitActionKey(`${normalizedItemKey}:${action.key}`);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}${action.api_path}`, {
        method: action.method,
        headers: action.body ? { "Content-Type": "application/json" } : undefined,
        body: action.body ? JSON.stringify(action.body) : undefined
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      await loadDailyCockpitSnapshot();
      setDailyCockpitNotice(
        t("cmdb.dailyCockpit.messages.actionDone", {
          action: action.label
        })
      );
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setRunningDailyCockpitActionKey(null);
    }
  }, [canWriteCmdb, loadDailyCockpitSnapshot, t]);

  const updateOpsChecklistStatus = useCallback(
    async (templateKey: string, action: "complete" | "exception", note?: string) => {
      if (!canWriteCmdb) {
        setError(t("auth.messages.forbiddenAction"));
        return null;
      }
      if (!templateKey || templateKey.trim().length === 0) {
        return null;
      }

      const normalizedTemplateKey = templateKey.trim();
      const actionKey = `${normalizedTemplateKey}:${action}`;
      setRunningOpsChecklistActionKey(actionKey);
      setError(null);
      setOpsChecklistNotice(null);

      try {
        const response = await apiFetch(
          `${API_BASE_URL}/api/v1/ops/cockpit/checklists/${encodeURIComponent(normalizedTemplateKey)}/${action}`,
          {
            method: "POST",
            headers: {
              "Content-Type": "application/json"
            },
            body: JSON.stringify({
              date: opsChecklistDate.trim().length > 0 ? opsChecklistDate.trim() : null,
              site: trimToNull(dailyCockpitSiteFilter),
              department: trimToNull(dailyCockpitDepartmentFilter),
              note: trimToNull(note ?? ""),
              mark_skipped: action === "exception"
            })
          }
        );
        if (!response.ok) {
          throw new Error(await readErrorMessage(response));
        }
        const payload: OpsChecklistUpdateResponse = await response.json();
        await Promise.all([loadOpsChecklist(), loadDailyCockpitQueue()]);
        setOpsChecklistNotice(
          t(
            action === "complete"
              ? "cmdb.dailyCockpit.checklist.messages.completeDone"
              : "cmdb.dailyCockpit.checklist.messages.exceptionDone",
            {
              key: payload.template_key
            }
          )
        );
        return payload;
      } catch (err) {
        setError(err instanceof Error ? err.message : "unknown error");
        return null;
      } finally {
        setRunningOpsChecklistActionKey(null);
      }
    },
    [
      canWriteCmdb,
      dailyCockpitDepartmentFilter,
      dailyCockpitSiteFilter,
      loadDailyCockpitQueue,
      loadOpsChecklist,
      opsChecklistDate,
      t
    ]
  );

  const completeOpsChecklistItem = useCallback(
    async (templateKey: string) => updateOpsChecklistStatus(templateKey, "complete"),
    [updateOpsChecklistStatus]
  );

  const recordOpsChecklistException = useCallback(
    async (templateKey: string) => {
      const note = typeof window !== "undefined"
        ? window.prompt(t("cmdb.dailyCockpit.checklist.messages.exceptionPrompt"), "")
        : null;
      if (note === null || note.trim().length === 0) {
        return null;
      }
      return updateOpsChecklistStatus(templateKey, "exception", note);
    },
    [t, updateOpsChecklistStatus]
  );

  const loadTicketEscalationPolicy = useCallback(async () => {
    setLoadingTicketEscalationPolicy(true);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/tickets/escalation/policy`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: TicketEscalationPolicyRecord = await response.json();
      setTicketEscalationPolicy(payload);
      setTicketEscalationPolicyDraft((prev) => ({
        ...buildTicketEscalationPolicyDraft(payload),
        note: prev.note
      }));
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setTicketEscalationPolicy(null);
      return null;
    } finally {
      setLoadingTicketEscalationPolicy(false);
    }
  }, []);

  const loadTicketEscalationQueue = useCallback(async () => {
    setLoadingTicketEscalationQueue(true);
    setError(null);
    try {
      const params = new URLSearchParams({
        limit: "120"
      });
      const response = await apiFetch(`${API_BASE_URL}/api/v1/tickets/escalation/queue?${params.toString()}`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: TicketListResponse = await response.json();
      setTicketEscalationQueue(payload.items);
      return payload.items;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setTicketEscalationQueue([]);
      return [];
    } finally {
      setLoadingTicketEscalationQueue(false);
    }
  }, []);

  const loadTicketEscalationActions = useCallback(async (ticketId?: number | null) => {
    setLoadingTicketEscalationActions(true);
    setError(null);
    try {
      const params = new URLSearchParams({
        limit: "60"
      });
      if (typeof ticketId === "number" && Number.isFinite(ticketId) && ticketId > 0) {
        params.set("ticket_id", String(ticketId));
      }

      const response = await apiFetch(`${API_BASE_URL}/api/v1/tickets/escalation/actions?${params.toString()}`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: TicketEscalationActionsResponse = await response.json();
      setTicketEscalationActions(payload.items);
      return payload.items;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setTicketEscalationActions([]);
      return [];
    } finally {
      setLoadingTicketEscalationActions(false);
    }
  }, []);

  const reloadTicketList = useCallback(async () => {
    setLoadingTickets(true);
    setError(null);
    try {
      const params = new URLSearchParams({
        limit: "120"
      });
      if (ticketStatusFilter !== "all") {
        params.set("status", ticketStatusFilter);
      }
      if (ticketPriorityFilter !== "all") {
        params.set("priority", ticketPriorityFilter);
      }
      if (ticketQueryFilter.trim().length > 0) {
        params.set("query", ticketQueryFilter.trim());
      }

      const response = await apiFetch(`${API_BASE_URL}/api/v1/tickets?${params.toString()}`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: TicketListResponse = await response.json();
      setTickets(payload.items);
      return payload.items;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setTickets([]);
      return [];
    } finally {
      setLoadingTickets(false);
    }
  }, [ticketPriorityFilter, ticketQueryFilter, ticketStatusFilter]);

  const updateTicketEscalationPolicy = useCallback(async () => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return null;
    }

    const parsePositiveInt = (label: string, value: string): number | null => {
      const parsed = Number.parseInt(value.trim(), 10);
      if (!Number.isFinite(parsed) || parsed <= 0) {
        setError(`${label} must be a positive integer.`);
        return null;
      }
      return parsed;
    };
    const validateWindow = (label: string, near: number, breach: number): boolean => {
      if (near >= breach) {
        setError(`${label} near window must be less than breach window.`);
        return false;
      }
      return true;
    };

    const nearCritical = parsePositiveInt("Critical near minutes", ticketEscalationPolicyDraft.near_critical_minutes);
    const breachCritical = parsePositiveInt("Critical breach minutes", ticketEscalationPolicyDraft.breach_critical_minutes);
    const nearHigh = parsePositiveInt("High near minutes", ticketEscalationPolicyDraft.near_high_minutes);
    const breachHigh = parsePositiveInt("High breach minutes", ticketEscalationPolicyDraft.breach_high_minutes);
    const nearMedium = parsePositiveInt("Medium near minutes", ticketEscalationPolicyDraft.near_medium_minutes);
    const breachMedium = parsePositiveInt("Medium breach minutes", ticketEscalationPolicyDraft.breach_medium_minutes);
    const nearLow = parsePositiveInt("Low near minutes", ticketEscalationPolicyDraft.near_low_minutes);
    const breachLow = parsePositiveInt("Low breach minutes", ticketEscalationPolicyDraft.breach_low_minutes);
    if (
      nearCritical === null
      || breachCritical === null
      || nearHigh === null
      || breachHigh === null
      || nearMedium === null
      || breachMedium === null
      || nearLow === null
      || breachLow === null
    ) {
      return null;
    }
    if (
      !validateWindow("Critical", nearCritical, breachCritical)
      || !validateWindow("High", nearHigh, breachHigh)
      || !validateWindow("Medium", nearMedium, breachMedium)
      || !validateWindow("Low", nearLow, breachLow)
    ) {
      return null;
    }

    const escalateToAssignee = ticketEscalationPolicyDraft.escalate_to_assignee.trim();
    if (!escalateToAssignee) {
      setError("Escalation owner is required.");
      return null;
    }

    setSavingTicketEscalationPolicy(true);
    setTicketNotice(null);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/tickets/escalation/policy`, {
        method: "PUT",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          name: ticketEscalationPolicyDraft.name.trim() || "Default Ticket SLA Policy",
          is_enabled: ticketEscalationPolicyDraft.is_enabled,
          near_critical_minutes: nearCritical,
          breach_critical_minutes: breachCritical,
          near_high_minutes: nearHigh,
          breach_high_minutes: breachHigh,
          near_medium_minutes: nearMedium,
          breach_medium_minutes: breachMedium,
          near_low_minutes: nearLow,
          breach_low_minutes: breachLow,
          escalate_to_assignee: escalateToAssignee,
          note: trimToNull(ticketEscalationPolicyDraft.note)
        })
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: TicketEscalationPolicyRecord = await response.json();
      setTicketEscalationPolicy(payload);
      setTicketEscalationPolicyDraft((prev) => ({
        ...buildTicketEscalationPolicyDraft(payload),
        note: prev.note
      }));
      await Promise.all([reloadTicketList(), loadTicketEscalationQueue()]);
      setTicketNotice(`Escalation policy '${payload.policy_key}' updated.`);
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      return null;
    } finally {
      setSavingTicketEscalationPolicy(false);
    }
  }, [canWriteCmdb, loadTicketEscalationQueue, reloadTicketList, t, ticketEscalationPolicyDraft]);

  const previewTicketEscalationPolicy = useCallback(async () => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return null;
    }

    const ageMinutes = Number.parseInt(ticketEscalationPreviewDraft.ticket_age_minutes.trim(), 10);
    if (!Number.isFinite(ageMinutes) || ageMinutes < 0) {
      setError("Preview ticket age must be a non-negative integer.");
      return null;
    }

    setPreviewingTicketEscalationPolicy(true);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/tickets/escalation/policy/preview`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          priority: ticketEscalationPreviewDraft.priority,
          status: ticketEscalationPreviewDraft.status,
          ticket_age_minutes: ageMinutes,
          current_assignee: trimToNull(ticketEscalationPreviewDraft.current_assignee)
        })
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: TicketEscalationPreviewResponse = await response.json();
      setTicketEscalationPreview(payload);
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setTicketEscalationPreview(null);
      return null;
    } finally {
      setPreviewingTicketEscalationPolicy(false);
    }
  }, [canWriteCmdb, t, ticketEscalationPreviewDraft]);

  const runTicketEscalation = useCallback(async (dryRun: boolean) => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return null;
    }

    setRunningTicketEscalation(true);
    setTicketNotice(null);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/tickets/escalation/run`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          dry_run: dryRun,
          note: trimToNull(ticketEscalationRunNote)
        })
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: TicketEscalationRunResponse = await response.json();
      setTicketEscalationRunResponse(payload);
      await Promise.all([reloadTicketList(), loadTicketEscalationQueue()]);
      const selectedId = Number.parseInt(selectedTicketId, 10);
      if (Number.isFinite(selectedId) && selectedId > 0) {
        const detailResponse = await apiFetch(`${API_BASE_URL}/api/v1/tickets/${selectedId}`);
        if (detailResponse.ok) {
          const detailPayload: TicketDetailResponse = await detailResponse.json();
          setTicketDetail(detailPayload);
          setTicketStatusDraft(detailPayload.ticket.status);
        }
        await loadTicketEscalationActions(selectedId);
      }
      setTicketNotice(
        `Escalation ${dryRun ? "dry-run" : "run"} completed: processed ${payload.processed}, escalated ${payload.escalated}, skipped ${payload.skipped}.`
      );
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      return null;
    } finally {
      setRunningTicketEscalation(false);
    }
  }, [
    canWriteCmdb,
    loadTicketEscalationActions,
    loadTicketEscalationQueue,
    reloadTicketList,
    selectedTicketId,
    t,
    ticketEscalationRunNote
  ]);

  const loadTickets = reloadTicketList;

  const loadTicketDetail = useCallback(async (ticketId: number) => {
    if (!Number.isFinite(ticketId) || ticketId <= 0) {
      setTicketDetail(null);
      return null;
    }

    setLoadingTicketDetail(true);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/tickets/${ticketId}`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: TicketDetailResponse = await response.json();
      setTicketDetail(payload);
      setTicketStatusDraft(payload.ticket.status);
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setTicketDetail(null);
      return null;
    } finally {
      setLoadingTicketDetail(false);
    }
  }, []);

  const loadMonitoringSources = useCallback(
    async (filters: MonitoringSourceFilterForm = defaultMonitoringSourceFilters) => {
      const activeFilters = filters;
      const params = new URLSearchParams();
      if (activeFilters.source_type.trim().length > 0) {
        params.set("source_type", activeFilters.source_type.trim());
      }
      if (activeFilters.site.trim().length > 0) {
        params.set("site", activeFilters.site.trim());
      }
      if (activeFilters.department.trim().length > 0) {
        params.set("department", activeFilters.department.trim());
      }
      if (activeFilters.is_enabled !== "all") {
        params.set("is_enabled", activeFilters.is_enabled);
      }

      setLoadingMonitoringSources(true);
      setError(null);
      try {
        const response = await apiFetch(`${API_BASE_URL}/api/v1/monitoring/sources?${params.toString()}`);
        if (!response.ok) {
          throw new Error(await readErrorMessage(response));
        }
        const payload: MonitoringSource[] = await response.json();
        setMonitoringSources(payload);
      } catch (err) {
        setError(err instanceof Error ? err.message : "unknown error");
      } finally {
        setLoadingMonitoringSources(false);
      }
    },
    []
  );

  const loadMonitoringOverview = useCallback(async (department?: string) => {
    setLoadingMonitoringOverview(true);
    setError(null);
    try {
      const params = new URLSearchParams();
      if (department && department !== "all") {
        params.set("department", department);
      }
      const query = params.toString();
      const response = await apiFetch(`${API_BASE_URL}/api/v1/monitoring/overview${query ? `?${query}` : ""}`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: MonitoringOverviewResponse = await response.json();
      setMonitoringOverview(payload);
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setMonitoringOverview(null);
    } finally {
      setLoadingMonitoringOverview(false);
    }
  }, []);

  const buildSetupTemplateDraft = useCallback((template: SetupTemplateCatalogItem | null) => {
    if (!template) {
      return {};
    }

    const draft: Record<string, string> = {};
    for (const field of template.param_schema.fields ?? []) {
      if (typeof field.key !== "string" || field.key.trim().length === 0) {
        continue;
      }
      draft[field.key] = typeof field.default === "string" ? field.default : "";
    }
    return draft;
  }, []);

  const loadSetupTemplates = useCallback(async () => {
    setLoadingSetupTemplates(true);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/setup/templates`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: SetupTemplateCatalogResponse = await response.json();
      setSetupTemplates(payload.items);
      return payload.items;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setSetupTemplates([]);
      return [];
    } finally {
      setLoadingSetupTemplates(false);
    }
  }, []);

  const loadSetupProfiles = useCallback(async () => {
    setLoadingSetupProfiles(true);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/setup/profiles`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: SetupProfileCatalogResponse = await response.json();
      setSetupProfiles(payload.items);
      return payload.items;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setSetupProfiles([]);
      return [];
    } finally {
      setLoadingSetupProfiles(false);
    }
  }, []);

  const loadSetupProfileHistory = useCallback(async () => {
    setLoadingSetupProfileHistory(true);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/setup/profiles/history?limit=20&offset=0`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: SetupProfileHistoryResponse = await response.json();
      setSetupProfileHistory(payload.items);
      return payload.items;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setSetupProfileHistory([]);
      return [];
    } finally {
      setLoadingSetupProfileHistory(false);
    }
  }, []);

  const loadSetupPreflight = useCallback(async () => {
    setLoadingSetupPreflight(true);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/setup/preflight`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: SetupChecklistResponse = await response.json();
      setSetupPreflight(payload);
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setSetupPreflight(null);
      return null;
    } finally {
      setLoadingSetupPreflight(false);
    }
  }, []);

  const loadSetupChecklist = useCallback(async () => {
    setLoadingSetupChecklist(true);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/setup/checklist`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: SetupChecklistResponse = await response.json();
      setSetupChecklist(payload);
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setSetupChecklist(null);
      return null;
    } finally {
      setLoadingSetupChecklist(false);
    }
  }, []);

  const refreshSetupWizard = useCallback(async () => {
    setSetupNotice(null);
    setSetupTemplateNotice(null);
    setSetupProfileNotice(null);
    await Promise.all([
      loadSetupPreflight(),
      loadSetupChecklist(),
      loadSetupTemplates(),
      loadSetupProfiles(),
      loadSetupProfileHistory()
    ]);
  }, [
    loadSetupChecklist,
    loadSetupPreflight,
    loadSetupProfileHistory,
    loadSetupProfiles,
    loadSetupTemplates
  ]);

  const completeSetupWizard = useCallback(async () => {
    const blockingChecks = [...(setupPreflight?.checks ?? []), ...(setupChecklist?.checks ?? [])]
      .filter((item) => item.critical && item.status === "fail");

    if (blockingChecks.length > 0) {
      setError(t("setupWizard.messages.completeBlocked", { count: blockingChecks.length }));
      return;
    }

    setSetupCompleted(true);
    setSetupNotice(t("setupWizard.messages.completed"));
  }, [setupChecklist?.checks, setupPreflight?.checks, t]);

  const setSetupTemplateParam = useCallback((key: string, value: string) => {
    setSetupTemplateParamsDraft((prev) => ({
      ...prev,
      [key]: value
    }));
  }, []);

  const previewSetupTemplate = useCallback(async () => {
    if (!selectedSetupTemplateKey) {
      return null;
    }

    setRunningSetupTemplatePreview(true);
    setSetupTemplateNotice(null);
    setError(null);
    setSetupTemplateApplyResult(null);

    const params = Object.entries(setupTemplateParamsDraft).reduce<Record<string, string>>((acc, [key, rawValue]) => {
      const value = rawValue.trim();
      if (value.length > 0) {
        acc[key] = value;
      }
      return acc;
    }, {});

    try {
      const response = await apiFetch(
        `${API_BASE_URL}/api/v1/setup/templates/${encodeURIComponent(selectedSetupTemplateKey)}/preview`,
        {
          method: "POST",
          headers: {
            "Content-Type": "application/json"
          },
          body: JSON.stringify({
            params,
            note: trimToNull(setupTemplateNote)
          })
        }
      );
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }

      const payload: SetupTemplatePreviewResponse = await response.json();
      setSetupTemplatePreview(payload);
      setSetupTemplateNotice(
        payload.ready
          ? t("setupWizard.templates.messages.previewReady", { count: payload.actions.length })
          : t("setupWizard.templates.messages.previewValidationFailed", {
            count: payload.validation_errors.length
          })
      );
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setSetupTemplatePreview(null);
      return null;
    } finally {
      setRunningSetupTemplatePreview(false);
    }
  }, [selectedSetupTemplateKey, setupTemplateNote, setupTemplateParamsDraft, t]);

  const applySetupTemplate = useCallback(async () => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return null;
    }
    if (!selectedSetupTemplateKey) {
      return null;
    }

    let preview = setupTemplatePreview;
    if (!preview || preview.template.key !== selectedSetupTemplateKey || !preview.ready) {
      preview = await previewSetupTemplate();
      if (!preview || !preview.ready) {
        return null;
      }
    }

    setRunningSetupTemplateApply(true);
    setSetupTemplateNotice(null);
    setError(null);

    const params = Object.entries(setupTemplateParamsDraft).reduce<Record<string, string>>((acc, [key, rawValue]) => {
      const value = rawValue.trim();
      if (value.length > 0) {
        acc[key] = value;
      }
      return acc;
    }, {});

    try {
      const response = await apiFetch(
        `${API_BASE_URL}/api/v1/setup/templates/${encodeURIComponent(selectedSetupTemplateKey)}/apply`,
        {
          method: "POST",
          headers: {
            "Content-Type": "application/json"
          },
          body: JSON.stringify({
            params,
            note: trimToNull(setupTemplateNote)
          })
        }
      );
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }

      const payload: SetupTemplateApplyResponse = await response.json();
      setSetupTemplateApplyResult(payload);
      setSetupTemplateNotice(t("setupWizard.templates.messages.applySuccess", { key: payload.template_key }));
      await Promise.all([loadSetupChecklist(), loadSetupPreflight()]);
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      return null;
    } finally {
      setRunningSetupTemplateApply(false);
    }
  }, [
    canWriteCmdb,
    loadSetupChecklist,
    loadSetupPreflight,
    previewSetupTemplate,
    selectedSetupTemplateKey,
    setupTemplateNote,
    setupTemplateParamsDraft,
    setupTemplatePreview,
    t
  ]);

  const previewSetupProfile = useCallback(async () => {
    if (!selectedSetupProfileKey) {
      return null;
    }

    setRunningSetupProfilePreview(true);
    setSetupProfileNotice(null);
    setError(null);
    setSetupProfileApplyResult(null);

    try {
      const response = await apiFetch(
        `${API_BASE_URL}/api/v1/setup/profiles/${encodeURIComponent(selectedSetupProfileKey)}/preview`,
        {
          method: "POST",
          headers: {
            "Content-Type": "application/json"
          },
          body: JSON.stringify({
            note: trimToNull(setupProfileNote)
          })
        }
      );
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }

      const payload: SetupProfilePreviewResponse = await response.json();
      setSetupProfilePreview(payload);
      const changedCount = payload.summary.filter((item) => item.changed).length;
      setSetupProfileNotice(
        changedCount > 0
          ? `Profile preview ready: ${changedCount} domain(s) will change.`
          : "Profile preview ready: no effective configuration drift detected."
      );
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setSetupProfilePreview(null);
      return null;
    } finally {
      setRunningSetupProfilePreview(false);
    }
  }, [selectedSetupProfileKey, setupProfileNote]);

  const applySetupProfile = useCallback(async () => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return null;
    }
    if (!selectedSetupProfileKey) {
      return null;
    }

    let preview = setupProfilePreview;
    if (!preview || preview.profile.key !== selectedSetupProfileKey || !preview.ready) {
      preview = await previewSetupProfile();
      if (!preview || !preview.ready) {
        return null;
      }
    }

    setRunningSetupProfileApply(true);
    setSetupProfileNotice(null);
    setError(null);

    try {
      const response = await apiFetch(
        `${API_BASE_URL}/api/v1/setup/profiles/${encodeURIComponent(selectedSetupProfileKey)}/apply`,
        {
          method: "POST",
          headers: {
            "Content-Type": "application/json"
          },
          body: JSON.stringify({
            note: trimToNull(setupProfileNote)
          })
        }
      );
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }

      const payload: SetupProfileApplyResponse = await response.json();
      setSetupProfileApplyResult(payload);
      setSetupProfileNotice(`Profile '${payload.profile_key}' applied (run #${payload.run_id}).`);
      await Promise.all([
        loadSetupChecklist(),
        loadSetupPreflight(),
        loadSetupProfileHistory()
      ]);
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      return null;
    } finally {
      setRunningSetupProfileApply(false);
    }
  }, [
    canWriteCmdb,
    loadSetupChecklist,
    loadSetupPreflight,
    loadSetupProfileHistory,
    previewSetupProfile,
    selectedSetupProfileKey,
    setupProfileNote,
    setupProfilePreview,
    t
  ]);

  const revertSetupProfileRun = useCallback(async (runId: number) => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return null;
    }
    if (!Number.isFinite(runId) || runId <= 0) {
      return null;
    }

    setRunningSetupProfileRevertId(runId);
    setSetupProfileNotice(null);
    setError(null);
    try {
      const response = await apiFetch(
        `${API_BASE_URL}/api/v1/setup/profiles/history/${runId}/revert`,
        {
          method: "POST",
          headers: {
            "Content-Type": "application/json"
          },
          body: JSON.stringify({
            note: trimToNull(setupProfileNote)
          })
        }
      );
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: SetupProfileRevertResponse = await response.json();
      setSetupProfileNotice(
        `Profile run #${payload.run_id} reverted by ${payload.reverted_by} at ${new Date(payload.reverted_at).toLocaleString()}.`
      );
      await Promise.all([
        loadSetupChecklist(),
        loadSetupPreflight(),
        loadSetupProfileHistory()
      ]);
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      return null;
    } finally {
      setRunningSetupProfileRevertId(null);
    }
  }, [
    canWriteCmdb,
    loadSetupChecklist,
    loadSetupPreflight,
    loadSetupProfileHistory,
    setupProfileNote,
    t
  ]);

  const loadAlerts = useCallback(async () => {
    setLoadingAlerts(true);
    setError(null);
    try {
      const params = new URLSearchParams({ limit: "120" });
      if (alertFilters.status !== "all") {
        params.set("status", alertFilters.status);
      }
      if (alertFilters.severity !== "all") {
        params.set("severity", alertFilters.severity);
      }
      if (alertFilters.suppressed !== "all") {
        params.set("suppressed", alertFilters.suppressed);
      }
      if (alertFilters.site.trim().length > 0) {
        params.set("site", alertFilters.site.trim());
      }
      if (alertFilters.query.trim().length > 0) {
        params.set("query", alertFilters.query.trim());
      }

      const response = await apiFetch(`${API_BASE_URL}/api/v1/alerts?${params.toString()}`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: AlertListResponse = await response.json();
      setAlerts(payload.items);
      setAlertsTotal(payload.total);
      return payload.items;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setAlerts([]);
      setAlertsTotal(0);
      return [];
    } finally {
      setLoadingAlerts(false);
    }
  }, [alertFilters]);

  const loadAlertDetail = useCallback(async (alertId: number) => {
    if (!Number.isFinite(alertId) || alertId <= 0) {
      setAlertDetail(null);
      return null;
    }

    setLoadingAlertDetail(true);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/alerts/${alertId}`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: AlertDetailResponse = await response.json();
      setAlertDetail(payload);
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setAlertDetail(null);
      return null;
    } finally {
      setLoadingAlertDetail(false);
    }
  }, []);

  const loadAlertPolicies = useCallback(async () => {
    setLoadingAlertPolicies(true);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/alerts/policies`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: AlertTicketPolicyListResponse = await response.json();
      setAlertPolicies(payload.items);
      return payload.items;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setAlertPolicies([]);
      return [];
    } finally {
      setLoadingAlertPolicies(false);
    }
  }, []);

  const previewAlertPolicy = useCallback(async () => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }

    const dedupWindowSeconds = Number.parseInt(alertPolicyDraft.dedup_window_seconds.trim(), 10);
    if (!Number.isFinite(dedupWindowSeconds) || dedupWindowSeconds <= 0) {
      setError("Dedup window must be a positive integer.");
      return;
    }

    setPreviewingAlertPolicy(true);
    setAlertPolicyNotice(null);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/alerts/policies/preview`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          match_source: trimToNull(alertPolicyDraft.match_source),
          match_severity: alertPolicyDraft.match_severity === "all" ? null : alertPolicyDraft.match_severity,
          match_status: alertPolicyDraft.match_status === "all" ? null : alertPolicyDraft.match_status,
          dedup_window_seconds: dedupWindowSeconds
        })
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: AlertPolicyPreviewResponse = await response.json();
      setAlertPolicyPreview(payload);
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setAlertPolicyPreview(null);
      return null;
    } finally {
      setPreviewingAlertPolicy(false);
    }
  }, [alertPolicyDraft, canWriteCmdb, t]);

  const createAlertPolicy = useCallback(async () => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }

    const name = alertPolicyDraft.name.trim();
    if (!name) {
      setError("Policy name is required.");
      return;
    }

    const autoPolicyKey = name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "");
    const policyKey = (alertPolicyDraft.policy_key.trim() || autoPolicyKey).slice(0, 64);
    if (!policyKey) {
      setError("Policy key is required.");
      return;
    }

    const dedupWindowSeconds = Number.parseInt(alertPolicyDraft.dedup_window_seconds.trim(), 10);
    if (!Number.isFinite(dedupWindowSeconds) || dedupWindowSeconds <= 0) {
      setError("Dedup window must be a positive integer.");
      return;
    }

    setCreatingAlertPolicy(true);
    setAlertPolicyNotice(null);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/alerts/policies`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          policy_key: policyKey,
          name,
          description: trimToNull(alertPolicyDraft.description),
          is_enabled: alertPolicyDraft.is_enabled,
          match_source: trimToNull(alertPolicyDraft.match_source),
          match_severity: alertPolicyDraft.match_severity === "all" ? null : alertPolicyDraft.match_severity,
          match_status: alertPolicyDraft.match_status === "all" ? null : alertPolicyDraft.match_status,
          dedup_window_seconds: dedupWindowSeconds,
          ticket_priority: alertPolicyDraft.ticket_priority,
          ticket_category: trimToNull(alertPolicyDraft.ticket_category) ?? "incident"
        })
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      setAlertPolicyDraft((prev) => ({ ...defaultAlertPolicyForm, match_source: prev.match_source }));
      setAlertPolicyPreview(null);
      await loadAlertPolicies();
      setAlertPolicyNotice(`Policy '${policyKey}' created.`);
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setCreatingAlertPolicy(false);
    }
  }, [alertPolicyDraft, canWriteCmdb, loadAlertPolicies, t]);

  const toggleAlertPolicyEnabled = useCallback(async (policy: AlertTicketPolicyRecord) => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }

    setUpdatingAlertPolicyId(policy.id);
    setAlertPolicyNotice(null);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/alerts/policies/${policy.id}`, {
        method: "PATCH",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          is_enabled: !policy.is_enabled
        })
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      await loadAlertPolicies();
      setAlertPolicyNotice(
        `Policy '${policy.policy_key}' ${policy.is_enabled ? "disabled" : "enabled"}.`
      );
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setUpdatingAlertPolicyId(null);
    }
  }, [canWriteCmdb, loadAlertPolicies, t]);

  const toggleAlertSelection = useCallback((alertId: number, focus?: boolean) => {
    if (!Number.isFinite(alertId) || alertId <= 0) {
      return;
    }
    setSelectedAlertIds((prev) => {
      if (prev.includes(alertId)) {
        return prev.filter((item) => item !== alertId);
      }
      return [...prev, alertId];
    });
    if (focus) {
      setSelectedAlertId(String(alertId));
    }
  }, []);

  const toggleSelectAllAlerts = useCallback(() => {
    setSelectedAlertIds((prev) => {
      const ids = alerts.map((item) => item.id);
      if (ids.length === 0) {
        return [];
      }
      const allSelected = ids.every((id) => prev.includes(id));
      return allSelected ? [] : ids;
    });

    if (!selectedAlertId && alerts.length > 0) {
      setSelectedAlertId(String(alerts[0].id));
    }
  }, [alerts, selectedAlertId]);

  const triggerSingleAcknowledge = useCallback(async (alertId: number) => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }
    if (!Number.isFinite(alertId) || alertId <= 0) {
      return;
    }

    setAlertActionRunningId(alertId);
    setAlertNotice(null);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/alerts/${alertId}/ack`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          note: "acknowledged from web console"
        })
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      setSelectedAlertId(String(alertId));
      await Promise.all([loadAlerts(), loadAlertDetail(alertId)]);
      setAlertNotice(t("alertsCenter.messages.ackSuccess", { id: alertId }));
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setAlertActionRunningId(null);
    }
  }, [canWriteCmdb, loadAlertDetail, loadAlerts, t]);

  const closeAlert = useCallback(async (alertId: number) => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }
    if (!Number.isFinite(alertId) || alertId <= 0) {
      return;
    }

    setAlertActionRunningId(alertId);
    setAlertNotice(null);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/alerts/${alertId}/close`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          note: "closed from web console"
        })
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      setSelectedAlertId(String(alertId));
      await Promise.all([loadAlerts(), loadAlertDetail(alertId)]);
      setAlertNotice(t("alertsCenter.messages.closeSuccess", { id: alertId }));
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setAlertActionRunningId(null);
    }
  }, [canWriteCmdb, loadAlertDetail, loadAlerts, t]);

  const triggerAlertRemediation = useCallback(async (alertId: number) => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }
    if (!Number.isFinite(alertId) || alertId <= 0) {
      return;
    }

    setAlertActionRunningId(alertId);
    setAlertNotice(null);
    setError(null);

    try {
      const remediationResponse = await apiFetch(`${API_BASE_URL}/api/v1/alerts/${alertId}/remediation`);
      if (!remediationResponse.ok) {
        throw new Error(await readErrorMessage(remediationResponse));
      }
      const remediationPlan: AlertRemediationPlanResponse = await remediationResponse.json();

      const assetRef = typeof remediationPlan.params.asset_ref === "string"
        ? remediationPlan.params.asset_ref.trim()
        : "";
      const dryRunResponse = await apiFetch(
        `${API_BASE_URL}/api/v1/workflow/playbooks/${encodeURIComponent(remediationPlan.playbook_key)}/dry-run`,
        {
          method: "POST",
          headers: {
            "Content-Type": "application/json"
          },
          body: JSON.stringify({
            params: remediationPlan.params,
            asset_ref: assetRef.length > 0 ? assetRef : null,
            related_alert_id: alertId
          })
        }
      );
      if (!dryRunResponse.ok) {
        throw new Error(await readErrorMessage(dryRunResponse));
      }
      const dryRunPayload: PlaybookDryRunResponse = await dryRunResponse.json();

      let executePayload: Record<string, unknown> = {
        params: remediationPlan.params,
        asset_ref: assetRef.length > 0 ? assetRef : null,
        related_alert_id: alertId
      };

      if (dryRunPayload.risk_summary.requires_confirmation) {
        const approvalResponse = await apiFetch(
          `${API_BASE_URL}/api/v1/workflow/playbooks/${encodeURIComponent(remediationPlan.playbook_key)}/approval-request`,
          {
            method: "POST",
            headers: {
              "Content-Type": "application/json"
            },
            body: JSON.stringify({
              dry_run_id: dryRunPayload.execution.id,
              note: `requested from alert remediation #${alertId}`
            })
          }
        );
        if (!approvalResponse.ok) {
          throw new Error(await readErrorMessage(approvalResponse));
        }
        const approvalPayload: PlaybookApprovalRequestRecord = await approvalResponse.json();

        setSelectedPlaybookKey(remediationPlan.playbook_key);
        setPlaybookDryRunResponse(dryRunPayload);
        setPlaybookConfirmationToken(dryRunPayload.confirmation?.token ?? "");
        setSelectedPlaybookApprovalId(String(approvalPayload.id));
        setPlaybookApprovalToken(approvalPayload.approval_token ?? "");

        setSelectedAlertId(String(alertId));
        await Promise.all([
          loadAlerts(),
          loadAlertDetail(alertId),
          loadDailyCockpitSnapshot(),
          loadPlaybookApprovalRequests()
        ]);
        setAlertNotice(
          `High-risk remediation requires two-person approval. Request #${approvalPayload.id} is pending in playbook queue.`
        );
        return;
      }

      const executeResponse = await apiFetch(
        `${API_BASE_URL}/api/v1/workflow/playbooks/${encodeURIComponent(remediationPlan.playbook_key)}/execute`,
        {
          method: "POST",
          headers: {
            "Content-Type": "application/json"
          },
          body: JSON.stringify(executePayload)
        }
      );
      if (!executeResponse.ok) {
        throw new Error(await readErrorMessage(executeResponse));
      }
      const executeResult: PlaybookExecutionDetail = await executeResponse.json();

      setSelectedAlertId(String(alertId));
      await Promise.all([
        loadAlerts(),
        loadAlertDetail(alertId),
        loadDailyCockpitSnapshot()
      ]);
      setAlertNotice(
        t("alertsCenter.messages.remediationSuccess", {
          key: remediationPlan.playbook_key,
          executionId: executeResult.id
        })
      );
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setAlertActionRunningId(null);
    }
  }, [
    canWriteCmdb,
    loadAlertDetail,
    loadAlerts,
    loadDailyCockpitSnapshot,
    loadPlaybookApprovalRequests,
    t
  ]);

  const runBulkAlertAction = useCallback(
    async (action: "ack" | "close") => {
      if (!canWriteCmdb) {
        setError(t("auth.messages.forbiddenAction"));
        return;
      }

      const ids = selectedAlertIds.filter((id) => Number.isFinite(id) && id > 0);
      if (ids.length === 0) {
        return;
      }

      setAlertBulkActionRunning(action);
      setAlertNotice(null);
      setError(null);

      try {
        const endpoint = action === "ack" ? "bulk/ack" : "bulk/close";
        const response = await apiFetch(`${API_BASE_URL}/api/v1/alerts/${endpoint}`, {
          method: "POST",
          headers: {
            "Content-Type": "application/json"
          },
          body: JSON.stringify({
            ids,
            note: action === "ack" ? "bulk acknowledge from web console" : "bulk close from web console"
          })
        });
        if (!response.ok) {
          throw new Error(await readErrorMessage(response));
        }

        const payload: AlertBulkActionResponse = await response.json();
        const selectedIdNumber = Number.parseInt(selectedAlertId, 10);
        await Promise.all([
          loadAlerts(),
          Number.isFinite(selectedIdNumber) && selectedIdNumber > 0
            ? loadAlertDetail(selectedIdNumber)
            : Promise.resolve(null)
        ]);
        setSelectedAlertIds([]);
        setAlertNotice(
          t(
            action === "ack"
              ? "alertsCenter.messages.bulkAckSuccess"
              : "alertsCenter.messages.bulkCloseSuccess",
            { updated: payload.updated, skipped: payload.skipped }
          )
        );
      } catch (err) {
        setError(err instanceof Error ? err.message : "unknown error");
      } finally {
        setAlertBulkActionRunning(null);
      }
    },
    [canWriteCmdb, loadAlertDetail, loadAlerts, selectedAlertId, selectedAlertIds, t]
  );

  const triggerBulkAcknowledge = useCallback(async () => {
    await runBulkAlertAction("ack");
  }, [runBulkAlertAction]);

  const triggerBulkClose = useCallback(async () => {
    await runBulkAlertAction("close");
  }, [runBulkAlertAction]);

  const loadMonitoringMetrics = useCallback(async (assetId: number, windowMinutes: number) => {
    if (!Number.isFinite(assetId) || assetId <= 0) {
      setMonitoringMetrics(null);
      setMonitoringMetricsError(null);
      return;
    }

    setLoadingMonitoringMetrics(true);
    setMonitoringMetricsError(null);
    try {
      const params = new URLSearchParams({
        asset_id: String(assetId),
        window_minutes: String(windowMinutes)
      });
      const response = await apiFetch(`${API_BASE_URL}/api/v1/monitoring/metrics?${params.toString()}`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: MonitoringMetricsResponse = await response.json();
      setMonitoringMetrics(payload);
    } catch (err) {
      setMonitoringMetricsError(err instanceof Error ? err.message : "unknown error");
      setMonitoringMetrics(null);
    } finally {
      setLoadingMonitoringMetrics(false);
    }
  }, []);

  const createSampleAsset = useCallback(async () => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }

    setCreatingSample(true);
    setError(null);
    try {
      const customFields: Record<string, unknown> = {};
      for (const definition of fieldDefinitions) {
        if (definition.required && definition.is_enabled) {
          customFields[definition.field_key] = sampleValueForField(definition);
        }
      }

      const stamp = Date.now();
      const body = {
        asset_class: "server",
        name: `sample-${stamp}`,
        hostname: `sample-${stamp}.local`,
        ip: "10.0.0.10",
        status: "idle",
        site: "dc-a",
        department: "platform",
        owner: "ops",
        qr_code: `QR-${stamp}`,
        barcode: `BC-${stamp}`,
        custom_fields: customFields
      };

      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/assets`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify(body)
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      await Promise.all([loadAssets(), loadAssetStats()]);
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setCreatingSample(false);
    }
  }, [canWriteCmdb, fieldDefinitions, loadAssetStats, loadAssets, t]);

  const createFieldDefinition = useCallback(async () => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }

    setCreatingField(true);
    setError(null);
    try {
      const payload: Record<string, unknown> = {
        field_key: newField.field_key,
        name: newField.name,
        field_type: newField.field_type,
        required: newField.required,
        scanner_enabled: newField.scanner_enabled,
        is_enabled: true
      };

      if (newField.max_length.trim() !== "") {
        payload.max_length = Number(newField.max_length.trim());
      }

      if (newField.field_type === "enum") {
        payload.options = newField.options_csv
          .split(",")
          .map((item) => item.trim())
          .filter((item) => item.length > 0);
      }

      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/field-definitions`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify(payload)
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }

      setNewField(defaultFieldForm);
      await loadFieldDefinitions();
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setCreatingField(false);
    }
  }, [canWriteCmdb, newField, loadFieldDefinitions, t]);

  const createRelation = useCallback(async () => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }

    const srcAssetId = Number.parseInt(selectedAssetId, 10);
    const dstAssetId = Number.parseInt(newRelation.dst_asset_id, 10);
    if (!Number.isFinite(srcAssetId) || srcAssetId <= 0) {
      setError(t("cmdb.relations.messages.selectSource"));
      return;
    }
    if (!Number.isFinite(dstAssetId) || dstAssetId <= 0) {
      setError(t("cmdb.relations.messages.selectTarget"));
      return;
    }
    if (srcAssetId === dstAssetId) {
      setError(t("cmdb.relations.messages.sameAsset"));
      return;
    }

    setCreatingRelation(true);
    setRelationNotice(null);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/relations`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          src_asset_id: srcAssetId,
          dst_asset_id: dstAssetId,
          relation_type: newRelation.relation_type,
          source: newRelation.source
        })
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }

      setNewRelation((prev) => ({ ...prev, dst_asset_id: "" }));
      await loadRelations(srcAssetId);
      setRelationNotice(t("cmdb.relations.messages.created"));
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setCreatingRelation(false);
    }
  }, [canWriteCmdb, loadRelations, newRelation.dst_asset_id, newRelation.relation_type, newRelation.source, selectedAssetId, t]);

  const deleteRelation = useCallback(
    async (relationId: number) => {
      if (!canWriteCmdb) {
        setError(t("auth.messages.forbiddenAction"));
        return;
      }

      const srcAssetId = Number.parseInt(selectedAssetId, 10);
      if (!Number.isFinite(srcAssetId) || srcAssetId <= 0) {
        return;
      }

      if (typeof window !== "undefined") {
        const shouldDelete = window.confirm(t("cmdb.relations.messages.deleteConfirm", { id: relationId }));
        if (!shouldDelete) {
          return;
        }
      }

      setDeletingRelationId(relationId);
      setRelationNotice(null);
      setError(null);
      try {
        const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/relations/${relationId}`, {
          method: "DELETE"
        });
        if (!response.ok) {
          throw new Error(await readErrorMessage(response));
        }
        await loadRelations(srcAssetId);
        setRelationNotice(t("cmdb.relations.messages.deleted"));
      } catch (err) {
        setError(err instanceof Error ? err.message : "unknown error");
      } finally {
        setDeletingRelationId(null);
      }
    },
    [canWriteCmdb, loadRelations, selectedAssetId, t]
  );

  const addOwnerDraft = useCallback(() => {
    setBindingOwnerDrafts((prev) => [...prev, createOwnerDraft("team", "")]);
  }, []);

  const updateOwnerDraftType = useCallback((key: string, ownerType: OwnerType) => {
    setBindingOwnerDrafts((prev) =>
      prev.map((item) => (item.key === key ? { ...item, owner_type: ownerType } : item))
    );
  }, []);

  const updateOwnerDraftRef = useCallback((key: string, ownerRef: string) => {
    setBindingOwnerDrafts((prev) =>
      prev.map((item) => (item.key === key ? { ...item, owner_ref: ownerRef } : item))
    );
  }, []);

  const removeOwnerDraft = useCallback((key: string) => {
    setBindingOwnerDrafts((prev) => prev.filter((item) => item.key !== key));
  }, []);

  const saveAssetBindings = useCallback(async () => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }

    const assetId = Number.parseInt(selectedAssetId, 10);
    if (!Number.isFinite(assetId) || assetId <= 0) {
      setError(t("cmdb.assetDetail.messages.selectAsset"));
      return;
    }

    setUpdatingAssetBindings(true);
    setBindingNotice(null);
    setError(null);
    try {
      const owners = bindingOwnerDrafts
        .map((item) => ({
          owner_type: item.owner_type,
          owner_ref: item.owner_ref.trim()
        }))
        .filter((item) => item.owner_ref.length > 0);

      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/assets/${assetId}/bindings`, {
        method: "PUT",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          departments: parseBindingList(bindingDepartmentsInput),
          business_services: parseBindingList(bindingBusinessServicesInput),
          owners
        })
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: AssetBindingsResponse = await response.json();
      setAssetBindings(payload);
      applyBindingsToForm(payload);
      setBindingNotice(t("cmdb.assetDetail.messages.bindingsSaved"));
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setUpdatingAssetBindings(false);
    }
  }, [
    applyBindingsToForm,
    bindingBusinessServicesInput,
    bindingDepartmentsInput,
    bindingOwnerDrafts,
    canWriteCmdb,
    selectedAssetId,
    t
  ]);

  const transitionAssetLifecycle = useCallback(async (status: LifecycleStatus) => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }

    const assetId = Number.parseInt(selectedAssetId, 10);
    if (!Number.isFinite(assetId) || assetId <= 0) {
      setError(t("cmdb.assetDetail.messages.selectAsset"));
      return;
    }

    setTransitioningLifecycleStatus(status);
    setLifecycleNotice(null);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/assets/${assetId}/lifecycle`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({ status })
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }

      const payload: AssetLifecycleTransitionResponse = await response.json();
      setAssets((prev) =>
        prev.map((item) =>
          item.id === payload.asset_id
            ? {
                ...item,
                status: payload.status
              }
            : item
        )
      );
      setAssetBindings((prev) =>
        prev
          ? {
              ...prev,
              readiness: payload.readiness
            }
          : prev
      );
      await loadAssetStats();
      setLifecycleNotice(t("cmdb.assetDetail.messages.lifecycleChanged", { status: payload.status }));
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setTransitioningLifecycleStatus(null);
    }
  }, [canWriteCmdb, loadAssetStats, selectedAssetId, t]);

  const triggerAssetMonitoringSync = useCallback(async () => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }

    const assetId = Number.parseInt(selectedAssetId, 10);
    if (!Number.isFinite(assetId) || assetId <= 0) {
      setError(t("cmdb.assetDetail.messages.selectAsset"));
      return;
    }

    setTriggeringMonitoringSync(true);
    setMonitoringNotice(null);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/assets/${assetId}/monitoring-sync`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          reason: "manual sync from readiness panel"
        })
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: { job_id: number } = await response.json();
      await loadAssetMonitoring(assetId);
      setMonitoringNotice(t("cmdb.assetDetail.monitoring.messages.syncQueued", { id: payload.job_id }));
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setTriggeringMonitoringSync(false);
    }
  }, [canWriteCmdb, loadAssetMonitoring, selectedAssetId, t]);

  const refreshImpact = useCallback(async () => {
    const assetId = Number.parseInt(selectedAssetId, 10);
    if (!Number.isFinite(assetId) || assetId <= 0) {
      setError(t("cmdb.assetDetail.messages.selectAsset"));
      return;
    }

    const depth = parseImpactDepth(impactDepth);
    if (depth === null) {
      setError(t("cmdb.assetDetail.impact.messages.invalidDepth"));
      return;
    }

    const relationTypes = parseImpactRelationTypesInput(impactRelationTypesInput, defaultImpactRelationTypes);
    setImpactNotice(null);
    const payload = await loadAssetImpact(assetId, impactDirection, depth, relationTypes);
    if (payload) {
      setImpactNotice(
        t("cmdb.assetDetail.impact.messages.loaded", {
          nodes: payload.nodes.length,
          edges: payload.edges.length
        })
      );
    }
  }, [impactDepth, impactDirection, impactRelationTypesInput, loadAssetImpact, selectedAssetId, t]);

  const runDiscoveryJob = useCallback(async (jobId: number) => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }

    setRunningDiscoveryJobId(jobId);
    setDiscoveryNotice(null);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/discovery/jobs/${jobId}/run`, {
        method: "POST"
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      await Promise.all([loadDiscoveryJobs(), loadDiscoveryCandidates(), loadAssets(), loadAssetStats()]);
      setDiscoveryNotice(t("cmdb.discovery.messages.jobRunTriggered", { id: jobId }));
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setRunningDiscoveryJobId(null);
    }
  }, [canWriteCmdb, loadAssetStats, loadAssets, loadDiscoveryCandidates, loadDiscoveryJobs, t]);

  const reviewDiscoveryCandidate = useCallback(
    async (candidateId: number, action: "approve" | "reject") => {
      if (!canWriteCmdb) {
        setError(t("auth.messages.forbiddenAction"));
        return;
      }

      setReviewingCandidateId(candidateId);
      setDiscoveryNotice(null);
      setError(null);
      try {
        const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/discovery/candidates/${candidateId}/${action}`, {
          method: "POST",
          headers: {
            "Content-Type": "application/json"
          },
          body: JSON.stringify({ reviewed_by: "web-console" })
        });
        if (!response.ok) {
          throw new Error(await readErrorMessage(response));
        }
        await Promise.all([loadDiscoveryCandidates(), loadAssets(), loadAssetStats()]);
        setDiscoveryNotice(
          action === "approve"
            ? t("cmdb.discovery.messages.candidateApproved", { id: candidateId })
            : t("cmdb.discovery.messages.candidateRejected", { id: candidateId })
        );
      } catch (err) {
        setError(err instanceof Error ? err.message : "unknown error");
      } finally {
        setReviewingCandidateId(null);
      }
    },
    [canWriteCmdb, loadAssetStats, loadAssets, loadDiscoveryCandidates, t]
  );

  const createNotificationChannel = useCallback(async () => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }

    const name = newNotificationChannel.name.trim();
    const target = newNotificationChannel.target.trim();
    if (!name) {
      setError(t("cmdb.notifications.validation.channelNameRequired"));
      return;
    }
    if (!target) {
      setError(t("cmdb.notifications.validation.targetRequired"));
      return;
    }

    let config: Record<string, unknown> = {};
    const configRaw = newNotificationChannel.config_json.trim();
    if (configRaw.length > 0) {
      try {
        const parsed = JSON.parse(configRaw) as unknown;
        if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
          setError(t("cmdb.notifications.validation.configMustBeObject"));
          return;
        }
        config = parsed as Record<string, unknown>;
      } catch {
        setError(t("cmdb.notifications.validation.configInvalidJson"));
        return;
      }
    }

    setCreatingNotificationChannel(true);
    setNotificationNotice(null);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/discovery/notification-channels`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          name,
          channel_type: newNotificationChannel.channel_type,
          target,
          config
        })
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      setNewNotificationChannel(defaultNotificationChannelForm);
      await loadNotificationChannels();
      setNotificationNotice(t("cmdb.notifications.messages.channelCreated", { name }));
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setCreatingNotificationChannel(false);
    }
  }, [canWriteCmdb, loadNotificationChannels, newNotificationChannel, t]);

  const createNotificationTemplate = useCallback(async () => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }

    const eventType = newNotificationTemplate.event_type.trim();
    const titleTemplate = newNotificationTemplate.title_template.trim();
    const bodyTemplate = newNotificationTemplate.body_template.trim();
    if (!eventType) {
      setError(t("cmdb.notifications.validation.eventTypeRequired"));
      return;
    }
    if (!titleTemplate) {
      setError(t("cmdb.notifications.validation.templateTitleRequired"));
      return;
    }
    if (!bodyTemplate) {
      setError(t("cmdb.notifications.validation.templateBodyRequired"));
      return;
    }

    setCreatingNotificationTemplate(true);
    setNotificationNotice(null);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/discovery/notification-templates`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          event_type: eventType,
          title_template: titleTemplate,
          body_template: bodyTemplate
        })
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      setNewNotificationTemplate((prev) => ({ ...prev, title_template: "", body_template: "" }));
      await loadNotificationTemplates();
      setNotificationNotice(t("cmdb.notifications.messages.templateCreated", { eventType }));
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setCreatingNotificationTemplate(false);
    }
  }, [canWriteCmdb, loadNotificationTemplates, newNotificationTemplate, t]);

  const createNotificationSubscription = useCallback(async () => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }

    const channelId = Number.parseInt(newNotificationSubscription.channel_id, 10);
    const eventType = newNotificationSubscription.event_type.trim();
    if (!Number.isFinite(channelId) || channelId <= 0) {
      setError(t("cmdb.notifications.validation.channelRequired"));
      return;
    }
    if (!eventType) {
      setError(t("cmdb.notifications.validation.eventTypeRequired"));
      return;
    }

    setCreatingNotificationSubscription(true);
    setNotificationNotice(null);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/discovery/notification-subscriptions`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          channel_id: channelId,
          event_type: eventType,
          site: trimToNull(newNotificationSubscription.site),
          department: trimToNull(newNotificationSubscription.department)
        })
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      setNewNotificationSubscription((prev) => ({ ...prev, site: "", department: "" }));
      await loadNotificationSubscriptions();
      setNotificationNotice(t("cmdb.notifications.messages.subscriptionCreated", { eventType }));
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setCreatingNotificationSubscription(false);
    }
  }, [canWriteCmdb, loadNotificationSubscriptions, newNotificationSubscription, t]);

  const addWorkflowStepToDraft = useCallback(() => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }

    const stepId = newWorkflowStep.id.trim().toLowerCase();
    const stepName = newWorkflowStep.name.trim();
    if (!stepId || !/^[a-z0-9_-]+$/.test(stepId)) {
      setError(t("cmdb.workflow.validation.stepIdRequired"));
      return;
    }
    if (!stepName) {
      setError(t("cmdb.workflow.validation.stepNameRequired"));
      return;
    }
    if (newWorkflowTemplateSteps.some((item) => item.id === stepId)) {
      setError(t("cmdb.workflow.validation.stepIdDuplicate", { id: stepId }));
      return;
    }
    if (newWorkflowStep.kind === "script" && newWorkflowStep.script.trim().length === 0) {
      setError(t("cmdb.workflow.validation.stepScriptRequired"));
      return;
    }

    const parsedTimeout = Number.parseInt(newWorkflowStep.timeout_seconds.trim(), 10);
    const timeoutSeconds = Number.isFinite(parsedTimeout) && parsedTimeout > 0
      ? Math.min(parsedTimeout, 3600)
      : 300;

    const normalizedStep: NewWorkflowTemplateStepForm = {
      id: stepId,
      name: stepName,
      kind: newWorkflowStep.kind,
      auto_run: newWorkflowStep.kind === "script" ? newWorkflowStep.auto_run : false,
      script: newWorkflowStep.kind === "script" ? newWorkflowStep.script.trim() : "",
      timeout_seconds: String(timeoutSeconds),
      approver_group: newWorkflowStep.approver_group.trim()
    };

    setError(null);
    setWorkflowNotice(null);
    setNewWorkflowTemplateSteps((prev) => [...prev, normalizedStep]);
    setNewWorkflowStep({
      ...defaultWorkflowStepForm,
      kind: normalizedStep.kind
    });
  }, [canWriteCmdb, newWorkflowStep, newWorkflowTemplateSteps, t]);

  const removeWorkflowStepFromDraft = useCallback((stepId: string) => {
    setNewWorkflowTemplateSteps((prev) => prev.filter((item) => item.id !== stepId));
  }, []);

  const createWorkflowTemplate = useCallback(async () => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }

    const name = newWorkflowTemplateName.trim();
    if (!name) {
      setError(t("cmdb.workflow.validation.templateNameRequired"));
      return;
    }
    if (newWorkflowTemplateSteps.length === 0) {
      setError(t("cmdb.workflow.validation.templateStepsRequired"));
      return;
    }

    setCreatingWorkflowTemplate(true);
    setWorkflowNotice(null);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/workflow/templates`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          name,
          description: trimToNull(newWorkflowTemplateDescription),
          definition: {
            steps: newWorkflowTemplateSteps.map((step) => ({
              id: step.id,
              name: step.name,
              kind: step.kind,
              auto_run: step.auto_run,
              script: step.kind === "script" ? step.script : null,
              timeout_seconds: Number.parseInt(step.timeout_seconds, 10) || 300,
              approver_group: step.approver_group.trim().length > 0 ? step.approver_group.trim() : null
            }))
          }
        })
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }

      setNewWorkflowTemplateName("");
      setNewWorkflowTemplateDescription("");
      setNewWorkflowTemplateSteps([]);
      setNewWorkflowStep(defaultWorkflowStepForm);
      await loadWorkflowTemplates();
      setWorkflowNotice(t("cmdb.workflow.messages.templateCreated", { name }));
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setCreatingWorkflowTemplate(false);
    }
  }, [
    canWriteCmdb,
    loadWorkflowTemplates,
    newWorkflowTemplateDescription,
    newWorkflowTemplateName,
    newWorkflowTemplateSteps,
    t
  ]);

  const createWorkflowRequest = useCallback(async () => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }

    const templateId = Number.parseInt(newWorkflowRequest.template_id, 10);
    const title = newWorkflowRequest.title.trim();
    if (!Number.isFinite(templateId) || templateId <= 0) {
      setError(t("cmdb.workflow.validation.templateRequired"));
      return;
    }
    if (!title) {
      setError(t("cmdb.workflow.validation.requestTitleRequired"));
      return;
    }

    let payloadJson: Record<string, unknown> = {};
    const payloadRaw = newWorkflowRequest.payload_json.trim();
    if (payloadRaw.length > 0) {
      try {
        const parsed = JSON.parse(payloadRaw) as unknown;
        if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
          setError(t("cmdb.workflow.validation.payloadMustBeObject"));
          return;
        }
        payloadJson = parsed as Record<string, unknown>;
      } catch {
        setError(t("cmdb.workflow.validation.payloadInvalidJson"));
        return;
      }
    }

    setCreatingWorkflowRequest(true);
    setWorkflowNotice(null);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/workflow/requests`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          template_id: templateId,
          title,
          payload: payloadJson
        })
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: WorkflowRequest = await response.json();
      setNewWorkflowRequest((prev) => ({
        ...prev,
        title: "",
        payload_json: "{}"
      }));
      setSelectedWorkflowRequestId(String(payload.id));
      await Promise.all([loadWorkflowRequests(), loadWorkflowLogs(payload.id)]);
      setWorkflowNotice(t("cmdb.workflow.messages.requestCreated", { id: payload.id }));
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setCreatingWorkflowRequest(false);
    }
  }, [canWriteCmdb, loadWorkflowLogs, loadWorkflowRequests, newWorkflowRequest, t]);

  const approveWorkflowRequest = useCallback(async (requestId: number) => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }

    setApprovingWorkflowRequestId(requestId);
    setWorkflowNotice(null);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/workflow/approvals/${requestId}/approve`, {
        method: "POST"
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      await loadWorkflowRequests();
      setWorkflowNotice(t("cmdb.workflow.messages.requestApproved", { id: requestId }));
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setApprovingWorkflowRequestId(null);
    }
  }, [canWriteCmdb, loadWorkflowRequests, t]);

  const rejectWorkflowRequest = useCallback(async (requestId: number) => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }

    setRejectingWorkflowRequestId(requestId);
    setWorkflowNotice(null);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/workflow/approvals/${requestId}/reject`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          reason: "rejected by operator from web console"
        })
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      await loadWorkflowRequests();
      setWorkflowNotice(t("cmdb.workflow.messages.requestRejected", { id: requestId }));
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setRejectingWorkflowRequestId(null);
    }
  }, [canWriteCmdb, loadWorkflowRequests, t]);

  const executeWorkflowRequest = useCallback(async (requestId: number) => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }

    setExecutingWorkflowRequestId(requestId);
    setWorkflowNotice(null);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/workflow/requests/${requestId}/execute`, {
        method: "POST"
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      await Promise.all([loadWorkflowRequests(), loadWorkflowLogs(requestId)]);
      setSelectedWorkflowRequestId(String(requestId));
      setWorkflowNotice(t("cmdb.workflow.messages.requestExecuted", { id: requestId }));
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setExecutingWorkflowRequestId(null);
    }
  }, [canWriteCmdb, loadWorkflowLogs, loadWorkflowRequests, t]);

  const completeWorkflowManualStep = useCallback(async (requestId: number) => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }

    setManualCompletingWorkflowRequestId(requestId);
    setWorkflowNotice(null);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/workflow/requests/${requestId}/manual-complete`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          note: "manual step completed from web console"
        })
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      await Promise.all([loadWorkflowRequests(), loadWorkflowLogs(requestId)]);
      setSelectedWorkflowRequestId(String(requestId));
      setWorkflowNotice(t("cmdb.workflow.messages.manualCompleted", { id: requestId }));
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setManualCompletingWorkflowRequestId(null);
    }
  }, [canWriteCmdb, loadWorkflowLogs, loadWorkflowRequests, t]);

  const runPlaybookDryRun = useCallback(async () => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }

    const selectedPlaybook = playbookCatalog.find((item) => item.key === selectedPlaybookKey) ?? null;
    if (!selectedPlaybook) {
      setError(t("cmdb.playbooks.messages.selectPlaybook"));
      return;
    }

    const params = normalizePlaybookParamDraft(
      extractPlaybookParamFields(selectedPlaybook.params),
      playbookParamsDraft
    );
    if (!params.ok) {
      setError(params.error);
      return;
    }
    const reservationId = Number.parseInt(playbookReservationId.trim(), 10);
    const normalizedReservationId = Number.isFinite(reservationId) && reservationId > 0 ? reservationId : null;

    setRunningPlaybookDryRun(true);
    setPlaybookNotice(null);
    setError(null);
    try {
      const response = await apiFetch(
        `${API_BASE_URL}/api/v1/workflow/playbooks/${encodeURIComponent(selectedPlaybook.key)}/dry-run`,
        {
          method: "POST",
          headers: {
            "Content-Type": "application/json"
          },
          body: JSON.stringify({
            params: params.value,
            asset_ref: trimToNull(playbookAssetRef),
            reservation_id: normalizedReservationId
          })
        }
      );
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: PlaybookDryRunResponse = await response.json();
      setPlaybookDryRunResponse(payload);
      setPlaybookExecutionResult(null);
      setPlaybookConfirmationToken(payload.confirmation?.token ?? "");
      setSelectedPlaybookApprovalId("");
      setPlaybookApprovalToken("");
      await loadPlaybookExecutions();
      await loadPlaybookApprovalRequests();
      setPlaybookNotice(
        t("cmdb.playbooks.messages.dryRunReady", {
          key: selectedPlaybook.key,
          id: payload.execution.id
        })
      );
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setRunningPlaybookDryRun(false);
    }
  }, [
    canWriteCmdb,
    loadPlaybookApprovalRequests,
    loadPlaybookExecutions,
    playbookAssetRef,
    playbookCatalog,
    playbookReservationId,
    playbookParamsDraft,
    selectedPlaybookKey,
    t
  ]);

  const runPlaybookExecute = useCallback(async () => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }

    const selectedPlaybook = playbookCatalog.find((item) => item.key === selectedPlaybookKey) ?? null;
    if (!selectedPlaybook) {
      setError(t("cmdb.playbooks.messages.selectPlaybook"));
      return;
    }

    const params = normalizePlaybookParamDraft(
      extractPlaybookParamFields(selectedPlaybook.params),
      playbookParamsDraft
    );
    if (!params.ok) {
      setError(params.error);
      return;
    }
    const reservationId = Number.parseInt(playbookReservationId.trim(), 10);
    const normalizedReservationId = Number.isFinite(reservationId) && reservationId > 0 ? reservationId : null;

    const requiresConfirmation = selectedPlaybook.requires_confirmation
      || selectedPlaybook.risk_level === "high"
      || selectedPlaybook.risk_level === "critical";
    const dryRunId = playbookDryRunResponse?.execution.id ?? null;
    if (requiresConfirmation && (!dryRunId || !playbookConfirmationToken.trim())) {
      setError(t("cmdb.playbooks.messages.confirmationRequired"));
      return;
    }
    const approvalId = Number.parseInt(selectedPlaybookApprovalId, 10);
    if (requiresConfirmation && (!Number.isFinite(approvalId) || approvalId <= 0)) {
      setError("Approved request is required for high-risk playbook execution.");
      return;
    }
    if (requiresConfirmation && !playbookApprovalToken.trim()) {
      setError("Approval token is required for high-risk playbook execution.");
      return;
    }

    setRunningPlaybookExecute(true);
    setPlaybookNotice(null);
    setError(null);
    try {
      const response = await apiFetch(
        `${API_BASE_URL}/api/v1/workflow/playbooks/${encodeURIComponent(selectedPlaybook.key)}/execute`,
        {
          method: "POST",
          headers: {
            "Content-Type": "application/json"
          },
          body: JSON.stringify({
            params: params.value,
            asset_ref: trimToNull(playbookAssetRef),
            reservation_id: normalizedReservationId,
            dry_run_id: dryRunId,
            confirmation_token: trimToNull(playbookConfirmationToken),
            approval_id: requiresConfirmation ? approvalId : null,
            approval_token: requiresConfirmation ? trimToNull(playbookApprovalToken) : null,
            maintenance_override_reason: trimToNull(playbookMaintenanceOverrideReason),
            maintenance_override_confirmed: playbookMaintenanceOverrideConfirmed
          })
        }
      );
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: PlaybookExecutionDetail = await response.json();
      setPlaybookExecutionResult(payload);
      setPlaybookMaintenanceOverrideReason("");
      setPlaybookMaintenanceOverrideConfirmed(false);
      setPlaybookApprovalToken("");
      await loadPlaybookExecutions();
      await loadPlaybookApprovalRequests();
      setPlaybookNotice(
        t("cmdb.playbooks.messages.executionSucceeded", {
          key: selectedPlaybook.key,
          id: payload.id
        })
      );
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setRunningPlaybookExecute(false);
    }
  }, [
    canWriteCmdb,
    loadPlaybookApprovalRequests,
    loadPlaybookExecutions,
    playbookAssetRef,
    playbookApprovalToken,
    playbookCatalog,
    playbookConfirmationToken,
    playbookDryRunResponse?.execution.id,
    playbookMaintenanceOverrideConfirmed,
    playbookMaintenanceOverrideReason,
    playbookReservationId,
    playbookParamsDraft,
    selectedPlaybookApprovalId,
    selectedPlaybookKey,
    t
  ]);

  const requestPlaybookApproval = useCallback(async () => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }

    const selectedPlaybook = playbookCatalog.find((item) => item.key === selectedPlaybookKey) ?? null;
    if (!selectedPlaybook) {
      setError(t("cmdb.playbooks.messages.selectPlaybook"));
      return;
    }

    const requiresConfirmation = selectedPlaybook.requires_confirmation
      || selectedPlaybook.risk_level === "high"
      || selectedPlaybook.risk_level === "critical";
    if (!requiresConfirmation) {
      setError("Approval request is only required for high-risk playbooks.");
      return;
    }

    const dryRunId = playbookDryRunResponse?.execution.id ?? null;
    if (!dryRunId) {
      setError("Run dry-run first, then request approval.");
      return;
    }

    setRequestingPlaybookApproval(true);
    setPlaybookNotice(null);
    setError(null);
    try {
      const response = await apiFetch(
        `${API_BASE_URL}/api/v1/workflow/playbooks/${encodeURIComponent(selectedPlaybook.key)}/approval-request`,
        {
          method: "POST",
          headers: {
            "Content-Type": "application/json"
          },
          body: JSON.stringify({
            dry_run_id: dryRunId,
            note: trimToNull(playbookApprovalRequestNote)
          })
        }
      );
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: PlaybookApprovalRequestRecord = await response.json();
      setSelectedPlaybookApprovalId(String(payload.id));
      setPlaybookApprovalToken(payload.approval_token ?? "");
      setPlaybookApprovalRequestNote("");
      await loadPlaybookApprovalRequests();
      setPlaybookNotice(`Approval request #${payload.id} submitted.`);
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setRequestingPlaybookApproval(false);
    }
  }, [
    canWriteCmdb,
    loadPlaybookApprovalRequests,
    playbookApprovalRequestNote,
    playbookCatalog,
    playbookDryRunResponse?.execution.id,
    selectedPlaybookKey,
    t
  ]);

  const approvePlaybookApprovalRequest = useCallback(async (requestId: number) => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }
    if (!Number.isFinite(requestId) || requestId <= 0) {
      return;
    }

    setApprovingPlaybookApprovalId(requestId);
    setPlaybookNotice(null);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/workflow/playbooks/approvals/${requestId}/approve`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          note: trimToNull(playbookApprovalDecisionNote)
        })
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: PlaybookApprovalRequestRecord = await response.json();
      setSelectedPlaybookApprovalId(String(payload.id));
      setPlaybookApprovalToken(payload.approval_token ?? "");
      setPlaybookApprovalDecisionNote("");
      await loadPlaybookApprovalRequests();
      setPlaybookNotice(`Approval request #${requestId} approved.`);
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setApprovingPlaybookApprovalId(null);
    }
  }, [canWriteCmdb, loadPlaybookApprovalRequests, playbookApprovalDecisionNote, t]);

  const rejectPlaybookApprovalRequest = useCallback(async (requestId: number) => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }
    if (!Number.isFinite(requestId) || requestId <= 0) {
      return;
    }

    setRejectingPlaybookApprovalId(requestId);
    setPlaybookNotice(null);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/workflow/playbooks/approvals/${requestId}/reject`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          note: trimToNull(playbookApprovalDecisionNote)
        })
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      setPlaybookApprovalDecisionNote("");
      if (selectedPlaybookApprovalId === String(requestId)) {
        setPlaybookApprovalToken("");
      }
      await loadPlaybookApprovalRequests();
      setPlaybookNotice(`Approval request #${requestId} rejected.`);
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setRejectingPlaybookApprovalId(null);
    }
  }, [canWriteCmdb, loadPlaybookApprovalRequests, playbookApprovalDecisionNote, selectedPlaybookApprovalId, t]);

  const createTicket = useCallback(async () => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }

    const title = newTicket.title.trim();
    if (!title) {
      setError("Ticket title is required.");
      return;
    }

    const normalizedAssetIds = newTicket.asset_ids_csv
      .split(",")
      .map((value) => Number.parseInt(value.trim(), 10))
      .filter((value) => Number.isFinite(value) && value > 0);
    const assetIds = Array.from(new Set(normalizedAssetIds));
    if (newTicket.asset_ids_csv.trim().length > 0 && assetIds.length === 0) {
      setError("Asset IDs must be comma-separated positive integers.");
      return;
    }

    const alertSource = newTicket.alert_source.trim();
    const alertKey = newTicket.alert_key.trim();
    if (!alertSource && alertKey) {
      setError("Alert source is required when alert key is provided.");
      return;
    }
    if (alertSource && !alertKey) {
      setError("Alert key is required when alert source is provided.");
      return;
    }

    const workflowTemplateId = Number.parseInt(newTicket.workflow_template_id.trim(), 10);
    if (newTicket.workflow_template_id.trim().length > 0 && (!Number.isFinite(workflowTemplateId) || workflowTemplateId <= 0)) {
      setError("Workflow template ID must be a positive integer.");
      return;
    }
    if (newTicket.trigger_workflow && (!Number.isFinite(workflowTemplateId) || workflowTemplateId <= 0)) {
      setError("Workflow template ID is required when auto-trigger workflow is enabled.");
      return;
    }

    setCreatingTicket(true);
    setTicketNotice(null);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/tickets`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          title,
          description: newTicket.description.trim() || undefined,
          priority: newTicket.priority,
          category: newTicket.category.trim() || "incident",
          assignee: newTicket.assignee.trim() || undefined,
          asset_ids: assetIds,
          alert_refs: alertSource && alertKey ? [{
            source: alertSource,
            alert_key: alertKey,
            alert_title: newTicket.alert_title.trim() || undefined,
            severity: newTicket.alert_severity.trim() || undefined
          }] : [],
          workflow_template_id:
            Number.isFinite(workflowTemplateId) && workflowTemplateId > 0 ? workflowTemplateId : undefined,
          trigger_workflow: newTicket.trigger_workflow
        })
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: TicketDetailResponse = await response.json();
      setNewTicket((prev) => ({
        ...defaultTicketForm,
        category: prev.category,
        priority: prev.priority,
        alert_source: prev.alert_source,
        alert_severity: prev.alert_severity
      }));
      setSelectedTicketId(String(payload.ticket.id));
      setTicketDetail(payload);
      setTicketStatusDraft(payload.ticket.status);
      await Promise.all([
        loadTickets(),
        loadTicketEscalationQueue(),
        loadTicketEscalationActions(payload.ticket.id)
      ]);
      setTicketNotice(`Ticket ${payload.ticket.ticket_no} created.`);
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setCreatingTicket(false);
    }
  }, [canWriteCmdb, loadTicketEscalationActions, loadTicketEscalationQueue, loadTickets, newTicket, t]);

  const updateTicketStatus = useCallback(async (ticketId: number, status: string) => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }

    setUpdatingTicketStatusId(ticketId);
    setTicketNotice(null);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/tickets/${ticketId}/status`, {
        method: "PATCH",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          status,
          note: "updated from web console"
        })
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: TicketDetailResponse = await response.json();
      setTicketDetail(payload);
      setTicketStatusDraft(payload.ticket.status);
      await Promise.all([
        loadTickets(),
        loadTicketEscalationQueue(),
        loadTicketEscalationActions(ticketId)
      ]);
      setTicketNotice(`Ticket ${payload.ticket.ticket_no} moved to '${payload.ticket.status}'.`);
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setUpdatingTicketStatusId(null);
    }
  }, [canWriteCmdb, loadTicketEscalationActions, loadTicketEscalationQueue, loadTickets, t]);

  const createMonitoringSource = useCallback(async () => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }

    const name = newMonitoringSource.name.trim();
    const endpoint = newMonitoringSource.endpoint.trim();
    const secretRef = newMonitoringSource.secret_ref.trim();
    const username = newMonitoringSource.username.trim();

    if (!name) {
      setError(t("cmdb.monitoringSources.validation.nameRequired"));
      return;
    }
    if (!endpoint) {
      setError(t("cmdb.monitoringSources.validation.endpointRequired"));
      return;
    }
    if (!secretRef) {
      setError(t("cmdb.monitoringSources.validation.secretRefRequired"));
      return;
    }
    if (newMonitoringSource.auth_type === "basic" && !username) {
      setError(t("cmdb.monitoringSources.validation.usernameRequired"));
      return;
    }

    setCreatingMonitoringSource(true);
    setMonitoringSourceNotice(null);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/monitoring/sources`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          name,
          source_type: newMonitoringSource.source_type,
          endpoint,
          proxy_endpoint: trimToNull(newMonitoringSource.proxy_endpoint),
          auth_type: newMonitoringSource.auth_type,
          username: newMonitoringSource.auth_type === "basic" ? username : null,
          secret_ref: secretRef,
          site: trimToNull(newMonitoringSource.site),
          department: trimToNull(newMonitoringSource.department),
          is_enabled: newMonitoringSource.is_enabled
        })
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }

      const payload: MonitoringSource = await response.json();
      setNewMonitoringSource((prev) => ({
        ...defaultMonitoringSourceForm,
        source_type: prev.source_type
      }));
      await loadMonitoringSources(monitoringSourceFilters);
      setMonitoringSourceNotice(t("cmdb.monitoringSources.messages.created", { name: payload.name }));
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setCreatingMonitoringSource(false);
    }
  }, [canWriteCmdb, loadMonitoringSources, monitoringSourceFilters, newMonitoringSource, t]);

  const probeMonitoringSource = useCallback(
    async (sourceId: number) => {
      if (!canWriteCmdb) {
        setError(t("auth.messages.forbiddenAction"));
        return;
      }

      setProbingMonitoringSourceId(sourceId);
      setMonitoringSourceNotice(null);
      setError(null);
      try {
        const response = await apiFetch(`${API_BASE_URL}/api/v1/monitoring/sources/${sourceId}/probe`, {
          method: "POST"
        });
        if (!response.ok) {
          throw new Error(await readErrorMessage(response));
        }
        const payload: { reachable: boolean; message: string; source: MonitoringSource } = await response.json();
        await loadMonitoringSources(monitoringSourceFilters);
        setMonitoringSourceNotice(
          t("cmdb.monitoringSources.messages.probed", {
            id: sourceId,
            result: payload.reachable
              ? t("cmdb.monitoringSources.messages.reachable")
              : t("cmdb.monitoringSources.messages.unreachable")
          })
        );
      } catch (err) {
        setError(err instanceof Error ? err.message : "unknown error");
      } finally {
        setProbingMonitoringSourceId(null);
      }
    },
    [canWriteCmdb, loadMonitoringSources, monitoringSourceFilters, t]
  );

  const findAssetByCode = useCallback(async () => {
    const normalized = scanCode.trim();
    if (!normalized) {
      setError(t("cmdb.scan.codeRequired"));
      return;
    }

    setScanning(true);
    setError(null);
    setScanResult(null);
    try {
      const response = await apiFetch(
        `${API_BASE_URL}/api/v1/cmdb/assets/by-code/${encodeURIComponent(normalized)}?mode=${scanMode}`
      );
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: Asset = await response.json();
      setScanResult(payload);
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setScanning(false);
    }
  }, [scanCode, scanMode, t]);

  useEffect(() => {
    if (!authIdentity) {
      return;
    }
    void Promise.all([
      loadAssets(),
      loadAssetStats(),
      loadFieldDefinitions(),
      loadMonitoringSources(defaultMonitoringSourceFilters),
      loadMonitoringOverview(),
    ]);
  }, [
    loadAssetStats,
    loadAssets,
    loadFieldDefinitions,
    loadMonitoringOverview,
    loadMonitoringSources,
    authIdentity
  ]);

  useEffect(() => {
    if (assets.length === 0 || selectedAssetId) {
      return;
    }
    const firstAssetId = String(assets[0].id);
    setSelectedAssetId(firstAssetId);
  }, [assets, selectedAssetId]);

  useEffect(() => {
    if (workflowRequests.length === 0 || selectedWorkflowRequestId) {
      return;
    }
    setSelectedWorkflowRequestId(String(workflowRequests[0].id));
  }, [selectedWorkflowRequestId, workflowRequests]);

  useEffect(() => {
    if (playbookCatalog.length === 0) {
      setSelectedPlaybookKey("");
      setPlaybookParamsDraft({});
      setPlaybookDryRunResponse(null);
      setPlaybookExecutionResult(null);
      setPlaybookConfirmationToken("");
      setSelectedPlaybookApprovalId("");
      setPlaybookApprovalToken("");
      setPlaybookApprovalRequestNote("");
      setPlaybookApprovalDecisionNote("");
      setPlaybookMaintenanceOverrideReason("");
      setPlaybookMaintenanceOverrideConfirmed(false);
      return;
    }

    if (!selectedPlaybookKey || !playbookCatalog.some((item) => item.key === selectedPlaybookKey)) {
      setSelectedPlaybookKey(playbookCatalog[0].key);
    }
  }, [playbookCatalog, selectedPlaybookKey]);

  useEffect(() => {
    if (!selectedPlaybookKey) {
      return;
    }

    const selected = playbookCatalog.find((item) => item.key === selectedPlaybookKey);
    if (!selected) {
      return;
    }

    const fields = extractPlaybookParamFields(selected.params);
    setPlaybookParamsDraft((prev) => {
      const next: Record<string, string> = {};
      for (const field of fields) {
        const prior = prev[field.key];
        if (prior !== undefined) {
          next[field.key] = prior;
          continue;
        }
        if (field.default !== undefined && field.default !== null) {
          next[field.key] = String(field.default);
          continue;
        }
        next[field.key] = "";
      }
      return next;
    });
    setPlaybookDryRunResponse(null);
    setPlaybookExecutionResult(null);
    setPlaybookConfirmationToken("");
    setSelectedPlaybookApprovalId("");
    setPlaybookApprovalToken("");
    setPlaybookApprovalRequestNote("");
    setPlaybookApprovalDecisionNote("");
    setPlaybookMaintenanceOverrideReason("");
    setPlaybookMaintenanceOverrideConfirmed(false);
  }, [playbookCatalog, selectedPlaybookKey]);

  useEffect(() => {
    if (playbookApprovalRequests.length === 0) {
      setSelectedPlaybookApprovalId("");
      setPlaybookApprovalToken("");
      return;
    }

    const currentId = Number.parseInt(selectedPlaybookApprovalId, 10);
    if (Number.isFinite(currentId) && currentId > 0 && playbookApprovalRequests.some((item) => item.id === currentId)) {
      return;
    }

    const dryRunId = playbookDryRunResponse?.execution.id ?? null;
    if (dryRunId) {
      const preferred = playbookApprovalRequests.find((item) => item.dry_run_execution_id === dryRunId);
      if (preferred) {
        setSelectedPlaybookApprovalId(String(preferred.id));
        return;
      }
    }

    const approved = playbookApprovalRequests.find((item) => item.status === "approved");
    setSelectedPlaybookApprovalId(String((approved ?? playbookApprovalRequests[0]).id));
  }, [playbookApprovalRequests, playbookDryRunResponse?.execution.id, selectedPlaybookApprovalId]);

  useEffect(() => {
    const approvalId = Number.parseInt(selectedPlaybookApprovalId, 10);
    if (!Number.isFinite(approvalId) || approvalId <= 0) {
      return;
    }
    const item = playbookApprovalRequests.find((candidate) => candidate.id === approvalId);
    if (!item) {
      return;
    }
    setPlaybookApprovalToken(item.approval_token ?? "");
  }, [playbookApprovalRequests, selectedPlaybookApprovalId]);

  useEffect(() => {
    if (selectedIncidentAlertId.trim().length === 0) {
      setIncidentCommandDetail(null);
      setIncidentCommandDraft(defaultIncidentCommandDraft);
      return;
    }
    void loadIncidentCommandDetail(selectedIncidentAlertId);
  }, [loadIncidentCommandDetail, selectedIncidentAlertId]);

  useEffect(() => {
    if (!incidentCommandDetail) {
      return;
    }
    setIncidentCommandDraft((current) => ({
      ...current,
      alert_id: String(incidentCommandDetail.item.alert_id),
      status: incidentCommandDetail.item.command_status,
      owner: incidentCommandDetail.item.command_owner ?? "",
      eta_at: incidentCommandDetail.item.eta_at ?? "",
      blocker: incidentCommandDetail.item.blocker ?? "",
      summary: incidentCommandDetail.item.summary ?? ""
    }));
  }, [incidentCommandDetail]);

  useEffect(() => {
    if (backupPolicies.length === 0) {
      setBackupPolicyDraft(defaultBackupPolicyForm);
      return;
    }

    setBackupPolicyDraft((current) => {
      const currentId = Number.parseInt(current.policy_id, 10);
      const selected = Number.isFinite(currentId) && currentId > 0
        ? backupPolicies.find((item) => item.id === currentId) ?? backupPolicies[0]
        : backupPolicies[0];

      if (selected.id === currentId) {
        return current;
      }

      return {
        ...current,
        policy_id: String(selected.id),
        policy_key: selected.policy_key,
        name: selected.name,
        frequency: selected.frequency,
        schedule_time_utc: selected.schedule_time_utc,
        schedule_weekday: String(selected.schedule_weekday ?? 1),
        retention_days: String(selected.retention_days),
        destination_type: selected.destination_type,
        destination_uri: selected.destination_uri,
        drill_enabled: selected.drill_enabled,
        drill_frequency: selected.drill_frequency,
        drill_weekday: String(selected.drill_weekday ?? 3),
        drill_time_utc: selected.drill_time_utc,
      };
    });
  }, [backupPolicies]);

  useEffect(() => {
    setBackupRestoreEvidenceDraft((current) => {
      const hasRun = Number.isFinite(Number.parseInt(current.run_id, 10))
        && Number.parseInt(current.run_id, 10) > 0;
      const nextRunId = backupPolicyRuns.length > 0 ? String(backupPolicyRuns[0].id) : "";
      const nextVerifier = current.verifier.trim().length > 0
        ? current.verifier
        : (authIdentity?.user.username ?? "");
      if (hasRun && nextVerifier === current.verifier) {
        return current;
      }
      return {
        ...current,
        run_id: hasRun ? current.run_id : nextRunId,
        verifier: nextVerifier
      };
    });
  }, [authIdentity?.user.username, backupPolicyRuns]);

  useEffect(() => {
    if (tickets.length === 0) {
      setSelectedTicketId("");
      setTicketDetail(null);
      return;
    }
    if (selectedTicketId) {
      return;
    }
    setSelectedTicketId(String(tickets[0].id));
  }, [selectedTicketId, tickets]);

  useEffect(() => {
    setRelationNotice(null);
    setBindingNotice(null);
    setLifecycleNotice(null);
    setMonitoringNotice(null);
    setImpactNotice(null);
    setSelectedTopologyEdgeKey(null);
  }, [selectedAssetId]);

  useEffect(() => {
    const assetId = Number.parseInt(selectedAssetId, 10);
    if (!Number.isFinite(assetId) || assetId <= 0) {
      setRelations([]);
      return;
    }
    void loadRelations(assetId);
  }, [loadRelations, selectedAssetId]);

  useEffect(() => {
    const assetId = Number.parseInt(selectedAssetId, 10);
    if (!Number.isFinite(assetId) || assetId <= 0) {
      setAssetBindings(null);
      setAssetMonitoring(null);
      setAssetImpact(null);
      setBindingOwnerDrafts([]);
      return;
    }

    const depth = parseImpactDepth(impactDepth) ?? 4;
    const relationTypes = parseImpactRelationTypesInput(impactRelationTypesInput, defaultImpactRelationTypes);
    void Promise.all([
      loadAssetBindings(assetId),
      loadAssetMonitoring(assetId),
      loadAssetImpact(assetId, impactDirection, depth, relationTypes)
    ]);
  }, [loadAssetBindings, loadAssetImpact, loadAssetMonitoring, selectedAssetId]);

  useEffect(() => {
    if (!authIdentity) {
      return;
    }
    const assetId = Number.parseInt(selectedAssetId, 10);
    const windowMinutes = parseMonitoringWindowMinutes(monitoringMetricsWindowMinutes) ?? 60;
    if (!Number.isFinite(assetId) || assetId <= 0) {
      setMonitoringMetrics(null);
      setMonitoringMetricsError(null);
      return;
    }
    void loadMonitoringMetrics(assetId, windowMinutes);
  }, [authIdentity, loadMonitoringMetrics, monitoringMetricsWindowMinutes, selectedAssetId]);

  useEffect(() => {
    if (!authIdentity) {
      return;
    }
    const scopedDepartment = menuAxis === "department" ? departmentWorkspace : undefined;
    void loadMonitoringOverview(scopedDepartment);
  }, [authIdentity, departmentWorkspace, loadMonitoringOverview, menuAxis]);

  useEffect(() => {
    if (!authIdentity) {
      return;
    }
    const requestId = Number.parseInt(selectedWorkflowRequestId, 10);
    if (!Number.isFinite(requestId) || requestId <= 0) {
      setWorkflowLogs([]);
      return;
    }
    void loadWorkflowLogs(requestId);
  }, [authIdentity, loadWorkflowLogs, selectedWorkflowRequestId]);

  const emptyState = useMemo(() => assets.length === 0, [assets]);
  const assetNameById = useMemo(() => {
    const map = new Map<number, string>();
    for (const asset of assets) {
      map.set(asset.id, asset.name);
    }
    return map;
  }, [assets]);
  const notificationChannelNameById = useMemo(() => {
    const map = new Map<number, string>();
    for (const channel of notificationChannels) {
      map.set(channel.id, channel.name);
    }
    return map;
  }, [notificationChannels]);
  const notificationChannelById = useMemo(() => {
    const map = new Map<number, NotificationChannel>();
    for (const channel of notificationChannels) {
      map.set(channel.id, channel);
    }
    return map;
  }, [notificationChannels]);
  const selectedWorkflowRequestNumericId = useMemo(
    () => Number.parseInt(selectedWorkflowRequestId, 10),
    [selectedWorkflowRequestId]
  );
  const selectedWorkflowRequest = useMemo(
    () => workflowRequests.find((item) => item.id === selectedWorkflowRequestNumericId) ?? null,
    [selectedWorkflowRequestNumericId, workflowRequests]
  );
  const selectedTicketNumericId = useMemo(
    () => Number.parseInt(selectedTicketId, 10),
    [selectedTicketId]
  );
  const selectedTicketSummary = useMemo(
    () => tickets.find((item) => item.id === selectedTicketNumericId) ?? null,
    [selectedTicketNumericId, tickets]
  );
  const playbookCategoryOptions = useMemo(
    () => Array.from(new Set(playbookCatalog.map((item) => item.category))).sort(),
    [playbookCatalog]
  );
  const selectedSetupTemplate = useMemo(
    () => setupTemplates.find((item) => item.key === selectedSetupTemplateKey) ?? null,
    [selectedSetupTemplateKey, setupTemplates]
  );
  const selectedPlaybook = useMemo(
    () => playbookCatalog.find((item) => item.key === selectedPlaybookKey) ?? null,
    [playbookCatalog, selectedPlaybookKey]
  );
  const selectedPlaybookParamFields = useMemo(
    () => extractPlaybookParamFields(selectedPlaybook?.params ?? null),
    [selectedPlaybook?.params]
  );
  const workflowStatusBuckets = useMemo(() => {
    const counters = new Map<string, number>();
    for (const request of workflowRequests) {
      const normalized = normalizeWorkflowStatus(request.status);
      counters.set(normalized, (counters.get(normalized) ?? 0) + 1);
    }
    return Array.from(counters.entries())
      .map(([key, count]) => ({ key, label: key, asset_total: count }))
      .sort((left, right) => right.asset_total - left.asset_total || left.label.localeCompare(right.label));
  }, [workflowRequests]);
  const workflowStatusMax = useMemo(() => maxBucketAssetTotal(workflowStatusBuckets), [workflowStatusBuckets]);
  const workflowTemplateUsageBuckets = useMemo(() => {
    const counters = new Map<string, number>();
    for (const request of workflowRequests) {
      const label = request.template_name.trim().length > 0 ? request.template_name : `#${request.template_id}`;
      counters.set(label, (counters.get(label) ?? 0) + 1);
    }
    return Array.from(counters.entries())
      .map(([key, count]) => ({ key, label: key, asset_total: count }))
      .sort((left, right) => right.asset_total - left.asset_total || left.label.localeCompare(right.label));
  }, [workflowRequests]);
  const workflowTemplateUsageMax = useMemo(
    () => maxBucketAssetTotal(workflowTemplateUsageBuckets),
    [workflowTemplateUsageBuckets]
  );
  const workflowRequesterBuckets = useMemo(() => {
    const counters = new Map<string, number>();
    for (const request of workflowRequests) {
      const label = request.requester.trim().length > 0 ? request.requester : "unknown";
      counters.set(label, (counters.get(label) ?? 0) + 1);
    }
    return Array.from(counters.entries())
      .map(([key, count]) => ({ key, label: key, asset_total: count }))
      .sort((left, right) => right.asset_total - left.asset_total || left.label.localeCompare(right.label));
  }, [workflowRequests]);
  const workflowRequesterMax = useMemo(() => maxBucketAssetTotal(workflowRequesterBuckets), [workflowRequesterBuckets]);
  const workflowDailyTrend = useMemo(() => {
    const points: WorkflowDailyTrendPoint[] = [];
    const byDay = new Map<string, WorkflowDailyTrendPoint>();
    const now = new Date();
    for (let offset = 6; offset >= 0; offset -= 1) {
      const day = new Date(now);
      day.setDate(now.getDate() - offset);
      const key = formatLocalDateKey(day);
      const point: WorkflowDailyTrendPoint = {
        day_key: key,
        day_label: `${day.getMonth() + 1}/${day.getDate()}`,
        total: 0,
        completed: 0,
        failed: 0,
        active: 0
      };
      points.push(point);
      byDay.set(key, point);
    }

    for (const request of workflowRequests) {
      const createdAt = new Date(request.created_at);
      if (Number.isNaN(createdAt.getTime())) {
        continue;
      }

      const point = byDay.get(formatLocalDateKey(createdAt));
      if (!point) {
        continue;
      }

      point.total += 1;
      const normalized = normalizeWorkflowStatus(request.status);
      if (isWorkflowSuccessStatus(normalized)) {
        point.completed += 1;
      } else if (isWorkflowFailureStatus(normalized)) {
        point.failed += 1;
      } else {
        point.active += 1;
      }
    }

    return points;
  }, [workflowRequests]);
  const workflowDailyTrendMax = useMemo(
    () => workflowDailyTrend.reduce((maxValue, point) => Math.max(maxValue, point.total), 0),
    [workflowDailyTrend]
  );
  const workflowKpis = useMemo(() => {
    let activeRequests = 0;
    let completedRequests = 0;
    let failedRequests = 0;
    let approvalQueue = 0;
    let manualQueue = 0;

    for (const request of workflowRequests) {
      const normalized = normalizeWorkflowStatus(request.status);
      if (normalized === "pending_approval") {
        approvalQueue += 1;
      }
      if (normalized === "waiting_manual") {
        manualQueue += 1;
      }
      if (isWorkflowSuccessStatus(normalized)) {
        completedRequests += 1;
      } else if (isWorkflowFailureStatus(normalized)) {
        failedRequests += 1;
      } else {
        activeRequests += 1;
      }
    }

    let automatedLogSteps = 0;
    let logDurationTotalMs = 0;
    let logDurationCount = 0;
    let executionSuccess = 0;
    let executionFailure = 0;

    for (const log of workflowLogs) {
      if (log.step_kind === "script") {
        automatedLogSteps += 1;
      }
      if (typeof log.duration_ms === "number" && Number.isFinite(log.duration_ms) && log.duration_ms >= 0) {
        logDurationTotalMs += log.duration_ms;
        logDurationCount += 1;
      }

      const normalized = normalizeWorkflowStatus(log.status);
      if (isWorkflowSuccessStatus(normalized)) {
        executionSuccess += 1;
      } else if (isWorkflowFailureStatus(normalized)) {
        executionFailure += 1;
      }
    }

    const totalRequests = workflowRequests.length;
    const completionRate = totalRequests > 0 ? Math.round((completedRequests / totalRequests) * 100) : 0;
    const failureRate = totalRequests > 0 ? Math.round((failedRequests / totalRequests) * 100) : 0;
    const automationShare = workflowLogs.length > 0 ? Math.round((automatedLogSteps / workflowLogs.length) * 100) : 0;
    const averageExecutionMs = logDurationCount > 0 ? Math.round(logDurationTotalMs / logDurationCount) : 0;
    const executionSampleSize = executionSuccess + executionFailure;
    const executionSuccessRate = executionSampleSize > 0 ? Math.round((executionSuccess / executionSampleSize) * 100) : 0;

    return {
      totalRequests,
      activeRequests,
      completedRequests,
      failedRequests,
      approvalQueue,
      manualQueue,
      completionRate,
      failureRate,
      automationShare,
      averageExecutionMs,
      executionSuccessRate,
      executionSampleSize
    };
  }, [workflowLogs, workflowRequests]);
  const workflowReportRangeValue = useMemo(
    () => parseWorkflowReportRangeDays(workflowReportRangeDays),
    [workflowReportRangeDays]
  );
  const workflowReportRows = useMemo(() => {
    const requesterQuery = workflowReportRequesterFilter.trim().toLowerCase();
    const now = Date.now();
    const cutoff = now - (workflowReportRangeValue - 1) * 24 * 60 * 60 * 1000;

    return workflowRequests
      .filter((request) => {
        const createdAtMs = parseDateMs(request.created_at);
        if (createdAtMs === null || createdAtMs < cutoff) {
          return false;
        }

        const normalizedStatus = normalizeWorkflowStatus(request.status);
        if (workflowReportStatusFilter !== "all" && normalizedStatus !== workflowReportStatusFilter) {
          return false;
        }

        const templateLabel = workflowTemplateDisplayName(request);
        if (workflowReportTemplateFilter !== "all" && templateLabel !== workflowReportTemplateFilter) {
          return false;
        }

        if (requesterQuery.length > 0 && !request.requester.toLowerCase().includes(requesterQuery)) {
          return false;
        }

        return true;
      })
      .slice()
      .sort((left, right) => {
        const leftTs = parseDateMs(left.updated_at) ?? parseDateMs(left.created_at) ?? 0;
        const rightTs = parseDateMs(right.updated_at) ?? parseDateMs(right.created_at) ?? 0;
        return rightTs - leftTs;
      });
  }, [
    workflowReportRangeValue,
    workflowReportRequesterFilter,
    workflowReportStatusFilter,
    workflowReportTemplateFilter,
    workflowRequests
  ]);
  const workflowReportStatusOptions = useMemo(
    () =>
      Array.from(new Set(workflowRequests.map((item) => normalizeWorkflowStatus(item.status))))
        .filter((item) => item.length > 0)
        .sort(),
    [workflowRequests]
  );
  const workflowReportTemplateOptions = useMemo(
    () => workflowTemplateUsageBuckets.map((item) => item.label),
    [workflowTemplateUsageBuckets]
  );
  const workflowReportStatusBuckets = useMemo(() => {
    const counters = new Map<string, number>();
    for (const request of workflowReportRows) {
      const status = normalizeWorkflowStatus(request.status);
      counters.set(status, (counters.get(status) ?? 0) + 1);
    }
    return Array.from(counters.entries())
      .map(([key, count]) => ({ key, label: key, asset_total: count }))
      .sort((left, right) => right.asset_total - left.asset_total || left.label.localeCompare(right.label));
  }, [workflowReportRows]);
  const workflowReportStatusMax = useMemo(
    () => maxBucketAssetTotal(workflowReportStatusBuckets),
    [workflowReportStatusBuckets]
  );
  const workflowReportTemplateBuckets = useMemo(() => {
    const counters = new Map<string, number>();
    for (const request of workflowReportRows) {
      const label = workflowTemplateDisplayName(request);
      counters.set(label, (counters.get(label) ?? 0) + 1);
    }
    return Array.from(counters.entries())
      .map(([key, count]) => ({ key, label: key, asset_total: count }))
      .sort((left, right) => right.asset_total - left.asset_total || left.label.localeCompare(right.label));
  }, [workflowReportRows]);
  const workflowReportTemplateMax = useMemo(
    () => maxBucketAssetTotal(workflowReportTemplateBuckets),
    [workflowReportTemplateBuckets]
  );
  const workflowReportDailyTrend = useMemo(
    () => buildWorkflowDailyTrend(workflowReportRows, workflowReportRangeValue),
    [workflowReportRangeValue, workflowReportRows]
  );
  const workflowReportDailyTrendMax = useMemo(
    () => workflowReportDailyTrend.reduce((maxValue, item) => Math.max(maxValue, item.total), 0),
    [workflowReportDailyTrend]
  );
  const workflowReportExecutionStats = useMemo(() => {
    const requestIds = new Set(workflowReportRows.map((item) => item.id));
    let durationTotal = 0;
    let durationCount = 0;
    let successCount = 0;
    let failureCount = 0;
    let automatedCount = 0;

    for (const log of workflowLogs) {
      if (!requestIds.has(log.request_id)) {
        continue;
      }

      if (typeof log.duration_ms === "number" && Number.isFinite(log.duration_ms) && log.duration_ms >= 0) {
        durationTotal += log.duration_ms;
        durationCount += 1;
      }
      if (log.step_kind === "script") {
        automatedCount += 1;
      }

      const normalized = normalizeWorkflowStatus(log.status);
      if (isWorkflowSuccessStatus(normalized)) {
        successCount += 1;
      } else if (isWorkflowFailureStatus(normalized)) {
        failureCount += 1;
      }
    }

    const sampleSize = successCount + failureCount;
    const successRate = sampleSize > 0 ? Math.round((successCount / sampleSize) * 100) : 0;
    const averageDurationMs = durationCount > 0 ? Math.round(durationTotal / durationCount) : 0;
    const automationShare = workflowLogs.length > 0 ? Math.round((automatedCount / workflowLogs.length) * 100) : 0;

    return {
      successRate,
      averageDurationMs,
      sampleSize,
      automationShare
    };
  }, [workflowLogs, workflowReportRows]);
  const workflowReportSummary = useMemo(() => {
    let completed = 0;
    let failed = 0;
    let active = 0;
    let approvalQueue = 0;
    let manualQueue = 0;

    for (const item of workflowReportRows) {
      const normalized = normalizeWorkflowStatus(item.status);
      if (normalized === "pending_approval") {
        approvalQueue += 1;
      }
      if (normalized === "waiting_manual") {
        manualQueue += 1;
      }
      if (isWorkflowSuccessStatus(normalized)) {
        completed += 1;
      } else if (isWorkflowFailureStatus(normalized)) {
        failed += 1;
      } else {
        active += 1;
      }
    }

    const total = workflowReportRows.length;
    const completionRate = total > 0 ? Math.round((completed / total) * 100) : 0;
    const failureRate = total > 0 ? Math.round((failed / total) * 100) : 0;

    return {
      total,
      completed,
      failed,
      active,
      approvalQueue,
      manualQueue,
      completionRate,
      failureRate
    };
  }, [workflowReportRows]);
  const workflowTemplateTrendRanks = useMemo(
    () => buildWorkflowTrendRankRows(workflowRequests, (request) => workflowTemplateDisplayName(request)),
    [workflowRequests]
  );
  const workflowRequesterTrendRanks = useMemo(
    () =>
      buildWorkflowTrendRankRows(workflowRequests, (request) =>
        request.requester.trim().length > 0 ? request.requester.trim() : "unknown"
      ),
    [workflowRequests]
  );
  const exportWorkflowReportCsv = useCallback(() => {
    if (typeof window === "undefined") {
      return;
    }

    const headers = [
      "id",
      "template_id",
      "template_name",
      "title",
      "status",
      "requester",
      "current_step_index",
      "created_at",
      "updated_at",
      "last_error"
    ];
    const lines = workflowReportRows.map((item) =>
      [
        String(item.id),
        String(item.template_id),
        workflowTemplateDisplayName(item),
        item.title,
        normalizeWorkflowStatus(item.status),
        item.requester,
        String(item.current_step_index),
        item.created_at,
        item.updated_at,
        item.last_error ?? ""
      ]
        .map(escapeCsvCell)
        .join(",")
    );
    const csv = [headers.join(","), ...lines].join("\n");
    const blob = new Blob([csv], { type: "text/csv;charset=utf-8;" });
    const url = window.URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = `workflow-report-${workflowReportRangeValue}d-${formatLocalDateKey(new Date())}.csv`;
    anchor.click();
    window.URL.revokeObjectURL(url);

    setWorkflowNotice(t("cmdb.workflow.reports.messages.exported", { count: workflowReportRows.length }));
  }, [t, workflowReportRangeValue, workflowReportRows]);
  const selectedAssetNumericId = useMemo(() => Number.parseInt(selectedAssetId, 10), [selectedAssetId]);
  const monitoringMetricsWindowValue = useMemo(
    () => parseMonitoringWindowMinutes(monitoringMetricsWindowMinutes) ?? 60,
    [monitoringMetricsWindowMinutes]
  );
  const selectedAsset = useMemo(
    () => assets.find((item) => item.id === selectedAssetNumericId) ?? null,
    [assets, selectedAssetNumericId]
  );
  const relationSummary = useMemo(() => {
    if (!Number.isFinite(selectedAssetNumericId) || selectedAssetNumericId <= 0) {
      return { upstream: 0, downstream: 0 };
    }
    const upstream = relations.filter((item) => item.dst_asset_id === selectedAssetNumericId).length;
    const downstream = relations.filter((item) => item.src_asset_id === selectedAssetNumericId).length;
    return { upstream, downstream };
  }, [relations, selectedAssetNumericId]);
  const impactNodeNameById = useMemo(() => {
    const map = new Map<number, string>();
    for (const node of assetImpact?.nodes ?? []) {
      map.set(node.id, node.name);
    }
    for (const asset of assets) {
      if (!map.has(asset.id)) {
        map.set(asset.id, asset.name);
      }
    }
    return map;
  }, [assetImpact?.nodes, assets]);
  const hierarchyHintEdges = useMemo(
    () => (assetImpact?.edges ?? []).filter((edge) => edge.relation_type === "contains").slice(0, 10),
    [assetImpact?.edges]
  );
  const impactRelationTypes = useMemo(
    () => parseImpactRelationTypesInput(impactRelationTypesInput, defaultImpactRelationTypes),
    [impactRelationTypesInput]
  );
  const topologyNodePositions = useMemo(
    () =>
      buildTopologyNodePositions(
        assetImpact?.nodes ?? [],
        assetImpact?.root_asset_id ?? selectedAssetNumericId,
        980,
        540,
        38
      ),
    [assetImpact?.nodes, assetImpact?.root_asset_id, selectedAssetNumericId]
  );
  const topologyEdgeRenderMeta = useMemo(
    () => buildParallelEdgeMeta(assetImpact?.edges ?? []),
    [assetImpact?.edges]
  );
  const selectedTopologyEdge = useMemo(() => {
    if (!selectedTopologyEdgeKey) {
      return null;
    }
    return (assetImpact?.edges ?? []).find((edge) => topologyEdgeKey(edge) === selectedTopologyEdgeKey) ?? null;
  }, [assetImpact?.edges, selectedTopologyEdgeKey]);
  const topologyMapEdgesForRender = useMemo(
    () => (topologyMap?.edges ?? []).map((edge) => ({ ...edge, direction: "scope" })),
    [topologyMap?.edges]
  );
  const topologyMapLayoutNodes = useMemo(
    () =>
      (topologyMap?.nodes ?? []).map((node, index) => ({
        id: node.id,
        status: node.health,
        depth: Math.floor(index / 24) + 1
      })),
    [topologyMap?.nodes]
  );
  const topologyMapRootId = useMemo(() => {
    const parsed = Number.parseInt(selectedTopologyMapNodeId, 10);
    if (Number.isFinite(parsed) && parsed > 0) {
      return parsed;
    }
    return topologyMap?.nodes[0]?.id ?? 0;
  }, [selectedTopologyMapNodeId, topologyMap?.nodes]);
  const topologyMapNodePositions = useMemo(
    () => buildTopologyNodePositions(topologyMapLayoutNodes, topologyMapRootId, 1080, 620, 52),
    [topologyMapLayoutNodes, topologyMapRootId]
  );
  const topologyMapEdgeRenderMeta = useMemo(
    () => buildParallelEdgeMeta(topologyMapEdgesForRender),
    [topologyMapEdgesForRender]
  );
  const selectedTopologyMapNode = useMemo(() => {
    const nodeId = Number.parseInt(selectedTopologyMapNodeId, 10);
    if (!Number.isFinite(nodeId) || nodeId <= 0) {
      return null;
    }
    return (topologyMap?.nodes ?? []).find((node) => node.id === nodeId) ?? null;
  }, [selectedTopologyMapNodeId, topologyMap?.nodes]);
  const selectedTopologyMapEdge = useMemo(() => {
    if (!selectedTopologyMapEdgeKey) {
      return null;
    }
    return topologyMapEdgesForRender.find((edge) => topologyEdgeKey(edge) === selectedTopologyMapEdgeKey) ?? null;
  }, [selectedTopologyMapEdgeKey, topologyMapEdgesForRender]);
  const topologyMapNodeEdgeSummary = useMemo(() => {
    const nodeId = selectedTopologyMapNode?.id;
    if (!nodeId) {
      return { outbound: 0, inbound: 0, total: 0 };
    }
    let outbound = 0;
    let inbound = 0;
    for (const edge of topologyMap?.edges ?? []) {
      if (edge.src_asset_id === nodeId) {
        outbound += 1;
      }
      if (edge.dst_asset_id === nodeId) {
        inbound += 1;
      }
    }
    return { outbound, inbound, total: outbound + inbound };
  }, [selectedTopologyMapNode?.id, topologyMap?.edges]);
  const monitoringSourceStats = useMemo(() => {
    let enabled = 0;
    let reachable = 0;
    let unreachable = 0;
    for (const source of monitoringSources) {
      if (source.is_enabled) {
        enabled += 1;
      }
      if (source.last_probe_status === "reachable") {
        reachable += 1;
      }
      if (source.last_probe_status === "unreachable") {
        unreachable += 1;
      }
    }
    return {
      total: monitoringSources.length,
      enabled,
      reachable,
      unreachable
    };
  }, [monitoringSources]);
  const assetStatsStatusBuckets = useMemo(() => assetStats?.status_buckets ?? [], [assetStats]);
  const assetStatsDepartmentBuckets = useMemo(() => assetStats?.department_buckets ?? [], [assetStats]);
  const assetStatsBusinessServiceBuckets = useMemo(
    () => assetStats?.business_service_buckets ?? [],
    [assetStats]
  );
  const assetStatsStatusMax = useMemo(() => maxBucketAssetTotal(assetStatsStatusBuckets), [assetStatsStatusBuckets]);
  const assetStatsDepartmentMax = useMemo(
    () => maxBucketAssetTotal(assetStatsDepartmentBuckets),
    [assetStatsDepartmentBuckets]
  );
  const assetStatsBusinessServiceMax = useMemo(
    () => maxBucketAssetTotal(assetStatsBusinessServiceBuckets),
    [assetStatsBusinessServiceBuckets]
  );
  const departmentWorkspaceOptions = useMemo(
    () => ["all", ...assetStatsDepartmentBuckets.map((bucket) => bucket.label)],
    [assetStatsDepartmentBuckets]
  );
  const businessWorkspaceOptions = useMemo(
    () => ["all", ...assetStatsBusinessServiceBuckets.map((bucket) => bucket.label)],
    [assetStatsBusinessServiceBuckets]
  );
  const selectedDepartmentAssetCount = useMemo(() => {
    if (departmentWorkspace === "all") {
      return assetStats?.total_assets ?? assets.length;
    }
    return assetStatsDepartmentBuckets.find((bucket) => bucket.label === departmentWorkspace)?.asset_total ?? 0;
  }, [assetStats?.total_assets, assetStatsDepartmentBuckets, assets.length, departmentWorkspace]);
  const selectedBusinessAssetCount = useMemo(() => {
    if (businessWorkspace === "all") {
      return assetStats?.total_assets ?? assets.length;
    }
    return assetStatsBusinessServiceBuckets.find((bucket) => bucket.label === businessWorkspace)?.asset_total ?? 0;
  }, [assetStats?.total_assets, assetStatsBusinessServiceBuckets, assets.length, businessWorkspace]);
  const perspectiveScopeLabel = useMemo(() => {
    if (menuAxis === "department") {
      return departmentWorkspace === "all" ? t("cmdb.cockpit.scope.departmentAll") : departmentWorkspace;
    }
    if (menuAxis === "business") {
      return businessWorkspace === "all" ? t("cmdb.cockpit.scope.businessAll") : businessWorkspace;
    }
    if (menuAxis === "screen") {
      return t("cmdb.cockpit.scope.screen");
    }
    return functionWorkspace;
  }, [businessWorkspace, departmentWorkspace, functionWorkspace, menuAxis, t]);
  const cockpitOperationalAssets = useMemo(
    () => assetStatsStatusBuckets.find((bucket) => bucket.label === "operational")?.asset_total ?? 0,
    [assetStatsStatusBuckets]
  );
  const cockpitCriticalAssets = useMemo(
    () => (monitoringOverview?.layers ?? []).reduce((sum, layer) => sum + (layer.health.critical ?? 0), 0),
    [monitoringOverview?.layers]
  );
  const visibleSections = useMemo(() => {
    if (activePage === "admin" && !canAccessAdmin) {
      return new Set<string>(consolePageSections.overview);
    }
    return new Set<string>(consolePageSections[activePage]);
  }, [activePage, canAccessAdmin]);
  useEffect(() => {
    if (!authIdentity || !visibleSections.has("section-setup-wizard")) {
      return;
    }
    void refreshSetupWizard();
  }, [authIdentity, refreshSetupWizard, visibleSections]);
  useEffect(() => {
    if (setupTemplates.length === 0) {
      setSelectedSetupTemplateKey("");
      return;
    }

    setSelectedSetupTemplateKey((current) => {
      if (setupTemplates.some((item) => item.key === current)) {
        return current;
      }
      return setupTemplates[0].key;
    });
  }, [setupTemplates]);
  useEffect(() => {
    if (setupProfiles.length === 0) {
      setSelectedSetupProfileKey("");
      return;
    }

    setSelectedSetupProfileKey((current) => {
      if (setupProfiles.some((item) => item.key === current)) {
        return current;
      }
      return setupProfiles[0].key;
    });
  }, [setupProfiles]);
  useEffect(() => {
    setSetupTemplateParamsDraft(buildSetupTemplateDraft(selectedSetupTemplate));
    setSetupTemplatePreview(null);
    setSetupTemplateApplyResult(null);
    setSetupTemplateNotice(null);
  }, [buildSetupTemplateDraft, selectedSetupTemplate?.key]);
  useEffect(() => {
    setSetupProfilePreview(null);
    setSetupProfileApplyResult(null);
    setSetupProfileNotice(null);
  }, [selectedSetupProfileKey]);
  useEffect(() => {
    if (!authIdentity || !visibleSections.has("section-alert-center")) {
      return;
    }
    void Promise.all([loadAlerts(), loadAlertPolicies()]);
  }, [authIdentity, loadAlertPolicies, loadAlerts, visibleSections]);
  useEffect(() => {
    if (alerts.length === 0) {
      setSelectedAlertId("");
      setAlertDetail(null);
      setSelectedAlertIds([]);
      return;
    }

    setSelectedAlertIds((prev) => prev.filter((id) => alerts.some((item) => item.id === id)));

    if (selectedAlertId) {
      const id = Number.parseInt(selectedAlertId, 10);
      if (Number.isFinite(id) && alerts.some((item) => item.id === id)) {
        return;
      }
    }
    setSelectedAlertId(String(alerts[0].id));
  }, [alerts, selectedAlertId]);
  useEffect(() => {
    if (!authIdentity || !visibleSections.has("section-alert-center")) {
      return;
    }
    const alertId = Number.parseInt(selectedAlertId, 10);
    if (!Number.isFinite(alertId) || alertId <= 0) {
      setAlertDetail(null);
      return;
    }
    void loadAlertDetail(alertId);
  }, [authIdentity, loadAlertDetail, selectedAlertId, visibleSections]);
  useEffect(() => {
    if (!authIdentity || !visibleSections.has("section-discovery")) {
      return;
    }
    void Promise.all([loadDiscoveryJobs(), loadDiscoveryCandidates()]);
  }, [authIdentity, loadDiscoveryCandidates, loadDiscoveryJobs, visibleSections]);
  useEffect(() => {
    if (!authIdentity || !visibleSections.has("section-notifications")) {
      return;
    }
    void Promise.all([
      loadNotificationChannels(),
      loadNotificationTemplates(),
      loadNotificationSubscriptions()
    ]);
  }, [
    authIdentity,
    loadNotificationChannels,
    loadNotificationSubscriptions,
    loadNotificationTemplates,
    visibleSections
  ]);
  useEffect(() => {
    if (!authIdentity || !visibleSections.has("section-daily-cockpit")) {
      return;
    }

    void loadDailyCockpitSnapshot();

    const timer = window.setInterval(() => {
      void apiFetch(`${API_BASE_URL}/api/v1/streams/metrics?window_minutes=10&sample_limit=200&scope_limit=5`)
        .catch(() => null)
        .finally(() => {
          void loadDailyCockpitSnapshot();
        });
    }, 30000);

    return () => window.clearInterval(timer);
  }, [authIdentity, loadDailyCockpitSnapshot, visibleSections]);
  useEffect(() => {
    if (!authIdentity || !visibleSections.has("section-workflow")) {
      return;
    }
    void Promise.all([loadWorkflowTemplates(), loadWorkflowRequests()]);
  }, [authIdentity, loadWorkflowRequests, loadWorkflowTemplates, visibleSections]);
  useEffect(() => {
    if (!authIdentity || !visibleSections.has("section-playbook-library")) {
      return;
    }
    void Promise.all([
      loadPlaybookCatalog(),
      loadPlaybookExecutions(),
      loadPlaybookExecutionPolicy(),
      loadPlaybookApprovalRequests()
    ]);
  }, [
    authIdentity,
    loadPlaybookApprovalRequests,
    loadPlaybookCatalog,
    loadPlaybookExecutionPolicy,
    loadPlaybookExecutions,
    playbookCategoryFilter,
    playbookQuery,
    selectedPlaybookKey,
    visibleSections
  ]);
  useEffect(() => {
    if (!authIdentity || !visibleSections.has("section-tickets")) {
      return;
    }
    void Promise.all([
      loadTickets(),
      loadTicketEscalationPolicy(),
      loadTicketEscalationQueue()
    ]);
  }, [authIdentity, loadTicketEscalationPolicy, loadTicketEscalationQueue, loadTickets, visibleSections]);
  useEffect(() => {
    if (!authIdentity || !visibleSections.has("section-tickets")) {
      return;
    }
    const ticketId = Number.parseInt(selectedTicketId, 10);
    if (!Number.isFinite(ticketId) || ticketId <= 0) {
      setTicketDetail(null);
      setTicketEscalationActions([]);
      return;
    }
    void Promise.all([loadTicketDetail(ticketId), loadTicketEscalationActions(ticketId)]);
  }, [authIdentity, loadTicketDetail, loadTicketEscalationActions, selectedTicketId, visibleSections]);
  useEffect(() => {
    if (!authIdentity || !visibleSections.has("section-topology-workspace")) {
      return;
    }
    if (topologyMap) {
      return;
    }
    void loadTopologyMap();
  }, [authIdentity, loadTopologyMap, topologyMap, visibleSections]);
  useEffect(() => {
    if (!authIdentity || !visibleSections.has("section-topology-workspace")) {
      return;
    }
    if (!selectedTopologyMapEdge) {
      setTopologyDiagnostics(null);
      setTopologyDiagnosticsNotice(null);
      return;
    }
    void loadTopologyEdgeDiagnostics(selectedTopologyMapEdge.id);
  }, [
    authIdentity,
    loadTopologyEdgeDiagnostics,
    selectedTopologyMapEdge,
    topologyDiagnosticsWindowMinutes,
    visibleSections
  ]);
  useEffect(() => {
    if (!departmentWorkspaceOptions.includes(departmentWorkspace)) {
      setDepartmentWorkspace("all");
    }
  }, [departmentWorkspace, departmentWorkspaceOptions]);
  useEffect(() => {
    if (!businessWorkspaceOptions.includes(businessWorkspace)) {
      setBusinessWorkspace("all");
    }
  }, [businessWorkspace, businessWorkspaceOptions]);
  useEffect(() => {
    if (menuAxis === "department" && departmentWorkspace !== "all") {
      setDailyCockpitDepartmentFilter(departmentWorkspace);
    }
  }, [departmentWorkspace, menuAxis]);
  const hasMonitoringSourceFilter = useMemo(
    () =>
      monitoringSourceFilters.source_type.trim().length > 0
      || monitoringSourceFilters.site.trim().length > 0
      || monitoringSourceFilters.department.trim().length > 0
      || monitoringSourceFilters.is_enabled !== "all",
    [monitoringSourceFilters]
  );
  const assetStatusOptions = useMemo(
    () => Array.from(new Set(assets.map((item) => item.status).filter((item) => item.trim().length > 0))).sort(),
    [assets]
  );
  const assetClassOptions = useMemo(
    () => Array.from(new Set(assets.map((item) => item.asset_class).filter((item) => item.trim().length > 0))).sort(),
    [assets]
  );
  const assetSiteOptions = useMemo(
    () => Array.from(new Set(assets.map((item) => item.site ?? "").filter((item) => item.trim().length > 0))).sort(),
    [assets]
  );
  const activeDepartmentScope = useMemo(() => {
    if (menuAxis !== "department" || departmentWorkspace === "all") {
      return null;
    }
    return departmentWorkspace;
  }, [departmentWorkspace, menuAxis]);
  const filteredAssets = useMemo(() => {
    const normalizedQuery = assetSearch.trim().toLowerCase();
    const filtered = assets.filter((asset) => {
      if (activeDepartmentScope && (asset.department ?? "") !== activeDepartmentScope) {
        return false;
      }
      if (assetStatusFilter && asset.status !== assetStatusFilter) {
        return false;
      }
      if (assetClassFilter && asset.asset_class !== assetClassFilter) {
        return false;
      }
      if (assetSiteFilter && (asset.site ?? "") !== assetSiteFilter) {
        return false;
      }
      if (!normalizedQuery) {
        return true;
      }

      const haystack = [
        asset.name,
        asset.hostname ?? "",
        asset.ip ?? "",
        asset.owner ?? "",
        asset.site ?? "",
        asset.department ?? "",
        asset.asset_class,
        String(asset.id)
      ]
        .join(" ")
        .toLowerCase();
      return haystack.includes(normalizedQuery);
    });

    filtered.sort((left, right) => {
      if (assetSortMode === "name_asc") {
        return left.name.localeCompare(right.name);
      }
      if (assetSortMode === "id_asc") {
        return left.id - right.id;
      }
      return new Date(right.updated_at).getTime() - new Date(left.updated_at).getTime();
    });

    return filtered;
  }, [activeDepartmentScope, assetClassFilter, assetSearch, assetSiteFilter, assetSortMode, assetStatusFilter, assets]);
  const hasAssetFilter = useMemo(
    () =>
      assetSearch.trim().length > 0
      || assetStatusFilter.length > 0
      || assetClassFilter.length > 0
      || assetSiteFilter.length > 0
      || assetSortMode !== "updated_desc",
    [assetClassFilter, assetSearch, assetSiteFilter, assetSortMode, assetStatusFilter]
  );
  const resetAssetFilters = useCallback(() => {
    setAssetSearch("");
    setAssetStatusFilter("");
    setAssetClassFilter("");
    setAssetSiteFilter("");
    setAssetSortMode("updated_desc");
  }, []);
  const setAlertStatusFilter = useCallback((status: AlertFilterForm["status"]) => {
    setAlertFilters((prev) => ({ ...prev, status }));
  }, []);
  const setAlertSeverityFilter = useCallback((severity: AlertFilterForm["severity"]) => {
    setAlertFilters((prev) => ({ ...prev, severity }));
  }, []);
  const setAlertSuppressedFilter = useCallback((suppressed: AlertFilterForm["suppressed"]) => {
    setAlertFilters((prev) => ({ ...prev, suppressed }));
  }, []);
  const setAlertSiteFilter = useCallback((site: string) => {
    setAlertFilters((prev) => ({ ...prev, site }));
  }, []);
  const setAlertQueryFilter = useCallback((query: string) => {
    setAlertFilters((prev) => ({ ...prev, query }));
  }, []);
  const navigationItems = useMemo(() => {
    const items: Array<{ page: ConsolePage; label: string }> = [
      { page: "setup", label: t("auth.navigation.setup") },
      { page: "overview", label: t("auth.navigation.overview") },
      { page: "cmdb", label: t("auth.navigation.cmdb") },
      { page: "monitoring", label: t("auth.navigation.monitoring") },
      { page: "alerts", label: t("auth.navigation.alerts") },
      { page: "topology", label: t("auth.navigation.topology") },
      { page: "workflow", label: t("auth.navigation.workflow") },
      { page: "tickets", label: t("auth.navigation.tickets") }
    ];

    if (canAccessAdmin) {
      items.push({ page: "admin", label: t("auth.navigation.admin") });
    }

    return items.map((item) => ({
      href: buildConsolePageHash(item.page),
      label: item.label,
      active: item.page === activePage
    }));
  }, [activePage, canAccessAdmin, t]);

  const changeLanguage = useCallback(
    async (language: string) => {
      const normalized = normalizeUiLanguage(language);
      if (normalized === currentLanguage) {
        return;
      }
      await i18n.changeLanguage(normalized);
    },
    [currentLanguage, i18n]
  );

  if (!authSession || !authIdentity) {
    return (
      <AuthGate
        title={t("app.title")}
        subtitle={t("app.subtitle")}
        notice={authNotice}
        error={authError}
      >
        <h2>{t("auth.title")}</h2>
        <p>{t("auth.subtitle")}</p>

        <div className="auth-form-row">
          <label>
            {t("auth.modeLabel")}{" "}
            <select value={loginMode} onChange={(event) => setLoginMode(event.target.value as AuthMode)}>
              <option value="header">{t("auth.modes.header")}</option>
              <option value="bearer">{t("auth.modes.bearer")}</option>
            </select>
          </label>

          {loginMode === "header" ? (
            <label>
              {t("auth.usernameLabel")}{" "}
              <input
                value={loginPrincipal}
                onChange={(event) => setLoginPrincipal(event.target.value)}
                placeholder={t("auth.usernamePlaceholder")}
              />
            </label>
          ) : (
            <label style={{ flex: "1 1 420px" }}>
              {t("auth.tokenLabel")}{" "}
              <input
                value={loginToken}
                onChange={(event) => setLoginToken(event.target.value)}
                placeholder={t("auth.tokenPlaceholder")}
                className="auth-token-input"
              />
            </label>
          )}

          <button onClick={() => void signIn()} disabled={authLoading}>
            {authLoading ? t("auth.signingIn") : t("auth.signIn")}
          </button>
        </div>
      </AuthGate>
    );
  }

  const workflowTicketSectionsProps = {
    addWorkflowStepToDraft,
    approveWorkflowRequest,
    approvingWorkflowRequestId,
    bucketBarWidth,
    canWriteCmdb,
    cellStyle,
    completeWorkflowManualStep,
    createTicket,
    createWorkflowRequest,
    createWorkflowTemplate,
    creatingTicket,
    creatingWorkflowRequest,
    creatingWorkflowTemplate,
    executeWorkflowRequest,
    executingWorkflowRequestId,
    exportWorkflowReportCsv,
    formatSignedDelta,
    loadTicketDetail,
    loadTickets,
    loadTicketEscalationActions,
    loadTicketEscalationPolicy,
    loadTicketEscalationQueue,
    loadPlaybookCatalog,
    loadPlaybookApprovalRequests,
    loadPlaybookExecutionPolicy,
    loadPlaybookExecutions,
    loadWorkflowLogs,
    loadWorkflowRequests,
    loadWorkflowTemplates,
    loadingTicketDetail,
    loadingTickets,
    loadingTicketEscalationActions,
    loadingTicketEscalationPolicy,
    loadingTicketEscalationQueue,
    previewingTicketEscalationPolicy,
    runningTicketEscalation,
    savingTicketEscalationPolicy,
    loadingWorkflowLogs,
    loadingWorkflowRequests,
    loadingWorkflowTemplates,
    manualCompletingWorkflowRequestId,
    newTicket,
    newWorkflowRequest,
    newWorkflowStep,
    newWorkflowTemplateDescription,
    newWorkflowTemplateName,
    newWorkflowTemplateSteps,
    loadingPlaybookCatalog,
    loadingPlaybookApprovals,
    loadingPlaybookExecutions,
    loadingPlaybookPolicy,
    playbookAssetRef,
    playbookCatalog,
    playbookCategoryFilter,
    playbookCategoryOptions,
    playbookConfirmationToken,
    playbookApprovalDecisionNote,
    playbookApprovalRequestNote,
    playbookApprovalRequests,
    playbookApprovalToken,
    playbookDryRunResponse,
    playbookExecutionPolicy,
    playbookExecutionResult,
    approvePlaybookApprovalRequest,
    approvingPlaybookApprovalId,
    playbookExecutions,
    playbookMaintenanceOverrideConfirmed,
    playbookMaintenanceOverrideReason,
    rejectPlaybookApprovalRequest,
    rejectingPlaybookApprovalId,
    requestPlaybookApproval,
    requestingPlaybookApproval,
    selectedPlaybookApprovalId,
    playbookNotice,
    playbookParamsDraft,
    playbookQuery,
    playbookReservationId,
    rejectWorkflowRequest,
    rejectingWorkflowRequestId,
    removeWorkflowStepFromDraft,
    runPlaybookDryRun,
    runPlaybookExecute,
    runningPlaybookDryRun,
    runningPlaybookExecute,
    selectedTicketId,
    selectedPlaybook,
    selectedPlaybookKey,
    selectedPlaybookParamFields,
    selectedTicketSummary,
    selectedWorkflowRequest,
    setPlaybookAssetRef,
    setPlaybookCategoryFilter,
    setPlaybookConfirmationToken,
    setPlaybookApprovalDecisionNote,
    setPlaybookApprovalRequestNote,
    setPlaybookApprovalToken,
    setPlaybookMaintenanceOverrideConfirmed,
    setPlaybookMaintenanceOverrideReason,
    setPlaybookParamsDraft,
    setPlaybookQuery,
    setPlaybookReservationId,
    setSelectedPlaybookApprovalId,
    setSelectedPlaybookKey,
    setNewTicket,
    setNewWorkflowRequest,
    setNewWorkflowStep,
    setNewWorkflowTemplateDescription,
    setNewWorkflowTemplateName,
    setSelectedTicketId,
    setSelectedWorkflowRequestId,
    setTicketPriorityFilter,
    setTicketQueryFilter,
    setTicketStatusDraft,
    setTicketStatusFilter,
    setWorkflowReportRangeDays,
    setWorkflowReportRequesterFilter,
    setWorkflowReportStatusFilter,
    setWorkflowReportTemplateFilter,
    statusChipClass,
    subSectionTitleStyle,
    t,
    ticketDetail,
    ticketEscalationActions,
    ticketEscalationPolicy,
    ticketEscalationPolicyDraft,
    ticketEscalationPreview,
    ticketEscalationPreviewDraft,
    ticketEscalationQueue,
    ticketEscalationRunNote,
    ticketEscalationRunResponse,
    ticketNotice,
    ticketPriorityFilter,
    ticketQueryFilter,
    ticketStatusDraft,
    ticketStatusFilter,
    tickets,
    setTicketEscalationPolicyDraft,
    setTicketEscalationPreviewDraft,
    setTicketEscalationRunNote,
    truncateTopologyLabel,
    previewTicketEscalationPolicy,
    runTicketEscalation,
    updateTicketEscalationPolicy,
    updateTicketStatus,
    updatingTicketStatusId,
    visibleSections,
    workflowDailyTrend,
    workflowDailyTrendMax,
    workflowKpis,
    workflowLogs,
    workflowNotice,
    workflowReportDailyTrend,
    workflowReportDailyTrendMax,
    workflowReportExecutionStats,
    workflowReportRangeDays,
    workflowReportRequesterFilter,
    workflowReportRows,
    workflowReportStatusBuckets,
    workflowReportStatusFilter,
    workflowReportStatusMax,
    workflowReportStatusOptions,
    workflowReportSummary,
    workflowReportTemplateBuckets,
    workflowReportTemplateFilter,
    workflowReportTemplateMax,
    workflowReportTemplateOptions,
    workflowRequesterBuckets,
    workflowRequesterMax,
    workflowRequesterTrendRanks,
    workflowRequests,
    workflowStatusBuckets,
    workflowStatusMax,
    workflowTemplateDisplayName,
    workflowTemplateTrendRanks,
    workflowTemplateUsageBuckets,
    workflowTemplateUsageMax,
    workflowTemplates
  };

  const cmdbSectionsProps = {
    addOwnerDraft,
    assetBindings,
    assetClassFilter,
    assetClassOptions,
    assetImpact,
    assetMonitoring,
    assetNameById,
    assetSearch,
    assetSiteFilter,
    assetSiteOptions,
    assetSortMode,
    assetStats,
    assetStatsBusinessServiceBuckets,
    assetStatsBusinessServiceMax,
    assetStatsDepartmentBuckets,
    assetStatsDepartmentMax,
    assetStatsStatusBuckets,
    assetStatsStatusMax,
    assetStatusFilter,
    assetStatusOptions,
    assets,
    bindingBusinessServicesInput,
    bindingDepartmentsInput,
    bindingNotice,
    bindingOwnerDrafts,
    bucketBarWidth,
    buildTopologyEdgePath,
    canWriteCmdb,
    cellStyle,
    createFieldDefinition,
    createRelation,
    creatingField,
    creatingRelation,
    defaultImpactRelationTypes,
    deleteRelation,
    deletingRelationId,
    emptyState,
    fieldDefinitions,
    filteredAssets,
    hasAssetFilter,
    hierarchyHintEdges,
    impactDepth,
    impactDirection,
    impactNodeNameById,
    impactNotice,
    impactRelationTypes,
    impactRelationTypesInput,
    lifecycleNotice,
    lifecycleStatuses,
    loadAssetBindings,
    loadAssetImpact,
    loadAssetMonitoring,
    loadAssetStats,
    loadRelations,
    loadingAssetBindings,
    loadingAssetImpact,
    loadingAssetMonitoring,
    loadingAssetStats,
    loadingAssets,
    loadingRelations,
    monitoringNotice,
    newField,
    newRelation,
    normalizeOwnerType,
    parseImpactDepth,
    parseImpactRelationTypesInput,
    refreshImpact,
    relationNotice,
    relationSummary,
    relationTypeColor,
    relations,
    removeOwnerDraft,
    renderCustomFields,
    resetAssetFilters,
    saveAssetBindings,
    selectedAsset,
    selectedAssetId,
    selectedAssetNumericId,
    selectedTopologyEdge,
    selectedTopologyEdgeKey,
    setAssetClassFilter,
    setAssetSearch,
    setAssetSiteFilter,
    setAssetSortMode,
    setAssetStatusFilter,
    setBindingBusinessServicesInput,
    setBindingDepartmentsInput,
    setBindingOwnerDrafts,
    setImpactDepth,
    setImpactDirection,
    setImpactRelationTypesInput,
    setNewField,
    setNewRelation,
    setSelectedAssetId,
    setSelectedTopologyEdgeKey,
    subSectionTitleStyle,
    t,
    topologyEdgeKey,
    topologyEdgeRenderMeta,
    topologyNodeFill,
    topologyNodePositions,
    transitionAssetLifecycle,
    transitioningLifecycleStatus,
    triggerAssetMonitoringSync,
    triggeringMonitoringSync,
    truncateTopologyLabel,
    updateOwnerDraftRef,
    updateOwnerDraftType,
    updatingAssetBindings,
    visibleSections
  };

  const integrationMonitoringSectionsProps = {
    assets,
    buildMetricPolylinePoints,
    canWriteCmdb,
    cellStyle,
    createMonitoringSource,
    createNotificationChannel,
    createNotificationSubscription,
    createNotificationTemplate,
    creatingMonitoringSource,
    creatingNotificationChannel,
    creatingNotificationSubscription,
    creatingNotificationTemplate,
    defaultMonitoringSourceFilters,
    discoveryCandidates,
    discoveryJobs,
    discoveryNotice,
    findAssetByCode,
    formatMetricValue,
    hasMonitoringSourceFilter,
    loadDiscoveryCandidates,
    loadDiscoveryJobs,
    loadMonitoringMetrics,
    loadMonitoringSources,
    loadNotificationChannels,
    loadNotificationSubscriptions,
    loadNotificationTemplates,
    loadingDiscoveryCandidates,
    loadingDiscoveryJobs,
    loadingMonitoringMetrics,
    loadingMonitoringSources,
    loadingNotificationChannels,
    loadingNotificationSubscriptions,
    loadingNotificationTemplates,
    monitoringMetrics,
    monitoringMetricsError,
    monitoringMetricsWindowMinutes,
    monitoringMetricsWindowValue,
    monitoringSourceFilters,
    monitoringSourceNotice,
    monitoringSourceStats,
    monitoringSources,
    newMonitoringSource,
    newNotificationChannel,
    newNotificationSubscription,
    newNotificationTemplate,
    notificationChannelById,
    notificationChannelNameById,
    notificationChannels,
    notificationNotice,
    notificationSubscriptions,
    notificationTemplates,
    probeMonitoringSource,
    probingMonitoringSourceId,
    readPayloadString,
    reviewDiscoveryCandidate,
    reviewingCandidateId,
    runDiscoveryJob,
    runningDiscoveryJobId,
    scanCode,
    scanMode,
    scanResult,
    scanning,
    selectedAssetId,
    setMonitoringMetricsWindowMinutes,
    setMonitoringSourceFilters,
    setNewMonitoringSource,
    setNewNotificationChannel,
    setNewNotificationSubscription,
    setNewNotificationTemplate,
    setScanCode,
    setScanMode,
    setSelectedAssetId,
    statusChipClass,
    subSectionTitleStyle,
    t,
    visibleSections
  };

  const setupAlertSectionsProps = {
    alertActionRunningId,
    alertBulkActionRunning,
    alertDetail,
    alertNotice,
    alertQueryFilter: alertFilters.query,
    alertSeverityFilter: alertFilters.severity,
    alertSiteFilter: alertFilters.site,
    alertSuppressedFilter: alertFilters.suppressed,
    alertStatusFilter: alertFilters.status,
    alertPolicies,
    alertPolicyDraft,
    alertPolicyNotice,
    alertPolicyPreview,
    alerts,
    alertsTotal,
    applySetupProfile,
    applySetupTemplate,
    createAlertPolicy,
    canWriteCmdb,
    closeAlert,
    completeSetupWizard,
    creatingAlertPolicy,
    loadingAlertDetail,
    loadingAlerts,
    loadingAlertPolicies,
    loadingSetupChecklist,
    loadingSetupPreflight,
    loadingSetupProfileHistory,
    loadingSetupProfiles,
    loadingSetupTemplates,
    previewSetupProfile,
    previewSetupTemplate,
    previewAlertPolicy,
    previewingAlertPolicy,
    refreshAlerts: loadAlerts,
    refreshAlertPolicies: loadAlertPolicies,
    refreshSetupWizard,
    runningSetupTemplateApply,
    runningSetupTemplatePreview,
    runningSetupProfileApply,
    runningSetupProfilePreview,
    runningSetupProfileRevertId,
    selectedAlertId,
    selectedAlertIds,
    selectedSetupProfileKey,
    selectedSetupTemplateKey,
    setAlertPolicyDraft,
    setAlertQueryFilter,
    setAlertSeverityFilter,
    setAlertSiteFilter,
    setAlertSuppressedFilter,
    setAlertStatusFilter,
    setSelectedSetupTemplateKey,
    setSelectedSetupProfileKey,
    setSetupProfileNote,
    setSetupStep,
    setSetupTemplateNote,
    setSetupTemplateParam,
    setupChecklist,
    setupCompleted,
    setupNotice,
    setupProfileApplyResult,
    setupProfileHistory,
    setupProfileNote,
    setupProfileNotice,
    setupProfilePreview,
    setupProfiles,
    setupTemplateApplyResult,
    setupTemplateNote,
    setupTemplateNotice,
    setupTemplateParamsDraft,
    setupTemplatePreview,
    setupPreflight,
    setupStep,
    setupTemplates,
    revertSetupProfileRun,
    t,
    toggleAlertPolicyEnabled,
    toggleAlertSelection,
    toggleSelectAllAlerts,
    triggerAlertRemediation,
    triggerBulkAcknowledge,
    triggerBulkClose,
    triggerSingleAcknowledge,
    updatingAlertPolicyId,
    visibleSections
  };

  const topologyWorkspaceSectionsProps = {
    buildTopologyEdgePath,
    canWriteCmdb,
    loadTopologyEdgeDiagnostics,
    loadTopologyMap,
    loadingTopologyDiagnostics,
    loadingTopologyMap,
    relationTypeColor,
    runTopologyDiagnosticsAction,
    runningTopologyDiagnosticsActionKey,
    selectedTopologyMapEdge,
    selectedTopologyMapEdgeKey,
    selectedTopologyMapNode,
    selectedTopologyMapNodeId,
    setSelectedTopologyMapEdgeKey,
    setSelectedTopologyMapNodeId,
    setTopologyDepartmentFilter,
    setTopologyDiagnosticsWindowMinutes,
    setTopologyScopeInput,
    setTopologySiteFilter,
    setTopologyWindowLimit,
    setTopologyWindowOffset,
    t,
    topologyDepartmentFilter,
    topologyDiagnostics,
    topologyDiagnosticsNotice,
    topologyDiagnosticsWindowMinutes,
    topologyEdgeKey,
    topologyMap,
    topologyMapEdgeRenderMeta,
    topologyMapEdgesForRender,
    topologyMapNodeEdgeSummary,
    topologyMapNodePositions,
    topologyMapNotice,
    topologyScopeInput,
    topologySiteFilter,
    topologyWindowLimit,
    topologyWindowOffset,
    truncateTopologyLabel,
    visibleSections
  };

  const overviewAdminSectionsProps = {
    activePage,
    assetStats,
    assetStatsBusinessServiceBuckets,
    assetStatsBusinessServiceMax,
    assetStatsDepartmentBuckets,
    assetStatsDepartmentMax,
    assetStatsStatusBuckets,
    assetStatsStatusMax,
    assets,
    bucketBarWidth,
    backupPolicies,
    backupPolicyDraft,
    backupPolicyNotice,
    backupPolicyRuns,
    backupEvidenceCompliancePolicy,
    backupEvidenceCompliancePolicyDraft,
    backupEvidenceComplianceScorecard,
    backupEvidenceComplianceWeekStart,
    backupRestoreEvidence,
    backupRestoreEvidenceCoverage,
    backupRestoreEvidenceDraft,
    backupRestoreEvidenceMissingRunIds,
    backupRestoreRunStatusFilter,
    changeCalendar,
    changeCalendarConflictDraft,
    changeCalendarConflictResult,
    changeCalendarReservationDraft,
    changeCalendarReservations,
    changeCalendarSlotRecommendations,
    changeCalendarEndDate,
    changeCalendarNotice,
    changeCalendarStartDate,
    businessWorkspace,
    businessWorkspaceOptions,
    canAccessAdmin,
    canWriteCmdb,
    closeHandoverCarryoverItem,
    checkChangeCalendarConflicts,
    createChangeCalendarReservation,
    closingHandoverItemKey,
    cockpitCriticalAssets,
    cockpitOperationalAssets,
    createSampleAsset,
    creatingSample,
    dailyCockpitDepartmentFilter,
    dailyCockpitNotice,
    dailyCockpitQueue,
    dailyCockpitSiteFilter,
    nextBestActions,
    runbookTemplates,
    runbookExecutions,
    runbookExecutionPolicy,
    runbookExecutionPolicyDraft,
    runbookExecutionMode,
    selectedRunbookTemplateKey,
    runbookParamDraft,
    runbookPreflightDraft,
    runbookEvidenceDraft,
    runbookNotice,
    incidentCommandDetail,
    incidentCommandDraft,
    incidentCommandNotice,
    incidentCommands,
    exportingHandoverDigest,
    exportingWeeklyDigest,
    exportingBackupEvidenceComplianceScorecard,
    exportBackupEvidenceComplianceScorecard,
    exportHandoverDigest,
    exportHandoverReminders,
    exportWeeklyDigest,
    handoverDigest,
    handoverReminders,
    handoverDigestNotice,
    handoverDigestShiftDate,
    loadBackupPolicies,
    loadRunbookTemplates,
    loadRunbookExecutionPolicy,
    loadRunbookTemplateExecutions,
    loadBackupPolicyRuns,
    loadBackupRestoreEvidence,
    loadBackupEvidenceCompliancePolicy,
    loadBackupEvidenceComplianceScorecard,
    loadChangeCalendar,
    loadChangeCalendarReservations,
    loadChangeCalendarSlotRecommendations,
    loadHandoverDigest,
    loadHandoverReminders,
    loadIncidentCommandDetail,
    loadIncidentCommands,
    loadNextBestActions,
    loadWeeklyDigest,
    completeOpsChecklistItem,
    departmentWorkspace,
    departmentWorkspaceOptions,
    functionWorkspace,
    loadDailyCockpitSnapshot,
    loadOpsChecklist,
    loadAssets,
    loadAssetStats,
    loadFieldDefinitions,
    runBackupPolicy,
    runBackupSchedulerTick,
    executeRunbookTemplate,
    saveRunbookExecutionPolicy,
    closeBackupRestoreEvidence,
    loadingDailyCockpit,
    loadingNextBestActions,
    loadingOpsChecklist,
    loadingIncidentCommandDetail,
    loadingIncidentCommands,
    loadingRunbookTemplates,
    loadingRunbookExecutions,
    loadingRunbookExecutionPolicy,
    executingRunbookTemplate,
    savingRunbookExecutionPolicy,
    loadingHandoverDigest,
    loadingHandoverReminders,
    loadingBackupPolicies,
    loadingBackupPolicyRuns,
    loadingBackupRestoreEvidence,
    loadingBackupEvidenceCompliancePolicy,
    savingBackupEvidenceCompliancePolicy,
    loadingBackupEvidenceComplianceScorecard,
    loadingChangeCalendar,
    checkingChangeCalendarConflict,
    loadingChangeCalendarReservations,
    loadingChangeCalendarRecommendations,
    creatingChangeCalendarReservation,
    loadingWeeklyDigest,
    exportingHandoverReminders,
    runningBackupPolicyActionId,
    loadingAssetStats,
    loadingAssets,
    loadingFields,
    loadingMonitoringOverview,
    menuAxis,
    monitoringOverview,
    monitoringSources,
    opsChecklist,
    opsChecklistDate,
    opsChecklistNotice,
    perspectiveScopeLabel,
    recordOpsChecklistException,
    runningOpsChecklistActionKey,
    selectedBusinessAssetCount,
    selectedDepartmentAssetCount,
    runDailyCockpitAction,
    runningDailyCockpitActionKey,
    saveIncidentCommand,
    savingIncidentCommand,
    saveBackupPolicy,
    saveBackupEvidenceCompliancePolicy,
    savingBackupPolicy,
    saveBackupRestoreEvidence,
    savingBackupRestoreEvidence,
    setBusinessWorkspace,
    setBackupEvidenceCompliancePolicyDraft,
    setBackupEvidenceComplianceWeekStart,
    setBackupPolicyDraft,
    setBackupRestoreEvidenceDraft,
    setBackupRestoreRunStatusFilter,
    setChangeCalendarConflictDraft,
    setChangeCalendarReservationDraft,
    setChangeCalendarEndDate,
    setChangeCalendarStartDate,
    setDailyCockpitDepartmentFilter,
    setDailyCockpitSiteFilter,
    setDepartmentWorkspace,
    setFunctionWorkspace,
    setHandoverDigestShiftDate,
    setIncidentCommandDraft,
    setSelectedRunbookTemplateKey,
    setRunbookExecutionMode,
    setRunbookExecutionPolicyDraft,
    setRunbookParamDraft,
    setRunbookPreflightDraft,
    setRunbookEvidenceDraft,
    setMenuAxis,
    setOpsChecklistDate,
    setSelectedIncidentAlertId,
    setWeeklyDigestWeekStart,
    selectedIncidentAlertId,
    subSectionTitleStyle,
    t,
    tickingBackupScheduler,
    weeklyDigest,
    weeklyDigestNotice,
    weeklyDigestWeekStart,
    visibleSections
  };

  return (
    <AppShell
      title={t("app.title")}
      subtitle={t("app.subtitle")}
      statusText={t("auth.status", { username: authIdentity.user.username, roles: roleText })}
      modeText={t("auth.statusMode", { mode: `${authSession.mode} | ${t(`auth.navigation.${activePage}`)}` })}
      signOutLabel={t("auth.signOut")}
      onSignOut={() => void signOut()}
      navigationItems={navigationItems}
      topbarActions={(
        <>
          <label className="topbar-language">
            <span>{t("auth.language.label")}</span>
            <select
              value={currentLanguage}
              onChange={(event) => {
                void changeLanguage(event.target.value);
              }}
              aria-label={t("auth.language.label")}
            >
              <option value="en-US">{t("auth.language.options.en-US")}</option>
              <option value="zh-CN">{t("auth.language.options.zh-CN")}</option>
            </select>
          </label>
          <button onClick={() => void signOut()}>{t("auth.signOut")}</button>
        </>
      )}
      notice={authNotice}
      error={error ? `${t("cmdb.messages.error")}: ${error}` : null}
      warning={!canWriteCmdb ? t("auth.messages.readOnly") : null}
    >
      <OverviewAdminSections {...overviewAdminSectionsProps} />
      <SetupAlertSections {...setupAlertSectionsProps} />
      <WorkflowTicketSections {...workflowTicketSectionsProps} />
      <IntegrationMonitoringSections {...integrationMonitoringSectionsProps} />
      <TopologyWorkspaceSections {...topologyWorkspaceSectionsProps} />
      <CmdbSections {...cmdbSectionsProps} />
    </AppShell>
  );
}

function normalizeUiLanguage(language: string): UiLanguage {
  const normalized = language.trim();
  if (SUPPORTED_UI_LANGUAGES.some((item) => item === normalized)) {
    return normalized as UiLanguage;
  }
  if (normalized.toLowerCase().startsWith("zh")) {
    return "zh-CN";
  }
  return "en-US";
}

function normalizeWorkflowStatus(value: string): string {
  return value.trim().toLowerCase();
}

function isWorkflowSuccessStatus(status: string): boolean {
  return status === "completed" || status === "succeeded" || status === "success";
}

function isWorkflowFailureStatus(status: string): boolean {
  return (
    status === "failed"
    || status === "error"
    || status === "rejected"
    || status === "cancelled"
    || status === "timeout"
  );
}

function formatLocalDateTimeInput(date: Date): string {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  const hours = String(date.getHours()).padStart(2, "0");
  const minutes = String(date.getMinutes()).padStart(2, "0");
  return `${year}-${month}-${day}T${hours}:${minutes}`;
}

function localDateTimeInputToUtcRfc3339(value: string): string | null {
  const trimmed = value.trim();
  if (!trimmed) {
    return null;
  }
  const local = new Date(trimmed);
  const timestamp = local.getTime();
  if (!Number.isFinite(timestamp)) {
    return null;
  }
  return new Date(timestamp).toISOString();
}

function formatLocalDateKey(date: Date): string {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

type PlaybookParamNormalizeResult =
  | { ok: true; value: Record<string, unknown> }
  | { ok: false; error: string };

function extractPlaybookParamFields(schema: PlaybookParameterSchema | null): PlaybookParameterField[] {
  if (!schema || !Array.isArray(schema.fields)) {
    return [];
  }
  return schema.fields
    .filter((field) => typeof field?.key === "string" && typeof field?.type === "string")
    .map((field) => ({
      ...field,
      key: field.key.trim()
    }))
    .filter((field) => field.key.length > 0);
}

function normalizePlaybookParamDraft(
  fields: PlaybookParameterField[],
  draft: Record<string, string>
): PlaybookParamNormalizeResult {
  const payload: Record<string, unknown> = {};

  for (const field of fields) {
    const rawInput = (draft[field.key] ?? "").trim();
    const hasInput = rawInput.length > 0;
    const effectiveInput = hasInput
      ? rawInput
      : (field.default !== undefined && field.default !== null ? String(field.default) : "");

    if (!hasInput && field.required && effectiveInput.length === 0) {
      return {
        ok: false,
        error: `Playbook param '${field.key}' is required.`
      };
    }
    if (effectiveInput.length === 0) {
      continue;
    }

    if (field.type === "string") {
      if (typeof field.max_length === "number" && effectiveInput.length > field.max_length) {
        return {
          ok: false,
          error: `Playbook param '${field.key}' length must be <= ${field.max_length}.`
        };
      }
      payload[field.key] = effectiveInput;
      continue;
    }

    if (field.type === "integer") {
      const parsed = Number.parseInt(effectiveInput, 10);
      if (!Number.isFinite(parsed)) {
        return { ok: false, error: `Playbook param '${field.key}' must be an integer.` };
      }
      if (typeof field.min === "number" && parsed < field.min) {
        return { ok: false, error: `Playbook param '${field.key}' must be >= ${field.min}.` };
      }
      if (typeof field.max === "number" && parsed > field.max) {
        return { ok: false, error: `Playbook param '${field.key}' must be <= ${field.max}.` };
      }
      payload[field.key] = parsed;
      continue;
    }

    if (field.type === "number") {
      const parsed = Number.parseFloat(effectiveInput);
      if (!Number.isFinite(parsed)) {
        return { ok: false, error: `Playbook param '${field.key}' must be numeric.` };
      }
      if (typeof field.min === "number" && parsed < field.min) {
        return { ok: false, error: `Playbook param '${field.key}' must be >= ${field.min}.` };
      }
      if (typeof field.max === "number" && parsed > field.max) {
        return { ok: false, error: `Playbook param '${field.key}' must be <= ${field.max}.` };
      }
      payload[field.key] = parsed;
      continue;
    }

    if (field.type === "boolean") {
      const parsed = parseBooleanDraft(effectiveInput);
      if (parsed === null) {
        return {
          ok: false,
          error: `Playbook param '${field.key}' must be true/false.`
        };
      }
      payload[field.key] = parsed;
      continue;
    }

    if (field.type === "enum") {
      const options = Array.isArray(field.options) ? field.options : [];
      if (!options.includes(effectiveInput)) {
        return {
          ok: false,
          error: `Playbook param '${field.key}' must be one of: ${options.join(", ")}`
        };
      }
      payload[field.key] = effectiveInput;
      continue;
    }
  }

  return { ok: true, value: payload };
}

function parseBooleanDraft(value: string): boolean | null {
  const normalized = value.trim().toLowerCase();
  if (["true", "1", "yes", "y", "on"].includes(normalized)) {
    return true;
  }
  if (["false", "0", "no", "n", "off"].includes(normalized)) {
    return false;
  }
  return null;
}

const subSectionTitleStyle: CSSProperties = {
  marginTop: 0,
  marginBottom: "0.5rem",
  fontSize: "1rem"
};

const cellStyle: CSSProperties = {
  border: "1px solid #ddd",
  padding: "0.5rem",
  textAlign: "left",
  whiteSpace: "nowrap",
  verticalAlign: "top"
};
