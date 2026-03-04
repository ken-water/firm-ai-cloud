import { useCallback, useEffect, useMemo, useState } from "react";
import type { CSSProperties } from "react";
import { useTranslation } from "react-i18next";
import { AppShell, AuthGate, SectionCard } from "./components/layout";

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

type DiscoveryJob = {
  id: number;
  name: string;
  source_type: string;
  scope: Record<string, unknown>;
  schedule: string | null;
  status: string;
  is_enabled: boolean;
  last_run_at: string | null;
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

type MenuAxis = "function" | "department" | "business" | "screen";
type FunctionWorkspace = "full" | "cmdb" | "monitoring" | "workflow";
type ConsolePage = "overview" | "cmdb" | "monitoring" | "workflow" | "admin";

type AssetSortMode = "updated_desc" | "name_asc" | "id_asc";
type LifecycleStatus = "idle" | "onboarding" | "operational" | "maintenance" | "retired";
type ImpactDirection = "downstream" | "upstream" | "both";
type OwnerType = "team" | "user" | "group" | "external";

type OwnerDraft = {
  key: string;
  owner_type: OwnerType;
  owner_ref: string;
};

const DEFAULT_API_BASE_URL =
  typeof window !== "undefined"
    ? `${window.location.protocol}//${window.location.hostname}:8080`
    : "http://127.0.0.1:8080";
const API_BASE_URL = import.meta.env.VITE_API_BASE_URL?.trim() || DEFAULT_API_BASE_URL;
const API_AUTH_USER = (import.meta.env.VITE_AUTH_USER ?? "admin").trim();
const API_AUTH_TOKEN = (import.meta.env.VITE_AUTH_TOKEN ?? "").trim();
const AUTH_SESSION_STORAGE_KEY = "cloudops.auth.session.v1";
const AUTH_SESSION_EXPIRED_EVENT = "cloudops.auth.session-expired";

type AuthMode = "header" | "bearer";

type AuthSession = {
  mode: AuthMode;
  principal: string;
  token: string | null;
};

type AuthIdentity = {
  user: {
    id: number;
    username: string;
    display_name: string | null;
    email: string | null;
  };
  roles: string[];
};

const runtimeDefaultSession = deriveDefaultAuthSession();
let runtimeAuthSession: AuthSession | null = loadStoredAuthSession() ?? runtimeDefaultSession;

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

const lifecycleStatuses: LifecycleStatus[] = [
  "idle",
  "onboarding",
  "operational",
  "maintenance",
  "retired"
];

const defaultImpactRelationTypes = ["contains", "depends_on", "runs_service", "owned_by"];
const defaultConsolePage: ConsolePage = "overview";

const consolePageSections: Record<ConsolePage, string[]> = {
  overview: ["section-cockpit", "section-monitoring-metrics", "section-topology", "section-asset-stats"],
  cmdb: [
    "section-scan",
    "section-fields",
    "section-relations",
    "section-readiness",
    "section-topology",
    "section-asset-stats",
    "section-assets"
  ],
  monitoring: ["section-cockpit", "section-monitoring-sources", "section-monitoring-metrics", "section-topology"],
  workflow: [
    "section-workflow-cockpit",
    "section-workflow-reports",
    "section-workflow",
    "section-discovery",
    "section-notifications"
  ],
  admin: ["section-admin"]
};

const legacySectionToPage: Record<string, ConsolePage> = {
  "section-admin": "admin",
  "section-workflow-cockpit": "workflow",
  "section-workflow-reports": "workflow",
  "section-workflow": "workflow",
  "section-discovery": "workflow",
  "section-notifications": "workflow",
  "section-monitoring-sources": "monitoring",
  "section-monitoring-metrics": "monitoring",
  "section-scan": "cmdb",
  "section-fields": "cmdb",
  "section-relations": "cmdb",
  "section-readiness": "cmdb",
  "section-assets": "cmdb",
  "section-cockpit": "overview",
  "section-topology": "overview",
  "section-asset-stats": "overview"
};

export function App() {
  const { t } = useTranslation();
  const [authSession, setAuthSession] = useState<AuthSession | null>(runtimeAuthSession);
  const [authIdentity, setAuthIdentity] = useState<AuthIdentity | null>(null);
  const [authLoading, setAuthLoading] = useState(false);
  const [authError, setAuthError] = useState<string | null>(null);
  const [authNotice, setAuthNotice] = useState<string | null>(null);
  const [loginMode, setLoginMode] = useState<AuthMode>(runtimeAuthSession?.mode ?? "header");
  const [loginPrincipal, setLoginPrincipal] = useState<string>(
    runtimeAuthSession?.mode === "header" ? runtimeAuthSession.principal : API_AUTH_USER
  );
  const [loginToken, setLoginToken] = useState<string>(
    runtimeAuthSession?.mode === "bearer" ? runtimeAuthSession.token ?? "" : API_AUTH_TOKEN
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
    runtimeAuthSession = session;
    persistAuthSession(session);
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
    if (runtimeAuthSession?.mode === "bearer" && runtimeAuthSession.token) {
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

    const relationTypes = parseImpactRelationTypesInput(impactRelationTypesInput);
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
    const relationTypes = parseImpactRelationTypesInput(impactRelationTypesInput);
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
    () => parseImpactRelationTypesInput(impactRelationTypesInput),
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
  const navigationItems = useMemo(() => {
    const items: Array<{ page: ConsolePage; label: string }> = [
      { page: "overview", label: t("auth.navigation.overview") },
      { page: "cmdb", label: t("auth.navigation.cmdb") },
      { page: "monitoring", label: t("auth.navigation.monitoring") },
      { page: "workflow", label: t("auth.navigation.workflow") }
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

  return (
    <AppShell
      title={t("app.title")}
      subtitle={t("app.subtitle")}
      statusText={t("auth.status", { username: authIdentity.user.username, roles: roleText })}
      modeText={t("auth.statusMode", { mode: `${authSession.mode} | ${t(`auth.navigation.${activePage}`)}` })}
      signOutLabel={t("auth.signOut")}
      onSignOut={() => void signOut()}
      navigationItems={navigationItems}
      notice={authNotice}
      error={error ? `${t("cmdb.messages.error")}: ${error}` : null}
      warning={!canWriteCmdb ? t("auth.messages.readOnly") : null}
    >

      {canAccessAdmin && (
        <>
          {visibleSections.has("section-admin") && (
            <SectionCard id="section-admin" title={t("auth.adminPanel.title")}>
              <p style={{ marginTop: 0 }}>{t("auth.adminPanel.description")}</p>
            </SectionCard>
          )}
        </>
      )}

      {activePage !== "admin" && (
        <SectionCard>
          <div className="toolbar-row">
            <button onClick={() => void Promise.all([loadAssets(), loadAssetStats()])} disabled={loadingAssets || loadingAssetStats}>
              {loadingAssets || loadingAssetStats ? t("cmdb.actions.loading") : t("cmdb.actions.refreshAssets")}
            </button>
            <button onClick={() => void loadFieldDefinitions()} disabled={loadingFields}>
              {loadingFields ? t("cmdb.actions.loading") : t("cmdb.actions.refreshFields")}
            </button>
            {canWriteCmdb && (
              <button onClick={() => void createSampleAsset()} disabled={creatingSample}>
                {creatingSample ? t("cmdb.actions.creating") : t("cmdb.actions.createSample")}
              </button>
            )}
          </div>
        </SectionCard>
      )}

      {activePage === "overview" && (
      <SectionCard id="section-perspective" title={t("cmdb.perspective.title")}>
        <div className="filter-grid">
          <label className="control-field">
            <span>{t("cmdb.perspective.menuAxis")}</span>
            <select value={menuAxis} onChange={(event) => setMenuAxis(event.target.value as MenuAxis)}>
              <option value="function">{t("cmdb.perspective.menuAxisOptions.function")}</option>
              <option value="department">{t("cmdb.perspective.menuAxisOptions.department")}</option>
              <option value="business">{t("cmdb.perspective.menuAxisOptions.business")}</option>
              <option value="screen">{t("cmdb.perspective.menuAxisOptions.screen")}</option>
            </select>
          </label>
          {menuAxis === "function" && (
            <label className="control-field">
              <span>{t("cmdb.perspective.functionWorkspace")}</span>
              <select
                value={functionWorkspace}
                onChange={(event) => setFunctionWorkspace(event.target.value as FunctionWorkspace)}
              >
                <option value="full">{t("cmdb.perspective.functionWorkspaceOptions.full")}</option>
                <option value="cmdb">{t("cmdb.perspective.functionWorkspaceOptions.cmdb")}</option>
                <option value="monitoring">{t("cmdb.perspective.functionWorkspaceOptions.monitoring")}</option>
                <option value="workflow">{t("cmdb.perspective.functionWorkspaceOptions.workflow")}</option>
              </select>
            </label>
          )}
          {menuAxis === "department" && (
            <label className="control-field">
              <span>{t("cmdb.perspective.departmentWorkspace")}</span>
              <select value={departmentWorkspace} onChange={(event) => setDepartmentWorkspace(event.target.value)}>
                {departmentWorkspaceOptions.map((item) => (
                  <option key={`dept-workspace-${item}`} value={item}>
                    {item === "all" ? t("cmdb.perspective.workspaceAll") : item}
                  </option>
                ))}
              </select>
            </label>
          )}
          {menuAxis === "business" && (
            <label className="control-field">
              <span>{t("cmdb.perspective.businessWorkspace")}</span>
              <select value={businessWorkspace} onChange={(event) => setBusinessWorkspace(event.target.value)}>
                {businessWorkspaceOptions.map((item) => (
                  <option key={`biz-workspace-${item}`} value={item}>
                    {item === "all" ? t("cmdb.perspective.workspaceAll") : item}
                  </option>
                ))}
              </select>
            </label>
          )}
        </div>
        <p className="section-note">
          {t("cmdb.perspective.summary", {
            axis: menuAxis,
            scope: perspectiveScopeLabel
          })}
        </p>
      </SectionCard>
      )}

      {visibleSections.has("section-cockpit") && (
        <SectionCard id="section-cockpit" title={t("cmdb.cockpit.title")}>
          <div className="detail-grid">
            <div className="detail-panel">
              <h3 style={subSectionTitleStyle}>{t("cmdb.cockpit.cards.assetTotal")}</h3>
              <p style={{ fontSize: "2rem", margin: "0.2rem 0" }}>{assetStats?.total_assets ?? assets.length}</p>
              <p className="section-note">{t("cmdb.cockpit.scopeLabel", { value: perspectiveScopeLabel })}</p>
            </div>
            <div className="detail-panel">
              <h3 style={subSectionTitleStyle}>{t("cmdb.cockpit.cards.operational")}</h3>
              <p style={{ fontSize: "2rem", margin: "0.2rem 0" }}>{cockpitOperationalAssets}</p>
              <p className="section-note">{t("cmdb.cockpit.cards.monitored", { value: monitoringOverview?.summary.monitored_asset_total ?? 0 })}</p>
            </div>
            <div className="detail-panel">
              <h3 style={subSectionTitleStyle}>{t("cmdb.cockpit.cards.critical")}</h3>
              <p style={{ fontSize: "2rem", margin: "0.2rem 0" }}>{cockpitCriticalAssets}</p>
              <p className="section-note">{t("cmdb.cockpit.cards.sources", { value: monitoringOverview?.summary.source_total ?? monitoringSources.length })}</p>
            </div>
            <div className="detail-panel">
              <h3 style={subSectionTitleStyle}>{t("cmdb.cockpit.cards.responsibility")}</h3>
              <p style={{ fontSize: "2rem", margin: "0.2rem 0" }}>
                {menuAxis === "department" ? selectedDepartmentAssetCount : selectedBusinessAssetCount}
              </p>
              <p className="section-note">
                {menuAxis === "department"
                  ? t("cmdb.cockpit.cards.departmentAssets")
                  : menuAxis === "business"
                    ? t("cmdb.cockpit.cards.businessAssets")
                    : t("cmdb.cockpit.cards.globalAssets")}
              </p>
            </div>
          </div>

          <div className="detail-grid" style={{ marginTop: "0.75rem" }}>
            <div className="detail-panel">
              <h3 style={subSectionTitleStyle}>{t("cmdb.cockpit.charts.status")}</h3>
              {assetStatsStatusBuckets.length === 0 ? (
                <p>{t("cmdb.assetStats.messages.noBuckets")}</p>
              ) : (
                assetStatsStatusBuckets.slice(0, 6).map((bucket) => (
                  <div key={`cockpit-status-${bucket.key}`} style={{ display: "grid", gridTemplateColumns: "120px 1fr auto", gap: "0.5rem", marginBottom: "0.35rem", alignItems: "center" }}>
                    <span>{bucket.label}</span>
                    <div style={{ background: "#e2e8f0", height: "8px", borderRadius: "999px", overflow: "hidden" }}>
                      <div style={{ width: bucketBarWidth(bucket.asset_total, assetStatsStatusMax), height: "100%", background: "#1d4ed8" }} />
                    </div>
                    <span>{bucket.asset_total}</span>
                  </div>
                ))
              )}
            </div>
            <div className="detail-panel">
              <h3 style={subSectionTitleStyle}>{t("cmdb.cockpit.charts.department")}</h3>
              {assetStatsDepartmentBuckets.length === 0 ? (
                <p>{t("cmdb.assetStats.messages.noBuckets")}</p>
              ) : (
                assetStatsDepartmentBuckets.slice(0, 6).map((bucket) => (
                  <div key={`cockpit-dept-${bucket.key}`} style={{ display: "grid", gridTemplateColumns: "120px 1fr auto", gap: "0.5rem", marginBottom: "0.35rem", alignItems: "center" }}>
                    <span>{bucket.label}</span>
                    <div style={{ background: "#e2e8f0", height: "8px", borderRadius: "999px", overflow: "hidden" }}>
                      <div style={{ width: bucketBarWidth(bucket.asset_total, assetStatsDepartmentMax), height: "100%", background: "#0f766e" }} />
                    </div>
                    <span>{bucket.asset_total}</span>
                  </div>
                ))
              )}
            </div>
            <div className="detail-panel">
              <h3 style={subSectionTitleStyle}>{t("cmdb.cockpit.charts.business")}</h3>
              {assetStatsBusinessServiceBuckets.length === 0 ? (
                <p>{t("cmdb.assetStats.messages.noBuckets")}</p>
              ) : (
                assetStatsBusinessServiceBuckets.slice(0, 6).map((bucket) => (
                  <div key={`cockpit-biz-${bucket.key}`} style={{ display: "grid", gridTemplateColumns: "120px 1fr auto", gap: "0.5rem", marginBottom: "0.35rem", alignItems: "center" }}>
                    <span title={bucket.label} style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{bucket.label}</span>
                    <div style={{ background: "#e2e8f0", height: "8px", borderRadius: "999px", overflow: "hidden" }}>
                      <div style={{ width: bucketBarWidth(bucket.asset_total, assetStatsBusinessServiceMax), height: "100%", background: "#be123c" }} />
                    </div>
                    <span>{bucket.asset_total}</span>
                  </div>
                ))
              )}
            </div>
          </div>
          {loadingMonitoringOverview && <p className="inline-note">{t("cmdb.cockpit.messages.loadingOverview")}</p>}
        </SectionCard>
      )}

      {visibleSections.has("section-workflow-cockpit") && (
        <SectionCard id="section-workflow-cockpit" title={t("cmdb.workflow.cockpit.title")}>
          <p className="section-note">
            {t("cmdb.workflow.cockpit.summary", {
              requests: workflowKpis.totalRequests,
              active: workflowKpis.activeRequests,
              completed: workflowKpis.completedRequests,
              failed: workflowKpis.failedRequests
            })}
          </p>

          <div className="detail-grid">
            <div className="detail-panel">
              <h3 style={subSectionTitleStyle}>{t("cmdb.workflow.cockpit.cards.totalRequests")}</h3>
              <p style={{ fontSize: "2rem", margin: "0.2rem 0" }}>{workflowKpis.totalRequests}</p>
            </div>
            <div className="detail-panel">
              <h3 style={subSectionTitleStyle}>{t("cmdb.workflow.cockpit.cards.activeRequests")}</h3>
              <p style={{ fontSize: "2rem", margin: "0.2rem 0" }}>{workflowKpis.activeRequests}</p>
            </div>
            <div className="detail-panel">
              <h3 style={subSectionTitleStyle}>{t("cmdb.workflow.cockpit.cards.approvalQueue")}</h3>
              <p style={{ fontSize: "2rem", margin: "0.2rem 0" }}>{workflowKpis.approvalQueue}</p>
            </div>
            <div className="detail-panel">
              <h3 style={subSectionTitleStyle}>{t("cmdb.workflow.cockpit.cards.manualQueue")}</h3>
              <p style={{ fontSize: "2rem", margin: "0.2rem 0" }}>{workflowKpis.manualQueue}</p>
            </div>
            <div className="detail-panel">
              <h3 style={subSectionTitleStyle}>{t("cmdb.workflow.cockpit.cards.completionRate")}</h3>
              <p style={{ fontSize: "2rem", margin: "0.2rem 0" }}>{workflowKpis.completionRate}%</p>
            </div>
            <div className="detail-panel">
              <h3 style={subSectionTitleStyle}>{t("cmdb.workflow.cockpit.cards.automationShare")}</h3>
              <p style={{ fontSize: "2rem", margin: "0.2rem 0" }}>{workflowKpis.automationShare}%</p>
            </div>
          </div>

          <div className="detail-grid" style={{ marginTop: "0.75rem" }}>
            <div className="detail-panel">
              <h3 style={subSectionTitleStyle}>{t("cmdb.workflow.cockpit.charts.status")}</h3>
              {workflowStatusBuckets.length === 0 ? (
                <p>{t("cmdb.workflow.messages.noRequests")}</p>
              ) : (
                workflowStatusBuckets.slice(0, 8).map((bucket) => (
                  <div
                    key={`workflow-status-${bucket.key}`}
                    style={{
                      display: "grid",
                      gridTemplateColumns: "160px 1fr auto",
                      gap: "0.5rem",
                      marginBottom: "0.35rem",
                      alignItems: "center"
                    }}
                  >
                    <span title={bucket.label} style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                      {bucket.label}
                    </span>
                    <div style={{ background: "#e2e8f0", height: "8px", borderRadius: "999px", overflow: "hidden" }}>
                      <div style={{ width: bucketBarWidth(bucket.asset_total, workflowStatusMax), height: "100%", background: "#1d4ed8" }} />
                    </div>
                    <span>{bucket.asset_total}</span>
                  </div>
                ))
              )}
            </div>

            <div className="detail-panel">
              <h3 style={subSectionTitleStyle}>{t("cmdb.workflow.cockpit.charts.templateUsage")}</h3>
              {workflowTemplateUsageBuckets.length === 0 ? (
                <p>{t("cmdb.workflow.messages.noRequests")}</p>
              ) : (
                workflowTemplateUsageBuckets.slice(0, 8).map((bucket) => (
                  <div
                    key={`workflow-template-usage-${bucket.key}`}
                    style={{
                      display: "grid",
                      gridTemplateColumns: "180px 1fr auto",
                      gap: "0.5rem",
                      marginBottom: "0.35rem",
                      alignItems: "center"
                    }}
                  >
                    <span title={bucket.label} style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                      {bucket.label}
                    </span>
                    <div style={{ background: "#e2e8f0", height: "8px", borderRadius: "999px", overflow: "hidden" }}>
                      <div style={{ width: bucketBarWidth(bucket.asset_total, workflowTemplateUsageMax), height: "100%", background: "#0f766e" }} />
                    </div>
                    <span>{bucket.asset_total}</span>
                  </div>
                ))
              )}
            </div>

            <div className="detail-panel">
              <h3 style={subSectionTitleStyle}>{t("cmdb.workflow.cockpit.charts.requesterLoad")}</h3>
              {workflowRequesterBuckets.length === 0 ? (
                <p>{t("cmdb.workflow.messages.noRequests")}</p>
              ) : (
                workflowRequesterBuckets.slice(0, 8).map((bucket) => (
                  <div
                    key={`workflow-requester-${bucket.key}`}
                    style={{
                      display: "grid",
                      gridTemplateColumns: "160px 1fr auto",
                      gap: "0.5rem",
                      marginBottom: "0.35rem",
                      alignItems: "center"
                    }}
                  >
                    <span title={bucket.label} style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                      {bucket.label}
                    </span>
                    <div style={{ background: "#e2e8f0", height: "8px", borderRadius: "999px", overflow: "hidden" }}>
                      <div style={{ width: bucketBarWidth(bucket.asset_total, workflowRequesterMax), height: "100%", background: "#be123c" }} />
                    </div>
                    <span>{bucket.asset_total}</span>
                  </div>
                ))
              )}
            </div>
          </div>

          <div className="detail-grid" style={{ marginTop: "0.75rem" }}>
            <div className="detail-panel">
              <h3 style={subSectionTitleStyle}>{t("cmdb.workflow.cockpit.charts.dailyTrend")}</h3>
              {workflowDailyTrendMax <= 0 ? (
                <p>{t("cmdb.workflow.cockpit.labels.noRecentData")}</p>
              ) : (
                <div style={{ display: "flex", gap: "0.45rem", alignItems: "end", minHeight: "168px" }}>
                  {workflowDailyTrend.map((point) => (
                    <div
                      key={point.day_key}
                      style={{ flex: "1 1 0", display: "flex", flexDirection: "column", alignItems: "center", gap: "0.3rem" }}
                      title={t("cmdb.workflow.cockpit.trendTooltip", {
                        day: point.day_label,
                        total: point.total,
                        completed: point.completed,
                        failed: point.failed,
                        active: point.active
                      })}
                    >
                      <div
                        style={{
                          position: "relative",
                          width: "100%",
                          maxWidth: "38px",
                          height: "118px",
                          background: "#e2e8f0",
                          borderRadius: "10px",
                          overflow: "hidden"
                        }}
                      >
                        <div
                          style={{
                            position: "absolute",
                            left: 0,
                            right: 0,
                            bottom: 0,
                            height: bucketBarWidth(point.total, workflowDailyTrendMax),
                            background: "linear-gradient(180deg, #38bdf8 0%, #1d4ed8 100%)"
                          }}
                        />
                      </div>
                      <span style={{ fontSize: "0.78rem", color: "#4f6478" }}>{point.day_label}</span>
                      <span style={{ fontSize: "0.8rem", fontWeight: 600 }}>{point.total}</span>
                    </div>
                  ))}
                </div>
              )}
            </div>

            <div className="detail-panel">
              <h3 style={subSectionTitleStyle}>{t("cmdb.workflow.cockpit.charts.executionQuality")}</h3>
              <p className="section-note">
                {t("cmdb.workflow.cockpit.executionSummary", {
                  avgExecution: workflowKpis.averageExecutionMs,
                  successRate: workflowKpis.executionSuccessRate,
                  sampleSize: workflowKpis.executionSampleSize
                })}
              </p>
              <div className="toolbar-row">
                <span className="status-chip status-chip-success">
                  {t("cmdb.workflow.cockpit.labels.completed")}: {workflowKpis.completedRequests}
                </span>
                <span className="status-chip status-chip-danger">
                  {t("cmdb.workflow.cockpit.labels.failed")}: {workflowKpis.failedRequests}
                </span>
                <span className="status-chip">
                  {t("cmdb.workflow.cockpit.labels.active")}: {workflowKpis.activeRequests}
                </span>
              </div>
              <div className="toolbar-row" style={{ marginTop: "0.35rem" }}>
                <span className="status-chip">
                  {t("cmdb.workflow.cockpit.labels.avgExecution")}: {workflowKpis.averageExecutionMs} ms
                </span>
                <span className="status-chip">
                  {t("cmdb.workflow.cockpit.labels.executionSuccessRate")}: {workflowKpis.executionSuccessRate}%
                </span>
                <span className="status-chip">
                  {t("cmdb.workflow.cockpit.labels.sampleSize")}: {workflowKpis.executionSampleSize}
                </span>
              </div>
            </div>
          </div>
        </SectionCard>
      )}

      {visibleSections.has("section-workflow-reports") && (
        <SectionCard
          id="section-workflow-reports"
          title={t("cmdb.workflow.reports.title")}
          actions={(
            <div className="toolbar-row">
              <button onClick={() => exportWorkflowReportCsv()}>{t("cmdb.workflow.reports.actions.exportCsv")}</button>
              <button
                onClick={() => {
                  setWorkflowReportRangeDays("30");
                  setWorkflowReportStatusFilter("all");
                  setWorkflowReportTemplateFilter("all");
                  setWorkflowReportRequesterFilter("");
                }}
              >
                {t("cmdb.workflow.reports.actions.resetFilters")}
              </button>
            </div>
          )}
        >
          <div className="filter-grid">
            <label className="control-field">
              <span>{t("cmdb.workflow.reports.filters.rangeDaysLabel")}</span>
              <select value={workflowReportRangeDays} onChange={(event) => setWorkflowReportRangeDays(event.target.value)}>
                <option value="7">{t("cmdb.workflow.reports.filters.rangeOptions.7")}</option>
                <option value="30">{t("cmdb.workflow.reports.filters.rangeOptions.30")}</option>
                <option value="90">{t("cmdb.workflow.reports.filters.rangeOptions.90")}</option>
              </select>
            </label>
            <label className="control-field">
              <span>{t("cmdb.workflow.reports.filters.statusLabel")}</span>
              <select
                value={workflowReportStatusFilter}
                onChange={(event) => setWorkflowReportStatusFilter(event.target.value)}
              >
                <option value="all">{t("cmdb.workflow.reports.filters.statusAll")}</option>
                {workflowReportStatusOptions.map((status) => (
                  <option key={`workflow-report-status-${status}`} value={status}>
                    {status}
                  </option>
                ))}
              </select>
            </label>
            <label className="control-field">
              <span>{t("cmdb.workflow.reports.filters.templateLabel")}</span>
              <select
                value={workflowReportTemplateFilter}
                onChange={(event) => setWorkflowReportTemplateFilter(event.target.value)}
              >
                <option value="all">{t("cmdb.workflow.reports.filters.templateAll")}</option>
                {workflowReportTemplateOptions.map((templateName) => (
                  <option key={`workflow-report-template-${templateName}`} value={templateName}>
                    {templateName}
                  </option>
                ))}
              </select>
            </label>
            <label className="control-field">
              <span>{t("cmdb.workflow.reports.filters.requesterLabel")}</span>
              <input
                value={workflowReportRequesterFilter}
                onChange={(event) => setWorkflowReportRequesterFilter(event.target.value)}
                placeholder={t("cmdb.workflow.reports.filters.requesterPlaceholder")}
              />
            </label>
          </div>

          <p className="section-note">
            {t("cmdb.workflow.reports.summary", {
              total: workflowReportSummary.total,
              completed: workflowReportSummary.completed,
              failed: workflowReportSummary.failed,
              active: workflowReportSummary.active,
              completionRate: workflowReportSummary.completionRate,
              failureRate: workflowReportSummary.failureRate
            })}
          </p>

          <div className="detail-grid">
            <div className="detail-panel">
              <h3 style={subSectionTitleStyle}>{t("cmdb.workflow.reports.charts.statusDistribution")}</h3>
              {workflowReportStatusBuckets.length === 0 ? (
                <p>{t("cmdb.workflow.reports.messages.noResult")}</p>
              ) : (
                workflowReportStatusBuckets.map((bucket) => (
                  <div
                    key={`workflow-report-status-${bucket.key}`}
                    style={{
                      display: "grid",
                      gridTemplateColumns: "160px 1fr auto",
                      gap: "0.5rem",
                      marginBottom: "0.35rem",
                      alignItems: "center"
                    }}
                  >
                    <span title={bucket.label} style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                      {bucket.label}
                    </span>
                    <div style={{ background: "#e2e8f0", height: "8px", borderRadius: "999px", overflow: "hidden" }}>
                      <div style={{ width: bucketBarWidth(bucket.asset_total, workflowReportStatusMax), height: "100%", background: "#0f766e" }} />
                    </div>
                    <span>{bucket.asset_total}</span>
                  </div>
                ))
              )}
            </div>
            <div className="detail-panel">
              <h3 style={subSectionTitleStyle}>{t("cmdb.workflow.reports.charts.templateDistribution")}</h3>
              {workflowReportTemplateBuckets.length === 0 ? (
                <p>{t("cmdb.workflow.reports.messages.noResult")}</p>
              ) : (
                workflowReportTemplateBuckets.slice(0, 10).map((bucket) => (
                  <div
                    key={`workflow-report-template-${bucket.key}`}
                    style={{
                      display: "grid",
                      gridTemplateColumns: "180px 1fr auto",
                      gap: "0.5rem",
                      marginBottom: "0.35rem",
                      alignItems: "center"
                    }}
                  >
                    <span title={bucket.label} style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                      {bucket.label}
                    </span>
                    <div style={{ background: "#e2e8f0", height: "8px", borderRadius: "999px", overflow: "hidden" }}>
                      <div style={{ width: bucketBarWidth(bucket.asset_total, workflowReportTemplateMax), height: "100%", background: "#1d4ed8" }} />
                    </div>
                    <span>{bucket.asset_total}</span>
                  </div>
                ))
              )}
            </div>
            <div className="detail-panel">
              <h3 style={subSectionTitleStyle}>{t("cmdb.workflow.reports.charts.dailyTrend")}</h3>
              {workflowReportDailyTrendMax <= 0 ? (
                <p>{t("cmdb.workflow.reports.messages.noResult")}</p>
              ) : (
                <div style={{ display: "flex", gap: "0.45rem", alignItems: "end", minHeight: "176px", overflowX: "auto", paddingBottom: "0.35rem" }}>
                  {workflowReportDailyTrend.map((point) => (
                    <div
                      key={`workflow-report-trend-${point.day_key}`}
                      style={{ flex: "0 0 34px", display: "flex", flexDirection: "column", alignItems: "center", gap: "0.3rem" }}
                      title={t("cmdb.workflow.cockpit.trendTooltip", {
                        day: point.day_label,
                        total: point.total,
                        completed: point.completed,
                        failed: point.failed,
                        active: point.active
                      })}
                    >
                      <div
                        style={{
                          position: "relative",
                          width: "100%",
                          height: "120px",
                          background: "#e2e8f0",
                          borderRadius: "8px",
                          overflow: "hidden"
                        }}
                      >
                        <div
                          style={{
                            position: "absolute",
                            left: 0,
                            right: 0,
                            bottom: 0,
                            height: bucketBarWidth(point.total, workflowReportDailyTrendMax),
                            background: "linear-gradient(180deg, #7dd3fc 0%, #1d4ed8 100%)"
                          }}
                        />
                        {point.total > 0 && (
                          <>
                            <div
                              style={{
                                position: "absolute",
                                left: 0,
                                right: 0,
                                bottom: 0,
                                height: `${(point.failed / point.total) * Number.parseFloat(bucketBarWidth(point.total, workflowReportDailyTrendMax))}%`,
                                background: "rgba(190, 24, 93, 0.75)"
                              }}
                            />
                            <div
                              style={{
                                position: "absolute",
                                left: 0,
                                right: 0,
                                bottom: `${(point.failed / point.total) * Number.parseFloat(bucketBarWidth(point.total, workflowReportDailyTrendMax))}%`,
                                height: `${(point.completed / point.total) * Number.parseFloat(bucketBarWidth(point.total, workflowReportDailyTrendMax))}%`,
                                background: "rgba(15, 118, 110, 0.78)"
                              }}
                            />
                          </>
                        )}
                      </div>
                      <span style={{ fontSize: "0.72rem", color: "#4f6478" }}>{point.day_label}</span>
                      <span style={{ fontSize: "0.75rem", fontWeight: 600 }}>{point.total}</span>
                    </div>
                  ))}
                </div>
              )}
              <p className="inline-note">
                {t("cmdb.workflow.reports.executionSummary", {
                  avgExecution: workflowReportExecutionStats.averageDurationMs,
                  successRate: workflowReportExecutionStats.successRate,
                  sampleSize: workflowReportExecutionStats.sampleSize,
                  automationShare: workflowReportExecutionStats.automationShare
                })}
              </p>
            </div>
          </div>

          <div className="detail-grid" style={{ marginTop: "0.75rem" }}>
            <div className="detail-panel">
              <h3 style={subSectionTitleStyle}>{t("cmdb.workflow.reports.charts.templateRanking")}</h3>
              {workflowTemplateTrendRanks.length === 0 ? (
                <p>{t("cmdb.workflow.reports.messages.noResult")}</p>
              ) : (
                <div style={{ overflowX: "auto" }}>
                  <table style={{ borderCollapse: "collapse", minWidth: "920px", width: "100%" }}>
                    <thead>
                      <tr>
                        <th style={cellStyle}>{t("cmdb.workflow.reports.ranking.columns.name")}</th>
                        <th style={cellStyle}>{t("cmdb.workflow.reports.ranking.columns.thisWeek")}</th>
                        <th style={cellStyle}>{t("cmdb.workflow.reports.ranking.columns.lastWeek")}</th>
                        <th style={cellStyle}>{t("cmdb.workflow.reports.ranking.columns.weekDelta")}</th>
                        <th style={cellStyle}>{t("cmdb.workflow.reports.ranking.columns.thisMonth")}</th>
                        <th style={cellStyle}>{t("cmdb.workflow.reports.ranking.columns.lastMonth")}</th>
                        <th style={cellStyle}>{t("cmdb.workflow.reports.ranking.columns.monthDelta")}</th>
                      </tr>
                    </thead>
                    <tbody>
                      {workflowTemplateTrendRanks.slice(0, 8).map((item) => (
                        <tr key={`workflow-rank-template-${item.key}`}>
                          <td style={cellStyle}>{item.label}</td>
                          <td style={cellStyle}>{item.week_current}</td>
                          <td style={cellStyle}>{item.week_previous}</td>
                          <td style={cellStyle}>{formatSignedDelta(item.week_delta)}</td>
                          <td style={cellStyle}>{item.month_current}</td>
                          <td style={cellStyle}>{item.month_previous}</td>
                          <td style={cellStyle}>{formatSignedDelta(item.month_delta)}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}
            </div>

            <div className="detail-panel">
              <h3 style={subSectionTitleStyle}>{t("cmdb.workflow.reports.charts.requesterRanking")}</h3>
              {workflowRequesterTrendRanks.length === 0 ? (
                <p>{t("cmdb.workflow.reports.messages.noResult")}</p>
              ) : (
                <div style={{ overflowX: "auto" }}>
                  <table style={{ borderCollapse: "collapse", minWidth: "920px", width: "100%" }}>
                    <thead>
                      <tr>
                        <th style={cellStyle}>{t("cmdb.workflow.reports.ranking.columns.name")}</th>
                        <th style={cellStyle}>{t("cmdb.workflow.reports.ranking.columns.thisWeek")}</th>
                        <th style={cellStyle}>{t("cmdb.workflow.reports.ranking.columns.lastWeek")}</th>
                        <th style={cellStyle}>{t("cmdb.workflow.reports.ranking.columns.weekDelta")}</th>
                        <th style={cellStyle}>{t("cmdb.workflow.reports.ranking.columns.thisMonth")}</th>
                        <th style={cellStyle}>{t("cmdb.workflow.reports.ranking.columns.lastMonth")}</th>
                        <th style={cellStyle}>{t("cmdb.workflow.reports.ranking.columns.monthDelta")}</th>
                      </tr>
                    </thead>
                    <tbody>
                      {workflowRequesterTrendRanks.slice(0, 8).map((item) => (
                        <tr key={`workflow-rank-requester-${item.key}`}>
                          <td style={cellStyle}>{item.label}</td>
                          <td style={cellStyle}>{item.week_current}</td>
                          <td style={cellStyle}>{item.week_previous}</td>
                          <td style={cellStyle}>{formatSignedDelta(item.week_delta)}</td>
                          <td style={cellStyle}>{item.month_current}</td>
                          <td style={cellStyle}>{item.month_previous}</td>
                          <td style={cellStyle}>{formatSignedDelta(item.month_delta)}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}
            </div>
          </div>

          <h3 style={{ ...subSectionTitleStyle, marginTop: "0.9rem" }}>{t("cmdb.workflow.reports.table.title")}</h3>
          {workflowReportRows.length === 0 ? (
            <p>{t("cmdb.workflow.reports.messages.noResult")}</p>
          ) : (
            <div style={{ overflowX: "auto" }}>
              <table style={{ borderCollapse: "collapse", minWidth: "1180px", width: "100%" }}>
                <thead>
                  <tr>
                    <th style={cellStyle}>{t("cmdb.workflow.reports.table.columns.id")}</th>
                    <th style={cellStyle}>{t("cmdb.workflow.reports.table.columns.template")}</th>
                    <th style={cellStyle}>{t("cmdb.workflow.reports.table.columns.requester")}</th>
                    <th style={cellStyle}>{t("cmdb.workflow.reports.table.columns.status")}</th>
                    <th style={cellStyle}>{t("cmdb.workflow.reports.table.columns.createdAt")}</th>
                    <th style={cellStyle}>{t("cmdb.workflow.reports.table.columns.updatedAt")}</th>
                    <th style={cellStyle}>{t("cmdb.workflow.reports.table.columns.lastError")}</th>
                  </tr>
                </thead>
                <tbody>
                  {workflowReportRows.slice(0, 200).map((item) => (
                    <tr key={`workflow-report-row-${item.id}`}>
                      <td style={cellStyle}>#{item.id}</td>
                      <td style={cellStyle}>{workflowTemplateDisplayName(item)}</td>
                      <td style={cellStyle}>{item.requester}</td>
                      <td style={cellStyle}>
                        <span className={statusChipClass(item.status)}>{item.status}</span>
                      </td>
                      <td style={cellStyle}>{new Date(item.created_at).toLocaleString()}</td>
                      <td style={cellStyle}>{new Date(item.updated_at).toLocaleString()}</td>
                      <td style={cellStyle}>{item.last_error ?? "-"}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </SectionCard>
      )}

      {visibleSections.has("section-workflow") && (
        <SectionCard id="section-workflow" title={t("cmdb.workflow.title")}>
          <div className="toolbar-row" style={{ marginBottom: "0.75rem" }}>
            <button onClick={() => void loadWorkflowTemplates()} disabled={loadingWorkflowTemplates}>
              {loadingWorkflowTemplates ? t("cmdb.actions.loading") : t("cmdb.workflow.actions.refreshTemplates")}
            </button>
            <button onClick={() => void loadWorkflowRequests()} disabled={loadingWorkflowRequests}>
              {loadingWorkflowRequests ? t("cmdb.actions.loading") : t("cmdb.workflow.actions.refreshRequests")}
            </button>
            {selectedWorkflowRequest && (
              <button
                onClick={() => void loadWorkflowLogs(selectedWorkflowRequest.id)}
                disabled={loadingWorkflowLogs}
              >
                {loadingWorkflowLogs ? t("cmdb.actions.loading") : t("cmdb.workflow.actions.refreshLogs")}
              </button>
            )}
          </div>

          {workflowNotice && <p className="banner banner-success">{workflowNotice}</p>}
          <p className="section-note">
            {t("cmdb.workflow.summary", {
              templates: workflowTemplates.length,
              requests: workflowRequests.length
            })}
          </p>
          {!canWriteCmdb && <p className="inline-note">{t("cmdb.workflow.messages.readOnlyHint")}</p>}

          <h3 style={subSectionTitleStyle}>{t("cmdb.workflow.templatesTitle")}</h3>
          {canWriteCmdb && (
            <>
              <div className="form-grid" style={{ marginBottom: "0.75rem" }}>
                <label className="control-field">
                  <span>{t("cmdb.workflow.form.templateName")}</span>
                  <input
                    value={newWorkflowTemplateName}
                    onChange={(event) => setNewWorkflowTemplateName(event.target.value)}
                    placeholder={t("cmdb.workflow.form.templateNamePlaceholder")}
                  />
                </label>
                <label className="control-field">
                  <span>{t("cmdb.workflow.form.templateDescription")}</span>
                  <input
                    value={newWorkflowTemplateDescription}
                    onChange={(event) => setNewWorkflowTemplateDescription(event.target.value)}
                    placeholder={t("cmdb.workflow.form.templateDescriptionPlaceholder")}
                  />
                </label>
              </div>

              <div className="form-grid" style={{ marginBottom: "0.75rem" }}>
                <label className="control-field">
                  <span>{t("cmdb.workflow.form.stepId")}</span>
                  <input
                    value={newWorkflowStep.id}
                    onChange={(event) =>
                      setNewWorkflowStep((prev) => ({
                        ...prev,
                        id: event.target.value
                      }))
                    }
                    placeholder="apply-patch"
                  />
                </label>
                <label className="control-field">
                  <span>{t("cmdb.workflow.form.stepName")}</span>
                  <input
                    value={newWorkflowStep.name}
                    onChange={(event) =>
                      setNewWorkflowStep((prev) => ({
                        ...prev,
                        name: event.target.value
                      }))
                    }
                    placeholder={t("cmdb.workflow.form.stepNamePlaceholder")}
                  />
                </label>
                <label className="control-field">
                  <span>{t("cmdb.workflow.form.stepKind")}</span>
                  <select
                    value={newWorkflowStep.kind}
                    onChange={(event) =>
                      setNewWorkflowStep((prev) => ({
                        ...prev,
                        kind: event.target.value as WorkflowStepKind
                      }))
                    }
                  >
                    <option value="script">script</option>
                    <option value="manual">manual</option>
                    <option value="approval">approval</option>
                  </select>
                </label>
                <label className="control-field">
                  <span>{t("cmdb.workflow.form.timeoutSeconds")}</span>
                  <input
                    value={newWorkflowStep.timeout_seconds}
                    onChange={(event) =>
                      setNewWorkflowStep((prev) => ({
                        ...prev,
                        timeout_seconds: event.target.value
                      }))
                    }
                    placeholder="300"
                    disabled={newWorkflowStep.kind !== "script"}
                  />
                </label>
                <label className="control-field">
                  <span>{t("cmdb.workflow.form.approverGroup")}</span>
                  <input
                    value={newWorkflowStep.approver_group}
                    onChange={(event) =>
                      setNewWorkflowStep((prev) => ({
                        ...prev,
                        approver_group: event.target.value
                      }))
                    }
                    placeholder="ops-lead"
                    disabled={newWorkflowStep.kind === "script"}
                  />
                </label>
              </div>
              <div style={{ marginBottom: "0.75rem" }}>
                <label className="control-field">
                  <span>{t("cmdb.workflow.form.stepScript")}</span>
                  <textarea
                    value={newWorkflowStep.script}
                    onChange={(event) =>
                      setNewWorkflowStep((prev) => ({
                        ...prev,
                        script: event.target.value
                      }))
                    }
                    rows={4}
                    style={{ width: "100%" }}
                    placeholder="echo 'run automation...'"
                    disabled={newWorkflowStep.kind !== "script"}
                  />
                </label>
              </div>
              <div className="toolbar-row" style={{ marginBottom: "0.75rem" }}>
                <label>
                  <input
                    type="checkbox"
                    checked={newWorkflowStep.auto_run}
                    onChange={(event) =>
                      setNewWorkflowStep((prev) => ({
                        ...prev,
                        auto_run: event.target.checked
                      }))
                    }
                    disabled={newWorkflowStep.kind !== "script"}
                  />{" "}
                  {t("cmdb.workflow.form.autoRun")}
                </label>
                <button onClick={() => addWorkflowStepToDraft()}>
                  {t("cmdb.workflow.actions.addStep")}
                </button>
                <button onClick={() => void createWorkflowTemplate()} disabled={creatingWorkflowTemplate}>
                  {creatingWorkflowTemplate ? t("cmdb.actions.creating") : t("cmdb.workflow.actions.createTemplate")}
                </button>
              </div>
            </>
          )}

          {newWorkflowTemplateSteps.length > 0 && (
            <div style={{ overflowX: "auto", marginBottom: "1rem" }}>
              <table style={{ borderCollapse: "collapse", minWidth: "980px", width: "100%" }}>
                <thead>
                  <tr>
                    <th style={cellStyle}>{t("cmdb.workflow.table.step.id")}</th>
                    <th style={cellStyle}>{t("cmdb.workflow.table.step.name")}</th>
                    <th style={cellStyle}>{t("cmdb.workflow.table.step.kind")}</th>
                    <th style={cellStyle}>{t("cmdb.workflow.table.step.autoRun")}</th>
                    <th style={cellStyle}>{t("cmdb.workflow.table.step.timeout")}</th>
                    <th style={cellStyle}>{t("cmdb.workflow.table.step.script")}</th>
                    <th style={cellStyle}>{t("cmdb.workflow.table.step.approver")}</th>
                    <th style={cellStyle}>{t("cmdb.workflow.table.step.actions")}</th>
                  </tr>
                </thead>
                <tbody>
                  {newWorkflowTemplateSteps.map((step) => (
                    <tr key={`draft-step-${step.id}`}>
                      <td style={cellStyle}>{step.id}</td>
                      <td style={cellStyle}>{step.name}</td>
                      <td style={cellStyle}>{step.kind}</td>
                      <td style={cellStyle}>{step.auto_run ? "Yes" : "No"}</td>
                      <td style={cellStyle}>{step.timeout_seconds}</td>
                      <td style={cellStyle}>{step.kind === "script" ? truncateTopologyLabel(step.script, 72) : "-"}</td>
                      <td style={cellStyle}>{step.approver_group || "-"}</td>
                      <td style={cellStyle}>
                        <button onClick={() => removeWorkflowStepFromDraft(step.id)}>
                          {t("cmdb.workflow.actions.removeStep")}
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}

          {loadingWorkflowTemplates && workflowTemplates.length === 0 ? (
            <p>{t("cmdb.workflow.messages.loadingTemplates")}</p>
          ) : workflowTemplates.length === 0 ? (
            <p>{t("cmdb.workflow.messages.noTemplates")}</p>
          ) : (
            <div style={{ overflowX: "auto", marginBottom: "1rem" }}>
              <table style={{ borderCollapse: "collapse", minWidth: "1100px", width: "100%" }}>
                <thead>
                  <tr>
                    <th style={cellStyle}>{t("cmdb.workflow.table.template.id")}</th>
                    <th style={cellStyle}>{t("cmdb.workflow.table.template.name")}</th>
                    <th style={cellStyle}>{t("cmdb.workflow.table.template.steps")}</th>
                    <th style={cellStyle}>{t("cmdb.workflow.table.template.enabled")}</th>
                    <th style={cellStyle}>{t("cmdb.workflow.table.template.updatedAt")}</th>
                  </tr>
                </thead>
                <tbody>
                  {workflowTemplates.map((template) => (
                    <tr key={template.id}>
                      <td style={cellStyle}>{template.id}</td>
                      <td style={cellStyle}>{template.name}</td>
                      <td style={cellStyle}>{template.definition.steps.length}</td>
                      <td style={cellStyle}>{template.is_enabled ? "Yes" : "No"}</td>
                      <td style={cellStyle}>{new Date(template.updated_at).toLocaleString()}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}

          <h3 style={subSectionTitleStyle}>{t("cmdb.workflow.requestsTitle")}</h3>
          {canWriteCmdb && (
            <div className="form-grid" style={{ marginBottom: "0.75rem" }}>
              <label className="control-field">
                <span>{t("cmdb.workflow.form.requestTemplate")}</span>
                <select
                  value={newWorkflowRequest.template_id}
                  onChange={(event) =>
                    setNewWorkflowRequest((prev) => ({
                      ...prev,
                      template_id: event.target.value
                    }))
                  }
                >
                  <option value="">{t("cmdb.workflow.form.selectTemplate")}</option>
                  {workflowTemplates.map((template) => (
                    <option key={`workflow-template-${template.id}`} value={template.id}>
                      #{template.id} {template.name}
                    </option>
                  ))}
                </select>
              </label>
              <label className="control-field">
                <span>{t("cmdb.workflow.form.requestTitle")}</span>
                <input
                  value={newWorkflowRequest.title}
                  onChange={(event) =>
                    setNewWorkflowRequest((prev) => ({
                      ...prev,
                      title: event.target.value
                    }))
                  }
                  placeholder={t("cmdb.workflow.form.requestTitlePlaceholder")}
                />
              </label>
              <label className="control-field">
                <span>{t("cmdb.workflow.form.requestPayload")}</span>
                <input
                  value={newWorkflowRequest.payload_json}
                  onChange={(event) =>
                    setNewWorkflowRequest((prev) => ({
                      ...prev,
                      payload_json: event.target.value
                    }))
                  }
                  placeholder='{"asset_id": 101}'
                />
              </label>
              <div className="toolbar-row" style={{ alignSelf: "end" }}>
                <button onClick={() => void createWorkflowRequest()} disabled={creatingWorkflowRequest}>
                  {creatingWorkflowRequest ? t("cmdb.actions.creating") : t("cmdb.workflow.actions.createRequest")}
                </button>
              </div>
            </div>
          )}

          {loadingWorkflowRequests && workflowRequests.length === 0 ? (
            <p>{t("cmdb.workflow.messages.loadingRequests")}</p>
          ) : workflowRequests.length === 0 ? (
            <p>{t("cmdb.workflow.messages.noRequests")}</p>
          ) : (
            <div style={{ overflowX: "auto", marginBottom: "1rem" }}>
              <table style={{ borderCollapse: "collapse", minWidth: "1380px", width: "100%" }}>
                <thead>
                  <tr>
                    <th style={cellStyle}>{t("cmdb.workflow.table.request.id")}</th>
                    <th style={cellStyle}>{t("cmdb.workflow.table.request.template")}</th>
                    <th style={cellStyle}>{t("cmdb.workflow.table.request.title")}</th>
                    <th style={cellStyle}>{t("cmdb.workflow.table.request.status")}</th>
                    <th style={cellStyle}>{t("cmdb.workflow.table.request.stepIndex")}</th>
                    <th style={cellStyle}>{t("cmdb.workflow.table.request.requester")}</th>
                    <th style={cellStyle}>{t("cmdb.workflow.table.request.lastError")}</th>
                    <th style={cellStyle}>{t("cmdb.workflow.table.request.updatedAt")}</th>
                    <th style={cellStyle}>{t("cmdb.workflow.table.request.actions")}</th>
                  </tr>
                </thead>
                <tbody>
                  {workflowRequests.map((request) => (
                    <tr key={request.id}>
                      <td style={cellStyle}>{request.id}</td>
                      <td style={cellStyle}>#{request.template_id} {request.template_name}</td>
                      <td style={cellStyle}>{request.title}</td>
                      <td style={cellStyle}>
                        <span className={statusChipClass(request.status)}>{request.status}</span>
                      </td>
                      <td style={cellStyle}>{request.current_step_index}</td>
                      <td style={cellStyle}>{request.requester}</td>
                      <td style={cellStyle}>{request.last_error ?? "-"}</td>
                      <td style={cellStyle}>{new Date(request.updated_at).toLocaleString()}</td>
                      <td style={cellStyle}>
                        <div style={{ display: "flex", gap: "0.35rem", flexWrap: "wrap" }}>
                          <button
                            onClick={() => {
                              setSelectedWorkflowRequestId(String(request.id));
                              void loadWorkflowLogs(request.id);
                            }}
                          >
                            {t("cmdb.workflow.actions.viewLogs")}
                          </button>
                          <button
                            onClick={() => void approveWorkflowRequest(request.id)}
                            disabled={approvingWorkflowRequestId === request.id || request.status !== "pending_approval"}
                          >
                            {approvingWorkflowRequestId === request.id ? t("cmdb.actions.loading") : t("cmdb.workflow.actions.approve")}
                          </button>
                          <button
                            onClick={() => void rejectWorkflowRequest(request.id)}
                            disabled={rejectingWorkflowRequestId === request.id || request.status !== "pending_approval"}
                          >
                            {rejectingWorkflowRequestId === request.id ? t("cmdb.actions.loading") : t("cmdb.workflow.actions.reject")}
                          </button>
                          <button
                            onClick={() => void executeWorkflowRequest(request.id)}
                            disabled={
                              executingWorkflowRequestId === request.id
                              || (request.status !== "approved" && request.status !== "running")
                            }
                          >
                            {executingWorkflowRequestId === request.id ? t("cmdb.actions.loading") : t("cmdb.workflow.actions.execute")}
                          </button>
                          <button
                            onClick={() => void completeWorkflowManualStep(request.id)}
                            disabled={manualCompletingWorkflowRequestId === request.id || request.status !== "waiting_manual"}
                          >
                            {manualCompletingWorkflowRequestId === request.id
                              ? t("cmdb.actions.loading")
                              : t("cmdb.workflow.actions.completeManual")}
                          </button>
                        </div>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}

          <h3 style={subSectionTitleStyle}>{t("cmdb.workflow.logsTitle")}</h3>
          {!selectedWorkflowRequest ? (
            <p>{t("cmdb.workflow.messages.selectRequest")}</p>
          ) : loadingWorkflowLogs && workflowLogs.length === 0 ? (
            <p>{t("cmdb.workflow.messages.loadingLogs")}</p>
          ) : workflowLogs.length === 0 ? (
            <p>{t("cmdb.workflow.messages.noLogs")}</p>
          ) : (
            <div style={{ overflowX: "auto" }}>
              <table style={{ borderCollapse: "collapse", minWidth: "1350px", width: "100%" }}>
                <thead>
                  <tr>
                    <th style={cellStyle}>{t("cmdb.workflow.table.log.id")}</th>
                    <th style={cellStyle}>{t("cmdb.workflow.table.log.step")}</th>
                    <th style={cellStyle}>{t("cmdb.workflow.table.log.kind")}</th>
                    <th style={cellStyle}>{t("cmdb.workflow.table.log.status")}</th>
                    <th style={cellStyle}>{t("cmdb.workflow.table.log.executor")}</th>
                    <th style={cellStyle}>{t("cmdb.workflow.table.log.exitCode")}</th>
                    <th style={cellStyle}>{t("cmdb.workflow.table.log.duration")}</th>
                    <th style={cellStyle}>{t("cmdb.workflow.table.log.output")}</th>
                    <th style={cellStyle}>{t("cmdb.workflow.table.log.time")}</th>
                  </tr>
                </thead>
                <tbody>
                  {workflowLogs.map((log) => (
                    <tr key={`workflow-log-${log.id}`}>
                      <td style={cellStyle}>{log.id}</td>
                      <td style={cellStyle}>
                        #{log.step_index} {log.step_id} / {log.step_name}
                      </td>
                      <td style={cellStyle}>{log.step_kind}</td>
                      <td style={cellStyle}>
                        <span className={statusChipClass(log.status)}>{log.status}</span>
                      </td>
                      <td style={cellStyle}>{log.executor ?? "-"}</td>
                      <td style={cellStyle}>{log.exit_code ?? "-"}</td>
                      <td style={cellStyle}>{log.duration_ms ?? "-"}</td>
                      <td style={cellStyle}>
                        <pre style={{ margin: 0, maxWidth: "420px", maxHeight: "120px", overflow: "auto", whiteSpace: "pre-wrap" }}>
                          {truncateTopologyLabel(log.output ?? log.error ?? "-", 3000)}
                        </pre>
                      </td>
                      <td style={cellStyle}>
                        {log.finished_at ? new Date(log.finished_at).toLocaleString() : (log.created_at ? new Date(log.created_at).toLocaleString() : "-")}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </SectionCard>
      )}

      {visibleSections.has("section-scan") && (
        <SectionCard id="section-scan" title={t("cmdb.scan.title")}>
          <div style={{ display: "flex", gap: "0.5rem", flexWrap: "wrap", alignItems: "center" }}>
            <input
              value={scanCode}
              onChange={(event) => setScanCode(event.target.value)}
              placeholder={t("cmdb.scan.placeholder")}
              style={{ minWidth: "220px" }}
            />
            <select value={scanMode} onChange={(event) => setScanMode(event.target.value as "auto" | "qr" | "barcode")}>
              <option value="auto">{t("cmdb.scan.modes.auto")}</option>
              <option value="qr">{t("cmdb.scan.modes.qr")}</option>
              <option value="barcode">{t("cmdb.scan.modes.barcode")}</option>
            </select>
            <button onClick={() => void findAssetByCode()} disabled={scanning}>
              {scanning ? t("cmdb.actions.loading") : t("cmdb.scan.find")}
            </button>
          </div>
          {scanResult && (
            <p style={{ marginTop: "0.5rem" }}>
              {t("cmdb.scan.hit")}: #{scanResult.id} {scanResult.name} ({scanResult.asset_class})
            </p>
          )}
        </SectionCard>
      )}

      {visibleSections.has("section-discovery") && (
        <SectionCard id="section-discovery" title={t("cmdb.discovery.title")}>
        <div className="toolbar-row" style={{ marginBottom: "0.75rem" }}>
          <button onClick={() => void loadDiscoveryJobs()} disabled={loadingDiscoveryJobs}>
            {loadingDiscoveryJobs ? t("cmdb.actions.loading") : t("cmdb.discovery.actions.refreshJobs")}
          </button>
          <button onClick={() => void loadDiscoveryCandidates()} disabled={loadingDiscoveryCandidates}>
            {loadingDiscoveryCandidates ? t("cmdb.actions.loading") : t("cmdb.discovery.actions.refreshCandidates")}
          </button>
        </div>

        {discoveryNotice && <p className="banner banner-success">{discoveryNotice}</p>}
        <p className="section-note">
          {t("cmdb.discovery.summary", { jobs: discoveryJobs.length, candidates: discoveryCandidates.length })}
        </p>
        {!canWriteCmdb && <p className="inline-note">{t("cmdb.discovery.messages.readOnlyHint")}</p>}

        <h3 style={subSectionTitleStyle}>{t("cmdb.discovery.jobsTitle")}</h3>
        {loadingDiscoveryJobs && discoveryJobs.length === 0 ? (
          <p>{t("cmdb.discovery.messages.loadingJobs")}</p>
        ) : discoveryJobs.length === 0 ? (
          <p>{t("cmdb.discovery.messages.noJobs")}</p>
        ) : (
          <div style={{ overflowX: "auto", marginBottom: "1rem" }}>
            <table style={{ borderCollapse: "collapse", minWidth: "980px", width: "100%" }}>
              <thead>
                <tr>
                  <th style={cellStyle}>{t("cmdb.discovery.table.job.id")}</th>
                  <th style={cellStyle}>{t("cmdb.discovery.table.job.name")}</th>
                  <th style={cellStyle}>{t("cmdb.discovery.table.job.sourceType")}</th>
                  <th style={cellStyle}>{t("cmdb.discovery.table.job.status")}</th>
                  <th style={cellStyle}>{t("cmdb.discovery.table.job.lastRunStatus")}</th>
                  <th style={cellStyle}>{t("cmdb.discovery.table.job.lastRunAt")}</th>
                  <th style={cellStyle}>{t("cmdb.discovery.table.job.actions")}</th>
                </tr>
              </thead>
              <tbody>
                {discoveryJobs.map((job) => (
                  <tr key={job.id}>
                    <td style={cellStyle}>{job.id}</td>
                    <td style={cellStyle}>{job.name}</td>
                    <td style={cellStyle}>{job.source_type}</td>
                    <td style={cellStyle}>
                      <span className={statusChipClass(job.status)}>{job.status}</span>
                    </td>
                    <td style={cellStyle}>
                      {job.last_run_status ? <span className={statusChipClass(job.last_run_status)}>{job.last_run_status}</span> : "-"}
                    </td>
                    <td style={cellStyle}>{job.last_run_at ? new Date(job.last_run_at).toLocaleString() : "-"}</td>
                    <td style={cellStyle}>
                      {canWriteCmdb ? (
                        <button onClick={() => void runDiscoveryJob(job.id)} disabled={runningDiscoveryJobId === job.id}>
                          {runningDiscoveryJobId === job.id ? t("cmdb.actions.loading") : t("cmdb.discovery.actions.run")}
                        </button>
                      ) : (
                        <span>{t("auth.labels.readOnly")}</span>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}

        <h3 style={subSectionTitleStyle}>{t("cmdb.discovery.candidatesTitle")}</h3>
        {loadingDiscoveryCandidates && discoveryCandidates.length === 0 ? (
          <p>{t("cmdb.discovery.messages.loadingCandidates")}</p>
        ) : discoveryCandidates.length === 0 ? (
          <p>{t("cmdb.discovery.messages.noCandidates")}</p>
        ) : (
          <div style={{ overflowX: "auto" }}>
            <table style={{ borderCollapse: "collapse", minWidth: "1100px", width: "100%" }}>
              <thead>
                <tr>
                  <th style={cellStyle}>{t("cmdb.discovery.table.candidate.id")}</th>
                  <th style={cellStyle}>{t("cmdb.discovery.table.candidate.fingerprint")}</th>
                  <th style={cellStyle}>{t("cmdb.discovery.table.candidate.name")}</th>
                  <th style={cellStyle}>{t("cmdb.discovery.table.candidate.class")}</th>
                  <th style={cellStyle}>{t("cmdb.discovery.table.candidate.ip")}</th>
                  <th style={cellStyle}>{t("cmdb.discovery.table.candidate.discoveredAt")}</th>
                  <th style={cellStyle}>{t("cmdb.discovery.table.candidate.actions")}</th>
                </tr>
              </thead>
              <tbody>
                {discoveryCandidates.map((candidate) => (
                  <tr key={candidate.id}>
                    <td style={cellStyle}>{candidate.id}</td>
                    <td style={cellStyle}>{candidate.fingerprint}</td>
                    <td style={cellStyle}>{readPayloadString(candidate.payload, "name") ?? "-"}</td>
                    <td style={cellStyle}>{readPayloadString(candidate.payload, "asset_class") ?? "-"}</td>
                    <td style={cellStyle}>{readPayloadString(candidate.payload, "ip") ?? "-"}</td>
                    <td style={cellStyle}>{new Date(candidate.discovered_at).toLocaleString()}</td>
                    <td style={cellStyle}>
                      {canWriteCmdb ? (
                        <div style={{ display: "flex", gap: "0.5rem" }}>
                          <button
                            onClick={() => void reviewDiscoveryCandidate(candidate.id, "approve")}
                            disabled={reviewingCandidateId === candidate.id}
                          >
                            {reviewingCandidateId === candidate.id
                              ? t("cmdb.actions.loading")
                              : t("cmdb.discovery.actions.approve")}
                          </button>
                          <button
                            onClick={() => void reviewDiscoveryCandidate(candidate.id, "reject")}
                            disabled={reviewingCandidateId === candidate.id}
                          >
                            {reviewingCandidateId === candidate.id
                              ? t("cmdb.actions.loading")
                              : t("cmdb.discovery.actions.reject")}
                          </button>
                        </div>
                      ) : (
                        <span>{t("auth.labels.readOnly")}</span>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
        </SectionCard>
      )}

      {visibleSections.has("section-monitoring-sources") && (
        <SectionCard id="section-monitoring-sources" title={t("cmdb.monitoringSources.title")}>
        <div className="toolbar-row" style={{ marginBottom: "0.75rem" }}>
          <button onClick={() => void loadMonitoringSources(monitoringSourceFilters)} disabled={loadingMonitoringSources}>
            {loadingMonitoringSources
              ? t("cmdb.actions.loading")
              : t("cmdb.monitoringSources.actions.refresh")}
          </button>
          <span className="section-meta">
            {t("cmdb.monitoringSources.summary", {
              total: monitoringSourceStats.total,
              enabled: monitoringSourceStats.enabled,
              reachable: monitoringSourceStats.reachable,
              unreachable: monitoringSourceStats.unreachable
            })}
          </span>
        </div>

        {monitoringSourceNotice && <p className="banner banner-success">{monitoringSourceNotice}</p>}

        <div className="filter-grid" style={{ marginBottom: "0.75rem" }}>
          <label className="control-field">
            <span>{t("cmdb.monitoringSources.filters.sourceTypeLabel")}</span>
            <select
              value={monitoringSourceFilters.source_type}
              onChange={(event) =>
                setMonitoringSourceFilters((prev) => ({
                  ...prev,
                  source_type: event.target.value
                }))
              }
            >
              <option value="">{t("cmdb.monitoringSources.filters.allSourceTypes")}</option>
              <option value="zabbix">zabbix</option>
            </select>
          </label>
          <label className="control-field">
            <span>{t("cmdb.monitoringSources.filters.siteLabel")}</span>
            <input
              value={monitoringSourceFilters.site}
              onChange={(event) =>
                setMonitoringSourceFilters((prev) => ({
                  ...prev,
                  site: event.target.value
                }))
              }
              placeholder={t("cmdb.monitoringSources.filters.sitePlaceholder")}
            />
          </label>
          <label className="control-field">
            <span>{t("cmdb.monitoringSources.filters.departmentLabel")}</span>
            <input
              value={monitoringSourceFilters.department}
              onChange={(event) =>
                setMonitoringSourceFilters((prev) => ({
                  ...prev,
                  department: event.target.value
                }))
              }
              placeholder={t("cmdb.monitoringSources.filters.departmentPlaceholder")}
            />
          </label>
          <label className="control-field">
            <span>{t("cmdb.monitoringSources.filters.enabledLabel")}</span>
            <select
              value={monitoringSourceFilters.is_enabled}
              onChange={(event) =>
                setMonitoringSourceFilters((prev) => ({
                  ...prev,
                  is_enabled: event.target.value as "all" | "true" | "false"
                }))
              }
            >
              <option value="all">{t("cmdb.monitoringSources.filters.enabledAll")}</option>
              <option value="true">{t("cmdb.monitoringSources.filters.enabledOnly")}</option>
              <option value="false">{t("cmdb.monitoringSources.filters.disabledOnly")}</option>
            </select>
          </label>
        </div>
        <div className="toolbar-row" style={{ marginBottom: "0.75rem" }}>
          <button onClick={() => void loadMonitoringSources(monitoringSourceFilters)} disabled={loadingMonitoringSources}>
            {loadingMonitoringSources ? t("cmdb.actions.loading") : t("cmdb.monitoringSources.actions.applyFilters")}
          </button>
          <button
            onClick={() => {
              const next = { ...defaultMonitoringSourceFilters };
              setMonitoringSourceFilters(next);
              void loadMonitoringSources(next);
            }}
            disabled={!hasMonitoringSourceFilter || loadingMonitoringSources}
          >
            {t("cmdb.monitoringSources.actions.resetFilters")}
          </button>
        </div>

        {!canWriteCmdb && <p className="inline-note">{t("cmdb.monitoringSources.messages.readOnlyHint")}</p>}

        {canWriteCmdb && (
          <div className="form-grid" style={{ marginBottom: "0.9rem" }}>
            <label className="control-field">
              <span>{t("cmdb.monitoringSources.form.name")}</span>
              <input
                value={newMonitoringSource.name}
                onChange={(event) =>
                  setNewMonitoringSource((prev) => ({
                    ...prev,
                    name: event.target.value
                  }))
                }
                placeholder={t("cmdb.monitoringSources.form.namePlaceholder")}
              />
            </label>
            <label className="control-field">
              <span>{t("cmdb.monitoringSources.form.sourceType")}</span>
              <select
                value={newMonitoringSource.source_type}
                onChange={(event) =>
                  setNewMonitoringSource((prev) => ({
                    ...prev,
                    source_type: event.target.value as "zabbix"
                  }))
                }
              >
                <option value="zabbix">zabbix</option>
              </select>
            </label>
            <label className="control-field">
              <span>{t("cmdb.monitoringSources.form.authType")}</span>
              <select
                value={newMonitoringSource.auth_type}
                onChange={(event) =>
                  setNewMonitoringSource((prev) => ({
                    ...prev,
                    auth_type: event.target.value as "token" | "basic"
                  }))
                }
              >
                <option value="token">token</option>
                <option value="basic">basic</option>
              </select>
            </label>
            <label className="control-field">
              <span>{t("cmdb.monitoringSources.form.endpoint")}</span>
              <input
                value={newMonitoringSource.endpoint}
                onChange={(event) =>
                  setNewMonitoringSource((prev) => ({
                    ...prev,
                    endpoint: event.target.value
                  }))
                }
                placeholder={t("cmdb.monitoringSources.form.endpointPlaceholder")}
              />
            </label>
            <label className="control-field">
              <span>{t("cmdb.monitoringSources.form.proxyEndpoint")}</span>
              <input
                value={newMonitoringSource.proxy_endpoint}
                onChange={(event) =>
                  setNewMonitoringSource((prev) => ({
                    ...prev,
                    proxy_endpoint: event.target.value
                  }))
                }
                placeholder={t("cmdb.monitoringSources.form.proxyEndpointPlaceholder")}
              />
            </label>
            <label className="control-field">
              <span>{t("cmdb.monitoringSources.form.secretRef")}</span>
              <input
                value={newMonitoringSource.secret_ref}
                onChange={(event) =>
                  setNewMonitoringSource((prev) => ({
                    ...prev,
                    secret_ref: event.target.value
                  }))
                }
                placeholder={t("cmdb.monitoringSources.form.secretRefPlaceholder")}
              />
            </label>
            <label className="control-field">
              <span>{t("cmdb.monitoringSources.form.username")}</span>
              <input
                value={newMonitoringSource.username}
                onChange={(event) =>
                  setNewMonitoringSource((prev) => ({
                    ...prev,
                    username: event.target.value
                  }))
                }
                placeholder={t("cmdb.monitoringSources.form.usernamePlaceholder")}
                disabled={newMonitoringSource.auth_type !== "basic"}
              />
            </label>
            <label className="control-field">
              <span>{t("cmdb.monitoringSources.form.site")}</span>
              <input
                value={newMonitoringSource.site}
                onChange={(event) =>
                  setNewMonitoringSource((prev) => ({
                    ...prev,
                    site: event.target.value
                  }))
                }
                placeholder={t("cmdb.monitoringSources.form.sitePlaceholder")}
              />
            </label>
            <label className="control-field">
              <span>{t("cmdb.monitoringSources.form.department")}</span>
              <input
                value={newMonitoringSource.department}
                onChange={(event) =>
                  setNewMonitoringSource((prev) => ({
                    ...prev,
                    department: event.target.value
                  }))
                }
                placeholder={t("cmdb.monitoringSources.form.departmentPlaceholder")}
              />
            </label>
            <label className="control-field">
              <span>{t("cmdb.monitoringSources.form.enabled")}</span>
              <select
                value={newMonitoringSource.is_enabled ? "true" : "false"}
                onChange={(event) =>
                  setNewMonitoringSource((prev) => ({
                    ...prev,
                    is_enabled: event.target.value === "true"
                  }))
                }
              >
                <option value="true">{t("cmdb.monitoringSources.form.enabledTrue")}</option>
                <option value="false">{t("cmdb.monitoringSources.form.enabledFalse")}</option>
              </select>
            </label>
          </div>
        )}
        {canWriteCmdb && (
          <div className="toolbar-row" style={{ marginBottom: "0.8rem" }}>
            <button onClick={() => void createMonitoringSource()} disabled={creatingMonitoringSource}>
              {creatingMonitoringSource
                ? t("cmdb.actions.creating")
                : t("cmdb.monitoringSources.actions.create")}
            </button>
          </div>
        )}

        {loadingMonitoringSources && monitoringSources.length === 0 ? (
          <p>{t("cmdb.monitoringSources.messages.loading")}</p>
        ) : monitoringSources.length === 0 ? (
          <p>{t("cmdb.monitoringSources.messages.noSources")}</p>
        ) : (
          <div style={{ overflowX: "auto" }}>
            <table style={{ borderCollapse: "collapse", minWidth: "1500px", width: "100%" }}>
              <thead>
                <tr>
                  <th style={cellStyle}>{t("cmdb.monitoringSources.table.id")}</th>
                  <th style={cellStyle}>{t("cmdb.monitoringSources.table.name")}</th>
                  <th style={cellStyle}>{t("cmdb.monitoringSources.table.type")}</th>
                  <th style={cellStyle}>{t("cmdb.monitoringSources.table.endpoint")}</th>
                  <th style={cellStyle}>{t("cmdb.monitoringSources.table.authType")}</th>
                  <th style={cellStyle}>{t("cmdb.monitoringSources.table.scope")}</th>
                  <th style={cellStyle}>{t("cmdb.monitoringSources.table.enabled")}</th>
                  <th style={cellStyle}>{t("cmdb.monitoringSources.table.probeStatus")}</th>
                  <th style={cellStyle}>{t("cmdb.monitoringSources.table.probeTime")}</th>
                  <th style={cellStyle}>{t("cmdb.monitoringSources.table.probeMessage")}</th>
                  <th style={cellStyle}>{t("cmdb.monitoringSources.table.actions")}</th>
                </tr>
              </thead>
              <tbody>
                {monitoringSources.map((source) => (
                  <tr key={source.id}>
                    <td style={cellStyle}>{source.id}</td>
                    <td style={cellStyle}>{source.name}</td>
                    <td style={cellStyle}>{source.source_type}</td>
                    <td style={cellStyle}>
                      <div>{source.endpoint}</div>
                      {source.proxy_endpoint && (
                        <div className="section-meta">
                          {t("cmdb.monitoringSources.table.proxyLabel")}: {source.proxy_endpoint}
                        </div>
                      )}
                    </td>
                    <td style={cellStyle}>
                      {source.auth_type}
                      {source.username ? ` (${source.username})` : ""}
                    </td>
                    <td style={cellStyle}>
                      {(source.site ?? "*")} / {(source.department ?? "*")}
                    </td>
                    <td style={cellStyle}>
                      {source.is_enabled
                        ? t("cmdb.monitoringSources.form.enabledTrue")
                        : t("cmdb.monitoringSources.form.enabledFalse")}
                    </td>
                    <td style={cellStyle}>
                      <span className={statusChipClass(source.last_probe_status ?? "unknown")}>
                        {source.last_probe_status ?? t("cmdb.monitoringSources.messages.neverProbed")}
                      </span>
                    </td>
                    <td style={cellStyle}>
                      {source.last_probe_at ? new Date(source.last_probe_at).toLocaleString() : "-"}
                    </td>
                    <td style={cellStyle}>{source.last_probe_message ?? "-"}</td>
                    <td style={cellStyle}>
                      {canWriteCmdb ? (
                        <button
                          onClick={() => void probeMonitoringSource(source.id)}
                          disabled={probingMonitoringSourceId === source.id}
                        >
                          {probingMonitoringSourceId === source.id
                            ? t("cmdb.actions.loading")
                            : t("cmdb.monitoringSources.actions.probe")}
                        </button>
                      ) : (
                        <span>{t("auth.labels.readOnly")}</span>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
        </SectionCard>
      )}

      {visibleSections.has("section-monitoring-metrics") && (
        <SectionCard id="section-monitoring-metrics" title={t("cmdb.monitoringMetrics.title")}>
        <div style={{ display: "flex", gap: "0.75rem", flexWrap: "wrap", alignItems: "flex-end", marginBottom: "0.75rem" }}>
          <label className="control-field" style={{ minWidth: "220px" }}>
            <span>{t("cmdb.monitoringMetrics.filters.assetLabel")}</span>
            <select value={selectedAssetId} onChange={(event) => setSelectedAssetId(event.target.value)}>
              <option value="">{t("cmdb.monitoringMetrics.filters.selectAsset")}</option>
              {assets.map((asset) => (
                <option key={asset.id} value={asset.id}>
                  #{asset.id} {asset.name}
                </option>
              ))}
            </select>
          </label>
          <label className="control-field" style={{ minWidth: "170px" }}>
            <span>{t("cmdb.monitoringMetrics.filters.windowLabel")}</span>
            <select
              value={monitoringMetricsWindowMinutes}
              onChange={(event) => setMonitoringMetricsWindowMinutes(event.target.value)}
            >
              <option value="30">30m</option>
              <option value="60">60m</option>
              <option value="180">180m</option>
              <option value="360">360m</option>
            </select>
          </label>
          <button
            onClick={() => {
              const assetId = Number.parseInt(selectedAssetId, 10);
              if (!Number.isFinite(assetId) || assetId <= 0) {
                return;
              }
              void loadMonitoringMetrics(assetId, monitoringMetricsWindowValue);
            }}
            disabled={loadingMonitoringMetrics || !selectedAssetId}
          >
            {loadingMonitoringMetrics ? t("cmdb.actions.loading") : t("cmdb.monitoringMetrics.actions.refresh")}
          </button>
        </div>

        {!selectedAssetId ? (
          <p>{t("cmdb.monitoringMetrics.messages.selectAsset")}</p>
        ) : loadingMonitoringMetrics && !monitoringMetrics ? (
          <p>{t("cmdb.monitoringMetrics.messages.loading")}</p>
        ) : monitoringMetricsError ? (
          <p className="inline-note">
            {t("cmdb.monitoringMetrics.messages.error", { error: monitoringMetricsError })}
          </p>
        ) : !monitoringMetrics ? (
          <p>{t("cmdb.monitoringMetrics.messages.noData")}</p>
        ) : (
          <>
            <p className="section-note">
              {t("cmdb.monitoringMetrics.summary", {
                asset: monitoringMetrics.asset_name,
                host: monitoringMetrics.host_id,
                source: monitoringMetrics.source.name,
                window: monitoringMetrics.window_minutes
              })}
            </p>

            <div className="detail-grid">
              {monitoringMetrics.series.map((series) => (
                <div key={series.metric} className="detail-panel">
                  <h3 style={subSectionTitleStyle}>{series.label}</h3>
                  <p className="section-note">
                    {t("cmdb.monitoringMetrics.latest", {
                      value: series.latest ? formatMetricValue(series.latest.value, series.unit) : "-",
                      time: series.latest ? new Date(series.latest.timestamp).toLocaleString() : "-"
                    })}
                  </p>
                  {series.note && <p className="inline-note">{series.note}</p>}
                  {series.points.length === 0 ? (
                    <p>{t("cmdb.monitoringMetrics.messages.emptySeries")}</p>
                  ) : (
                    <div style={{ border: "1px solid #e2e8f0", borderRadius: "10px", padding: "0.35rem" }}>
                      <svg viewBox="0 0 320 120" style={{ width: "100%", height: "120px", display: "block" }} aria-label={series.label}>
                        <line x1="12" y1="12" x2="12" y2="108" stroke="#cbd5e1" strokeWidth="1" />
                        <line x1="12" y1="108" x2="308" y2="108" stroke="#cbd5e1" strokeWidth="1" />
                        <polyline
                          fill="none"
                          stroke="#2563eb"
                          strokeWidth="2"
                          points={buildMetricPolylinePoints(series.points, 320, 120, 12)}
                        />
                      </svg>
                    </div>
                  )}
                </div>
              ))}
            </div>
          </>
        )}
        </SectionCard>
      )}

      {visibleSections.has("section-notifications") && (
        <SectionCard id="section-notifications" title={t("cmdb.notifications.title")}>
        <div className="toolbar-row" style={{ marginBottom: "0.75rem" }}>
          <button onClick={() => void loadNotificationChannels()} disabled={loadingNotificationChannels}>
            {loadingNotificationChannels ? t("cmdb.actions.loading") : t("cmdb.notifications.actions.refreshChannels")}
          </button>
          <button onClick={() => void loadNotificationTemplates()} disabled={loadingNotificationTemplates}>
            {loadingNotificationTemplates
              ? t("cmdb.actions.loading")
              : t("cmdb.notifications.actions.refreshTemplates")}
          </button>
          <button onClick={() => void loadNotificationSubscriptions()} disabled={loadingNotificationSubscriptions}>
            {loadingNotificationSubscriptions
              ? t("cmdb.actions.loading")
              : t("cmdb.notifications.actions.refreshSubscriptions")}
          </button>
        </div>

        {notificationNotice && <p className="banner banner-success">{notificationNotice}</p>}
        <p className="section-note">
          {t("cmdb.notifications.summary", {
            channels: notificationChannels.length,
            templates: notificationTemplates.length,
            subscriptions: notificationSubscriptions.length
          })}
        </p>
        {!canWriteCmdb && <p className="inline-note">{t("cmdb.notifications.messages.readOnlyHint")}</p>}

        <h3 style={subSectionTitleStyle}>{t("cmdb.notifications.channelsTitle")}</h3>
        {canWriteCmdb && (
          <div style={{ display: "flex", gap: "0.5rem", flexWrap: "wrap", marginBottom: "0.75rem" }}>
            <input
              value={newNotificationChannel.name}
              onChange={(event) =>
                setNewNotificationChannel((prev) => ({
                  ...prev,
                  name: event.target.value
                }))
              }
              placeholder={t("cmdb.notifications.form.channelName")}
            />
            <select
              value={newNotificationChannel.channel_type}
              onChange={(event) =>
                setNewNotificationChannel((prev) => ({
                  ...prev,
                  channel_type: event.target.value as "email" | "webhook"
                }))
              }
            >
              <option value="webhook">webhook</option>
              <option value="email">email</option>
            </select>
            <input
              value={newNotificationChannel.target}
              onChange={(event) =>
                setNewNotificationChannel((prev) => ({
                  ...prev,
                  target: event.target.value
                }))
              }
              placeholder={t("cmdb.notifications.form.target")}
              style={{ minWidth: "260px" }}
            />
            <input
              value={newNotificationChannel.config_json}
              onChange={(event) =>
                setNewNotificationChannel((prev) => ({
                  ...prev,
                  config_json: event.target.value
                }))
              }
              placeholder={t("cmdb.notifications.form.configJson")}
              style={{ minWidth: "240px" }}
            />
            <button onClick={() => void createNotificationChannel()} disabled={creatingNotificationChannel}>
              {creatingNotificationChannel ? t("cmdb.actions.creating") : t("cmdb.notifications.actions.createChannel")}
            </button>
          </div>
        )}
        {loadingNotificationChannels && notificationChannels.length === 0 ? (
          <p>{t("cmdb.notifications.messages.loadingChannels")}</p>
        ) : notificationChannels.length === 0 ? (
          <p>{t("cmdb.notifications.messages.noChannels")}</p>
        ) : (
          <div style={{ overflowX: "auto", marginBottom: "1rem" }}>
            <table style={{ borderCollapse: "collapse", minWidth: "900px", width: "100%" }}>
              <thead>
                <tr>
                  <th style={cellStyle}>{t("cmdb.notifications.table.channel.id")}</th>
                  <th style={cellStyle}>{t("cmdb.notifications.table.channel.name")}</th>
                  <th style={cellStyle}>{t("cmdb.notifications.table.channel.type")}</th>
                  <th style={cellStyle}>{t("cmdb.notifications.table.channel.target")}</th>
                  <th style={cellStyle}>{t("cmdb.notifications.table.channel.enabled")}</th>
                  <th style={cellStyle}>{t("cmdb.notifications.table.channel.updatedAt")}</th>
                </tr>
              </thead>
              <tbody>
                {notificationChannels.map((channel) => (
                  <tr key={channel.id}>
                    <td style={cellStyle}>{channel.id}</td>
                    <td style={cellStyle}>{channel.name}</td>
                    <td style={cellStyle}>{channel.channel_type}</td>
                    <td style={cellStyle}>{channel.target}</td>
                    <td style={cellStyle}>{channel.is_enabled ? "Yes" : "No"}</td>
                    <td style={cellStyle}>{new Date(channel.updated_at).toLocaleString()}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}

        <h3 style={subSectionTitleStyle}>{t("cmdb.notifications.templatesTitle")}</h3>
        {canWriteCmdb && (
          <div style={{ display: "flex", gap: "0.5rem", flexWrap: "wrap", marginBottom: "0.75rem" }}>
            <input
              value={newNotificationTemplate.event_type}
              onChange={(event) =>
                setNewNotificationTemplate((prev) => ({
                  ...prev,
                  event_type: event.target.value
                }))
              }
              placeholder={t("cmdb.notifications.form.eventType")}
            />
            <input
              value={newNotificationTemplate.title_template}
              onChange={(event) =>
                setNewNotificationTemplate((prev) => ({
                  ...prev,
                  title_template: event.target.value
                }))
              }
              placeholder={t("cmdb.notifications.form.titleTemplate")}
              style={{ minWidth: "260px" }}
            />
            <input
              value={newNotificationTemplate.body_template}
              onChange={(event) =>
                setNewNotificationTemplate((prev) => ({
                  ...prev,
                  body_template: event.target.value
                }))
              }
              placeholder={t("cmdb.notifications.form.bodyTemplate")}
              style={{ minWidth: "320px" }}
            />
            <button onClick={() => void createNotificationTemplate()} disabled={creatingNotificationTemplate}>
              {creatingNotificationTemplate ? t("cmdb.actions.creating") : t("cmdb.notifications.actions.createTemplate")}
            </button>
          </div>
        )}
        {loadingNotificationTemplates && notificationTemplates.length === 0 ? (
          <p>{t("cmdb.notifications.messages.loadingTemplates")}</p>
        ) : notificationTemplates.length === 0 ? (
          <p>{t("cmdb.notifications.messages.noTemplates")}</p>
        ) : (
          <div style={{ overflowX: "auto", marginBottom: "1rem" }}>
            <table style={{ borderCollapse: "collapse", minWidth: "1100px", width: "100%" }}>
              <thead>
                <tr>
                  <th style={cellStyle}>{t("cmdb.notifications.table.template.id")}</th>
                  <th style={cellStyle}>{t("cmdb.notifications.table.template.eventType")}</th>
                  <th style={cellStyle}>{t("cmdb.notifications.table.template.titleTemplate")}</th>
                  <th style={cellStyle}>{t("cmdb.notifications.table.template.bodyTemplate")}</th>
                  <th style={cellStyle}>{t("cmdb.notifications.table.template.enabled")}</th>
                  <th style={cellStyle}>{t("cmdb.notifications.table.template.updatedAt")}</th>
                </tr>
              </thead>
              <tbody>
                {notificationTemplates.map((template) => (
                  <tr key={template.id}>
                    <td style={cellStyle}>{template.id}</td>
                    <td style={cellStyle}>{template.event_type}</td>
                    <td style={cellStyle}>{template.title_template}</td>
                    <td style={cellStyle}>{template.body_template}</td>
                    <td style={cellStyle}>{template.is_enabled ? "Yes" : "No"}</td>
                    <td style={cellStyle}>{new Date(template.updated_at).toLocaleString()}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}

        <h3 style={subSectionTitleStyle}>{t("cmdb.notifications.subscriptionsTitle")}</h3>
        {canWriteCmdb && (
          <div style={{ display: "flex", gap: "0.5rem", flexWrap: "wrap", marginBottom: "0.75rem" }}>
            <select
              value={newNotificationSubscription.channel_id}
              onChange={(event) =>
                setNewNotificationSubscription((prev) => ({
                  ...prev,
                  channel_id: event.target.value
                }))
              }
            >
              <option value="">{t("cmdb.notifications.form.selectChannel")}</option>
              {notificationChannels.map((channel) => (
                <option key={channel.id} value={channel.id}>
                  #{channel.id} {channel.name} ({channel.channel_type})
                </option>
              ))}
            </select>
            <input
              value={newNotificationSubscription.event_type}
              onChange={(event) =>
                setNewNotificationSubscription((prev) => ({
                  ...prev,
                  event_type: event.target.value
                }))
              }
              placeholder={t("cmdb.notifications.form.eventType")}
            />
            <input
              value={newNotificationSubscription.site}
              onChange={(event) =>
                setNewNotificationSubscription((prev) => ({
                  ...prev,
                  site: event.target.value
                }))
              }
              placeholder={t("cmdb.notifications.form.siteOptional")}
            />
            <input
              value={newNotificationSubscription.department}
              onChange={(event) =>
                setNewNotificationSubscription((prev) => ({
                  ...prev,
                  department: event.target.value
                }))
              }
              placeholder={t("cmdb.notifications.form.departmentOptional")}
            />
            <button onClick={() => void createNotificationSubscription()} disabled={creatingNotificationSubscription}>
              {creatingNotificationSubscription
                ? t("cmdb.actions.creating")
                : t("cmdb.notifications.actions.createSubscription")}
            </button>
          </div>
        )}
        {loadingNotificationSubscriptions && notificationSubscriptions.length === 0 ? (
          <p>{t("cmdb.notifications.messages.loadingSubscriptions")}</p>
        ) : notificationSubscriptions.length === 0 ? (
          <p>{t("cmdb.notifications.messages.noSubscriptions")}</p>
        ) : (
          <div style={{ overflowX: "auto" }}>
            <table style={{ borderCollapse: "collapse", minWidth: "1100px", width: "100%" }}>
              <thead>
                <tr>
                  <th style={cellStyle}>{t("cmdb.notifications.table.subscription.id")}</th>
                  <th style={cellStyle}>{t("cmdb.notifications.table.subscription.eventType")}</th>
                  <th style={cellStyle}>{t("cmdb.notifications.table.subscription.channel")}</th>
                  <th style={cellStyle}>{t("cmdb.notifications.table.subscription.target")}</th>
                  <th style={cellStyle}>{t("cmdb.notifications.table.subscription.scope")}</th>
                  <th style={cellStyle}>{t("cmdb.notifications.table.subscription.enabled")}</th>
                  <th style={cellStyle}>{t("cmdb.notifications.table.subscription.updatedAt")}</th>
                </tr>
              </thead>
              <tbody>
                {notificationSubscriptions.map((subscription) => {
                  const channel = notificationChannelById.get(subscription.channel_id);
                  return (
                    <tr key={subscription.id}>
                      <td style={cellStyle}>{subscription.id}</td>
                      <td style={cellStyle}>{subscription.event_type}</td>
                      <td style={cellStyle}>
                        #{subscription.channel_id} {notificationChannelNameById.get(subscription.channel_id) ?? "-"}
                      </td>
                      <td style={cellStyle}>{channel?.target ?? "-"}</td>
                      <td style={cellStyle}>
                        {subscription.site ?? "*"} / {subscription.department ?? "*"}
                      </td>
                      <td style={cellStyle}>{subscription.is_enabled ? "Yes" : "No"}</td>
                      <td style={cellStyle}>{new Date(subscription.updated_at).toLocaleString()}</td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
        </SectionCard>
      )}

      {visibleSections.has("section-fields") && (
        <SectionCard id="section-fields" title={t("cmdb.fields.title")}>
        {canWriteCmdb ? (
          <div style={{ display: "flex", gap: "0.5rem", flexWrap: "wrap", marginBottom: "0.75rem" }}>
            <input
              value={newField.field_key}
              onChange={(event) => setNewField((prev) => ({ ...prev, field_key: event.target.value }))}
              placeholder={t("cmdb.fields.form.fieldKey")}
            />
            <input
              value={newField.name}
              onChange={(event) => setNewField((prev) => ({ ...prev, name: event.target.value }))}
              placeholder={t("cmdb.fields.form.name")}
            />
            <select
              value={newField.field_type}
              onChange={(event) => setNewField((prev) => ({ ...prev, field_type: event.target.value }))}
            >
              <option value="text">text</option>
              <option value="integer">integer</option>
              <option value="float">float</option>
              <option value="boolean">boolean</option>
              <option value="enum">enum</option>
              <option value="date">date</option>
              <option value="datetime">datetime</option>
            </select>
            <input
              value={newField.max_length}
              onChange={(event) => setNewField((prev) => ({ ...prev, max_length: event.target.value }))}
              placeholder={t("cmdb.fields.form.maxLength")}
              style={{ width: "140px" }}
            />
            {newField.field_type === "enum" && (
              <input
                value={newField.options_csv}
                onChange={(event) => setNewField((prev) => ({ ...prev, options_csv: event.target.value }))}
                placeholder={t("cmdb.fields.form.enumOptions")}
                style={{ minWidth: "250px" }}
              />
            )}
            <label>
              <input
                type="checkbox"
                checked={newField.required}
                onChange={(event) => setNewField((prev) => ({ ...prev, required: event.target.checked }))}
              />{" "}
              {t("cmdb.fields.form.required")}
            </label>
            <label>
              <input
                type="checkbox"
                checked={newField.scanner_enabled}
                onChange={(event) => setNewField((prev) => ({ ...prev, scanner_enabled: event.target.checked }))}
              />{" "}
              {t("cmdb.fields.form.scannerEnabled")}
            </label>
            <button onClick={() => void createFieldDefinition()} disabled={creatingField}>
              {creatingField ? t("cmdb.actions.creating") : t("cmdb.fields.form.create")}
            </button>
          </div>
        ) : (
          <p>{t("auth.labels.readOnly")}</p>
        )}

        {fieldDefinitions.length === 0 ? (
          <p>{t("cmdb.fields.empty")}</p>
        ) : (
          <div style={{ overflowX: "auto" }}>
            <table style={{ borderCollapse: "collapse", minWidth: "900px", width: "100%" }}>
              <thead>
                <tr>
                  <th style={cellStyle}>{t("cmdb.fields.table.key")}</th>
                  <th style={cellStyle}>{t("cmdb.fields.table.name")}</th>
                  <th style={cellStyle}>{t("cmdb.fields.table.type")}</th>
                  <th style={cellStyle}>{t("cmdb.fields.table.maxLength")}</th>
                  <th style={cellStyle}>{t("cmdb.fields.table.required")}</th>
                  <th style={cellStyle}>{t("cmdb.fields.table.options")}</th>
                  <th style={cellStyle}>{t("cmdb.fields.table.scannerEnabled")}</th>
                  <th style={cellStyle}>{t("cmdb.fields.table.enabled")}</th>
                </tr>
              </thead>
              <tbody>
                {fieldDefinitions.map((item) => (
                  <tr key={item.id}>
                    <td style={cellStyle}>{item.field_key}</td>
                    <td style={cellStyle}>{item.name}</td>
                    <td style={cellStyle}>{item.field_type}</td>
                    <td style={cellStyle}>{item.max_length ?? "-"}</td>
                    <td style={cellStyle}>{item.required ? "Yes" : "No"}</td>
                    <td style={cellStyle}>{item.options?.join(", ") ?? "-"}</td>
                    <td style={cellStyle}>{item.scanner_enabled ? "Yes" : "No"}</td>
                    <td style={cellStyle}>{item.is_enabled ? "Yes" : "No"}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
        </SectionCard>
      )}

      {visibleSections.has("section-relations") && (
        <SectionCard
          id="section-relations"
          title={t("cmdb.relations.title")}
          actions={
            selectedAsset
              ? (
                <span className="section-meta">
                  {t("cmdb.relations.selectedAsset", { id: selectedAsset.id, name: selectedAsset.name })}
                </span>
              )
              : undefined
          }
        >
        {emptyState ? (
          <p>{t("cmdb.relations.messages.noAssets")}</p>
        ) : (
          <>
            {relationNotice && <p className="banner banner-success">{relationNotice}</p>}

            <div className="toolbar-row">
              <span>{t("cmdb.relations.form.sourceAsset")}</span>
              <select value={selectedAssetId} onChange={(event) => setSelectedAssetId(event.target.value)}>
                {assets.map((asset) => (
                  <option key={asset.id} value={asset.id}>
                    #{asset.id} {asset.name}
                  </option>
                ))}
              </select>
              <button
                onClick={() => {
                  const id = Number.parseInt(selectedAssetId, 10);
                  if (Number.isFinite(id) && id > 0) {
                    void loadRelations(id);
                  }
                }}
                disabled={loadingRelations}
              >
                {loadingRelations ? t("cmdb.actions.loading") : t("cmdb.relations.actions.refresh")}
              </button>
            </div>

            <p className="section-note">
              {t("cmdb.relations.summary", {
                upstream: relationSummary.upstream,
                downstream: relationSummary.downstream
              })}
            </p>

            {selectedAsset && (
              <p className="section-note">
                {t("cmdb.relations.messages.sourceDetails", {
                  class: selectedAsset.asset_class,
                  status: selectedAsset.status,
                  ip: selectedAsset.ip ?? "-"
                })}
              </p>
            )}

            {canWriteCmdb ? (
              <div className="toolbar-row">
                <span>{t("cmdb.relations.form.targetAsset")}</span>
                <select
                  value={newRelation.dst_asset_id}
                  onChange={(event) => setNewRelation((prev) => ({ ...prev, dst_asset_id: event.target.value }))}
                >
                  <option value="">{t("cmdb.relations.form.selectTarget")}</option>
                  {assets.map((asset) => (
                    <option key={asset.id} value={asset.id}>
                      #{asset.id} {asset.name}
                    </option>
                  ))}
                </select>

                <input
                  value={newRelation.relation_type}
                  onChange={(event) => setNewRelation((prev) => ({ ...prev, relation_type: event.target.value }))}
                  placeholder={t("cmdb.relations.form.relationType")}
                />

                <select
                  value={newRelation.source}
                  onChange={(event) => setNewRelation((prev) => ({ ...prev, source: event.target.value }))}
                >
                  <option value="manual">manual</option>
                  <option value="discovery">discovery</option>
                  <option value="import">import</option>
                </select>

                <button onClick={() => void createRelation()} disabled={creatingRelation}>
                  {creatingRelation ? t("cmdb.actions.creating") : t("cmdb.relations.actions.create")}
                </button>
              </div>
            ) : (
              <p className="inline-note">{t("cmdb.relations.messages.readOnlyHint")}</p>
            )}

            {loadingRelations && relations.length === 0 ? (
              <p>{t("cmdb.relations.messages.loading")}</p>
            ) : relations.length === 0 ? (
              <p>{t("cmdb.relations.messages.empty")}</p>
            ) : (
              <div style={{ overflowX: "auto" }}>
                <table style={{ borderCollapse: "collapse", minWidth: "900px", width: "100%" }}>
                  <thead>
                    <tr>
                      <th style={cellStyle}>{t("cmdb.relations.table.id")}</th>
                      <th style={cellStyle}>{t("cmdb.relations.table.source")}</th>
                      <th style={cellStyle}>{t("cmdb.relations.table.target")}</th>
                      <th style={cellStyle}>{t("cmdb.relations.table.type")}</th>
                      <th style={cellStyle}>{t("cmdb.relations.table.origin")}</th>
                      <th style={cellStyle}>{t("cmdb.relations.table.updatedAt")}</th>
                      <th style={cellStyle}>{t("cmdb.relations.table.actions")}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {relations.map((relation) => (
                      <tr key={relation.id}>
                        <td style={cellStyle}>{relation.id}</td>
                        <td style={cellStyle}>
                          #{relation.src_asset_id} {assetNameById.get(relation.src_asset_id) ?? "-"}
                        </td>
                        <td style={cellStyle}>
                          #{relation.dst_asset_id} {assetNameById.get(relation.dst_asset_id) ?? "-"}
                        </td>
                        <td style={cellStyle}>{relation.relation_type}</td>
                        <td style={cellStyle}>{relation.source}</td>
                        <td style={cellStyle}>{new Date(relation.updated_at).toLocaleString()}</td>
                        <td style={cellStyle}>
                          {canWriteCmdb ? (
                            <button
                              onClick={() => void deleteRelation(relation.id)}
                              disabled={deletingRelationId === relation.id}
                            >
                              {deletingRelationId === relation.id
                                ? t("cmdb.actions.loading")
                                : t("cmdb.relations.actions.delete")}
                            </button>
                          ) : (
                            <span>{t("auth.labels.readOnly")}</span>
                          )}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </>
        )}
        </SectionCard>
      )}

      {visibleSections.has("section-readiness") && (
        <SectionCard
          id="section-readiness"
          title={t("cmdb.assetDetail.title")}
          actions={selectedAsset ? <span className="section-meta">#{selectedAsset.id} {selectedAsset.name}</span> : undefined}
        >
        {emptyState ? (
          <p>{t("cmdb.assetDetail.messages.noAssets")}</p>
        ) : !selectedAsset ? (
          <p>{t("cmdb.assetDetail.messages.selectAsset")}</p>
        ) : (
          <>
            <div className="toolbar-row" style={{ marginBottom: "0.75rem" }}>
              <button
                onClick={() => {
                  const assetId = Number.parseInt(selectedAssetId, 10);
                  if (!Number.isFinite(assetId) || assetId <= 0) {
                    return;
                  }
                  const depth = parseImpactDepth(impactDepth) ?? 4;
                  const relationTypes = parseImpactRelationTypesInput(impactRelationTypesInput);
                  void Promise.all([
                    loadAssetBindings(assetId),
                    loadAssetMonitoring(assetId),
                    loadAssetImpact(assetId, impactDirection, depth, relationTypes)
                  ]);
                }}
                disabled={loadingAssetBindings || loadingAssetMonitoring || loadingAssetImpact}
              >
                {t("cmdb.assetDetail.actions.refresh")}
              </button>
              <span className="section-meta">
                {t("cmdb.assetDetail.assetSummary", {
                  class: selectedAsset.asset_class,
                  status: selectedAsset.status,
                  ip: selectedAsset.ip ?? "-"
                })}
              </span>
            </div>

            {bindingNotice && <p className="banner banner-success">{bindingNotice}</p>}
            {lifecycleNotice && <p className="banner banner-success">{lifecycleNotice}</p>}
            {monitoringNotice && <p className="banner banner-success">{monitoringNotice}</p>}
            {impactNotice && <p className="banner banner-success">{impactNotice}</p>}

            <div className="detail-grid">
              <div className="detail-panel">
                <h3 style={subSectionTitleStyle}>{t("cmdb.assetDetail.lifecycle.title")}</h3>
                {loadingAssetBindings && !assetBindings ? (
                  <p>{t("cmdb.assetDetail.lifecycle.loading")}</p>
                ) : !assetBindings ? (
                  <p>{t("cmdb.assetDetail.lifecycle.empty")}</p>
                ) : (
                  <>
                    <p className="section-note">
                      {t("cmdb.assetDetail.lifecycle.summary", {
                        status: selectedAsset.status,
                        departments: assetBindings.readiness.department_count,
                        services: assetBindings.readiness.business_service_count,
                        owners: assetBindings.readiness.owner_count
                      })}
                    </p>
                    <div className="readiness-checklist">
                      <span
                        className={`status-chip ${assetBindings.readiness.department_count > 0 ? "status-chip-success" : "status-chip-warn"}`}
                      >
                        {t("cmdb.assetDetail.readiness.department")}
                      </span>
                      <span
                        className={`status-chip ${assetBindings.readiness.business_service_count > 0 ? "status-chip-success" : "status-chip-warn"}`}
                      >
                        {t("cmdb.assetDetail.readiness.businessService")}
                      </span>
                      <span
                        className={`status-chip ${assetBindings.readiness.owner_count > 0 ? "status-chip-success" : "status-chip-warn"}`}
                      >
                        {t("cmdb.assetDetail.readiness.owner")}
                      </span>
                    </div>

                    {assetBindings.readiness.can_transition_operational ? (
                      <p className="inline-note">{t("cmdb.assetDetail.readiness.ready")}</p>
                    ) : (
                      <p className="inline-note">
                        {t("cmdb.assetDetail.readiness.blocked", {
                          missing: assetBindings.readiness.missing
                            .map((item) => t(`cmdb.assetDetail.readiness.missing.${item}`))
                            .join(", ")
                        })}
                      </p>
                    )}

                    <div className="toolbar-row">
                      {lifecycleStatuses.map((status) => (
                        <button
                          key={status}
                          onClick={() => void transitionAssetLifecycle(status)}
                          disabled={
                            !canWriteCmdb
                            || transitioningLifecycleStatus !== null
                            || selectedAsset.status === status
                          }
                        >
                          {transitioningLifecycleStatus === status
                            ? t("cmdb.actions.loading")
                            : t("cmdb.assetDetail.lifecycle.transitionTo", { status })}
                        </button>
                      ))}
                    </div>
                  </>
                )}
              </div>

              <div className="detail-panel">
                <h3 style={subSectionTitleStyle}>{t("cmdb.assetDetail.monitoring.title")}</h3>
                {loadingAssetMonitoring && !assetMonitoring ? (
                  <p>{t("cmdb.assetDetail.monitoring.loading")}</p>
                ) : !assetMonitoring ? (
                  <p>{t("cmdb.assetDetail.monitoring.empty")}</p>
                ) : (
                  <>
                    <p className="section-note">
                      {t("cmdb.assetDetail.monitoring.bindingSummary", {
                        source: assetMonitoring.binding?.source_system ?? "-",
                        status: assetMonitoring.binding?.last_sync_status ?? "unknown",
                        host: assetMonitoring.binding?.external_host_id ?? "-"
                      })}
                    </p>
                    <p className="section-note">
                      {t("cmdb.assetDetail.monitoring.latestJob", {
                        status: assetMonitoring.latest_job?.status ?? "-",
                        attempt: assetMonitoring.latest_job?.attempt ?? 0,
                        maxAttempts: assetMonitoring.latest_job?.max_attempts ?? 0,
                        error: assetMonitoring.latest_job?.last_error ?? "-"
                      })}
                    </p>
                    <div className="toolbar-row">
                      <button
                        onClick={() => void triggerAssetMonitoringSync()}
                        disabled={!canWriteCmdb || triggeringMonitoringSync}
                      >
                        {triggeringMonitoringSync
                          ? t("cmdb.actions.loading")
                          : t("cmdb.assetDetail.monitoring.actions.triggerSync")}
                      </button>
                    </div>
                  </>
                )}
              </div>
            </div>

            <div className="detail-grid" style={{ marginTop: "0.75rem" }}>
              <div className="detail-panel">
                <h3 style={subSectionTitleStyle}>{t("cmdb.assetDetail.bindings.title")}</h3>
                <div className="form-grid">
                  <label className="control-field">
                    <span>{t("cmdb.assetDetail.bindings.departments")}</span>
                    <input
                      value={bindingDepartmentsInput}
                      onChange={(event) => setBindingDepartmentsInput(event.target.value)}
                      placeholder={t("cmdb.assetDetail.bindings.departmentsPlaceholder")}
                      disabled={!canWriteCmdb}
                    />
                  </label>
                  <label className="control-field">
                    <span>{t("cmdb.assetDetail.bindings.businessServices")}</span>
                    <input
                      value={bindingBusinessServicesInput}
                      onChange={(event) => setBindingBusinessServicesInput(event.target.value)}
                      placeholder={t("cmdb.assetDetail.bindings.businessServicesPlaceholder")}
                      disabled={!canWriteCmdb}
                    />
                  </label>
                </div>

                <p className="section-note">{t("cmdb.assetDetail.bindings.ownerHint")}</p>
                {bindingOwnerDrafts.length === 0 ? (
                  <p className="inline-note">{t("cmdb.assetDetail.bindings.noOwners")}</p>
                ) : (
                  <div className="owner-list">
                    {bindingOwnerDrafts.map((owner) => (
                      <div className="owner-row" key={owner.key}>
                        <select
                          value={owner.owner_type}
                          onChange={(event) => updateOwnerDraftType(owner.key, normalizeOwnerType(event.target.value))}
                          disabled={!canWriteCmdb}
                        >
                          <option value="team">team</option>
                          <option value="user">user</option>
                          <option value="group">group</option>
                          <option value="external">external</option>
                        </select>
                        <input
                          value={owner.owner_ref}
                          onChange={(event) => updateOwnerDraftRef(owner.key, event.target.value)}
                          placeholder={t("cmdb.assetDetail.bindings.ownerRefPlaceholder")}
                          disabled={!canWriteCmdb}
                        />
                        <button onClick={() => removeOwnerDraft(owner.key)} disabled={!canWriteCmdb}>
                          {t("cmdb.assetDetail.bindings.actions.removeOwner")}
                        </button>
                      </div>
                    ))}
                  </div>
                )}

                <div className="toolbar-row" style={{ marginTop: "0.75rem" }}>
                  <button onClick={() => addOwnerDraft()} disabled={!canWriteCmdb}>
                    {t("cmdb.assetDetail.bindings.actions.addOwner")}
                  </button>
                  <button onClick={() => void saveAssetBindings()} disabled={!canWriteCmdb || updatingAssetBindings}>
                    {updatingAssetBindings
                      ? t("cmdb.actions.loading")
                      : t("cmdb.assetDetail.bindings.actions.save")}
                  </button>
                  <button
                    onClick={() => {
                      setBindingDepartmentsInput("");
                      setBindingBusinessServicesInput("");
                      setBindingOwnerDrafts([]);
                    }}
                    disabled={!canWriteCmdb || updatingAssetBindings}
                  >
                    {t("cmdb.assetDetail.bindings.actions.clear")}
                  </button>
                </div>
              </div>

              <div className="detail-panel">
                <h3 style={subSectionTitleStyle}>{t("cmdb.assetDetail.impact.title")}</h3>
                <div className="toolbar-row">
                  <label>
                    {t("cmdb.assetDetail.impact.directionLabel")}{" "}
                    <select
                      value={impactDirection}
                      onChange={(event) => setImpactDirection(event.target.value as ImpactDirection)}
                    >
                      <option value="downstream">downstream</option>
                      <option value="upstream">upstream</option>
                      <option value="both">both</option>
                    </select>
                  </label>
                  <label>
                    {t("cmdb.assetDetail.impact.depthLabel")}{" "}
                    <input
                      value={impactDepth}
                      onChange={(event) => setImpactDepth(event.target.value)}
                      style={{ width: "72px" }}
                    />
                  </label>
                  <button onClick={() => void refreshImpact()} disabled={loadingAssetImpact}>
                    {loadingAssetImpact ? t("cmdb.actions.loading") : t("cmdb.assetDetail.impact.actions.refresh")}
                  </button>
                </div>

                {loadingAssetImpact && !assetImpact ? (
                  <p>{t("cmdb.assetDetail.impact.loading")}</p>
                ) : !assetImpact ? (
                  <p>{t("cmdb.assetDetail.impact.empty")}</p>
                ) : (
                  <>
                    <p className="section-note">
                      {t("cmdb.assetDetail.impact.summary", {
                        direction: assetImpact.direction,
                        depth: assetImpact.depth_limit,
                        nodes: assetImpact.nodes.length,
                        edges: assetImpact.edges.length,
                        services: assetImpact.affected_business_services.length,
                        owners: assetImpact.affected_owners.length
                      })}
                    </p>
                    {hierarchyHintEdges.length === 0 ? (
                      <p className="inline-note">{t("cmdb.assetDetail.impact.noHierarchyHints")}</p>
                    ) : (
                      <div className="hint-list">
                        {hierarchyHintEdges.map((edge) => (
                          <div key={`${edge.id}-${edge.direction}`} className="hint-row">
                            #{edge.id}: {impactNodeNameById.get(edge.src_asset_id) ?? edge.src_asset_id} {"-> "}
                            {impactNodeNameById.get(edge.dst_asset_id) ?? edge.dst_asset_id}
                            {" "}({edge.direction}, d={edge.depth})
                          </div>
                        ))}
                      </div>
                    )}
                    <p className="section-note">
                      {t("cmdb.assetDetail.impact.affectedServices", {
                        value: assetImpact.affected_business_services.map((item) => item.name).join(", ") || "-"
                      })}
                    </p>
                    <p className="section-note">
                      {t("cmdb.assetDetail.impact.affectedOwners", {
                        value: assetImpact.affected_owners.map((item) => item.name).join(", ") || "-"
                      })}
                    </p>
                  </>
                )}
              </div>
            </div>
          </>
        )}
        </SectionCard>
      )}

      {visibleSections.has("section-topology") && (
        <SectionCard id="section-topology" title={t("cmdb.topology.title")}>
        {emptyState ? (
          <p>{t("cmdb.topology.messages.noAssets")}</p>
        ) : (
          <>
            <div className="filter-grid" style={{ marginBottom: "0.75rem" }}>
              <label className="control-field">
                <span>{t("cmdb.topology.filters.asset")}</span>
                <select value={selectedAssetId} onChange={(event) => setSelectedAssetId(event.target.value)}>
                  <option value="">{t("cmdb.topology.filters.selectAsset")}</option>
                  {assets.map((asset) => (
                    <option key={asset.id} value={asset.id}>
                      #{asset.id} {asset.name}
                    </option>
                  ))}
                </select>
              </label>
              <label className="control-field">
                <span>{t("cmdb.topology.filters.direction")}</span>
                <select value={impactDirection} onChange={(event) => setImpactDirection(event.target.value as ImpactDirection)}>
                  <option value="downstream">downstream</option>
                  <option value="upstream">upstream</option>
                  <option value="both">both</option>
                </select>
              </label>
              <label className="control-field">
                <span>{t("cmdb.topology.filters.depth")}</span>
                <input value={impactDepth} onChange={(event) => setImpactDepth(event.target.value)} />
              </label>
              <label className="control-field">
                <span>{t("cmdb.topology.filters.relationTypes")}</span>
                <input
                  value={impactRelationTypesInput}
                  onChange={(event) => setImpactRelationTypesInput(event.target.value)}
                  placeholder="contains,depends_on,runs_service,owned_by"
                />
              </label>
            </div>
            <div className="toolbar-row" style={{ marginBottom: "0.75rem" }}>
              <button onClick={() => void refreshImpact()} disabled={loadingAssetImpact || !selectedAssetId}>
                {loadingAssetImpact ? t("cmdb.actions.loading") : t("cmdb.topology.actions.refresh")}
              </button>
              {assetImpact && (
                <span className="section-meta">
                  {t("cmdb.topology.summary", {
                    root: `${assetImpact.root_asset_id}`,
                    nodes: assetImpact.nodes.length,
                    edges: assetImpact.edges.length,
                    depth: assetImpact.depth_limit,
                    direction: assetImpact.direction
                  })}
                </span>
              )}
            </div>
            <p className="section-note">
              {t("cmdb.topology.filters.activeRelationTypes", {
                value: impactRelationTypes.join(", ")
              })}
            </p>

            {!selectedAssetId ? (
              <p>{t("cmdb.topology.messages.selectAsset")}</p>
            ) : loadingAssetImpact && !assetImpact ? (
              <p>{t("cmdb.topology.messages.loading")}</p>
            ) : !assetImpact ? (
              <p>{t("cmdb.topology.messages.noData")}</p>
            ) : (
              <>
                <div style={{ overflowX: "auto", border: "1px solid #e2e8f0", borderRadius: "12px", padding: "0.5rem" }}>
                  <svg viewBox="0 0 980 540" style={{ width: "100%", minWidth: "780px", height: "540px", display: "block", background: "linear-gradient(180deg, #f8fafc 0%, #eef2ff 100%)", borderRadius: "8px" }}>
                    {assetImpact.edges.map((edge) => {
                      const src = topologyNodePositions.get(edge.src_asset_id);
                      const dst = topologyNodePositions.get(edge.dst_asset_id);
                      if (!src || !dst) {
                        return null;
                      }
                      const meta = topologyEdgeRenderMeta.get(topologyEdgeKey(edge)) ?? { index: 0, total: 1 };
                      const path = buildTopologyEdgePath(src, dst, meta.index, meta.total);
                      const selected = selectedTopologyEdgeKey === topologyEdgeKey(edge);
                      const stroke = relationTypeColor(edge.relation_type);

                      return (
                        <path
                          key={topologyEdgeKey(edge)}
                          d={path}
                          fill="none"
                          stroke={stroke}
                          strokeWidth={selected ? 3.2 : 1.8}
                          opacity={selected ? 1 : 0.75}
                          style={{ cursor: "pointer" }}
                          onClick={() => setSelectedTopologyEdgeKey(topologyEdgeKey(edge))}
                        />
                      );
                    })}

                    {assetImpact.nodes.map((node) => {
                      const pos = topologyNodePositions.get(node.id);
                      if (!pos) {
                        return null;
                      }

                      const isRoot = node.id === assetImpact.root_asset_id;
                      const selected = node.id === selectedAssetNumericId;
                      return (
                        <g
                          key={`topology-node-${node.id}`}
                          style={{ cursor: "pointer" }}
                          onClick={() => setSelectedAssetId(String(node.id))}
                        >
                          <circle
                            cx={pos.x}
                            cy={pos.y}
                            r={isRoot ? 19 : 15}
                            fill={topologyNodeFill(node.status, isRoot)}
                            stroke={selected ? "#1d4ed8" : "#0f172a"}
                            strokeWidth={selected ? 3 : 1.5}
                          />
                          <text
                            x={pos.x}
                            y={pos.y + 4}
                            textAnchor="middle"
                            fill="#ffffff"
                            style={{ fontSize: "10px", fontWeight: 600 }}
                          >
                            {node.id}
                          </text>
                          <text
                            x={pos.x}
                            y={pos.y + (isRoot ? 34 : 30)}
                            textAnchor="middle"
                            fill="#0f172a"
                            style={{ fontSize: "11px", fontWeight: isRoot ? 700 : 500 }}
                          >
                            {truncateTopologyLabel(node.name, 24)}
                          </text>
                        </g>
                      );
                    })}
                  </svg>
                </div>

                <div className="toolbar-row" style={{ marginTop: "0.75rem" }}>
                  {assetImpact.relation_types.map((relationType) => (
                    <span key={relationType} className="status-chip" style={{ borderColor: relationTypeColor(relationType), color: relationTypeColor(relationType) }}>
                      {relationType}
                    </span>
                  ))}
                </div>
                <p className="inline-note">{t("cmdb.topology.messages.nodeHint")}</p>

                {selectedTopologyEdge ? (
                  <div className="detail-panel" style={{ marginTop: "0.75rem" }}>
                    <h3 style={subSectionTitleStyle}>{t("cmdb.topology.edgeDetail.title")}</h3>
                    <p className="section-note">
                      {t("cmdb.topology.edgeDetail.summary", {
                        id: selectedTopologyEdge.id,
                        src: `${selectedTopologyEdge.src_asset_id}`,
                        dst: `${selectedTopologyEdge.dst_asset_id}`,
                        relationType: selectedTopologyEdge.relation_type,
                        direction: selectedTopologyEdge.direction,
                        depth: selectedTopologyEdge.depth,
                        source: selectedTopologyEdge.source
                      })}
                    </p>
                    <div className="toolbar-row">
                      <button onClick={() => setSelectedAssetId(String(selectedTopologyEdge.src_asset_id))}>
                        {t("cmdb.topology.edgeDetail.focusSource")}
                      </button>
                      <button onClick={() => setSelectedAssetId(String(selectedTopologyEdge.dst_asset_id))}>
                        {t("cmdb.topology.edgeDetail.focusTarget")}
                      </button>
                    </div>
                  </div>
                ) : (
                  <p className="inline-note">{t("cmdb.topology.messages.selectEdge")}</p>
                )}
              </>
            )}
          </>
        )}
        </SectionCard>
      )}

      {visibleSections.has("section-asset-stats") && (
        <SectionCard
          id="section-asset-stats"
          title={t("cmdb.assetStats.title")}
          actions={(
            <button onClick={() => void loadAssetStats()} disabled={loadingAssetStats}>
              {loadingAssetStats ? t("cmdb.actions.loading") : t("cmdb.assetStats.actions.refresh")}
            </button>
          )}
        >
        {loadingAssetStats && !assetStats ? (
          <p>{t("cmdb.assetStats.messages.loading")}</p>
        ) : !assetStats || assetStats.total_assets === 0 ? (
          <p>{t("cmdb.assetStats.messages.noData")}</p>
        ) : (
          <>
            <p className="section-note">
              {t("cmdb.assetStats.summary", {
                total: assetStats.total_assets,
                departmentUnbound: assetStats.unbound.department_assets,
                businessUnbound: assetStats.unbound.business_service_assets
              })}
            </p>

            <div className="detail-grid">
              <div className="detail-panel">
                <h3 style={subSectionTitleStyle}>{t("cmdb.assetStats.groups.status")}</h3>
                {assetStatsStatusBuckets.length === 0 ? (
                  <p>{t("cmdb.assetStats.messages.noBuckets")}</p>
                ) : (
                  <div>
                    {assetStatsStatusBuckets.map((bucket) => (
                      <div
                        key={`status-${bucket.key}`}
                        style={{
                          display: "grid",
                          gridTemplateColumns: "minmax(140px, 180px) 1fr auto",
                          alignItems: "center",
                          gap: "0.5rem",
                          marginBottom: "0.4rem"
                        }}
                      >
                        <span title={bucket.label} style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                          {bucket.label}
                        </span>
                        <div style={{ background: "#e2e8f0", borderRadius: "999px", overflow: "hidden", minWidth: "140px", height: "10px" }}>
                          <div style={{ width: bucketBarWidth(bucket.asset_total, assetStatsStatusMax), height: "100%", background: "#2563eb" }} />
                        </div>
                        <span>{bucket.asset_total}</span>
                      </div>
                    ))}
                  </div>
                )}
              </div>

              <div className="detail-panel">
                <h3 style={subSectionTitleStyle}>{t("cmdb.assetStats.groups.department")}</h3>
                {assetStatsDepartmentBuckets.length === 0 ? (
                  <p>{t("cmdb.assetStats.messages.noBuckets")}</p>
                ) : (
                  <div>
                    {assetStatsDepartmentBuckets.map((bucket) => (
                      <div
                        key={`department-${bucket.key}`}
                        style={{
                          display: "grid",
                          gridTemplateColumns: "minmax(140px, 180px) 1fr auto",
                          alignItems: "center",
                          gap: "0.5rem",
                          marginBottom: "0.4rem"
                        }}
                      >
                        <span title={bucket.label} style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                          {bucket.label}
                        </span>
                        <div style={{ background: "#e2e8f0", borderRadius: "999px", overflow: "hidden", minWidth: "140px", height: "10px" }}>
                          <div
                            style={{
                              width: bucketBarWidth(bucket.asset_total, assetStatsDepartmentMax),
                              height: "100%",
                              background: "#0f766e"
                            }}
                          />
                        </div>
                        <span>{bucket.asset_total}</span>
                      </div>
                    ))}
                  </div>
                )}
              </div>

              <div className="detail-panel">
                <h3 style={subSectionTitleStyle}>{t("cmdb.assetStats.groups.businessService")}</h3>
                {assetStatsBusinessServiceBuckets.length === 0 ? (
                  <p>{t("cmdb.assetStats.messages.noBuckets")}</p>
                ) : (
                  <div>
                    {assetStatsBusinessServiceBuckets.map((bucket) => (
                      <div
                        key={`business-service-${bucket.key}`}
                        style={{
                          display: "grid",
                          gridTemplateColumns: "minmax(140px, 180px) 1fr auto",
                          alignItems: "center",
                          gap: "0.5rem",
                          marginBottom: "0.4rem"
                        }}
                      >
                        <span title={bucket.label} style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                          {bucket.label}
                        </span>
                        <div style={{ background: "#e2e8f0", borderRadius: "999px", overflow: "hidden", minWidth: "140px", height: "10px" }}>
                          <div
                            style={{
                              width: bucketBarWidth(bucket.asset_total, assetStatsBusinessServiceMax),
                              height: "100%",
                              background: "#ea580c"
                            }}
                          />
                        </div>
                        <span>{bucket.asset_total}</span>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            </div>
          </>
        )}
        </SectionCard>
      )}

      {visibleSections.has("section-assets") && (
        <SectionCard
          id="section-assets"
          title={t("cmdb.assets.title")}
          actions={(
            <button onClick={resetAssetFilters} disabled={!hasAssetFilter}>
              {t("cmdb.assets.actions.resetFilters")}
            </button>
          )}
        >
        <div className="filter-grid">
          <label className="control-field">
            <span>{t("cmdb.assets.filters.searchLabel")}</span>
            <input
              value={assetSearch}
              onChange={(event) => setAssetSearch(event.target.value)}
              placeholder={t("cmdb.assets.filters.searchPlaceholder")}
            />
          </label>
          <label className="control-field">
            <span>{t("cmdb.assets.filters.statusLabel")}</span>
            <select value={assetStatusFilter} onChange={(event) => setAssetStatusFilter(event.target.value)}>
              <option value="">{t("cmdb.assets.filters.allStatuses")}</option>
              {assetStatusOptions.map((status) => (
                <option key={status} value={status}>
                  {status}
                </option>
              ))}
            </select>
          </label>
          <label className="control-field">
            <span>{t("cmdb.assets.filters.classLabel")}</span>
            <select value={assetClassFilter} onChange={(event) => setAssetClassFilter(event.target.value)}>
              <option value="">{t("cmdb.assets.filters.allClasses")}</option>
              {assetClassOptions.map((assetClass) => (
                <option key={assetClass} value={assetClass}>
                  {assetClass}
                </option>
              ))}
            </select>
          </label>
          <label className="control-field">
            <span>{t("cmdb.assets.filters.siteLabel")}</span>
            <select value={assetSiteFilter} onChange={(event) => setAssetSiteFilter(event.target.value)}>
              <option value="">{t("cmdb.assets.filters.allSites")}</option>
              {assetSiteOptions.map((site) => (
                <option key={site} value={site}>
                  {site}
                </option>
              ))}
            </select>
          </label>
          <label className="control-field">
            <span>{t("cmdb.assets.filters.sortLabel")}</span>
            <select value={assetSortMode} onChange={(event) => setAssetSortMode(event.target.value as AssetSortMode)}>
              <option value="updated_desc">{t("cmdb.assets.filters.sort.updatedDesc")}</option>
              <option value="name_asc">{t("cmdb.assets.filters.sort.nameAsc")}</option>
              <option value="id_asc">{t("cmdb.assets.filters.sort.idAsc")}</option>
            </select>
          </label>
        </div>

        <p className="section-note">
          {t("cmdb.assets.summary", { shown: filteredAssets.length, total: assets.length })}
        </p>

        {loadingAssets && assets.length === 0 ? (
          <p>{t("cmdb.assets.messages.loading")}</p>
        ) : emptyState ? (
          <p>{t("cmdb.messages.empty")}</p>
        ) : filteredAssets.length === 0 ? (
          <div className="empty-state">
            <p>{t("cmdb.assets.messages.noFilterResult")}</p>
            <button onClick={resetAssetFilters}>{t("cmdb.assets.actions.clearAndShowAll")}</button>
          </div>
        ) : (
          <div style={{ overflowX: "auto" }}>
            <table style={{ borderCollapse: "collapse", minWidth: "1200px", width: "100%" }}>
              <thead>
                <tr>
                  <th style={cellStyle}>{t("cmdb.table.id")}</th>
                  <th style={cellStyle}>{t("cmdb.table.class")}</th>
                  <th style={cellStyle}>{t("cmdb.table.name")}</th>
                  <th style={cellStyle}>{t("cmdb.table.hostname")}</th>
                  <th style={cellStyle}>{t("cmdb.table.ip")}</th>
                  <th style={cellStyle}>{t("cmdb.table.status")}</th>
                  <th style={cellStyle}>{t("cmdb.table.site")}</th>
                  <th style={cellStyle}>{t("cmdb.table.department")}</th>
                  <th style={cellStyle}>{t("cmdb.table.owner")}</th>
                  <th style={cellStyle}>{t("cmdb.table.qrCode")}</th>
                  <th style={cellStyle}>{t("cmdb.table.barcode")}</th>
                  <th style={cellStyle}>{t("cmdb.table.customFields")}</th>
                  <th style={cellStyle}>{t("cmdb.table.updatedAt")}</th>
                </tr>
              </thead>
              <tbody>
                {filteredAssets.map((asset) => (
                  <tr key={asset.id}>
                    <td style={cellStyle}>{asset.id}</td>
                    <td style={cellStyle}>{asset.asset_class}</td>
                    <td style={cellStyle}>{asset.name}</td>
                    <td style={cellStyle}>{asset.hostname ?? "-"}</td>
                    <td style={cellStyle}>{asset.ip ?? "-"}</td>
                    <td style={cellStyle}>{asset.status}</td>
                    <td style={cellStyle}>{asset.site ?? "-"}</td>
                    <td style={cellStyle}>{asset.department ?? "-"}</td>
                    <td style={cellStyle}>{asset.owner ?? "-"}</td>
                    <td style={cellStyle}>{asset.qr_code ?? "-"}</td>
                    <td style={cellStyle}>{asset.barcode ?? "-"}</td>
                    <td style={cellStyle}>{renderCustomFields(asset.custom_fields)}</td>
                    <td style={cellStyle}>{new Date(asset.updated_at).toLocaleString()}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
        </SectionCard>
      )}
    </AppShell>
  );
}

function buildConsolePageHash(page: ConsolePage): string {
  return `#/${page}`;
}

function resolveConsolePageFromHash(hash: string, canAccessAdmin: boolean): ConsolePage {
  const normalized = hash.trim().replace(/^#/, "");
  const primary = normalized.split("?")[0];
  const candidate = primary.replace(/^\/+/, "").split("/")[0];
  const directPage = parseConsolePage(candidate);
  if (directPage) {
    if (directPage === "admin" && !canAccessAdmin) {
      return defaultConsolePage;
    }
    return directPage;
  }

  const legacyPage = legacySectionToPage[candidate];
  if (legacyPage === "admin" && !canAccessAdmin) {
    return defaultConsolePage;
  }
  if (legacyPage) {
    return legacyPage;
  }

  return defaultConsolePage;
}

function parseConsolePage(value: string): ConsolePage | null {
  switch (value.trim().toLowerCase()) {
    case "overview":
    case "cmdb":
    case "monitoring":
    case "workflow":
    case "admin":
      return value.trim().toLowerCase() as ConsolePage;
    default:
      return null;
  }
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

async function readErrorMessage(response: Response): Promise<string> {
  const message = await extractApiErrorMessage(response.clone());
  if (response.status === 403) {
    if (message && isSessionExpiredError(message)) {
      return "Session is invalid or expired. Please sign in again.";
    }
    if (message) {
      return `Unauthorized: ${message}`;
    }
    return "Unauthorized: your current role cannot perform this action.";
  }

  return message ?? `HTTP ${response.status}`;
}

async function apiFetch(input: string, init?: RequestInit): Promise<Response> {
  const headers = new Headers(init?.headers ?? undefined);
  if (runtimeAuthSession?.mode === "header") {
    const principal = runtimeAuthSession.principal.trim();
    if (principal.length > 0) {
      headers.set("x-auth-user", principal);
    }
  }
  if (runtimeAuthSession?.mode === "bearer") {
    const token = runtimeAuthSession.token?.trim() ?? "";
    if (token.length > 0) {
      headers.set("Authorization", `Bearer ${token}`);
    }
  }

  const response = await fetch(input, {
    ...init,
    headers
  });

  if (response.status === 403 && runtimeAuthSession?.mode === "bearer") {
    const message = await extractApiErrorMessage(response.clone());
    if (message && isSessionExpiredError(message)) {
      runtimeAuthSession = null;
      persistAuthSession(null);
      if (typeof window !== "undefined") {
        window.dispatchEvent(new Event(AUTH_SESSION_EXPIRED_EVENT));
      }
    }
  }

  return response;
}

function trimToNull(value: string): string | null {
  const normalized = value.trim();
  return normalized.length > 0 ? normalized : null;
}

function sampleValueForField(definition: FieldDefinition): unknown {
  switch (definition.field_type) {
    case "text":
      return "sample";
    case "integer":
      return 1;
    case "float":
      return 1.5;
    case "boolean":
      return true;
    case "enum":
      return definition.options?.[0] ?? "sample";
    case "date":
      return new Date().toISOString().slice(0, 10);
    case "datetime":
      return new Date().toISOString();
    default:
      return "sample";
  }
}

function readPayloadString(payload: Record<string, unknown>, key: string): string | null {
  const value = payload[key];
  if (typeof value === "string" && value.trim().length > 0) {
    return value;
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  return null;
}

function renderCustomFields(value: Record<string, unknown>): string {
  const entries = Object.entries(value);
  if (entries.length === 0) {
    return "-";
  }

  const preview = entries
    .slice(0, 3)
    .map(([key, fieldValue]) => `${key}:${String(fieldValue)}`)
    .join("; ");

  if (entries.length <= 3) {
    return preview;
  }
  return `${preview} ...`;
}

function statusChipClass(value: string): string {
  const normalized = value.trim().toLowerCase();
  if (normalized.includes("active") || normalized.includes("enabled") || normalized.includes("success") || normalized === "ok") {
    return "status-chip status-chip-success";
  }
  if (
    normalized.includes("fail")
    || normalized.includes("error")
    || normalized.includes("disabled")
    || normalized.includes("reject")
  ) {
    return "status-chip status-chip-danger";
  }
  if (normalized.includes("pending") || normalized.includes("review") || normalized.includes("running")) {
    return "status-chip status-chip-warn";
  }
  return "status-chip";
}

let ownerDraftSequence = 0;

function createOwnerDraft(ownerType: OwnerType, ownerRef: string, keyHint?: string): OwnerDraft {
  ownerDraftSequence += 1;
  return {
    key: keyHint ? `${keyHint}-${ownerDraftSequence}` : `owner-${ownerDraftSequence}`,
    owner_type: ownerType,
    owner_ref: ownerRef
  };
}

function normalizeOwnerType(value: string): OwnerType {
  if (value === "team" || value === "user" || value === "group" || value === "external") {
    return value;
  }
  return "team";
}

function parseBindingList(value: string): string[] {
  const parts = value
    .split(/[\n,]/)
    .map((item) => item.trim())
    .filter((item) => item.length > 0);
  const seen = new Set<string>();
  const normalized: string[] = [];
  for (const item of parts) {
    const key = item.toLowerCase();
    if (!seen.has(key)) {
      seen.add(key);
      normalized.push(item);
    }
  }
  return normalized;
}

function parseImpactDepth(value: string): number | null {
  const parsed = Number.parseInt(value.trim(), 10);
  if (!Number.isFinite(parsed) || parsed < 1 || parsed > 8) {
    return null;
  }
  return parsed;
}

function parseImpactRelationTypesInput(value: string): string[] {
  const normalized = value
    .split(",")
    .map((item) => item.trim().toLowerCase())
    .filter((item) => item.length > 0)
    .filter((item) => /^[a-z0-9_-]+$/.test(item));

  if (normalized.length === 0) {
    return [...defaultImpactRelationTypes];
  }

  const unique: string[] = [];
  const seen = new Set<string>();
  for (const item of normalized) {
    if (!seen.has(item)) {
      seen.add(item);
      unique.push(item);
    }
  }

  return unique;
}

function topologyEdgeKey(edge: ImpactEdge): string {
  return `${edge.id}-${edge.direction}`;
}

function buildParallelEdgeMeta(edges: ImpactEdge[]): Map<string, { index: number; total: number }> {
  const groups = new Map<string, ImpactEdge[]>();
  for (const edge of edges) {
    const left = Math.min(edge.src_asset_id, edge.dst_asset_id);
    const right = Math.max(edge.src_asset_id, edge.dst_asset_id);
    const groupKey = `${left}-${right}`;
    const group = groups.get(groupKey) ?? [];
    group.push(edge);
    groups.set(groupKey, group);
  }

  const meta = new Map<string, { index: number; total: number }>();
  for (const group of groups.values()) {
    group.sort((left, right) => {
      if (left.relation_type !== right.relation_type) {
        return left.relation_type.localeCompare(right.relation_type);
      }
      if (left.direction !== right.direction) {
        return left.direction.localeCompare(right.direction);
      }
      return left.id - right.id;
    });

    for (let index = 0; index < group.length; index += 1) {
      meta.set(topologyEdgeKey(group[index]), {
        index,
        total: group.length
      });
    }
  }

  return meta;
}

function buildTopologyNodePositions(
  nodes: ImpactNode[],
  rootId: number,
  width: number,
  height: number,
  padding: number
): Map<number, { x: number; y: number }> {
  const positions = new Map<number, { x: number; y: number }>();
  if (nodes.length === 0) {
    return positions;
  }

  const centerX = width / 2;
  const centerY = height / 2;
  const radiusLimit = Math.max(60, Math.min(width, height) / 2 - padding);
  const rings = new Map<number, ImpactNode[]>();
  for (const node of nodes) {
    const depth = Math.max(0, node.depth);
    const group = rings.get(depth) ?? [];
    group.push(node);
    rings.set(depth, group);
  }

  const depthLevels = Array.from(rings.keys()).sort((left, right) => left - right);
  const outerLevels = depthLevels.filter((depth) => depth > 0);
  const ringStep = outerLevels.length > 0 ? radiusLimit / outerLevels.length : 0;

  positions.set(rootId, { x: centerX, y: centerY });
  for (const node of nodes) {
    if (node.id === rootId) {
      positions.set(node.id, { x: centerX, y: centerY });
    }
  }

  for (const depth of depthLevels) {
    if (depth === 0) {
      continue;
    }
    const ring = rings.get(depth) ?? [];
    if (ring.length === 0) {
      continue;
    }

    const radius = ringStep * depth;
    for (let index = 0; index < ring.length; index += 1) {
      const angle = ((Math.PI * 2) / ring.length) * index - Math.PI / 2;
      positions.set(ring[index].id, {
        x: centerX + Math.cos(angle) * radius,
        y: centerY + Math.sin(angle) * radius
      });
    }
  }

  return positions;
}

function buildTopologyEdgePath(
  src: { x: number; y: number },
  dst: { x: number; y: number },
  index: number,
  total: number
): string {
  const dx = dst.x - src.x;
  const dy = dst.y - src.y;
  const distance = Math.sqrt(dx * dx + dy * dy);
  if (distance <= 1) {
    const radius = 22 + index * 7;
    return `M ${src.x} ${src.y} C ${src.x + radius} ${src.y - radius}, ${src.x + radius * 1.4} ${src.y + radius * 0.8}, ${src.x} ${src.y + 0.1}`;
  }

  const midX = (src.x + dst.x) / 2;
  const midY = (src.y + dst.y) / 2;
  const normalX = -dy / distance;
  const normalY = dx / distance;
  const centerIndex = (total - 1) / 2;
  const offset = (index - centerIndex) * 18;
  const controlX = midX + normalX * offset;
  const controlY = midY + normalY * offset;
  return `M ${src.x.toFixed(2)} ${src.y.toFixed(2)} Q ${controlX.toFixed(2)} ${controlY.toFixed(2)} ${dst.x.toFixed(2)} ${dst.y.toFixed(2)}`;
}

function relationTypeColor(relationType: string): string {
  switch (relationType) {
    case "contains":
      return "#0f766e";
    case "depends_on":
      return "#0369a1";
    case "runs_service":
      return "#be123c";
    case "owned_by":
      return "#b45309";
    default:
      return "#475569";
  }
}

function topologyNodeFill(status: string, isRoot: boolean): string {
  if (isRoot) {
    return "#1d4ed8";
  }

  const normalized = status.trim().toLowerCase();
  if (normalized === "operational" || normalized === "active") {
    return "#059669";
  }
  if (normalized === "maintenance") {
    return "#d97706";
  }
  if (normalized === "retired") {
    return "#6b7280";
  }
  return "#0f172a";
}

function truncateTopologyLabel(value: string, maxLength: number): string {
  if (value.length <= maxLength) {
    return value;
  }
  return `${value.slice(0, Math.max(0, maxLength - 1))}...`;
}

function parseMonitoringWindowMinutes(value: string): number | null {
  const parsed = Number.parseInt(value.trim(), 10);
  if (!Number.isFinite(parsed) || parsed < 5 || parsed > 1440) {
    return null;
  }
  return parsed;
}

function formatMetricValue(value: number, unit: string): string {
  if (!Number.isFinite(value)) {
    return "-";
  }
  const normalizedUnit = unit.trim();
  const text = Math.abs(value) >= 100 ? value.toFixed(0) : value.toFixed(2);
  return normalizedUnit ? `${text} ${normalizedUnit}` : text;
}

function buildMetricPolylinePoints(
  points: MonitoringMetricPoint[],
  width: number,
  height: number,
  padding: number
): string {
  if (points.length === 0) {
    return "";
  }
  if (points.length === 1) {
    const y = Math.max(padding, height - padding - (height - padding * 2) / 2);
    return `${padding},${y} ${width - padding},${y}`;
  }

  const values = points.map((point) => point.value);
  const minValue = Math.min(...values);
  const maxValue = Math.max(...values);
  const valueRange = maxValue - minValue;
  const chartWidth = Math.max(1, width - padding * 2);
  const chartHeight = Math.max(1, height - padding * 2);

  return points
    .map((point, index) => {
      const x = padding + (index / (points.length - 1)) * chartWidth;
      const ratio = valueRange === 0 ? 0.5 : (point.value - minValue) / valueRange;
      const y = height - padding - ratio * chartHeight;
      return `${x.toFixed(2)},${y.toFixed(2)}`;
    })
    .join(" ");
}

function maxBucketAssetTotal(buckets: AssetStatsBucket[]): number {
  return buckets.reduce((maxValue, bucket) => Math.max(maxValue, bucket.asset_total), 0);
}

function bucketBarWidth(value: number, maxValue: number): string {
  if (value <= 0 || maxValue <= 0) {
    return "0%";
  }
  const percent = Math.round((value / maxValue) * 100);
  return `${Math.max(percent, 6)}%`;
}

function parseWorkflowReportRangeDays(value: string): number {
  const parsed = Number.parseInt(value.trim(), 10);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    return 30;
  }
  if (parsed === 7 || parsed === 30 || parsed === 90) {
    return parsed;
  }
  return 30;
}

function parseDateMs(value: string): number | null {
  const parsed = new Date(value).getTime();
  if (Number.isNaN(parsed)) {
    return null;
  }
  return parsed;
}

function workflowTemplateDisplayName(request: WorkflowRequest): string {
  return request.template_name.trim().length > 0 ? request.template_name.trim() : `#${request.template_id}`;
}

function buildWorkflowDailyTrend(requests: WorkflowRequest[], rangeDays: number): WorkflowDailyTrendPoint[] {
  const safeRangeDays = Math.max(1, Math.min(rangeDays, 120));
  const points: WorkflowDailyTrendPoint[] = [];
  const byDay = new Map<string, WorkflowDailyTrendPoint>();
  const now = new Date();
  for (let offset = safeRangeDays - 1; offset >= 0; offset -= 1) {
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

  for (const request of requests) {
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
}

function buildWorkflowTrendRankRows(
  requests: WorkflowRequest[],
  keySelector: (request: WorkflowRequest) => string
): WorkflowTrendRankRow[] {
  const dayMs = 24 * 60 * 60 * 1000;
  const now = Date.now();
  const thisWeekStart = now - 7 * dayMs;
  const previousWeekStart = now - 14 * dayMs;
  const thisMonthStart = now - 30 * dayMs;
  const previousMonthStart = now - 60 * dayMs;

  const counters = new Map<string, WorkflowTrendRankRow>();
  for (const request of requests) {
    const labelRaw = keySelector(request).trim();
    const label = labelRaw.length > 0 ? labelRaw : "unknown";
    const key = label.toLowerCase();
    const createdAt = parseDateMs(request.created_at);
    if (createdAt === null) {
      continue;
    }

    const row = counters.get(key) ?? {
      key,
      label,
      week_current: 0,
      week_previous: 0,
      week_delta: 0,
      month_current: 0,
      month_previous: 0,
      month_delta: 0
    };

    if (createdAt >= thisWeekStart) {
      row.week_current += 1;
    } else if (createdAt >= previousWeekStart) {
      row.week_previous += 1;
    }

    if (createdAt >= thisMonthStart) {
      row.month_current += 1;
    } else if (createdAt >= previousMonthStart) {
      row.month_previous += 1;
    }

    counters.set(key, row);
  }

  return Array.from(counters.values())
    .map((row) => ({
      ...row,
      week_delta: row.week_current - row.week_previous,
      month_delta: row.month_current - row.month_previous
    }))
    .filter((row) => row.week_current > 0 || row.week_previous > 0 || row.month_current > 0 || row.month_previous > 0)
    .sort((left, right) => {
      if (left.month_current !== right.month_current) {
        return right.month_current - left.month_current;
      }
      if (left.week_current !== right.week_current) {
        return right.week_current - left.week_current;
      }
      return left.label.localeCompare(right.label);
    });
}

function formatSignedDelta(value: number): string {
  if (value > 0) {
    return `+${value}`;
  }
  return String(value);
}

function escapeCsvCell(value: string): string {
  const needsQuote = value.includes(",") || value.includes("\"") || value.includes("\n") || value.includes("\r");
  const escaped = value.replaceAll("\"", "\"\"");
  return needsQuote ? `"${escaped}"` : escaped;
}

function deriveDefaultAuthSession(): AuthSession | null {
  if (API_AUTH_TOKEN.length > 0) {
    return {
      mode: "bearer",
      principal: "oidc-session",
      token: API_AUTH_TOKEN
    };
  }

  if (API_AUTH_USER.length > 0) {
    return {
      mode: "header",
      principal: API_AUTH_USER,
      token: null
    };
  }

  return null;
}

function loadStoredAuthSession(): AuthSession | null {
  if (typeof window === "undefined") {
    return null;
  }

  const raw = window.localStorage.getItem(AUTH_SESSION_STORAGE_KEY);
  if (!raw) {
    return null;
  }

  try {
    const parsed = JSON.parse(raw) as unknown;
    if (!parsed || typeof parsed !== "object") {
      return null;
    }

    const mode = (parsed as { mode?: unknown }).mode;
    const principal = (parsed as { principal?: unknown }).principal;
    const token = (parsed as { token?: unknown }).token;

    if (mode === "header" && typeof principal === "string" && principal.trim().length > 0) {
      return {
        mode: "header",
        principal: principal.trim(),
        token: null
      };
    }

    if (mode === "bearer" && typeof token === "string" && token.trim().length > 0) {
      return {
        mode: "bearer",
        principal: typeof principal === "string" ? principal : "oidc-session",
        token: token.trim()
      };
    }
  } catch {
    return null;
  }

  return null;
}

function persistAuthSession(session: AuthSession | null): void {
  if (typeof window === "undefined") {
    return;
  }

  if (!session) {
    window.localStorage.removeItem(AUTH_SESSION_STORAGE_KEY);
    return;
  }

  window.localStorage.setItem(AUTH_SESSION_STORAGE_KEY, JSON.stringify(session));
}

function isSessionExpiredError(message: string): boolean {
  const normalized = message.toLowerCase();
  return (
    normalized.includes("invalid or expired")
    || normalized.includes("bearer token cannot be empty")
    || normalized.includes("authorization header is invalid")
  );
}

async function extractApiErrorMessage(response: Response): Promise<string | null> {
  try {
    const payload = (await response.json()) as unknown;
    if (payload && typeof payload === "object" && "error" in payload) {
      const value = (payload as { error?: unknown }).error;
      if (typeof value === "string" && value.trim().length > 0) {
        return value.trim();
      }
    }
  } catch {
    return null;
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
