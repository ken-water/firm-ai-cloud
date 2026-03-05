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

type TicketDetailResponse = {
  ticket: TicketRecord;
  asset_links: TicketAssetLink[];
  alert_links: TicketAlertLink[];
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
};

type AlertBulkActionResponse = {
  action: string;
  requested: number;
  updated: number;
  skipped: number;
  updated_ids: number[];
  skipped_ids: number[];
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
  site: "",
  query: ""
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

const lifecycleStatuses: LifecycleStatus[] = [
  "idle",
  "onboarding",
  "operational",
  "maintenance",
  "retired"
];

const defaultImpactRelationTypes = ["contains", "depends_on", "runs_service", "owned_by"];

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

  const loadTickets = useCallback(async () => {
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
    await Promise.all([loadSetupPreflight(), loadSetupChecklist()]);
  }, [loadSetupChecklist, loadSetupPreflight]);

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
      await loadTickets();
      setTicketNotice(`Ticket ${payload.ticket.ticket_no} created.`);
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setCreatingTicket(false);
    }
  }, [canWriteCmdb, loadTickets, newTicket, t]);

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
      await loadTickets();
      setTicketNotice(`Ticket ${payload.ticket.ticket_no} moved to '${payload.ticket.status}'.`);
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setUpdatingTicketStatusId(null);
    }
  }, [canWriteCmdb, loadTickets, t]);

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
    if (!authIdentity || !visibleSections.has("section-alert-center")) {
      return;
    }
    void loadAlerts();
  }, [authIdentity, loadAlerts, visibleSections]);
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
    if (!authIdentity || !visibleSections.has("section-workflow")) {
      return;
    }
    void Promise.all([loadWorkflowTemplates(), loadWorkflowRequests()]);
  }, [authIdentity, loadWorkflowRequests, loadWorkflowTemplates, visibleSections]);
  useEffect(() => {
    if (!authIdentity || !visibleSections.has("section-tickets")) {
      return;
    }
    void loadTickets();
  }, [authIdentity, loadTickets, visibleSections]);
  useEffect(() => {
    if (!authIdentity || !visibleSections.has("section-tickets")) {
      return;
    }
    const ticketId = Number.parseInt(selectedTicketId, 10);
    if (!Number.isFinite(ticketId) || ticketId <= 0) {
      setTicketDetail(null);
      return;
    }
    void loadTicketDetail(ticketId);
  }, [authIdentity, loadTicketDetail, selectedTicketId, visibleSections]);
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
    if (!departmentWorkspaceOptions.includes(departmentWorkspace)) {
      setDepartmentWorkspace("all");
    }
  }, [departmentWorkspace, departmentWorkspaceOptions]);
  useEffect(() => {
    if (!businessWorkspaceOptions.includes(businessWorkspace)) {
      setBusinessWorkspace("all");
    }
  }, [businessWorkspace, businessWorkspaceOptions]);
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
    loadWorkflowLogs,
    loadWorkflowRequests,
    loadWorkflowTemplates,
    loadingTicketDetail,
    loadingTickets,
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
    rejectWorkflowRequest,
    rejectingWorkflowRequestId,
    removeWorkflowStepFromDraft,
    selectedTicketId,
    selectedTicketSummary,
    selectedWorkflowRequest,
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
    ticketNotice,
    ticketPriorityFilter,
    ticketQueryFilter,
    ticketStatusDraft,
    ticketStatusFilter,
    tickets,
    truncateTopologyLabel,
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
    alertStatusFilter: alertFilters.status,
    alerts,
    alertsTotal,
    canWriteCmdb,
    closeAlert,
    completeSetupWizard,
    loadingAlertDetail,
    loadingAlerts,
    loadingSetupChecklist,
    loadingSetupPreflight,
    refreshAlerts: loadAlerts,
    refreshSetupWizard,
    selectedAlertId,
    selectedAlertIds,
    setAlertQueryFilter,
    setAlertSeverityFilter,
    setAlertSiteFilter,
    setAlertStatusFilter,
    setSetupStep,
    setupChecklist,
    setupCompleted,
    setupNotice,
    setupPreflight,
    setupStep,
    t,
    toggleAlertSelection,
    toggleSelectAllAlerts,
    triggerBulkAcknowledge,
    triggerBulkClose,
    triggerSingleAcknowledge,
    visibleSections
  };

  const topologyWorkspaceSectionsProps = {
    buildTopologyEdgePath,
    canWriteCmdb,
    loadTopologyMap,
    loadingTopologyMap,
    relationTypeColor,
    selectedTopologyMapEdge,
    selectedTopologyMapEdgeKey,
    selectedTopologyMapNode,
    selectedTopologyMapNodeId,
    setSelectedTopologyMapEdgeKey,
    setSelectedTopologyMapNodeId,
    setTopologyDepartmentFilter,
    setTopologyScopeInput,
    setTopologySiteFilter,
    setTopologyWindowLimit,
    setTopologyWindowOffset,
    t,
    topologyDepartmentFilter,
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
    businessWorkspace,
    businessWorkspaceOptions,
    canAccessAdmin,
    canWriteCmdb,
    cockpitCriticalAssets,
    cockpitOperationalAssets,
    createSampleAsset,
    creatingSample,
    departmentWorkspace,
    departmentWorkspaceOptions,
    functionWorkspace,
    loadAssets,
    loadAssetStats,
    loadFieldDefinitions,
    loadingAssetStats,
    loadingAssets,
    loadingFields,
    loadingMonitoringOverview,
    menuAxis,
    monitoringOverview,
    monitoringSources,
    perspectiveScopeLabel,
    selectedBusinessAssetCount,
    selectedDepartmentAssetCount,
    setBusinessWorkspace,
    setDepartmentWorkspace,
    setFunctionWorkspace,
    setMenuAxis,
    subSectionTitleStyle,
    t,
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

function formatLocalDateKey(date: Date): string {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
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
