use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration as StdDuration, Instant};

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, patch, post},
};
use chrono::{DateTime, Duration, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value, json};
use sqlx::{FromRow, Postgres, QueryBuilder};
use tokio::net::TcpStream;
use tokio::time::{sleep, timeout};

use crate::{
    audit::{AuditLogWriteInput, write_audit_log_best_effort},
    auth::resolve_auth_user,
    error::{AppError, AppResult},
    state::AppState,
};

const MAX_TEMPLATE_KEY_LEN: usize = 64;
const MAX_TEXT_FIELD_LEN: usize = 256;
const MAX_NOTE_LEN: usize = 1024;
const MAX_PRESET_NAME_LEN: usize = 128;
const MAX_PRESET_DESCRIPTION_LEN: usize = 512;
const MAX_TICKET_REF_LEN: usize = 128;
const MAX_ARTIFACT_URL_LEN: usize = 1024;
const MAX_RISK_TICKET_TITLE_LEN: usize = 255;
const MAX_RISK_TICKET_DESCRIPTION_LEN: usize = 8_000;
const RISK_TICKET_CATEGORY: &str = "runbook-risk";
const DEFAULT_EXECUTION_LIMIT: u32 = 30;
const MAX_EXECUTION_LIMIT: u32 = 120;
const DEFAULT_PRESET_LIMIT: u32 = 50;
const MAX_PRESET_LIMIT: u32 = 120;
const DEFAULT_ANALYTICS_DAYS: u32 = 14;
const MAX_ANALYTICS_DAYS: u32 = 90;
const MAX_ANALYTICS_SCAN_ROWS: u32 = 5000;
const DEFAULT_FAILURE_FEED_LIMIT: u32 = 30;
const MAX_FAILURE_FEED_LIMIT: u32 = 120;
const MAX_FAILED_STEP_HOTSPOTS: usize = 12;
const DEFAULT_ALERT_LIMIT: u32 = 20;
const MAX_ALERT_LIMIT: u32 = 120;
const MAX_ALERT_LINK_SCAN_ROWS: u32 = 2000;
const MAX_OWNER_DIRECTORY_ITEMS: usize = 200;
const MAX_OWNER_KEY_LEN: usize = 64;
const MAX_OWNER_DISPLAY_NAME_LEN: usize = 128;
const MAX_OWNER_REF_LEN: usize = 128;
const MAX_NOTIFICATION_TARGET_LEN: usize = 512;
const MAX_ROUTING_RULES: usize = 200;
const DEFAULT_ANALYTICS_POLICY_KEY: &str = "global";
const DEFAULT_ANALYTICS_FAILURE_RATE_THRESHOLD_PERCENT: i32 = 20;
const MIN_ANALYTICS_FAILURE_RATE_THRESHOLD_PERCENT: i32 = 1;
const MAX_ANALYTICS_FAILURE_RATE_THRESHOLD_PERCENT: i32 = 100;
const DEFAULT_ANALYTICS_MINIMUM_SAMPLE_SIZE: i32 = 5;
const MIN_ANALYTICS_MINIMUM_SAMPLE_SIZE: i32 = 1;
const MAX_ANALYTICS_MINIMUM_SAMPLE_SIZE: i32 = 500;
const DEFAULT_EXECUTION_POLICY_KEY: &str = "global";
const EXECUTION_MODE_SIMULATE: &str = "simulate";
const EXECUTION_MODE_LIVE: &str = "live";
const EXECUTION_POLICY_MODE_SIMULATE_ONLY: &str = "simulate_only";
const EXECUTION_POLICY_MODE_HYBRID_LIVE: &str = "hybrid_live";
const DEFAULT_LIVE_STEP_TIMEOUT_SECONDS: i32 = 10;
const MIN_LIVE_STEP_TIMEOUT_SECONDS: i32 = 1;
const MAX_LIVE_STEP_TIMEOUT_SECONDS: i32 = 120;
const MAX_LIVE_TEMPLATE_COUNT: usize = 32;
const TICKET_STATUS_OPEN: &str = "open";
const TICKET_STATUS_IN_PROGRESS: &str = "in_progress";
const TICKET_STATUS_RESOLVED: &str = "resolved";
const TICKET_STATUS_CLOSED: &str = "closed";
const TICKET_STATUS_CANCELLED: &str = "cancelled";
const TICKET_PRIORITY_LOW: &str = "low";
const TICKET_PRIORITY_MEDIUM: &str = "medium";
const TICKET_PRIORITY_HIGH: &str = "high";
const TICKET_PRIORITY_CRITICAL: &str = "critical";
const NOTIFICATION_EVENT_RUNBOOK_RISK_TICKET_LINKED: &str = "runbook_risk.ticket_linked";
const NOTIFICATION_STATUS_QUEUED: &str = "queued";
const NOTIFICATION_STATUS_DELIVERED: &str = "delivered";
const NOTIFICATION_STATUS_FAILED: &str = "failed";
const NOTIFICATION_STATUS_SKIPPED: &str = "skipped";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/cockpit/runbook-templates", get(list_runbook_templates))
        .route(
            "/cockpit/runbook-templates/presets",
            get(list_runbook_execution_presets).post(create_runbook_execution_preset),
        )
        .route(
            "/cockpit/runbook-templates/presets/{id}",
            patch(update_runbook_execution_preset).delete(delete_runbook_execution_preset),
        )
        .route(
            "/cockpit/runbook-templates/execution-policy",
            get(get_runbook_execution_policy).put(update_runbook_execution_policy),
        )
        .route(
            "/cockpit/runbook-templates/executions",
            get(list_runbook_template_executions),
        )
        .route(
            "/cockpit/runbook-templates/executions/{id}",
            get(get_runbook_template_execution),
        )
        .route(
            "/cockpit/runbook-templates/analytics/summary",
            get(get_runbook_analytics_summary),
        )
        .route(
            "/cockpit/runbook-templates/analytics/policy",
            get(get_runbook_analytics_policy).put(update_runbook_analytics_policy),
        )
        .route(
            "/cockpit/runbook-templates/analytics/alerts",
            get(list_runbook_risk_alerts),
        )
        .route(
            "/cockpit/runbook-templates/analytics/owners",
            get(list_runbook_risk_owner_directory).put(replace_runbook_risk_owner_directory),
        )
        .route(
            "/cockpit/runbook-templates/analytics/owner-routing-rules",
            get(list_runbook_risk_owner_routing_rules).put(replace_runbook_risk_owner_routing_rules),
        )
        .route(
            "/cockpit/runbook-templates/analytics/alerts/notifications",
            get(list_runbook_risk_alert_notification_deliveries),
        )
        .route(
            "/cockpit/runbook-templates/analytics/alerts/tickets",
            post(create_runbook_risk_alert_ticket),
        )
        .route(
            "/cockpit/runbook-templates/analytics/failures",
            get(list_runbook_failure_feed),
        )
        .route(
            "/cockpit/runbook-templates/executions/{id}/replay",
            post(replay_runbook_template_execution),
        )
        .route(
            "/cockpit/runbook-templates/{key}/execute",
            post(execute_runbook_template),
        )
}

#[derive(Debug, Clone)]
struct RunbookTemplateDefinition {
    key: &'static str,
    name: &'static str,
    description: &'static str,
    category: &'static str,
    supports_live: bool,
    params: Vec<RunbookTemplateParamDefinition>,
    preflight: Vec<RunbookTemplateChecklistDefinition>,
    steps: Vec<RunbookTemplateStepDefinition>,
}

#[derive(Debug, Clone)]
struct RunbookTemplateParamDefinition {
    key: &'static str,
    label: &'static str,
    field_type: &'static str,
    required: bool,
    options: Vec<&'static str>,
    min_value: Option<i64>,
    max_value: Option<i64>,
    default_value: Option<&'static str>,
    placeholder: Option<&'static str>,
}

#[derive(Debug, Clone)]
struct RunbookTemplateChecklistDefinition {
    key: &'static str,
    label: &'static str,
    detail: &'static str,
}

#[derive(Debug, Clone)]
struct RunbookTemplateStepDefinition {
    step_id: &'static str,
    name: &'static str,
    detail: &'static str,
    failure_hint: &'static str,
}

#[derive(Debug, Serialize, Clone)]
struct RunbookTemplateParamItem {
    key: String,
    label: String,
    field_type: String,
    required: bool,
    options: Vec<String>,
    min_value: Option<i64>,
    max_value: Option<i64>,
    default_value: Option<String>,
    placeholder: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
struct RunbookTemplateChecklistItem {
    key: String,
    label: String,
    detail: String,
}

#[derive(Debug, Serialize, Clone)]
struct RunbookTemplateStepItem {
    step_id: String,
    name: String,
    detail: String,
}

#[derive(Debug, Serialize, Clone)]
struct RunbookTemplateCatalogItem {
    key: String,
    name: String,
    description: String,
    category: String,
    execution_modes: Vec<String>,
    params: Vec<RunbookTemplateParamItem>,
    preflight: Vec<RunbookTemplateChecklistItem>,
    steps: Vec<RunbookTemplateStepItem>,
}

#[derive(Debug, Serialize)]
struct ListRunbookTemplatesResponse {
    generated_at: DateTime<Utc>,
    total: usize,
    items: Vec<RunbookTemplateCatalogItem>,
}

#[derive(Debug, Deserialize)]
struct ExecuteRunbookTemplateRequest {
    execution_mode: Option<String>,
    params: Value,
    preflight_confirmations: Vec<String>,
    evidence: RunbookEvidenceInput,
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RunbookEvidenceInput {
    summary: String,
    ticket_ref: Option<String>,
    artifact_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct RunbookStepTimelineEvent {
    step_id: String,
    name: String,
    detail: String,
    status: String,
    started_at: DateTime<Utc>,
    finished_at: DateTime<Utc>,
    output: String,
    remediation_hint: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct RunbookEvidenceRecord {
    summary: String,
    ticket_ref: Option<String>,
    artifact_url: Option<String>,
    captured_at: DateTime<Utc>,
    execution_status: String,
    operator: String,
}

#[derive(Debug, Serialize, Clone)]
struct RunbookTemplateExecutionItem {
    id: i64,
    template_key: String,
    template_name: String,
    status: String,
    execution_mode: String,
    replay_source_execution_id: Option<i64>,
    actor: String,
    params: Value,
    preflight: Value,
    timeline: Vec<RunbookStepTimelineEvent>,
    evidence: RunbookEvidenceRecord,
    runtime_summary: Value,
    remediation_hints: Vec<String>,
    note: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct RunbookTemplateExecutionRow {
    id: i64,
    template_key: String,
    template_name: String,
    status: String,
    execution_mode: String,
    replay_source_execution_id: Option<i64>,
    actor: String,
    params: Value,
    preflight: Value,
    timeline: Value,
    evidence: Value,
    runtime_summary: Value,
    remediation_hints: Value,
    note: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct ExecuteRunbookTemplateResponse {
    generated_at: DateTime<Utc>,
    template: RunbookTemplateCatalogItem,
    execution: RunbookTemplateExecutionItem,
}

#[derive(Debug, Deserialize)]
struct ReplayRunbookTemplateExecutionRequest {
    execution_mode: Option<String>,
    evidence: Option<RunbookEvidenceInput>,
    note: Option<String>,
}

#[derive(Debug, Serialize)]
struct ReplayRunbookTemplateExecutionResponse {
    generated_at: DateTime<Utc>,
    template: RunbookTemplateCatalogItem,
    source_execution_id: i64,
    execution: RunbookTemplateExecutionItem,
}

#[derive(Debug, Deserialize, Default)]
struct ListRunbookTemplateExecutionsQuery {
    template_key: Option<String>,
    status: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Debug, Deserialize, Default)]
struct RunbookAnalyticsSummaryQuery {
    template_key: Option<String>,
    execution_mode: Option<String>,
    days: Option<u32>,
}

#[derive(Debug, Deserialize, Default)]
struct RunbookFailureFeedQuery {
    template_key: Option<String>,
    execution_mode: Option<String>,
    days: Option<u32>,
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Debug, Deserialize, Default)]
struct RunbookRiskAlertQuery {
    template_key: Option<String>,
    execution_mode: Option<String>,
    days: Option<u32>,
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Debug, Deserialize, Default)]
struct RunbookRiskAlertNotificationDeliveryQuery {
    template_key: Option<String>,
    execution_mode: Option<String>,
    days: Option<u32>,
    source_key: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct ReplaceRunbookRiskOwnerDirectoryRequest {
    items: Vec<UpsertRunbookRiskOwnerDirectoryItem>,
}

#[derive(Debug, Deserialize)]
struct UpsertRunbookRiskOwnerDirectoryItem {
    owner_key: String,
    display_name: String,
    owner_type: String,
    owner_ref: String,
    notification_target: Option<String>,
    note: Option<String>,
    is_enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct ReplaceRunbookRiskOwnerRoutingRulesRequest {
    items: Vec<UpsertRunbookRiskOwnerRoutingRuleItem>,
}

#[derive(Debug, Deserialize)]
struct UpsertRunbookRiskOwnerRoutingRuleItem {
    template_key: String,
    execution_mode: Option<String>,
    severity: Option<String>,
    owner_key: String,
    priority: Option<i32>,
    note: Option<String>,
    is_enabled: Option<bool>,
}

#[derive(Debug, Serialize)]
struct ListRunbookTemplateExecutionsResponse {
    generated_at: DateTime<Utc>,
    total: i64,
    limit: u32,
    offset: u32,
    items: Vec<RunbookTemplateExecutionItem>,
}

#[derive(Debug, Serialize)]
struct RunbookTemplateExecutionDetailResponse {
    generated_at: DateTime<Utc>,
    item: RunbookTemplateExecutionItem,
}

#[derive(Debug, Serialize)]
struct RunbookAnalyticsWindow {
    start_at: DateTime<Utc>,
    end_at: DateTime<Utc>,
    days: u32,
}

#[derive(Debug, Serialize)]
struct RunbookAnalyticsFilters {
    template_key: Option<String>,
    execution_mode: Option<String>,
}

#[derive(Debug, Serialize)]
struct RunbookAnalyticsSummaryTotals {
    executions: usize,
    succeeded: usize,
    failed: usize,
    simulate: usize,
    live: usize,
    replayed: usize,
    success_rate_percent: f64,
    sampled_rows: usize,
    truncated: bool,
}

#[derive(Debug, Serialize)]
struct RunbookAnalyticsTemplateItem {
    template_key: String,
    template_name: String,
    executions: usize,
    succeeded: usize,
    failed: usize,
    simulate: usize,
    live: usize,
    replayed: usize,
    success_rate_percent: f64,
}

#[derive(Debug, Serialize)]
struct RunbookAnalyticsFailedStepItem {
    template_key: String,
    template_name: String,
    step_id: String,
    failures: usize,
}

#[derive(Debug, Serialize)]
struct RunbookAnalyticsSummaryResponse {
    generated_at: DateTime<Utc>,
    window: RunbookAnalyticsWindow,
    filters: RunbookAnalyticsFilters,
    totals: RunbookAnalyticsSummaryTotals,
    templates: Vec<RunbookAnalyticsTemplateItem>,
    failed_steps: Vec<RunbookAnalyticsFailedStepItem>,
}

#[derive(Debug, Serialize)]
struct RunbookFailureFeedItem {
    id: i64,
    template_key: String,
    template_name: String,
    execution_mode: String,
    replay_source_execution_id: Option<i64>,
    actor: String,
    failed_step_id: Option<String>,
    failed_output: Option<String>,
    remediation_hint: Option<String>,
    evidence_summary: String,
    runtime_summary: Value,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct RunbookFailureFeedResponse {
    generated_at: DateTime<Utc>,
    window: RunbookAnalyticsWindow,
    filters: RunbookAnalyticsFilters,
    total: i64,
    limit: u32,
    offset: u32,
    items: Vec<RunbookFailureFeedItem>,
}

#[derive(Debug, FromRow)]
struct RunbookAnalyticsPolicyRow {
    policy_key: String,
    failure_rate_threshold_percent: i32,
    minimum_sample_size: i32,
    note: Option<String>,
    updated_by: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Clone)]
struct RunbookAnalyticsPolicyItem {
    policy_key: String,
    failure_rate_threshold_percent: i32,
    minimum_sample_size: i32,
    note: Option<String>,
    updated_by: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct RunbookAnalyticsPolicyResponse {
    generated_at: DateTime<Utc>,
    policy: RunbookAnalyticsPolicyItem,
}

#[derive(Debug, Deserialize)]
struct UpdateRunbookAnalyticsPolicyRequest {
    failure_rate_threshold_percent: Option<i32>,
    minimum_sample_size: Option<i32>,
    note: Option<String>,
}

#[derive(Debug, Serialize)]
struct RunbookRiskAlertItem {
    template_key: String,
    template_name: String,
    severity: String,
    executions: usize,
    failed: usize,
    failure_rate_percent: f64,
    top_failed_step_id: Option<String>,
    latest_failed_execution_id: Option<i64>,
    latest_failed_at: Option<DateTime<Utc>>,
    ticket_link: Option<RunbookRiskAlertTicketLinkItem>,
    notification_summary: Option<RunbookRiskAlertNotificationSummaryItem>,
    recommended_action: String,
}

#[derive(Debug, Serialize, Clone)]
struct RunbookRiskAlertTicketLinkItem {
    link_id: i64,
    ticket_id: i64,
    ticket_no: String,
    ticket_status: String,
    ticket_priority: String,
    ticket_assignee: Option<String>,
    owner_route: Option<RunbookRiskAlertOwnerRouteItem>,
    status: String,
    source_key: String,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Clone)]
struct RunbookRiskAlertOwnerRouteItem {
    owner: String,
    owner_key: Option<String>,
    owner_label: Option<String>,
    source: String,
    reason: String,
}

#[derive(Debug, Serialize, Clone)]
struct RunbookRiskOwnerDirectoryItem {
    owner_key: String,
    display_name: String,
    owner_type: String,
    owner_ref: String,
    notification_target: Option<String>,
    note: Option<String>,
    is_enabled: bool,
    updated_by: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow, Clone)]
struct RunbookRiskOwnerDirectoryRow {
    id: i64,
    owner_key: String,
    display_name: String,
    owner_type: String,
    owner_ref: String,
    notification_target: Option<String>,
    note: Option<String>,
    is_enabled: bool,
    updated_by: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Clone)]
struct RunbookRiskOwnerRoutingRuleItem {
    rule_id: i64,
    template_key: String,
    execution_mode: Option<String>,
    severity: Option<String>,
    owner_key: String,
    owner_label: Option<String>,
    owner_ref: Option<String>,
    priority: i32,
    note: Option<String>,
    is_enabled: bool,
    updated_by: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow, Clone)]
struct RunbookRiskOwnerRoutingRuleRow {
    id: i64,
    template_key: String,
    execution_mode: Option<String>,
    severity: Option<String>,
    owner_key: String,
    priority: i32,
    note: Option<String>,
    is_enabled: bool,
    updated_by: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct RunbookRiskOwnerDirectoryResponse {
    generated_at: DateTime<Utc>,
    total: usize,
    items: Vec<RunbookRiskOwnerDirectoryItem>,
}

#[derive(Debug, Serialize)]
struct RunbookRiskOwnerRoutingRulesResponse {
    generated_at: DateTime<Utc>,
    total: usize,
    items: Vec<RunbookRiskOwnerRoutingRuleItem>,
}

#[derive(Debug, FromRow)]
struct RunbookRiskAlertTicketLinkRow {
    id: i64,
    template_key: String,
    execution_mode: Option<String>,
    window_days: i32,
    source_key: String,
    status: String,
    ticket_id: i64,
    ticket_no: String,
    ticket_status: String,
    ticket_priority: String,
    ticket_assignee: Option<String>,
    ticket_metadata: Value,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Clone)]
struct RunbookRiskAlertNotificationSummaryItem {
    total: usize,
    delivered: usize,
    failed: usize,
    skipped: usize,
    latest_status: String,
    latest_channel_type: Option<String>,
    latest_target: String,
    latest_delivered_at: Option<DateTime<Utc>>,
    latest_created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Clone)]
struct RunbookRiskAlertNotificationDeliveryItem {
    delivery_id: i64,
    source_key: String,
    template_key: String,
    execution_mode: Option<String>,
    window_days: u32,
    ticket_id: i64,
    ticket_no: String,
    event_type: String,
    dispatch_status: String,
    subscription_id: Option<i64>,
    channel_id: Option<i64>,
    channel_type: Option<String>,
    target: String,
    attempts: i32,
    response_code: Option<i32>,
    last_error: Option<String>,
    delivered_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct RunbookRiskAlertNotificationDeliveryRow {
    id: i64,
    source_key: String,
    template_key: String,
    execution_mode: Option<String>,
    window_days: i32,
    ticket_id: i64,
    ticket_no: String,
    event_type: String,
    dispatch_status: String,
    subscription_id: Option<i64>,
    channel_id: Option<i64>,
    channel_type: Option<String>,
    target: String,
    attempts: i32,
    response_code: Option<i32>,
    last_error: Option<String>,
    delivered_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct NotificationDispatchTarget {
    subscription_id: i64,
    channel_id: i64,
    channel_type: String,
    target: String,
    config: Value,
}

#[derive(Debug, FromRow)]
struct NotificationTemplateRecord {
    title_template: String,
    body_template: String,
}

#[derive(Debug, Serialize)]
struct RunbookRiskAlertResponse {
    generated_at: DateTime<Utc>,
    window: RunbookAnalyticsWindow,
    filters: RunbookAnalyticsFilters,
    policy: RunbookAnalyticsPolicyItem,
    total: usize,
    limit: u32,
    offset: u32,
    items: Vec<RunbookRiskAlertItem>,
}

#[derive(Debug, Serialize)]
struct RunbookRiskAlertNotificationDeliveryResponse {
    generated_at: DateTime<Utc>,
    window: RunbookAnalyticsWindow,
    filters: RunbookAnalyticsFilters,
    total: usize,
    limit: u32,
    offset: u32,
    items: Vec<RunbookRiskAlertNotificationDeliveryItem>,
}

#[derive(Debug, Deserialize)]
struct CreateRunbookRiskAlertTicketRequest {
    template_key: String,
    execution_mode: Option<String>,
    days: Option<u32>,
    note: Option<String>,
}

#[derive(Debug, Serialize)]
struct CreateRunbookRiskAlertTicketResponse {
    generated_at: DateTime<Utc>,
    created: bool,
    source_key: String,
    alert: RunbookRiskAlertItem,
    ticket_link: RunbookRiskAlertTicketLinkItem,
    notification_summary: Option<RunbookRiskAlertNotificationSummaryItem>,
}

#[derive(Debug, Default)]
struct RunbookTemplateAggregate {
    template_name: String,
    executions: usize,
    succeeded: usize,
    failed: usize,
    simulate: usize,
    live: usize,
    replayed: usize,
}

#[derive(Debug, Default)]
struct RunbookRiskAggregate {
    template_name: String,
    executions: usize,
    failed: usize,
    failed_step_counts: BTreeMap<String, usize>,
    latest_failed_execution_id: Option<i64>,
    latest_failed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, FromRow)]
struct RunbookExecutionPolicyRow {
    policy_key: String,
    mode: String,
    live_templates: Value,
    max_live_step_timeout_seconds: i32,
    allow_simulate_failure: bool,
    note: Option<String>,
    updated_by: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct RunbookExecutionPolicyItem {
    policy_key: String,
    mode: String,
    live_templates: Vec<String>,
    max_live_step_timeout_seconds: i32,
    allow_simulate_failure: bool,
    note: Option<String>,
    updated_by: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct RunbookExecutionPolicyResponse {
    generated_at: DateTime<Utc>,
    policy: RunbookExecutionPolicyItem,
}

#[derive(Debug, Deserialize)]
struct UpdateRunbookExecutionPolicyRequest {
    mode: Option<String>,
    live_templates: Option<Vec<String>>,
    max_live_step_timeout_seconds: Option<i32>,
    allow_simulate_failure: Option<bool>,
    note: Option<String>,
}

#[derive(Debug, FromRow)]
struct RunbookExecutionPresetRow {
    id: i64,
    template_key: String,
    template_name: String,
    name: String,
    description: Option<String>,
    execution_mode: String,
    params: Value,
    preflight_confirmations: Value,
    updated_by: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Clone)]
struct RunbookExecutionPresetItem {
    id: i64,
    template_key: String,
    template_name: String,
    name: String,
    description: Option<String>,
    execution_mode: String,
    params: Value,
    preflight_confirmations: Vec<String>,
    updated_by: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Default)]
struct ListRunbookExecutionPresetsQuery {
    template_key: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Debug, Serialize)]
struct ListRunbookExecutionPresetsResponse {
    generated_at: DateTime<Utc>,
    total: i64,
    limit: u32,
    offset: u32,
    items: Vec<RunbookExecutionPresetItem>,
}

#[derive(Debug, Deserialize)]
struct CreateRunbookExecutionPresetRequest {
    template_key: String,
    name: String,
    description: Option<String>,
    execution_mode: Option<String>,
    params: Value,
    preflight_confirmations: Vec<String>,
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateRunbookExecutionPresetRequest {
    name: Option<String>,
    description: Option<String>,
    execution_mode: Option<String>,
    params: Option<Value>,
    preflight_confirmations: Option<Vec<String>>,
    note: Option<String>,
}

#[derive(Debug, Serialize)]
struct RunbookExecutionPresetDetailResponse {
    generated_at: DateTime<Utc>,
    item: RunbookExecutionPresetItem,
}

#[derive(Debug, Serialize)]
struct RunbookExecutionPresetDeleteResponse {
    generated_at: DateTime<Utc>,
    deleted_id: i64,
}

async fn list_runbook_templates() -> AppResult<Json<ListRunbookTemplatesResponse>> {
    let mut items = built_in_runbook_templates()
        .into_iter()
        .map(|template| runbook_template_to_catalog_item(&template))
        .collect::<Vec<_>>();
    items.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.key.cmp(&right.key))
    });

    Ok(Json(ListRunbookTemplatesResponse {
        generated_at: Utc::now(),
        total: items.len(),
        items,
    }))
}

async fn list_runbook_execution_presets(
    State(state): State<AppState>,
    Query(query): Query<ListRunbookExecutionPresetsQuery>,
) -> AppResult<Json<ListRunbookExecutionPresetsResponse>> {
    let template_key = query.template_key.map(normalize_template_key).transpose()?;
    let limit = query
        .limit
        .unwrap_or(DEFAULT_PRESET_LIMIT)
        .clamp(1, MAX_PRESET_LIMIT) as i64;
    let offset = query.offset.unwrap_or(0) as i64;

    let mut count_builder: QueryBuilder<Postgres> =
        QueryBuilder::new("SELECT COUNT(*) FROM ops_runbook_execution_presets p WHERE 1=1");
    append_preset_filters(&mut count_builder, template_key.clone());
    let total: i64 = count_builder
        .build_query_scalar()
        .fetch_one(&state.db)
        .await?;

    let mut list_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT id, template_key, template_name, name, description, execution_mode,
                params, preflight_confirmations, updated_by, created_at, updated_at
         FROM ops_runbook_execution_presets p
         WHERE 1=1",
    );
    append_preset_filters(&mut list_builder, template_key);
    list_builder
        .push(" ORDER BY p.template_key ASC, p.name ASC, p.id ASC LIMIT ")
        .push_bind(limit)
        .push(" OFFSET ")
        .push_bind(offset);

    let rows: Vec<RunbookExecutionPresetRow> =
        list_builder.build_query_as().fetch_all(&state.db).await?;
    let mut items = Vec::new();
    for row in rows {
        items.push(parse_runbook_execution_preset_row(row)?);
    }

    Ok(Json(ListRunbookExecutionPresetsResponse {
        generated_at: Utc::now(),
        total,
        limit: limit as u32,
        offset: offset as u32,
        items,
    }))
}

async fn create_runbook_execution_preset(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateRunbookExecutionPresetRequest>,
) -> AppResult<Json<RunbookExecutionPresetDetailResponse>> {
    let actor = resolve_auth_user(&state, &headers).await?;
    let template = resolve_template_by_key(payload.template_key.as_str())?;
    let name = normalize_preset_name(payload.name)?;
    let description = trim_optional(payload.description, MAX_PRESET_DESCRIPTION_LEN);
    let execution_mode = normalize_execution_mode(
        payload
            .execution_mode
            .unwrap_or_else(|| EXECUTION_MODE_SIMULATE.to_string()),
    )?;
    enforce_template_supports_execution_mode(&template, execution_mode.as_str())?;
    let params = normalize_runbook_params(&template, payload.params)?;
    let preflight_confirmations =
        normalize_preflight_confirmations(&template, payload.preflight_confirmations)?;
    let note = trim_optional(payload.note, MAX_NOTE_LEN);

    let existing: Option<i64> = sqlx::query_scalar(
        "SELECT id
         FROM ops_runbook_execution_presets
         WHERE template_key = $1
           AND name = $2",
    )
    .bind(template.key)
    .bind(name.as_str())
    .fetch_optional(&state.db)
    .await?;
    if existing.is_some() {
        return Err(AppError::Validation(format!(
            "runbook preset '{}' already exists for template '{}'",
            name, template.key
        )));
    }

    let row: RunbookExecutionPresetRow = sqlx::query_as(
        "INSERT INTO ops_runbook_execution_presets (
            template_key, template_name, name, description, execution_mode,
            params, preflight_confirmations, updated_by
         )
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING id, template_key, template_name, name, description, execution_mode,
                   params, preflight_confirmations, updated_by, created_at, updated_at",
    )
    .bind(template.key)
    .bind(template.name)
    .bind(name.as_str())
    .bind(description.clone())
    .bind(execution_mode.as_str())
    .bind(Value::Object(params.clone()))
    .bind(
        serde_json::to_value(&preflight_confirmations).map_err(|err| {
            AppError::Validation(format!(
                "failed to serialize preset preflight_confirmations: {err}"
            ))
        })?,
    )
    .bind(actor.as_str())
    .fetch_one(&state.db)
    .await?;

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor: actor.clone(),
            action: "ops.runbook.preset.create".to_string(),
            target_type: "ops_runbook_execution_preset".to_string(),
            target_id: Some(row.id.to_string()),
            result: "success".to_string(),
            message: note,
            metadata: json!({
                "template_key": template.key,
                "preset_name": name,
                "execution_mode": execution_mode,
                "param_count": params.len(),
                "preflight_count": preflight_confirmations.len()
            }),
        },
    )
    .await;

    let item = parse_runbook_execution_preset_row(row)?;
    Ok(Json(RunbookExecutionPresetDetailResponse {
        generated_at: item.updated_at,
        item,
    }))
}

async fn update_runbook_execution_preset(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateRunbookExecutionPresetRequest>,
) -> AppResult<Json<RunbookExecutionPresetDetailResponse>> {
    if id <= 0 {
        return Err(AppError::Validation(
            "preset id must be a positive integer".to_string(),
        ));
    }

    let has_mutation = payload.name.is_some()
        || payload.description.is_some()
        || payload.execution_mode.is_some()
        || payload.params.is_some()
        || payload.preflight_confirmations.is_some();
    if !has_mutation {
        return Err(AppError::Validation(
            "at least one preset field must be provided".to_string(),
        ));
    }

    let actor = resolve_auth_user(&state, &headers).await?;
    let existing: Option<RunbookExecutionPresetRow> = sqlx::query_as(
        "SELECT id, template_key, template_name, name, description, execution_mode,
                params, preflight_confirmations, updated_by, created_at, updated_at
         FROM ops_runbook_execution_presets
         WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;
    let existing =
        existing.ok_or_else(|| AppError::NotFound(format!("runbook preset {id} not found")))?;

    let template = resolve_template_by_key(existing.template_key.as_str())?;
    let name = match payload.name {
        Some(value) => normalize_preset_name(value)?,
        None => existing.name.clone(),
    };
    let description = if payload.description.is_some() {
        trim_optional(payload.description, MAX_PRESET_DESCRIPTION_LEN)
    } else {
        existing.description.clone()
    };
    let execution_mode = payload
        .execution_mode
        .map(normalize_execution_mode)
        .transpose()?
        .unwrap_or(existing.execution_mode.clone());
    enforce_template_supports_execution_mode(&template, execution_mode.as_str())?;
    let params = match payload.params {
        Some(value) => normalize_runbook_params(&template, value)?,
        None => parse_params_object(existing.params.clone())?,
    };
    let preflight_confirmations = match payload.preflight_confirmations {
        Some(value) => normalize_preflight_confirmations(&template, value)?,
        None => parse_preflight_confirmations(existing.preflight_confirmations.clone())?,
    };
    let note = trim_optional(payload.note, MAX_NOTE_LEN);

    let duplicate: Option<i64> = sqlx::query_scalar(
        "SELECT id
         FROM ops_runbook_execution_presets
         WHERE template_key = $1
           AND name = $2
           AND id <> $3",
    )
    .bind(template.key)
    .bind(name.as_str())
    .bind(id)
    .fetch_optional(&state.db)
    .await?;
    if duplicate.is_some() {
        return Err(AppError::Validation(format!(
            "runbook preset '{}' already exists for template '{}'",
            name, template.key
        )));
    }

    let row: RunbookExecutionPresetRow = sqlx::query_as(
        "UPDATE ops_runbook_execution_presets
         SET name = $1,
             description = $2,
             execution_mode = $3,
             params = $4,
             preflight_confirmations = $5,
             updated_by = $6,
             updated_at = NOW()
         WHERE id = $7
         RETURNING id, template_key, template_name, name, description, execution_mode,
                   params, preflight_confirmations, updated_by, created_at, updated_at",
    )
    .bind(name.as_str())
    .bind(description.clone())
    .bind(execution_mode.as_str())
    .bind(Value::Object(params.clone()))
    .bind(
        serde_json::to_value(&preflight_confirmations).map_err(|err| {
            AppError::Validation(format!(
                "failed to serialize preset preflight_confirmations: {err}"
            ))
        })?,
    )
    .bind(actor.as_str())
    .bind(id)
    .fetch_one(&state.db)
    .await?;

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor: actor.clone(),
            action: "ops.runbook.preset.update".to_string(),
            target_type: "ops_runbook_execution_preset".to_string(),
            target_id: Some(id.to_string()),
            result: "success".to_string(),
            message: note,
            metadata: json!({
                "template_key": template.key,
                "preset_name": name,
                "execution_mode": execution_mode,
                "param_count": params.len(),
                "preflight_count": preflight_confirmations.len()
            }),
        },
    )
    .await;

    let item = parse_runbook_execution_preset_row(row)?;
    Ok(Json(RunbookExecutionPresetDetailResponse {
        generated_at: item.updated_at,
        item,
    }))
}

async fn delete_runbook_execution_preset(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Json<RunbookExecutionPresetDeleteResponse>> {
    if id <= 0 {
        return Err(AppError::Validation(
            "preset id must be a positive integer".to_string(),
        ));
    }

    let actor = resolve_auth_user(&state, &headers).await?;
    let row: Option<RunbookExecutionPresetRow> = sqlx::query_as(
        "DELETE FROM ops_runbook_execution_presets
         WHERE id = $1
         RETURNING id, template_key, template_name, name, description, execution_mode,
                   params, preflight_confirmations, updated_by, created_at, updated_at",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;
    let row = row.ok_or_else(|| AppError::NotFound(format!("runbook preset {id} not found")))?;
    let note = Some(format!("deleted runbook preset '{}'", row.name));

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor,
            action: "ops.runbook.preset.delete".to_string(),
            target_type: "ops_runbook_execution_preset".to_string(),
            target_id: Some(id.to_string()),
            result: "success".to_string(),
            message: note,
            metadata: json!({
                "template_key": row.template_key,
                "preset_name": row.name,
                "execution_mode": row.execution_mode
            }),
        },
    )
    .await;

    Ok(Json(RunbookExecutionPresetDeleteResponse {
        generated_at: Utc::now(),
        deleted_id: id,
    }))
}

async fn get_runbook_execution_policy(
    State(state): State<AppState>,
) -> AppResult<Json<RunbookExecutionPolicyResponse>> {
    let row = load_or_seed_execution_policy(&state).await?;
    let policy = parse_execution_policy_row(row)?;
    Ok(Json(RunbookExecutionPolicyResponse {
        generated_at: Utc::now(),
        policy,
    }))
}

async fn update_runbook_execution_policy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UpdateRunbookExecutionPolicyRequest>,
) -> AppResult<Json<RunbookExecutionPolicyResponse>> {
    let actor = resolve_auth_user(&state, &headers).await?;
    let current = parse_execution_policy_row(load_or_seed_execution_policy(&state).await?)?;

    let mode = payload
        .mode
        .map(normalize_execution_policy_mode)
        .transpose()?
        .unwrap_or(current.mode.clone());
    let live_templates = payload
        .live_templates
        .map(normalize_live_template_keys)
        .transpose()?
        .unwrap_or_else(|| current.live_templates.clone());
    let max_live_step_timeout_seconds = payload
        .max_live_step_timeout_seconds
        .unwrap_or(current.max_live_step_timeout_seconds);
    let allow_simulate_failure = payload
        .allow_simulate_failure
        .unwrap_or(current.allow_simulate_failure);
    let note = match payload.note {
        Some(value) => trim_optional(Some(value), MAX_NOTE_LEN),
        None => current.note.clone(),
    };

    if !(MIN_LIVE_STEP_TIMEOUT_SECONDS..=MAX_LIVE_STEP_TIMEOUT_SECONDS)
        .contains(&max_live_step_timeout_seconds)
    {
        return Err(AppError::Validation(format!(
            "max_live_step_timeout_seconds must be between {} and {}",
            MIN_LIVE_STEP_TIMEOUT_SECONDS, MAX_LIVE_STEP_TIMEOUT_SECONDS
        )));
    }
    if mode == EXECUTION_POLICY_MODE_HYBRID_LIVE && live_templates.is_empty() {
        return Err(AppError::Validation(
            "live_templates cannot be empty when mode=hybrid_live".to_string(),
        ));
    }

    let row: RunbookExecutionPolicyRow = sqlx::query_as(
        "INSERT INTO ops_runbook_execution_policies (
            policy_key, mode, live_templates, max_live_step_timeout_seconds,
            allow_simulate_failure, note, updated_by
         )
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         ON CONFLICT (policy_key) DO UPDATE
         SET mode = EXCLUDED.mode,
             live_templates = EXCLUDED.live_templates,
             max_live_step_timeout_seconds = EXCLUDED.max_live_step_timeout_seconds,
             allow_simulate_failure = EXCLUDED.allow_simulate_failure,
             note = EXCLUDED.note,
             updated_by = EXCLUDED.updated_by,
             updated_at = NOW()
         RETURNING policy_key, mode, live_templates, max_live_step_timeout_seconds,
                   allow_simulate_failure, note, updated_by, created_at, updated_at",
    )
    .bind(DEFAULT_EXECUTION_POLICY_KEY)
    .bind(mode.as_str())
    .bind(serde_json::to_value(&live_templates).map_err(|err| {
        AppError::Validation(format!("failed to serialize live_templates: {err}"))
    })?)
    .bind(max_live_step_timeout_seconds)
    .bind(allow_simulate_failure)
    .bind(note.clone())
    .bind(actor.as_str())
    .fetch_one(&state.db)
    .await?;

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor: actor.clone(),
            action: "ops.runbook.execution_policy.update".to_string(),
            target_type: "ops_runbook_execution_policy".to_string(),
            target_id: Some(DEFAULT_EXECUTION_POLICY_KEY.to_string()),
            result: "success".to_string(),
            message: note,
            metadata: json!({
                "mode": mode,
                "live_template_count": live_templates.len(),
                "max_live_step_timeout_seconds": max_live_step_timeout_seconds,
                "allow_simulate_failure": allow_simulate_failure
            }),
        },
    )
    .await;

    let policy = parse_execution_policy_row(row)?;
    Ok(Json(RunbookExecutionPolicyResponse {
        generated_at: Utc::now(),
        policy,
    }))
}

async fn execute_runbook_template(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(key): Path<String>,
    Json(payload): Json<ExecuteRunbookTemplateRequest>,
) -> AppResult<Json<ExecuteRunbookTemplateResponse>> {
    let actor = resolve_auth_user(&state, &headers).await?;
    let template = resolve_template_by_key(key.as_str())?;
    let execution_policy =
        parse_execution_policy_row(load_or_seed_execution_policy(&state).await?)?;
    let execution_mode = normalize_execution_mode(
        payload
            .execution_mode
            .unwrap_or_else(|| EXECUTION_MODE_SIMULATE.to_string()),
    )?;
    enforce_execution_mode_policy(&execution_policy, &template, execution_mode.as_str())?;

    let normalized_params = normalize_runbook_params(&template, payload.params)?;
    if execution_mode == EXECUTION_MODE_LIVE
        && string_param(&normalized_params, "simulate_failure_step").is_some()
    {
        return Err(AppError::Validation(
            "simulate_failure_step is not allowed in live execution mode".to_string(),
        ));
    }
    if execution_mode == EXECUTION_MODE_SIMULATE
        && !execution_policy.allow_simulate_failure
        && string_param(&normalized_params, "simulate_failure_step").is_some()
    {
        return Err(AppError::Validation(
            "simulate_failure_step is disabled by execution policy".to_string(),
        ));
    }

    let preflight_confirmations =
        normalize_preflight_confirmations(&template, payload.preflight_confirmations)?;
    let note = trim_optional(payload.note, MAX_NOTE_LEN);
    let evidence_input = normalize_runbook_evidence_input(payload.evidence)?;

    let outcome = if execution_mode == EXECUTION_MODE_LIVE {
        execute_live_template(
            &template,
            &normalized_params,
            execution_policy.max_live_step_timeout_seconds,
        )
        .await?
    } else {
        execute_simulated_template(&template, &normalized_params)
    };
    let final_status = outcome.status.clone();

    let evidence = RunbookEvidenceRecord {
        summary: evidence_input.summary,
        ticket_ref: evidence_input.ticket_ref,
        artifact_url: evidence_input.artifact_url,
        captured_at: Utc::now(),
        execution_status: final_status.clone(),
        operator: actor.clone(),
    };

    let preflight_snapshot = json!({
        "confirmed": preflight_confirmations,
        "total_required": template.preflight.len(),
    });

    let row: RunbookTemplateExecutionRow = sqlx::query_as(
        "INSERT INTO ops_runbook_template_executions (
            template_key,
            template_name,
            status,
            execution_mode,
            replay_source_execution_id,
            actor,
            params,
            preflight,
            timeline,
            evidence,
            runtime_summary,
            remediation_hints,
            note
         )
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
         RETURNING id, template_key, template_name, status, execution_mode, replay_source_execution_id,
                   actor, params, preflight, timeline, evidence, runtime_summary, remediation_hints,
                   note, created_at, updated_at",
    )
    .bind(template.key)
    .bind(template.name)
    .bind(final_status.as_str())
    .bind(execution_mode.as_str())
    .bind(None::<i64>)
    .bind(actor.as_str())
    .bind(Value::Object(normalized_params.clone()))
    .bind(preflight_snapshot)
    .bind(serde_json::to_value(&outcome.timeline).map_err(|err| {
        AppError::Validation(format!("failed to serialize runbook timeline: {err}"))
    })?)
    .bind(serde_json::to_value(&evidence).map_err(|err| {
        AppError::Validation(format!("failed to serialize runbook evidence: {err}"))
    })?)
    .bind(outcome.runtime_summary.clone())
    .bind(serde_json::to_value(&outcome.remediation_hints).map_err(|err| {
        AppError::Validation(format!("failed to serialize remediation hints: {err}"))
    })?)
    .bind(note.clone())
    .fetch_one(&state.db)
    .await?;

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor: actor.clone(),
            action: "ops.runbook.template.execute".to_string(),
            target_type: "ops_runbook_template_execution".to_string(),
            target_id: Some(row.id.to_string()),
            result: "success".to_string(),
            message: note,
            metadata: json!({
                "template_key": template.key,
                "status": final_status,
                "execution_mode": execution_mode,
                "step_count": outcome.timeline.len(),
                "remediation_hint_count": outcome.remediation_hints.len(),
                "policy_mode": execution_policy.mode,
            }),
        },
    )
    .await;

    let execution = parse_execution_row(row)?;

    Ok(Json(ExecuteRunbookTemplateResponse {
        generated_at: execution.created_at,
        template: runbook_template_to_catalog_item(&template),
        execution,
    }))
}

async fn replay_runbook_template_execution(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(payload): Json<ReplayRunbookTemplateExecutionRequest>,
) -> AppResult<Json<ReplayRunbookTemplateExecutionResponse>> {
    if id <= 0 {
        return Err(AppError::Validation(
            "execution id must be a positive integer".to_string(),
        ));
    }

    let actor = resolve_auth_user(&state, &headers).await?;
    let source_row: Option<RunbookTemplateExecutionRow> = sqlx::query_as(
        "SELECT id, template_key, template_name, status, execution_mode, replay_source_execution_id,
                actor, params, preflight, timeline, evidence, runtime_summary, remediation_hints,
                note, created_at, updated_at
         FROM ops_runbook_template_executions
         WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;
    let source_row = source_row
        .ok_or_else(|| AppError::NotFound(format!("runbook execution {id} not found")))?;
    let source_execution = parse_execution_row(source_row)?;

    let template = resolve_template_by_key(source_execution.template_key.as_str())?;
    let execution_policy =
        parse_execution_policy_row(load_or_seed_execution_policy(&state).await?)?;
    let execution_mode = normalize_execution_mode(
        payload
            .execution_mode
            .unwrap_or_else(|| source_execution.execution_mode.clone()),
    )?;
    enforce_execution_mode_policy(&execution_policy, &template, execution_mode.as_str())?;

    let normalized_params = normalize_runbook_params(&template, source_execution.params.clone())?;
    if execution_mode == EXECUTION_MODE_LIVE
        && string_param(&normalized_params, "simulate_failure_step").is_some()
    {
        return Err(AppError::Validation(
            "simulate_failure_step is not allowed in live execution mode".to_string(),
        ));
    }
    if execution_mode == EXECUTION_MODE_SIMULATE
        && !execution_policy.allow_simulate_failure
        && string_param(&normalized_params, "simulate_failure_step").is_some()
    {
        return Err(AppError::Validation(
            "simulate_failure_step is disabled by execution policy".to_string(),
        ));
    }

    let source_preflight_confirmations =
        parse_preflight_snapshot_confirmed(source_execution.preflight.clone())?;
    let preflight_confirmations =
        normalize_preflight_confirmations(&template, source_preflight_confirmations)?;
    let note = trim_optional(payload.note, MAX_NOTE_LEN);

    let source_evidence = source_execution.evidence;
    let evidence_input = match payload.evidence {
        Some(value) => normalize_runbook_evidence_input(value)?,
        None => normalize_runbook_evidence_input(RunbookEvidenceInput {
            summary: source_evidence.summary,
            ticket_ref: source_evidence.ticket_ref,
            artifact_url: source_evidence.artifact_url,
        })?,
    };

    let mut outcome = if execution_mode == EXECUTION_MODE_LIVE {
        execute_live_template(
            &template,
            &normalized_params,
            execution_policy.max_live_step_timeout_seconds,
        )
        .await?
    } else {
        execute_simulated_template(&template, &normalized_params)
    };
    if let Value::Object(runtime_summary) = &mut outcome.runtime_summary {
        runtime_summary.insert("replay_source_execution_id".to_string(), json!(id));
    }
    let final_status = outcome.status.clone();

    let evidence = RunbookEvidenceRecord {
        summary: evidence_input.summary,
        ticket_ref: evidence_input.ticket_ref,
        artifact_url: evidence_input.artifact_url,
        captured_at: Utc::now(),
        execution_status: final_status.clone(),
        operator: actor.clone(),
    };
    let preflight_snapshot = json!({
        "confirmed": preflight_confirmations,
        "total_required": template.preflight.len(),
    });

    let row: RunbookTemplateExecutionRow = sqlx::query_as(
        "INSERT INTO ops_runbook_template_executions (
            template_key,
            template_name,
            status,
            execution_mode,
            replay_source_execution_id,
            actor,
            params,
            preflight,
            timeline,
            evidence,
            runtime_summary,
            remediation_hints,
            note
         )
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
         RETURNING id, template_key, template_name, status, execution_mode, replay_source_execution_id,
                   actor, params, preflight, timeline, evidence, runtime_summary, remediation_hints,
                   note, created_at, updated_at",
    )
    .bind(template.key)
    .bind(template.name)
    .bind(final_status.as_str())
    .bind(execution_mode.as_str())
    .bind(id)
    .bind(actor.as_str())
    .bind(Value::Object(normalized_params.clone()))
    .bind(preflight_snapshot)
    .bind(serde_json::to_value(&outcome.timeline).map_err(|err| {
        AppError::Validation(format!("failed to serialize runbook timeline: {err}"))
    })?)
    .bind(serde_json::to_value(&evidence).map_err(|err| {
        AppError::Validation(format!("failed to serialize runbook evidence: {err}"))
    })?)
    .bind(outcome.runtime_summary.clone())
    .bind(serde_json::to_value(&outcome.remediation_hints).map_err(|err| {
        AppError::Validation(format!("failed to serialize remediation hints: {err}"))
    })?)
    .bind(note.clone())
    .fetch_one(&state.db)
    .await?;

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor: actor.clone(),
            action: "ops.runbook.template.replay".to_string(),
            target_type: "ops_runbook_template_execution".to_string(),
            target_id: Some(row.id.to_string()),
            result: "success".to_string(),
            message: note,
            metadata: json!({
                "template_key": template.key,
                "status": final_status,
                "execution_mode": execution_mode,
                "step_count": outcome.timeline.len(),
                "remediation_hint_count": outcome.remediation_hints.len(),
                "policy_mode": execution_policy.mode,
                "replay_source_execution_id": id
            }),
        },
    )
    .await;

    let execution = parse_execution_row(row)?;
    Ok(Json(ReplayRunbookTemplateExecutionResponse {
        generated_at: execution.created_at,
        template: runbook_template_to_catalog_item(&template),
        source_execution_id: id,
        execution,
    }))
}

async fn list_runbook_template_executions(
    State(state): State<AppState>,
    Query(query): Query<ListRunbookTemplateExecutionsQuery>,
) -> AppResult<Json<ListRunbookTemplateExecutionsResponse>> {
    let template_key = query.template_key.map(normalize_template_key).transpose()?;
    let status = query.status.map(normalize_execution_status).transpose()?;
    let limit = query
        .limit
        .unwrap_or(DEFAULT_EXECUTION_LIMIT)
        .clamp(1, MAX_EXECUTION_LIMIT) as i64;
    let offset = query.offset.unwrap_or(0) as i64;

    let mut count_builder: QueryBuilder<Postgres> =
        QueryBuilder::new("SELECT COUNT(*) FROM ops_runbook_template_executions e WHERE 1=1");
    append_execution_filters(&mut count_builder, template_key.clone(), status.clone());
    let total: i64 = count_builder
        .build_query_scalar()
        .fetch_one(&state.db)
        .await?;

    let mut list_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT id, template_key, template_name, status, execution_mode, replay_source_execution_id,
                actor, params, preflight, timeline, evidence, runtime_summary, remediation_hints,
                note, created_at, updated_at
         FROM ops_runbook_template_executions e
         WHERE 1=1",
    );
    append_execution_filters(&mut list_builder, template_key, status);
    list_builder
        .push(" ORDER BY e.created_at DESC, e.id DESC LIMIT ")
        .push_bind(limit)
        .push(" OFFSET ")
        .push_bind(offset);

    let rows: Vec<RunbookTemplateExecutionRow> =
        list_builder.build_query_as().fetch_all(&state.db).await?;
    let mut items = Vec::new();
    for row in rows {
        items.push(parse_execution_row(row)?);
    }

    Ok(Json(ListRunbookTemplateExecutionsResponse {
        generated_at: Utc::now(),
        total,
        limit: limit as u32,
        offset: offset as u32,
        items,
    }))
}

async fn get_runbook_template_execution(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Json<RunbookTemplateExecutionDetailResponse>> {
    if id <= 0 {
        return Err(AppError::Validation(
            "execution id must be a positive integer".to_string(),
        ));
    }

    let row: Option<RunbookTemplateExecutionRow> = sqlx::query_as(
        "SELECT id, template_key, template_name, status, execution_mode, replay_source_execution_id,
                actor, params, preflight, timeline, evidence, runtime_summary, remediation_hints,
                note, created_at, updated_at
         FROM ops_runbook_template_executions
         WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;

    let row = row.ok_or_else(|| AppError::NotFound(format!("runbook execution {id} not found")))?;
    let item = parse_execution_row(row)?;

    Ok(Json(RunbookTemplateExecutionDetailResponse {
        generated_at: item.updated_at,
        item,
    }))
}

async fn get_runbook_analytics_policy(
    State(state): State<AppState>,
) -> AppResult<Json<RunbookAnalyticsPolicyResponse>> {
    let policy = parse_runbook_analytics_policy_row(load_or_seed_analytics_policy(&state).await?)?;
    Ok(Json(RunbookAnalyticsPolicyResponse {
        generated_at: Utc::now(),
        policy,
    }))
}

async fn list_runbook_risk_owner_directory(
    State(state): State<AppState>,
) -> AppResult<Json<RunbookRiskOwnerDirectoryResponse>> {
    let rows = load_runbook_risk_owner_directory_rows(&state.db).await?;
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        items.push(parse_runbook_risk_owner_directory_row(row)?);
    }
    Ok(Json(RunbookRiskOwnerDirectoryResponse {
        generated_at: Utc::now(),
        total: items.len(),
        items,
    }))
}

async fn replace_runbook_risk_owner_directory(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ReplaceRunbookRiskOwnerDirectoryRequest>,
) -> AppResult<Json<RunbookRiskOwnerDirectoryResponse>> {
    if payload.items.len() > MAX_OWNER_DIRECTORY_ITEMS {
        return Err(AppError::Validation(format!(
            "owner directory item count must be <= {}",
            MAX_OWNER_DIRECTORY_ITEMS
        )));
    }

    let actor = resolve_auth_user(&state, &headers).await?;
    let mut normalized = Vec::with_capacity(payload.items.len());
    let mut seen_keys = BTreeSet::new();
    for item in payload.items {
        let normalized_item = normalize_runbook_risk_owner_directory_item(item)?;
        if !seen_keys.insert(normalized_item.owner_key.clone()) {
            return Err(AppError::Validation(format!(
                "duplicate owner_key '{}'",
                normalized_item.owner_key
            )));
        }
        normalized.push(normalized_item);
    }

    let mut tx = state.db.begin().await?;
    let owner_keys: Vec<String> = normalized.iter().map(|item| item.owner_key.clone()).collect();
    if owner_keys.is_empty() {
        sqlx::query("DELETE FROM ops_runbook_risk_owner_routing_rules")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM ops_runbook_risk_owner_directory")
            .execute(&mut *tx)
            .await?;
    } else {
        sqlx::query(
            "DELETE FROM ops_runbook_risk_owner_routing_rules
             WHERE owner_key NOT IN (SELECT unnest($1::text[]))",
        )
        .bind(&owner_keys)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "DELETE FROM ops_runbook_risk_owner_directory
             WHERE owner_key NOT IN (SELECT unnest($1::text[]))",
        )
        .bind(&owner_keys)
        .execute(&mut *tx)
        .await?;

        for item in &normalized {
            sqlx::query(
                "INSERT INTO ops_runbook_risk_owner_directory (
                    owner_key, display_name, owner_type, owner_ref, notification_target, note, is_enabled, updated_by
                 )
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                 ON CONFLICT (owner_key) DO UPDATE
                 SET display_name = EXCLUDED.display_name,
                     owner_type = EXCLUDED.owner_type,
                     owner_ref = EXCLUDED.owner_ref,
                     notification_target = EXCLUDED.notification_target,
                     note = EXCLUDED.note,
                     is_enabled = EXCLUDED.is_enabled,
                     updated_by = EXCLUDED.updated_by,
                     updated_at = NOW()",
            )
            .bind(item.owner_key.as_str())
            .bind(item.display_name.as_str())
            .bind(item.owner_type.as_str())
            .bind(item.owner_ref.as_str())
            .bind(item.notification_target.as_deref())
            .bind(item.note.as_deref())
            .bind(item.is_enabled)
            .bind(actor.as_str())
            .execute(&mut *tx)
            .await?;
        }
    }
    tx.commit().await?;

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor,
            action: "ops.runbook.risk_owner_directory.replace".to_string(),
            target_type: "ops_runbook_risk_owner_directory".to_string(),
            target_id: None,
            result: "success".to_string(),
            message: None,
            metadata: json!({
                "item_count": normalized.len()
            }),
        },
    )
    .await;

    list_runbook_risk_owner_directory(State(state)).await
}

async fn list_runbook_risk_owner_routing_rules(
    State(state): State<AppState>,
) -> AppResult<Json<RunbookRiskOwnerRoutingRulesResponse>> {
    let owner_directory = load_runbook_risk_owner_directory_map(&state.db).await?;
    let rows = load_runbook_risk_owner_routing_rule_rows(&state.db).await?;
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        items.push(parse_runbook_risk_owner_routing_rule_row(row, &owner_directory)?);
    }
    Ok(Json(RunbookRiskOwnerRoutingRulesResponse {
        generated_at: Utc::now(),
        total: items.len(),
        items,
    }))
}

async fn replace_runbook_risk_owner_routing_rules(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ReplaceRunbookRiskOwnerRoutingRulesRequest>,
) -> AppResult<Json<RunbookRiskOwnerRoutingRulesResponse>> {
    if payload.items.len() > MAX_ROUTING_RULES {
        return Err(AppError::Validation(format!(
            "owner routing rule count must be <= {}",
            MAX_ROUTING_RULES
        )));
    }

    let actor = resolve_auth_user(&state, &headers).await?;
    let owner_directory = load_runbook_risk_owner_directory_map(&state.db).await?;
    let mut normalized = Vec::with_capacity(payload.items.len());
    let mut seen_keys = BTreeSet::new();
    for item in payload.items {
        let normalized_item = normalize_runbook_risk_owner_routing_rule_item(item, &owner_directory)?;
        let dedup_key = format!(
            "{}:{}:{}:{}",
            normalized_item.template_key,
            normalized_item.execution_mode.as_deref().unwrap_or("all"),
            normalized_item.severity.as_deref().unwrap_or("all"),
            normalized_item.owner_key
        );
        if !seen_keys.insert(dedup_key) {
            return Err(AppError::Validation(
                "duplicate owner routing rule detected".to_string(),
            ));
        }
        normalized.push(normalized_item);
    }

    let mut tx = state.db.begin().await?;
    sqlx::query("DELETE FROM ops_runbook_risk_owner_routing_rules")
        .execute(&mut *tx)
        .await?;
    for item in &normalized {
        sqlx::query(
            "INSERT INTO ops_runbook_risk_owner_routing_rules (
                template_key, execution_mode, severity, owner_key, priority, note, is_enabled, updated_by
             )
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(item.template_key.as_str())
        .bind(item.execution_mode.as_deref())
        .bind(item.severity.as_deref())
        .bind(item.owner_key.as_str())
        .bind(item.priority)
        .bind(item.note.as_deref())
        .bind(item.is_enabled)
        .bind(actor.as_str())
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor,
            action: "ops.runbook.risk_owner_routing_rules.replace".to_string(),
            target_type: "ops_runbook_risk_owner_routing_rules".to_string(),
            target_id: None,
            result: "success".to_string(),
            message: None,
            metadata: json!({
                "item_count": normalized.len()
            }),
        },
    )
    .await;

    list_runbook_risk_owner_routing_rules(State(state)).await
}

async fn update_runbook_analytics_policy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UpdateRunbookAnalyticsPolicyRequest>,
) -> AppResult<Json<RunbookAnalyticsPolicyResponse>> {
    let actor = resolve_auth_user(&state, &headers).await?;
    let current = parse_runbook_analytics_policy_row(load_or_seed_analytics_policy(&state).await?)?;

    let failure_rate_threshold_percent = payload
        .failure_rate_threshold_percent
        .unwrap_or(current.failure_rate_threshold_percent);
    if !(MIN_ANALYTICS_FAILURE_RATE_THRESHOLD_PERCENT..=MAX_ANALYTICS_FAILURE_RATE_THRESHOLD_PERCENT)
        .contains(&failure_rate_threshold_percent)
    {
        return Err(AppError::Validation(format!(
            "failure_rate_threshold_percent must be between {} and {}",
            MIN_ANALYTICS_FAILURE_RATE_THRESHOLD_PERCENT,
            MAX_ANALYTICS_FAILURE_RATE_THRESHOLD_PERCENT
        )));
    }

    let minimum_sample_size = payload
        .minimum_sample_size
        .unwrap_or(current.minimum_sample_size);
    if !(MIN_ANALYTICS_MINIMUM_SAMPLE_SIZE..=MAX_ANALYTICS_MINIMUM_SAMPLE_SIZE)
        .contains(&minimum_sample_size)
    {
        return Err(AppError::Validation(format!(
            "minimum_sample_size must be between {} and {}",
            MIN_ANALYTICS_MINIMUM_SAMPLE_SIZE, MAX_ANALYTICS_MINIMUM_SAMPLE_SIZE
        )));
    }

    let note = if payload.note.is_some() {
        trim_optional(payload.note, MAX_NOTE_LEN)
    } else {
        current.note
    };

    let row: RunbookAnalyticsPolicyRow = sqlx::query_as(
        "UPDATE ops_runbook_analytics_policies
         SET failure_rate_threshold_percent = $1,
             minimum_sample_size = $2,
             note = $3,
             updated_by = $4,
             updated_at = NOW()
         WHERE policy_key = $5
         RETURNING policy_key, failure_rate_threshold_percent, minimum_sample_size,
                   note, updated_by, created_at, updated_at",
    )
    .bind(failure_rate_threshold_percent)
    .bind(minimum_sample_size)
    .bind(note.clone())
    .bind(actor.as_str())
    .bind(DEFAULT_ANALYTICS_POLICY_KEY)
    .fetch_one(&state.db)
    .await?;

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor,
            action: "ops.runbook.analytics_policy.update".to_string(),
            target_type: "ops_runbook_analytics_policy".to_string(),
            target_id: Some(DEFAULT_ANALYTICS_POLICY_KEY.to_string()),
            result: "success".to_string(),
            message: note,
            metadata: json!({
                "failure_rate_threshold_percent": failure_rate_threshold_percent,
                "minimum_sample_size": minimum_sample_size
            }),
        },
    )
    .await;

    let policy = parse_runbook_analytics_policy_row(row)?;
    Ok(Json(RunbookAnalyticsPolicyResponse {
        generated_at: Utc::now(),
        policy,
    }))
}

async fn list_runbook_risk_alerts(
    State(state): State<AppState>,
    Query(query): Query<RunbookRiskAlertQuery>,
) -> AppResult<Json<RunbookRiskAlertResponse>> {
    let template_key = query.template_key.map(normalize_template_key).transpose()?;
    let execution_mode = query
        .execution_mode
        .map(normalize_execution_mode)
        .transpose()?;
    let days = normalize_analytics_days(query.days);
    let limit = query
        .limit
        .unwrap_or(DEFAULT_ALERT_LIMIT)
        .clamp(1, MAX_ALERT_LIMIT);
    let offset = query.offset.unwrap_or(0);
    let generated_at = Utc::now();
    let start_at = generated_at - Duration::days(days as i64);
    let policy = parse_runbook_analytics_policy_row(load_or_seed_analytics_policy(&state).await?)?;
    let ticket_links = load_runbook_risk_alert_ticket_links(
        &state,
        template_key.as_deref(),
        execution_mode.as_deref(),
        days,
    )
    .await?;
    let notification_summaries = load_runbook_risk_alert_notification_summaries(
        &state,
        template_key.as_deref(),
        execution_mode.as_deref(),
        days,
    )
    .await?;

    let mut list_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT id, template_key, template_name, status, execution_mode, replay_source_execution_id,
                actor, params, preflight, timeline, evidence, runtime_summary, remediation_hints,
                note, created_at, updated_at
         FROM ops_runbook_template_executions e
         WHERE e.created_at >= ",
    );
    list_builder.push_bind(start_at);
    append_analytics_filters(&mut list_builder, template_key.clone(), execution_mode.clone());
    list_builder
        .push(" ORDER BY e.created_at DESC, e.id DESC LIMIT ")
        .push_bind(MAX_ANALYTICS_SCAN_ROWS as i64);

    let rows: Vec<RunbookTemplateExecutionRow> =
        list_builder.build_query_as().fetch_all(&state.db).await?;

    let mut aggregates = BTreeMap::<String, RunbookRiskAggregate>::new();
    for row in rows {
        let execution = parse_execution_row(row)?;
        let aggregate = aggregates
            .entry(execution.template_key.clone())
            .or_insert_with(|| RunbookRiskAggregate {
                template_name: execution.template_name.clone(),
                ..RunbookRiskAggregate::default()
            });
        aggregate.executions += 1;

        if execution.status != "failed" {
            continue;
        }
        aggregate.failed += 1;
        if aggregate
            .latest_failed_at
            .map(|current| execution.created_at > current)
            .unwrap_or(true)
        {
            aggregate.latest_failed_at = Some(execution.created_at);
            aggregate.latest_failed_execution_id = Some(execution.id);
        }
        if let Some(step) = first_failed_timeline_step(&execution.timeline) {
            *aggregate
                .failed_step_counts
                .entry(step.step_id.clone())
                .or_insert(0) += 1;
        }
    }

    let mut alerts = aggregates
        .into_iter()
        .filter_map(|(template_key, aggregate)| {
            let ticket_link = ticket_links.get(&template_key).cloned();
            let notification_summary = notification_summaries.get(&template_key).cloned();
            build_runbook_risk_alert_item(
                template_key,
                aggregate,
                &policy,
                ticket_link,
                notification_summary,
            )
        })
        .collect::<Vec<_>>();
    alerts.sort_by(|left, right| {
        risk_severity_rank(right.severity.as_str())
            .cmp(&risk_severity_rank(left.severity.as_str()))
            .then_with(|| {
                right
                    .failure_rate_percent
                    .partial_cmp(&left.failure_rate_percent)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| right.failed.cmp(&left.failed))
            .then_with(|| left.template_key.cmp(&right.template_key))
    });

    let total = alerts.len();
    let items = alerts
        .into_iter()
        .skip(offset as usize)
        .take(limit as usize)
        .collect::<Vec<_>>();

    Ok(Json(RunbookRiskAlertResponse {
        generated_at,
        window: RunbookAnalyticsWindow {
            start_at,
            end_at: generated_at,
            days,
        },
        filters: RunbookAnalyticsFilters {
            template_key,
            execution_mode,
        },
        policy,
        total,
        limit,
        offset,
        items,
    }))
}

async fn list_runbook_risk_alert_notification_deliveries(
    State(state): State<AppState>,
    Query(query): Query<RunbookRiskAlertNotificationDeliveryQuery>,
) -> AppResult<Json<RunbookRiskAlertNotificationDeliveryResponse>> {
    let template_key = query.template_key.map(normalize_template_key).transpose()?;
    let execution_mode = query
        .execution_mode
        .map(normalize_execution_mode)
        .transpose()?;
    let source_key = trim_optional(query.source_key.clone(), 160);
    let days = normalize_analytics_days(query.days);
    let limit = query
        .limit
        .unwrap_or(DEFAULT_FAILURE_FEED_LIMIT)
        .clamp(1, MAX_FAILURE_FEED_LIMIT);
    let offset = query.offset.unwrap_or(0);
    let generated_at = Utc::now();
    let start_at = generated_at - Duration::days(days as i64);

    let rows = load_runbook_risk_alert_notification_delivery_rows(
        &state,
        template_key.clone(),
        execution_mode.clone(),
        days,
        source_key,
        Some(limit),
        Some(offset),
    )
    .await?;
    let total = load_runbook_risk_alert_notification_delivery_count(
        &state,
        template_key.clone(),
        execution_mode.clone(),
        days,
        query.source_key,
    )
    .await?;

    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        items.push(parse_runbook_risk_alert_notification_delivery_row(
            row,
            execution_mode.as_deref(),
            days,
        )?);
    }

    Ok(Json(RunbookRiskAlertNotificationDeliveryResponse {
        generated_at,
        window: RunbookAnalyticsWindow {
            start_at,
            end_at: generated_at,
            days,
        },
        filters: RunbookAnalyticsFilters {
            template_key,
            execution_mode,
        },
        total,
        limit,
        offset,
        items,
    }))
}

fn build_runbook_risk_alert_item(
    template_key: String,
    aggregate: RunbookRiskAggregate,
    policy: &RunbookAnalyticsPolicyItem,
    ticket_link: Option<RunbookRiskAlertTicketLinkItem>,
    notification_summary: Option<RunbookRiskAlertNotificationSummaryItem>,
) -> Option<RunbookRiskAlertItem> {
    if aggregate.executions < policy.minimum_sample_size as usize {
        return None;
    }
    if aggregate.failed == 0 {
        return None;
    }

    let failure_rate_percent = ((aggregate.failed as f64 * 1000.0) / aggregate.executions as f64)
        .round()
        / 10.0;
    if failure_rate_percent < policy.failure_rate_threshold_percent as f64 {
        return None;
    }

    let top_failed_step_id = aggregate
        .failed_step_counts
        .iter()
        .max_by(|(left_step, left_count), (right_step, right_count)| {
            left_count
                .cmp(right_count)
                .then_with(|| right_step.cmp(left_step))
        })
        .map(|(step_id, _)| step_id.clone());

    let severity = if failure_rate_percent >= (policy.failure_rate_threshold_percent + 20) as f64
        || aggregate.failed >= policy.minimum_sample_size as usize
    {
        "critical".to_string()
    } else {
        "warning".to_string()
    };

    let recommended_action = match top_failed_step_id.as_ref() {
        Some(step_id) => format!(
            "Review failed step '{}' and run guarded replay after dependency confirmation.",
            step_id
        ),
        None => "Review latest failed execution evidence and run guarded replay.".to_string(),
    };

    Some(RunbookRiskAlertItem {
        template_key,
        template_name: aggregate.template_name,
        severity,
        executions: aggregate.executions,
        failed: aggregate.failed,
        failure_rate_percent,
        top_failed_step_id,
        latest_failed_execution_id: aggregate.latest_failed_execution_id,
        latest_failed_at: aggregate.latest_failed_at,
        ticket_link,
        notification_summary,
        recommended_action,
    })
}

async fn load_runbook_risk_alert_ticket_links(
    state: &AppState,
    template_key: Option<&str>,
    execution_mode: Option<&str>,
    days: u32,
) -> AppResult<BTreeMap<String, RunbookRiskAlertTicketLinkItem>> {
    let normalized_template_key = template_key
        .map(|raw| normalize_template_key(raw.to_string()))
        .transpose()?;
    let normalized_execution_mode = execution_mode
        .map(|raw| normalize_execution_mode(raw.to_string()))
        .transpose()?;
    let expected_mode = normalized_execution_mode.as_deref();
    let expected_days = days as i32;

    let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT l.id, l.template_key, l.execution_mode, l.window_days, l.source_key, l.status,
                l.ticket_id, t.ticket_no, t.status AS ticket_status, t.priority AS ticket_priority,
                t.assignee AS ticket_assignee, t.metadata AS ticket_metadata, l.updated_at
         FROM ops_runbook_risk_alert_ticket_links l
         JOIN tickets t ON t.id = l.ticket_id
         WHERE l.window_days = ",
    );
    builder.push_bind(expected_days);

    match expected_mode {
        Some(mode) => {
            builder.push(" AND l.execution_mode = ").push_bind(mode);
        }
        None => {
            builder.push(" AND l.execution_mode IS NULL");
        }
    }
    if let Some(template) = normalized_template_key.as_ref() {
        builder.push(" AND l.template_key = ").push_bind(template);
    }
    builder
        .push(" ORDER BY l.updated_at DESC, l.id DESC LIMIT ")
        .push_bind(MAX_ALERT_LINK_SCAN_ROWS as i64);

    let rows: Vec<RunbookRiskAlertTicketLinkRow> = builder.build_query_as().fetch_all(&state.db).await?;
    let mut links = BTreeMap::new();
    for row in rows {
        let (item_template_key, item) =
            parse_runbook_risk_alert_ticket_link_row(row, expected_mode, days)?;
        links.entry(item_template_key).or_insert(item);
    }
    Ok(links)
}

fn parse_runbook_risk_alert_ticket_link_row(
    row: RunbookRiskAlertTicketLinkRow,
    expected_execution_mode: Option<&str>,
    expected_days: u32,
) -> AppResult<(String, RunbookRiskAlertTicketLinkItem)> {
    let RunbookRiskAlertTicketLinkRow {
        id,
        template_key,
        execution_mode,
        window_days,
        source_key,
        status,
        ticket_id,
        ticket_no,
        ticket_status,
        ticket_priority,
        ticket_assignee,
        ticket_metadata,
        updated_at,
    } = row;

    if id <= 0 {
        return Err(AppError::Validation(
            "runbook risk alert ticket link id must be a positive integer".to_string(),
        ));
    }
    if ticket_id <= 0 {
        return Err(AppError::Validation(
            "runbook risk alert ticket link ticket_id must be a positive integer".to_string(),
        ));
    }
    if window_days != expected_days as i32 {
        return Err(AppError::Validation(format!(
            "runbook risk alert ticket link window_days {} does not match expected days {}",
            window_days, expected_days
        )));
    }

    let normalized_template_key = normalize_template_key(template_key)?;
    let normalized_execution_mode = execution_mode.map(normalize_execution_mode).transpose()?;
    if normalized_execution_mode.as_deref() != expected_execution_mode {
        return Err(AppError::Validation(format!(
            "runbook risk alert ticket link execution_mode mismatch for template '{}'",
            normalized_template_key
        )));
    }

    let source_key = required_trimmed("source_key", source_key, 160)?;
    let expected_source_key = build_runbook_risk_alert_source_key(
        normalized_template_key.as_str(),
        normalized_execution_mode.as_deref(),
        expected_days,
    );
    if source_key != expected_source_key {
        return Err(AppError::Validation(format!(
            "runbook risk alert ticket link source_key mismatch for template '{}'",
            normalized_template_key
        )));
    }

    let status = normalize_ticket_lifecycle_status(status, "link status")?;
    let ticket_status = normalize_ticket_lifecycle_status(ticket_status, "ticket status")?;
    let ticket_priority = normalize_ticket_priority_for_link(ticket_priority)?;
    let ticket_no = required_trimmed("ticket_no", ticket_no, 64)?;
    let ticket_assignee = trim_optional(ticket_assignee, 128);
    let owner_route = parse_runbook_risk_alert_owner_route_from_ticket_metadata(
        &ticket_metadata,
        ticket_assignee.as_deref(),
    )?;

    Ok((
        normalized_template_key,
        RunbookRiskAlertTicketLinkItem {
            link_id: id,
            ticket_id,
            ticket_no,
            ticket_status,
            ticket_priority,
            ticket_assignee,
            owner_route,
            status,
            source_key,
            updated_at,
        },
    ))
}

#[derive(Debug, Clone)]
struct NormalizedRunbookRiskOwnerDirectoryItem {
    owner_key: String,
    display_name: String,
    owner_type: String,
    owner_ref: String,
    notification_target: Option<String>,
    note: Option<String>,
    is_enabled: bool,
}

#[derive(Debug, Clone)]
struct NormalizedRunbookRiskOwnerRoutingRuleItem {
    template_key: String,
    execution_mode: Option<String>,
    severity: Option<String>,
    owner_key: String,
    priority: i32,
    note: Option<String>,
    is_enabled: bool,
}

async fn load_runbook_risk_owner_directory_rows(
    db: &sqlx::PgPool,
) -> AppResult<Vec<RunbookRiskOwnerDirectoryRow>> {
    let rows: Vec<RunbookRiskOwnerDirectoryRow> = sqlx::query_as(
        "SELECT id, owner_key, display_name, owner_type, owner_ref, notification_target, note,
                is_enabled, updated_by, created_at, updated_at
         FROM ops_runbook_risk_owner_directory
         ORDER BY is_enabled DESC, display_name ASC, id ASC",
    )
    .fetch_all(db)
    .await?;
    Ok(rows)
}

fn parse_runbook_risk_owner_directory_row(
    row: RunbookRiskOwnerDirectoryRow,
) -> AppResult<RunbookRiskOwnerDirectoryItem> {
    if row.id <= 0 {
        return Err(AppError::Validation(
            "runbook risk owner directory id must be a positive integer".to_string(),
        ));
    }
    Ok(RunbookRiskOwnerDirectoryItem {
        owner_key: normalize_owner_key(row.owner_key)?,
        display_name: required_trimmed(
            "display_name",
            row.display_name,
            MAX_OWNER_DISPLAY_NAME_LEN,
        )?,
        owner_type: normalize_owner_type_ref(row.owner_type)?,
        owner_ref: required_trimmed("owner_ref", row.owner_ref, MAX_OWNER_REF_LEN)?,
        notification_target: trim_optional(row.notification_target, MAX_NOTIFICATION_TARGET_LEN),
        note: trim_optional(row.note, MAX_NOTE_LEN),
        is_enabled: row.is_enabled,
        updated_by: required_trimmed("updated_by", row.updated_by, MAX_OWNER_REF_LEN)?,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

async fn load_runbook_risk_owner_directory_map(
    db: &sqlx::PgPool,
) -> AppResult<BTreeMap<String, RunbookRiskOwnerDirectoryItem>> {
    let rows = load_runbook_risk_owner_directory_rows(db).await?;
    let mut items = BTreeMap::new();
    for row in rows {
        let item = parse_runbook_risk_owner_directory_row(row)?;
        items.insert(item.owner_key.clone(), item);
    }
    Ok(items)
}

async fn load_runbook_risk_owner_routing_rule_rows(
    db: &sqlx::PgPool,
) -> AppResult<Vec<RunbookRiskOwnerRoutingRuleRow>> {
    let rows: Vec<RunbookRiskOwnerRoutingRuleRow> = sqlx::query_as(
        "SELECT id, template_key, execution_mode, severity, owner_key, priority, note,
                is_enabled, updated_by, created_at, updated_at
         FROM ops_runbook_risk_owner_routing_rules
         ORDER BY priority ASC, updated_at DESC, id DESC",
    )
    .fetch_all(db)
    .await?;
    Ok(rows)
}

fn parse_runbook_risk_owner_routing_rule_row(
    row: RunbookRiskOwnerRoutingRuleRow,
    owner_directory: &BTreeMap<String, RunbookRiskOwnerDirectoryItem>,
) -> AppResult<RunbookRiskOwnerRoutingRuleItem> {
    if row.id <= 0 {
        return Err(AppError::Validation(
            "runbook risk owner routing rule id must be a positive integer".to_string(),
        ));
    }
    let template_key = normalize_template_key(row.template_key)?;
    let owner_key = normalize_owner_key(row.owner_key)?;
    let owner = owner_directory.get(&owner_key);
    Ok(RunbookRiskOwnerRoutingRuleItem {
        rule_id: row.id,
        template_key,
        execution_mode: row.execution_mode.map(normalize_execution_mode).transpose()?,
        severity: row.severity.map(normalize_risk_severity).transpose()?,
        owner_key,
        owner_label: owner.map(|item| item.display_name.clone()),
        owner_ref: owner.map(|item| item.owner_ref.clone()),
        priority: row.priority.clamp(1, 1000),
        note: trim_optional(row.note, MAX_NOTE_LEN),
        is_enabled: row.is_enabled,
        updated_by: required_trimmed("updated_by", row.updated_by, MAX_OWNER_REF_LEN)?,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

fn normalize_runbook_risk_owner_directory_item(
    item: UpsertRunbookRiskOwnerDirectoryItem,
) -> AppResult<NormalizedRunbookRiskOwnerDirectoryItem> {
    Ok(NormalizedRunbookRiskOwnerDirectoryItem {
        owner_key: normalize_owner_key(item.owner_key)?,
        display_name: required_trimmed("display_name", item.display_name, MAX_OWNER_DISPLAY_NAME_LEN)?,
        owner_type: normalize_owner_type_ref(item.owner_type)?,
        owner_ref: required_trimmed("owner_ref", item.owner_ref, MAX_OWNER_REF_LEN)?,
        notification_target: trim_optional(item.notification_target, MAX_NOTIFICATION_TARGET_LEN),
        note: trim_optional(item.note, MAX_NOTE_LEN),
        is_enabled: item.is_enabled.unwrap_or(true),
    })
}

fn normalize_runbook_risk_owner_routing_rule_item(
    item: UpsertRunbookRiskOwnerRoutingRuleItem,
    owner_directory: &BTreeMap<String, RunbookRiskOwnerDirectoryItem>,
) -> AppResult<NormalizedRunbookRiskOwnerRoutingRuleItem> {
    let owner_key = normalize_owner_key(item.owner_key)?;
    if !owner_directory.contains_key(&owner_key) {
        return Err(AppError::Validation(format!(
            "owner_key '{}' does not exist in owner directory",
            owner_key
        )));
    }
    Ok(NormalizedRunbookRiskOwnerRoutingRuleItem {
        template_key: normalize_template_key(item.template_key)?,
        execution_mode: item.execution_mode.map(normalize_execution_mode).transpose()?,
        severity: item.severity.map(normalize_risk_severity).transpose()?,
        owner_key,
        priority: item.priority.unwrap_or(100).clamp(1, 1000),
        note: trim_optional(item.note, MAX_NOTE_LEN),
        is_enabled: item.is_enabled.unwrap_or(true),
    })
}

async fn load_runbook_risk_alert_notification_summaries(
    state: &AppState,
    template_key: Option<&str>,
    execution_mode: Option<&str>,
    days: u32,
) -> AppResult<BTreeMap<String, RunbookRiskAlertNotificationSummaryItem>> {
    let rows = load_runbook_risk_alert_notification_delivery_rows(
        state,
        template_key.map(|value| value.to_string()),
        execution_mode.map(|value| value.to_string()),
        days,
        None,
        None,
        None,
    )
    .await?;

    let mut grouped = BTreeMap::<String, Vec<RunbookRiskAlertNotificationDeliveryItem>>::new();
    for row in rows {
        let item = parse_runbook_risk_alert_notification_delivery_row(
            row,
            execution_mode,
            days,
        )?;
        grouped
            .entry(item.template_key.clone())
            .or_default()
            .push(item);
    }

    let mut summaries = BTreeMap::new();
    for (item_template_key, deliveries) in grouped {
        let latest = deliveries
            .iter()
            .max_by(|left, right| {
                left.created_at
                    .cmp(&right.created_at)
                    .then_with(|| left.delivery_id.cmp(&right.delivery_id))
            })
            .cloned()
            .ok_or_else(|| {
                AppError::Validation(format!(
                    "notification delivery group for template '{}' is empty",
                    item_template_key
                ))
            })?;

        let mut delivered = 0usize;
        let mut failed = 0usize;
        let mut skipped = 0usize;
        for delivery in &deliveries {
            match delivery.dispatch_status.as_str() {
                NOTIFICATION_STATUS_DELIVERED => delivered += 1,
                NOTIFICATION_STATUS_FAILED => failed += 1,
                NOTIFICATION_STATUS_SKIPPED => skipped += 1,
                _ => {}
            }
        }

        summaries.insert(
            item_template_key,
            RunbookRiskAlertNotificationSummaryItem {
                total: deliveries.len(),
                delivered,
                failed,
                skipped,
                latest_status: latest.dispatch_status,
                latest_channel_type: latest.channel_type,
                latest_target: latest.target,
                latest_delivered_at: latest.delivered_at,
                latest_created_at: latest.created_at,
            },
        );
    }

    Ok(summaries)
}

fn parse_runbook_risk_alert_owner_route_from_ticket_metadata(
    metadata: &Value,
    ticket_assignee: Option<&str>,
) -> AppResult<Option<RunbookRiskAlertOwnerRouteItem>> {
    let fallback_assignee = ticket_assignee
        .map(|value| required_trimmed("ticket_assignee", value.to_string(), 128))
        .transpose()?;

    let Some(route) = metadata
        .as_object()
        .and_then(|item| item.get("runbook_risk_owner_route"))
        .and_then(Value::as_object)
    else {
        return Ok(fallback_assignee.map(|owner| RunbookRiskAlertOwnerRouteItem {
            owner,
            owner_key: None,
            owner_label: None,
            source: "ticket_assignee".to_string(),
            reason: "Existing ticket assignee retained.".to_string(),
        }));
    };

    let owner = match route.get("owner").and_then(Value::as_str) {
        Some(value) => required_trimmed("runbook_risk_owner_route.owner", value.to_string(), 128)?,
        None => fallback_assignee.clone().ok_or_else(|| {
            AppError::Validation(
                "runbook_risk_owner_route.owner is required when ticket assignee is empty"
                    .to_string(),
            )
        })?,
    };
    let source = required_trimmed(
        "runbook_risk_owner_route.source",
        route.get("source")
            .and_then(Value::as_str)
            .unwrap_or("ticket_assignee")
            .to_string(),
        64,
    )?;
    let reason = required_trimmed(
        "runbook_risk_owner_route.reason",
        route.get("reason")
            .and_then(Value::as_str)
            .unwrap_or("Owner route inferred from ticket state.")
            .to_string(),
        255,
    )?;

    Ok(Some(RunbookRiskAlertOwnerRouteItem {
        owner,
        owner_key: route
            .get("owner_key")
            .and_then(Value::as_str)
            .map(|value| value.to_string()),
        owner_label: route
            .get("owner_label")
            .and_then(Value::as_str)
            .map(|value| value.to_string()),
        source,
        reason,
    }))
}

async fn resolve_runbook_risk_alert_owner_route(
    state: &AppState,
    alert: &RunbookRiskAlertItem,
    execution_mode: Option<&str>,
    ticket_priority: &str,
    existing_assignee: Option<&str>,
) -> AppResult<RunbookRiskAlertOwnerRouteItem> {
    if let Some(owner) = existing_assignee {
        return Ok(RunbookRiskAlertOwnerRouteItem {
            owner: required_trimmed("ticket_assignee", owner.to_string(), 128)?,
            owner_key: None,
            owner_label: None,
            source: "ticket_assignee".to_string(),
            reason: "Existing ticket assignee retained.".to_string(),
        });
    }

    if let Some(configured) = resolve_runbook_risk_alert_owner_route_from_config(
        state,
        alert.template_key.as_str(),
        execution_mode,
        alert.severity.as_str(),
    )
    .await?
    {
        return Ok(configured);
    }

    if ticket_priority == TICKET_PRIORITY_CRITICAL {
        let policy = load_default_ticket_escalation_owner(&state.db).await?;
        return Ok(RunbookRiskAlertOwnerRouteItem {
            owner: policy,
            owner_key: None,
            owner_label: None,
            source: "escalation_policy".to_string(),
            reason: "Critical runbook-risk tickets route to the default escalation owner."
                .to_string(),
        });
    }

    let (owner, reason) = match alert.template_key.as_str() {
        "service-restart-safe" => (
            "service-owner",
            "Service restart follow-up routes to the service owner acknowledged in preflight.",
        ),
        "dependency-check" => (
            "dependency-owner",
            "Dependency probe failures route to the dependency owner for follow-up.",
        ),
        "backup-verify" => (
            "continuity-owner",
            "Backup verification gaps route to the continuity owner for restore follow-up.",
        ),
        "maintenance-closeout" => (
            "change-owner",
            "Maintenance closeout gaps route to the change owner for signoff and handover.",
        ),
        _ => {
            let policy = load_default_ticket_escalation_owner(&state.db).await?;
            return Ok(RunbookRiskAlertOwnerRouteItem {
                owner: policy,
                owner_key: None,
                owner_label: None,
                source: "escalation_policy".to_string(),
                reason: "No template-specific route exists, so the default escalation owner is used."
                    .to_string(),
            });
        }
    };

    Ok(RunbookRiskAlertOwnerRouteItem {
        owner: owner.to_string(),
        owner_key: None,
        owner_label: None,
        source: "template_rule".to_string(),
        reason: reason.to_string(),
    })
}

async fn resolve_runbook_risk_alert_owner_route_from_config(
    state: &AppState,
    template_key: &str,
    execution_mode: Option<&str>,
    severity: &str,
) -> AppResult<Option<RunbookRiskAlertOwnerRouteItem>> {
    let owner_directory = load_runbook_risk_owner_directory_map(&state.db).await?;
    if owner_directory.is_empty() {
        return Ok(None);
    }
    let rules = load_runbook_risk_owner_routing_rule_rows(&state.db).await?;
    let selected = select_runbook_risk_owner_routing_rule(
        &rules,
        template_key,
        execution_mode,
        severity,
    )?;
    let Some(rule) = selected else {
        return Ok(None);
    };
    let owner_key = normalize_owner_key(rule.owner_key.clone())?;
    let Some(owner) = owner_directory.get(&owner_key) else {
        return Ok(None);
    };
    if !owner.is_enabled {
        return Ok(None);
    }
    Ok(Some(RunbookRiskAlertOwnerRouteItem {
        owner: owner.owner_ref.clone(),
        owner_key: Some(owner.owner_key.clone()),
        owner_label: Some(owner.display_name.clone()),
        source: "configured_rule".to_string(),
        reason: format!(
            "Configured routing rule matched template='{}', severity='{}', mode='{}'.",
            template_key,
            severity,
            execution_mode.unwrap_or("all")
        ),
    }))
}

fn select_runbook_risk_owner_routing_rule(
    rows: &[RunbookRiskOwnerRoutingRuleRow],
    template_key: &str,
    execution_mode: Option<&str>,
    severity: &str,
) -> AppResult<Option<RunbookRiskOwnerRoutingRuleRow>> {
    let normalized_template_key = normalize_template_key(template_key.to_string())?;
    let normalized_execution_mode = execution_mode
        .map(|value| normalize_execution_mode(value.to_string()))
        .transpose()?;
    let normalized_severity = normalize_risk_severity(severity.to_string())?;

    let mut best: Option<(i32, RunbookRiskOwnerRoutingRuleRow)> = None;
    for row in rows {
        if !row.is_enabled {
            continue;
        }
        if normalize_template_key(row.template_key.clone())? != normalized_template_key {
            continue;
        }
        let row_mode = row.execution_mode.clone().map(normalize_execution_mode).transpose()?;
        if let Some(mode) = row_mode.as_deref() {
            if Some(mode) != normalized_execution_mode.as_deref() {
                continue;
            }
        }
        let row_severity = row.severity.clone().map(normalize_risk_severity).transpose()?;
        if let Some(item_severity) = row_severity.as_deref() {
            if item_severity != normalized_severity {
                continue;
            }
        }

        let specificity = match (row_mode.is_some(), row_severity.is_some()) {
            (true, true) => 0,
            (false, true) | (true, false) => 1,
            (false, false) => 2,
        };
        match &best {
            Some((best_specificity, best_row)) => {
                if specificity < *best_specificity
                    || (specificity == *best_specificity
                        && (row.priority < best_row.priority
                            || (row.priority == best_row.priority && row.id > best_row.id)))
                {
                    best = Some((specificity, row.clone()));
                }
            }
            None => best = Some((specificity, row.clone())),
        }
    }
    Ok(best.map(|(_, row)| row))
}

async fn load_default_ticket_escalation_owner(db: &sqlx::PgPool) -> AppResult<String> {
    let owner: String = sqlx::query_scalar(
        "SELECT escalate_to_assignee
         FROM ticket_escalation_policies
         WHERE policy_key = 'default-ticket-sla'
         ORDER BY id DESC
         LIMIT 1",
    )
    .fetch_optional(db)
    .await?
    .unwrap_or_else(|| "ops-escalation".to_string());
    required_trimmed("escalate_to_assignee", owner, 128)
}

fn merge_runbook_risk_owner_route_metadata(
    metadata: Value,
    owner_route: &RunbookRiskAlertOwnerRouteItem,
) -> Value {
    let mut object = match metadata {
        Value::Object(item) => item,
        _ => JsonMap::new(),
    };
    object.insert(
        "runbook_risk_owner_route".to_string(),
        json!({
            "owner": owner_route.owner,
            "owner_key": owner_route.owner_key,
            "owner_label": owner_route.owner_label,
            "source": owner_route.source,
            "reason": owner_route.reason
        }),
    );
    Value::Object(object)
}

async fn load_runbook_risk_alert_notification_delivery_count(
    state: &AppState,
    template_key: Option<String>,
    execution_mode: Option<String>,
    days: u32,
    source_key: Option<String>,
) -> AppResult<usize> {
    let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT COUNT(*) FROM ops_runbook_risk_alert_notification_deliveries d WHERE d.window_days = ",
    );
    append_runbook_risk_alert_notification_filters(
        &mut builder,
        template_key,
        execution_mode,
        days,
        source_key,
    )?;
    let total: i64 = builder.build_query_scalar().fetch_one(&state.db).await?;
    Ok(total.max(0) as usize)
}

async fn load_runbook_risk_alert_notification_delivery_rows(
    state: &AppState,
    template_key: Option<String>,
    execution_mode: Option<String>,
    days: u32,
    source_key: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> AppResult<Vec<RunbookRiskAlertNotificationDeliveryRow>> {
    let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT d.id, d.source_key, d.template_key, d.execution_mode, d.window_days,
                d.ticket_id, d.ticket_no, d.event_type, d.dispatch_status,
                d.subscription_id, d.channel_id, d.channel_type, d.target,
                d.attempts, d.response_code, d.last_error, d.delivered_at, d.created_at
         FROM ops_runbook_risk_alert_notification_deliveries d
         WHERE d.window_days = ",
    );
    append_runbook_risk_alert_notification_filters(
        &mut builder,
        template_key,
        execution_mode,
        days,
        source_key,
    )?;
    builder.push(" ORDER BY d.created_at DESC, d.id DESC");
    if let Some(limit_value) = limit {
        builder.push(" LIMIT ").push_bind(limit_value as i64);
    }
    if let Some(offset_value) = offset {
        builder.push(" OFFSET ").push_bind(offset_value as i64);
    }

    let rows: Vec<RunbookRiskAlertNotificationDeliveryRow> =
        builder.build_query_as().fetch_all(&state.db).await?;
    Ok(rows)
}

fn append_runbook_risk_alert_notification_filters(
    builder: &mut QueryBuilder<Postgres>,
    template_key: Option<String>,
    execution_mode: Option<String>,
    days: u32,
    source_key: Option<String>,
) -> AppResult<()> {
    builder.push_bind(days as i32);

    let normalized_execution_mode = execution_mode.map(normalize_execution_mode).transpose()?;
    match normalized_execution_mode {
        Some(mode) => {
            builder.push(" AND d.execution_mode = ").push_bind(mode);
        }
        None => {
            builder.push(" AND d.execution_mode IS NULL");
        }
    }

    if let Some(template) = template_key {
        let normalized_template = normalize_template_key(template)?;
        builder
            .push(" AND d.template_key = ")
            .push_bind(normalized_template);
    }
    if let Some(source) = trim_optional(source_key, 160) {
        builder.push(" AND d.source_key = ").push_bind(source);
    }

    Ok(())
}

fn parse_runbook_risk_alert_notification_delivery_row(
    row: RunbookRiskAlertNotificationDeliveryRow,
    expected_execution_mode: Option<&str>,
    expected_days: u32,
) -> AppResult<RunbookRiskAlertNotificationDeliveryItem> {
    let execution_mode = row
        .execution_mode
        .map(normalize_execution_mode)
        .transpose()?;
    if execution_mode.as_deref() != expected_execution_mode {
        return Err(AppError::Validation(format!(
            "runbook risk alert notification delivery execution_mode mismatch for template '{}'",
            row.template_key
        )));
    }
    if row.window_days != expected_days as i32 {
        return Err(AppError::Validation(format!(
            "runbook risk alert notification delivery window_days {} does not match expected days {}",
            row.window_days, expected_days
        )));
    }
    if row.id <= 0 || row.ticket_id <= 0 {
        return Err(AppError::Validation(
            "runbook risk alert notification delivery ids must be positive integers".to_string(),
        ));
    }
    if row.attempts < 0 {
        return Err(AppError::Validation(
            "runbook risk alert notification delivery attempts must be >= 0".to_string(),
        ));
    }

    let template_key = normalize_template_key(row.template_key)?;
    let source_key = required_trimmed("source_key", row.source_key, 160)?;
    let expected_source_key = build_runbook_risk_alert_source_key(
        template_key.as_str(),
        execution_mode.as_deref(),
        expected_days,
    );
    if source_key != expected_source_key {
        return Err(AppError::Validation(format!(
            "runbook risk alert notification delivery source_key mismatch for template '{}'",
            template_key
        )));
    }

    let dispatch_status =
        normalize_runbook_risk_alert_notification_status(row.dispatch_status.as_str())?;
    let event_type = required_trimmed("event_type", row.event_type, 64)?;
    let target = required_trimmed("target", row.target, 512)?;
    let ticket_no = required_trimmed("ticket_no", row.ticket_no, 64)?;

    Ok(RunbookRiskAlertNotificationDeliveryItem {
        delivery_id: row.id,
        source_key,
        template_key,
        execution_mode,
        window_days: expected_days,
        ticket_id: row.ticket_id,
        ticket_no,
        event_type,
        dispatch_status,
        subscription_id: row.subscription_id,
        channel_id: row.channel_id,
        channel_type: row.channel_type,
        target,
        attempts: row.attempts,
        response_code: row.response_code,
        last_error: row.last_error,
        delivered_at: row.delivered_at,
        created_at: row.created_at,
    })
}

async fn dispatch_runbook_risk_alert_notifications(
    state: &AppState,
    actor: &str,
    source_key: &str,
    alert: &RunbookRiskAlertItem,
    execution_mode: Option<&str>,
    days: u32,
    ticket_link: &RunbookRiskAlertTicketLinkItem,
    created: bool,
) -> AppResult<Option<RunbookRiskAlertNotificationSummaryItem>> {
    let mut payload = JsonMap::new();
    payload.insert(
        "source_key".to_string(),
        Value::String(source_key.to_string()),
    );
    payload.insert(
        "template_key".to_string(),
        Value::String(alert.template_key.clone()),
    );
    payload.insert(
        "template_name".to_string(),
        Value::String(alert.template_name.clone()),
    );
    payload.insert(
        "execution_mode".to_string(),
        execution_mode
            .map(|value| Value::String(value.to_string()))
            .unwrap_or(Value::String("all".to_string())),
    );
    payload.insert("window_days".to_string(), Value::from(days));
    payload.insert(
        "severity".to_string(),
        Value::String(alert.severity.clone()),
    );
    payload.insert("executions".to_string(), Value::from(alert.executions as i64));
    payload.insert("failed".to_string(), Value::from(alert.failed as i64));
    payload.insert(
        "failure_rate_percent".to_string(),
        Value::from(alert.failure_rate_percent),
    );
    payload.insert(
        "top_failed_step_id".to_string(),
        alert.top_failed_step_id
            .clone()
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    payload.insert(
        "ticket_id".to_string(),
        Value::from(ticket_link.ticket_id),
    );
    payload.insert(
        "ticket_no".to_string(),
        Value::String(ticket_link.ticket_no.clone()),
    );
    payload.insert(
        "ticket_status".to_string(),
        Value::String(ticket_link.ticket_status.clone()),
    );
    payload.insert("created".to_string(), Value::Bool(created));
    payload.insert(
        "action_label".to_string(),
        Value::String(if created { "created" } else { "reused" }.to_string()),
    );
    payload.insert(
        "recommended_action".to_string(),
        Value::String(alert.recommended_action.clone()),
    );
    let message_payload = Value::Object(payload.clone());

    let targets = load_runbook_risk_notification_targets(
        &state.db,
        NOTIFICATION_EVENT_RUNBOOK_RISK_TICKET_LINKED,
    )
    .await?;
    let template = load_runbook_risk_notification_template(
        &state.db,
        NOTIFICATION_EVENT_RUNBOOK_RISK_TICKET_LINKED,
    )
    .await?;

    if targets.is_empty() {
        create_runbook_risk_alert_notification_delivery_record(
            &state.db,
            source_key,
            alert.template_key.as_str(),
            execution_mode,
            days,
            ticket_link,
            NOTIFICATION_EVENT_RUNBOOK_RISK_TICKET_LINKED,
            None,
            None,
            None,
            "-",
            0,
            None,
            Some(
                "no enabled notification subscription matched runbook risk alert event"
                    .to_string(),
            ),
            &json!({
                "event_type": NOTIFICATION_EVENT_RUNBOOK_RISK_TICKET_LINKED,
                "payload": message_payload
            }),
            None,
            actor,
            NOTIFICATION_STATUS_SKIPPED,
        )
        .await?;
        return load_runbook_risk_alert_notification_summaries(
            state,
            Some(alert.template_key.as_str()),
            execution_mode,
            days,
        )
        .await
        .map(|items| items.get(alert.template_key.as_str()).cloned());
    }

    let title_template = template
        .as_ref()
        .map(|item| item.title_template.as_str())
        .unwrap_or("Runbook risk ticket {{ticket_no}}");
    let body_template = template
        .as_ref()
        .map(|item| item.body_template.as_str())
        .unwrap_or("Runbook risk alert {{template_key}} was {{action_label}} into ticket {{ticket_no}}.");
    let title = render_notification_template(title_template, &payload);
    let body = render_notification_template(body_template, &payload);
    let message = json!({
        "event_type": NOTIFICATION_EVENT_RUNBOOK_RISK_TICKET_LINKED,
        "title": title,
        "body": body,
        "payload": message_payload,
    });

    for target in targets {
        let delivery_id = create_runbook_risk_alert_notification_delivery_record(
            &state.db,
            source_key,
            alert.template_key.as_str(),
            execution_mode,
            days,
            ticket_link,
            NOTIFICATION_EVENT_RUNBOOK_RISK_TICKET_LINKED,
            Some(target.subscription_id),
            Some(target.channel_id),
            Some(target.channel_type.as_str()),
            target.target.as_str(),
            0,
            None,
            None,
            &message,
            None,
            actor,
            NOTIFICATION_STATUS_QUEUED,
        )
        .await?;
        let outcome = send_notification_to_target(&target, &message).await;
        finalize_runbook_risk_alert_notification_delivery_record(
            &state.db,
            delivery_id,
            outcome.status,
            outcome.attempts,
            outcome.response_code,
            outcome.last_error.as_deref(),
        )
        .await?;
    }

    let items = load_runbook_risk_alert_notification_summaries(
        state,
        Some(alert.template_key.as_str()),
        execution_mode,
        days,
    )
    .await?;
    Ok(items.get(alert.template_key.as_str()).cloned())
}

async fn load_runbook_risk_notification_targets(
    db: &sqlx::PgPool,
    event_type: &str,
) -> AppResult<Vec<NotificationDispatchTarget>> {
    let targets: Vec<NotificationDispatchTarget> = sqlx::query_as(
        "SELECT
            s.id AS subscription_id,
            c.id AS channel_id,
            c.channel_type,
            c.target,
            c.config
         FROM discovery_notification_subscriptions s
         INNER JOIN discovery_notification_channels c ON c.id = s.channel_id
         WHERE s.is_enabled = TRUE
           AND c.is_enabled = TRUE
           AND s.event_type = $1
         ORDER BY s.id ASC",
    )
    .bind(event_type)
    .fetch_all(db)
    .await?;

    Ok(targets)
}

async fn load_runbook_risk_notification_template(
    db: &sqlx::PgPool,
    event_type: &str,
) -> AppResult<Option<NotificationTemplateRecord>> {
    let item: Option<NotificationTemplateRecord> = sqlx::query_as(
        "SELECT title_template, body_template
         FROM discovery_notification_templates
         WHERE event_type = $1
           AND is_enabled = TRUE
         ORDER BY id DESC
         LIMIT 1",
    )
    .bind(event_type)
    .fetch_optional(db)
    .await?;

    Ok(item)
}

async fn create_runbook_risk_alert_notification_delivery_record(
    db: &sqlx::PgPool,
    source_key: &str,
    template_key: &str,
    execution_mode: Option<&str>,
    days: u32,
    ticket_link: &RunbookRiskAlertTicketLinkItem,
    event_type: &str,
    subscription_id: Option<i64>,
    channel_id: Option<i64>,
    channel_type: Option<&str>,
    target: &str,
    attempts: i32,
    response_code: Option<i32>,
    last_error: Option<String>,
    payload: &Value,
    delivered_at: Option<DateTime<Utc>>,
    actor: &str,
    dispatch_status: &str,
) -> AppResult<i64> {
    let delivery_id: i64 = sqlx::query_scalar(
        "INSERT INTO ops_runbook_risk_alert_notification_deliveries (
            source_key, template_key, execution_mode, window_days, ticket_id, ticket_no,
            event_type, dispatch_status, subscription_id, channel_id, channel_type,
            target, attempts, response_code, last_error, payload, delivered_at, created_by
         )
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
         RETURNING id",
    )
    .bind(source_key)
    .bind(template_key)
    .bind(execution_mode)
    .bind(days as i32)
    .bind(ticket_link.ticket_id)
    .bind(ticket_link.ticket_no.as_str())
    .bind(event_type)
    .bind(dispatch_status)
    .bind(subscription_id)
    .bind(channel_id)
    .bind(channel_type)
    .bind(target)
    .bind(attempts)
    .bind(response_code)
    .bind(last_error)
    .bind(payload)
    .bind(delivered_at)
    .bind(actor)
    .fetch_one(db)
    .await?;

    Ok(delivery_id)
}

async fn finalize_runbook_risk_alert_notification_delivery_record(
    db: &sqlx::PgPool,
    delivery_id: i64,
    dispatch_status: &str,
    attempts: i32,
    response_code: Option<i32>,
    last_error: Option<&str>,
) -> AppResult<()> {
    let delivered_at = if dispatch_status == NOTIFICATION_STATUS_DELIVERED {
        Some(Utc::now())
    } else {
        None
    };

    sqlx::query(
        "UPDATE ops_runbook_risk_alert_notification_deliveries
         SET dispatch_status = $2,
             attempts = $3,
             response_code = $4,
             last_error = $5,
             delivered_at = $6,
             updated_at = NOW()
         WHERE id = $1",
    )
    .bind(delivery_id)
    .bind(dispatch_status)
    .bind(attempts)
    .bind(response_code)
    .bind(last_error)
    .bind(delivered_at)
    .execute(db)
    .await?;

    Ok(())
}

struct DeliveryOutcome {
    status: &'static str,
    attempts: i32,
    response_code: Option<i32>,
    last_error: Option<String>,
}

async fn send_notification_to_target(
    target: &NotificationDispatchTarget,
    message: &Value,
) -> DeliveryOutcome {
    match target.channel_type.as_str() {
        "webhook" => send_webhook_with_retry(target, message).await,
        "email" => DeliveryOutcome {
            status: NOTIFICATION_STATUS_DELIVERED,
            attempts: 1,
            response_code: Some(202),
            last_error: None,
        },
        _ => DeliveryOutcome {
            status: NOTIFICATION_STATUS_FAILED,
            attempts: 1,
            response_code: None,
            last_error: Some(format!(
                "unsupported channel_type '{}'",
                target.channel_type
            )),
        },
    }
}

async fn send_webhook_with_retry(
    target: &NotificationDispatchTarget,
    message: &Value,
) -> DeliveryOutcome {
    let max_attempts = target
        .config
        .get("max_attempts")
        .and_then(Value::as_i64)
        .map(|value| value.clamp(1, 5) as i32)
        .unwrap_or(3);
    let base_delay_ms = target
        .config
        .get("base_delay_ms")
        .and_then(Value::as_u64)
        .map(|value| value.clamp(50, 10_000))
        .unwrap_or(200);

    let client = match Client::builder().timeout(StdDuration::from_secs(10)).build() {
        Ok(client) => client,
        Err(err) => {
            return DeliveryOutcome {
                status: NOTIFICATION_STATUS_FAILED,
                attempts: 1,
                response_code: None,
                last_error: Some(err.to_string()),
            };
        }
    };

    let mut attempts: i32 = 0;
    let mut last_error: Option<String> = None;
    let mut last_code: Option<i32> = None;

    while attempts < max_attempts {
        attempts += 1;
        let response = client.post(&target.target).json(message).send().await;
        match response {
            Ok(response) => {
                let code = response.status().as_u16() as i32;
                last_code = Some(code);
                if response.status().is_success() {
                    return DeliveryOutcome {
                        status: NOTIFICATION_STATUS_DELIVERED,
                        attempts,
                        response_code: Some(code),
                        last_error: None,
                    };
                }
                let body = response.text().await.unwrap_or_default();
                last_error = Some(format!("webhook responded with status {code}: {body}"));
            }
            Err(err) => {
                last_error = Some(err.to_string());
            }
        }

        if attempts < max_attempts {
            let factor = 1_u64 << (attempts as u32 - 1);
            sleep(StdDuration::from_millis(
                base_delay_ms.saturating_mul(factor),
            ))
            .await;
        }
    }

    DeliveryOutcome {
        status: NOTIFICATION_STATUS_FAILED,
        attempts,
        response_code: last_code,
        last_error,
    }
}

fn render_notification_template(template: &str, payload: &JsonMap<String, Value>) -> String {
    let mut rendered = template.to_string();
    for (key, value) in payload {
        let replacement = match value {
            Value::String(item) => item.clone(),
            Value::Number(item) => item.to_string(),
            Value::Bool(item) => item.to_string(),
            _ => continue,
        };
        rendered = rendered.replace(&format!("{{{{{key}}}}}"), &replacement);
    }
    rendered
}

fn normalize_runbook_risk_alert_notification_status(value: &str) -> AppResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        NOTIFICATION_STATUS_QUEUED => Ok(NOTIFICATION_STATUS_QUEUED.to_string()),
        NOTIFICATION_STATUS_DELIVERED => Ok(NOTIFICATION_STATUS_DELIVERED.to_string()),
        NOTIFICATION_STATUS_FAILED => Ok(NOTIFICATION_STATUS_FAILED.to_string()),
        NOTIFICATION_STATUS_SKIPPED => Ok(NOTIFICATION_STATUS_SKIPPED.to_string()),
        _ => Err(AppError::Validation(format!(
            "notification dispatch_status must be one of: {}, {}, {}, {}",
            NOTIFICATION_STATUS_QUEUED,
            NOTIFICATION_STATUS_DELIVERED,
            NOTIFICATION_STATUS_FAILED,
            NOTIFICATION_STATUS_SKIPPED
        ))),
    }
}

async fn create_runbook_risk_alert_ticket(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateRunbookRiskAlertTicketRequest>,
) -> AppResult<Json<CreateRunbookRiskAlertTicketResponse>> {
    let actor = resolve_auth_user(&state, &headers).await?;
    let template_key = normalize_template_key(payload.template_key)?;
    let execution_mode = payload
        .execution_mode
        .map(normalize_execution_mode)
        .transpose()?;
    let days = normalize_analytics_days(payload.days);
    let note = trim_optional(payload.note, MAX_NOTE_LEN);

    let policy = parse_runbook_analytics_policy_row(load_or_seed_analytics_policy(&state).await?)?;
    let aggregate = load_runbook_risk_template_aggregate(
        &state,
        template_key.as_str(),
        execution_mode.as_deref(),
        days,
    )
    .await?;
    let mut alert = build_runbook_risk_alert_item(
        template_key.clone(),
        aggregate,
        &policy,
        None,
        None,
    )
    .ok_or_else(|| {
        AppError::Validation(format!(
            "template '{}' has no active risk alert in current policy window",
            template_key
        ))
    })?;

    let source_key =
        build_runbook_risk_alert_source_key(template_key.as_str(), execution_mode.as_deref(), days);

    let mut tx = state.db.begin().await?;
    let existing_open_link: Option<RunbookRiskAlertTicketLinkRow> = sqlx::query_as(
        "SELECT l.id, l.template_key, l.execution_mode, l.window_days, l.source_key, l.status,
                l.ticket_id, t.ticket_no, t.status AS ticket_status, t.priority AS ticket_priority,
                t.assignee AS ticket_assignee, t.metadata AS ticket_metadata, l.updated_at
         FROM ops_runbook_risk_alert_ticket_links l
         JOIN tickets t ON t.id = l.ticket_id
         WHERE l.source_key = $1
           AND l.status IN ('open', 'in_progress')
           AND t.status IN ('open', 'in_progress')
         ORDER BY l.updated_at DESC, l.id DESC
         LIMIT 1",
    )
    .bind(source_key.as_str())
    .fetch_optional(&mut *tx)
    .await?;

    let (created, link_row) = if let Some(mut existing) = existing_open_link {
        if existing.ticket_assignee.is_none() {
            let owner_route = resolve_runbook_risk_alert_owner_route(
                &state,
                &alert,
                execution_mode.as_deref(),
                existing.ticket_priority.as_str(),
                None,
            )
            .await?;
            existing.ticket_metadata =
                merge_runbook_risk_owner_route_metadata(existing.ticket_metadata, &owner_route);

            sqlx::query(
                "UPDATE tickets
                 SET assignee = $2,
                     metadata = $3,
                     updated_at = NOW()
                 WHERE id = $1",
            )
            .bind(existing.ticket_id)
            .bind(owner_route.owner.as_str())
            .bind(&existing.ticket_metadata)
            .execute(&mut *tx)
            .await?;

            existing.ticket_assignee = Some(owner_route.owner);
        }
        (false, existing)
    } else {
        let ticket_id: i64 = sqlx::query_scalar("SELECT nextval('tickets_id_seq')")
            .fetch_one(&mut *tx)
            .await?;
        let ticket_no = format!("TKT-{}-{ticket_id:06}", Utc::now().format("%Y%m%d"));
        let title = format!(
            "Runbook risk [{}] {}",
            alert.severity.to_ascii_uppercase(),
            alert.template_name
        );
        let title: String = title.chars().take(MAX_RISK_TICKET_TITLE_LEN).collect();
        let description = format!(
            "Auto-generated from runbook risk alert.\n\nTemplate: {} ({})\nExecution mode: {}\nWindow days: {}\nExecutions: {}\nFailed: {}\nFailure rate: {}%\nTop failed step: {}\nLatest failed execution: {}\nRecommended action: {}",
            alert.template_name,
            alert.template_key,
            execution_mode.as_deref().unwrap_or("all"),
            days,
            alert.executions,
            alert.failed,
            alert.failure_rate_percent,
            alert
                .top_failed_step_id
                .clone()
                .unwrap_or_else(|| "-".to_string()),
            alert
                .latest_failed_execution_id
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            alert.recommended_action
        );
        let description: String = description
            .chars()
            .take(MAX_RISK_TICKET_DESCRIPTION_LEN)
            .collect();
        let ticket_priority = ticket_priority_for_risk_severity(alert.severity.as_str());
        let owner_route =
            resolve_runbook_risk_alert_owner_route(
                &state,
                &alert,
                execution_mode.as_deref(),
                ticket_priority,
                None,
            )
            .await?;
        let metadata = merge_runbook_risk_owner_route_metadata(
            json!({
            "source": "runbook_risk_alert",
            "source_key": source_key,
            "template_key": alert.template_key,
            "template_name": alert.template_name,
            "execution_mode": execution_mode,
            "window_days": days,
            "severity": alert.severity,
            "executions": alert.executions,
            "failed": alert.failed,
            "failure_rate_percent": alert.failure_rate_percent,
            "top_failed_step_id": alert.top_failed_step_id,
            "latest_failed_execution_id": alert.latest_failed_execution_id,
            "latest_failed_at": alert.latest_failed_at,
            "recommended_action": alert.recommended_action
            }),
            &owner_route,
        );

        sqlx::query(
            "INSERT INTO tickets (
                id, ticket_no, title, description, status, priority, category, requester, assignee, metadata
             )
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        )
        .bind(ticket_id)
        .bind(ticket_no)
        .bind(title)
        .bind(description)
        .bind(TICKET_STATUS_OPEN)
        .bind(ticket_priority)
        .bind(RISK_TICKET_CATEGORY)
        .bind(actor.as_str())
        .bind(owner_route.owner.as_str())
        .bind(metadata)
        .execute(&mut *tx)
        .await?;

        let link_id: i64 = sqlx::query_scalar(
            "INSERT INTO ops_runbook_risk_alert_ticket_links (
                template_key, execution_mode, window_days, source_key, status,
                ticket_id, note, created_by
             )
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             ON CONFLICT (source_key) DO UPDATE
             SET template_key = EXCLUDED.template_key,
                 execution_mode = EXCLUDED.execution_mode,
                 window_days = EXCLUDED.window_days,
                 status = EXCLUDED.status,
                 ticket_id = EXCLUDED.ticket_id,
                 note = COALESCE(EXCLUDED.note, ops_runbook_risk_alert_ticket_links.note),
                 updated_at = NOW()
             RETURNING id",
        )
        .bind(template_key.as_str())
        .bind(execution_mode.as_deref())
        .bind(days as i32)
        .bind(source_key.as_str())
        .bind(TICKET_STATUS_OPEN)
        .bind(ticket_id)
        .bind(note.clone())
        .bind(actor.as_str())
        .fetch_one(&mut *tx)
        .await?;

        let row: RunbookRiskAlertTicketLinkRow = sqlx::query_as(
            "SELECT l.id, l.template_key, l.execution_mode, l.window_days, l.source_key, l.status,
                    l.ticket_id, t.ticket_no, t.status AS ticket_status, t.priority AS ticket_priority,
                    t.assignee AS ticket_assignee, t.metadata AS ticket_metadata, l.updated_at
             FROM ops_runbook_risk_alert_ticket_links l
             JOIN tickets t ON t.id = l.ticket_id
             WHERE l.id = $1",
        )
        .bind(link_id)
        .fetch_one(&mut *tx)
        .await?;
        (true, row)
    };
    tx.commit().await?;

    let (_, ticket_link) =
        parse_runbook_risk_alert_ticket_link_row(link_row, execution_mode.as_deref(), days)?;
    alert.ticket_link = Some(ticket_link.clone());
    let notification_summary = dispatch_runbook_risk_alert_notifications(
        &state,
        actor.as_str(),
        source_key.as_str(),
        &alert,
        execution_mode.as_deref(),
        days,
        &ticket_link,
        created,
    )
    .await?;
    alert.notification_summary = notification_summary.clone();

    write_audit_log_best_effort(
        &state.db,
        AuditLogWriteInput {
            actor,
            action: "ops.runbook.risk_alert.ticket.link".to_string(),
            target_type: "ops_runbook_risk_alert_ticket_link".to_string(),
            target_id: Some(ticket_link.link_id.to_string()),
            result: "success".to_string(),
            message: note,
            metadata: json!({
                "created": created,
                "source_key": source_key,
                "template_key": alert.template_key,
                "execution_mode": execution_mode,
                "window_days": days,
                "ticket_id": ticket_link.ticket_id,
                "ticket_no": ticket_link.ticket_no,
                "ticket_status": ticket_link.ticket_status
            }),
        },
    )
    .await;

    Ok(Json(CreateRunbookRiskAlertTicketResponse {
        generated_at: Utc::now(),
        created,
        source_key,
        alert,
        ticket_link,
        notification_summary,
    }))
}

async fn load_runbook_risk_template_aggregate(
    state: &AppState,
    template_key: &str,
    execution_mode: Option<&str>,
    days: u32,
) -> AppResult<RunbookRiskAggregate> {
    let generated_at = Utc::now();
    let start_at = generated_at - Duration::days(days as i64);
    let mut list_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT id, template_key, template_name, status, execution_mode, replay_source_execution_id,
                actor, params, preflight, timeline, evidence, runtime_summary, remediation_hints,
                note, created_at, updated_at
         FROM ops_runbook_template_executions e
         WHERE e.created_at >= ",
    );
    list_builder.push_bind(start_at);
    append_analytics_filters(
        &mut list_builder,
        Some(template_key.to_string()),
        execution_mode.map(|value| value.to_string()),
    );
    list_builder
        .push(" ORDER BY e.created_at DESC, e.id DESC LIMIT ")
        .push_bind(MAX_ANALYTICS_SCAN_ROWS as i64);

    let rows: Vec<RunbookTemplateExecutionRow> =
        list_builder.build_query_as().fetch_all(&state.db).await?;

    let mut aggregate = RunbookRiskAggregate::default();
    for row in rows {
        let execution = parse_execution_row(row)?;
        if aggregate.template_name.is_empty() {
            aggregate.template_name = execution.template_name.clone();
        }
        aggregate.executions += 1;
        if execution.status != "failed" {
            continue;
        }

        aggregate.failed += 1;
        if aggregate
            .latest_failed_at
            .map(|current| execution.created_at > current)
            .unwrap_or(true)
        {
            aggregate.latest_failed_at = Some(execution.created_at);
            aggregate.latest_failed_execution_id = Some(execution.id);
        }
        if let Some(step) = first_failed_timeline_step(&execution.timeline) {
            *aggregate
                .failed_step_counts
                .entry(step.step_id.clone())
                .or_insert(0) += 1;
        }
    }

    if aggregate.template_name.is_empty() {
        aggregate.template_name = resolve_template_by_key(template_key)
            .map(|item| item.name.to_string())
            .unwrap_or_else(|_| template_key.to_string());
    }

    Ok(aggregate)
}

async fn get_runbook_analytics_summary(
    State(state): State<AppState>,
    Query(query): Query<RunbookAnalyticsSummaryQuery>,
) -> AppResult<Json<RunbookAnalyticsSummaryResponse>> {
    let template_key = query.template_key.map(normalize_template_key).transpose()?;
    let execution_mode = query
        .execution_mode
        .map(normalize_execution_mode)
        .transpose()?;
    let days = normalize_analytics_days(query.days);
    let generated_at = Utc::now();
    let start_at = generated_at - Duration::days(days as i64);

    let mut count_builder: QueryBuilder<Postgres> =
        QueryBuilder::new("SELECT COUNT(*) FROM ops_runbook_template_executions e WHERE e.created_at >= ");
    count_builder.push_bind(start_at);
    append_analytics_filters(&mut count_builder, template_key.clone(), execution_mode.clone());
    let total_rows: i64 = count_builder
        .build_query_scalar()
        .fetch_one(&state.db)
        .await?;

    let mut list_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT id, template_key, template_name, status, execution_mode, replay_source_execution_id,
                actor, params, preflight, timeline, evidence, runtime_summary, remediation_hints,
                note, created_at, updated_at
         FROM ops_runbook_template_executions e
         WHERE e.created_at >= ",
    );
    list_builder.push_bind(start_at);
    append_analytics_filters(&mut list_builder, template_key.clone(), execution_mode.clone());
    list_builder
        .push(" ORDER BY e.created_at DESC, e.id DESC LIMIT ")
        .push_bind(MAX_ANALYTICS_SCAN_ROWS as i64);

    let rows: Vec<RunbookTemplateExecutionRow> =
        list_builder.build_query_as().fetch_all(&state.db).await?;

    let mut totals = RunbookAnalyticsSummaryTotals {
        executions: 0,
        succeeded: 0,
        failed: 0,
        simulate: 0,
        live: 0,
        replayed: 0,
        success_rate_percent: 0.0,
        sampled_rows: rows.len(),
        truncated: total_rows > rows.len() as i64,
    };
    let mut template_aggregate = BTreeMap::<String, RunbookTemplateAggregate>::new();
    let mut failed_steps = BTreeMap::<(String, String, String), usize>::new();

    for row in rows {
        let item = parse_execution_row(row)?;
        totals.executions += 1;

        if item.status == "succeeded" {
            totals.succeeded += 1;
        } else if item.status == "failed" {
            totals.failed += 1;
        }
        if item.execution_mode == EXECUTION_MODE_LIVE {
            totals.live += 1;
        } else {
            totals.simulate += 1;
        }
        if item.replay_source_execution_id.is_some() {
            totals.replayed += 1;
        }

        let aggregate = template_aggregate
            .entry(item.template_key.clone())
            .or_insert_with(|| RunbookTemplateAggregate {
                template_name: item.template_name.clone(),
                ..RunbookTemplateAggregate::default()
            });
        aggregate.executions += 1;
        if item.status == "succeeded" {
            aggregate.succeeded += 1;
        } else if item.status == "failed" {
            aggregate.failed += 1;
        }
        if item.execution_mode == EXECUTION_MODE_LIVE {
            aggregate.live += 1;
        } else {
            aggregate.simulate += 1;
        }
        if item.replay_source_execution_id.is_some() {
            aggregate.replayed += 1;
        }

        if let Some(step) = first_failed_timeline_step(&item.timeline) {
            let key = (
                item.template_key.clone(),
                item.template_name.clone(),
                step.step_id.clone(),
            );
            *failed_steps.entry(key).or_insert(0) += 1;
        }
    }

    totals.success_rate_percent = calculate_success_rate_percent(totals.succeeded, totals.executions);

    let mut templates = template_aggregate
        .into_iter()
        .map(|(template_key, aggregate)| RunbookAnalyticsTemplateItem {
            template_key,
            template_name: aggregate.template_name,
            executions: aggregate.executions,
            succeeded: aggregate.succeeded,
            failed: aggregate.failed,
            simulate: aggregate.simulate,
            live: aggregate.live,
            replayed: aggregate.replayed,
            success_rate_percent: calculate_success_rate_percent(
                aggregate.succeeded,
                aggregate.executions,
            ),
        })
        .collect::<Vec<_>>();
    templates.sort_by(|left, right| {
        right
            .failed
            .cmp(&left.failed)
            .then_with(|| right.executions.cmp(&left.executions))
            .then_with(|| left.template_key.cmp(&right.template_key))
    });

    let mut failed_steps = failed_steps
        .into_iter()
        .map(
            |((template_key, template_name, step_id), failures)| RunbookAnalyticsFailedStepItem {
                template_key,
                template_name,
                step_id,
                failures,
            },
        )
        .collect::<Vec<_>>();
    failed_steps.sort_by(|left, right| {
        right
            .failures
            .cmp(&left.failures)
            .then_with(|| left.template_key.cmp(&right.template_key))
            .then_with(|| left.step_id.cmp(&right.step_id))
    });
    failed_steps.truncate(MAX_FAILED_STEP_HOTSPOTS);

    Ok(Json(RunbookAnalyticsSummaryResponse {
        generated_at,
        window: RunbookAnalyticsWindow {
            start_at,
            end_at: generated_at,
            days,
        },
        filters: RunbookAnalyticsFilters {
            template_key,
            execution_mode,
        },
        totals,
        templates,
        failed_steps,
    }))
}

async fn list_runbook_failure_feed(
    State(state): State<AppState>,
    Query(query): Query<RunbookFailureFeedQuery>,
) -> AppResult<Json<RunbookFailureFeedResponse>> {
    let template_key = query.template_key.map(normalize_template_key).transpose()?;
    let execution_mode = query
        .execution_mode
        .map(normalize_execution_mode)
        .transpose()?;
    let days = normalize_analytics_days(query.days);
    let limit = query
        .limit
        .unwrap_or(DEFAULT_FAILURE_FEED_LIMIT)
        .clamp(1, MAX_FAILURE_FEED_LIMIT);
    let offset = query.offset.unwrap_or(0);
    let generated_at = Utc::now();
    let start_at = generated_at - Duration::days(days as i64);

    let mut count_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT COUNT(*) FROM ops_runbook_template_executions e WHERE e.created_at >= ",
    );
    count_builder.push_bind(start_at);
    append_analytics_filters(&mut count_builder, template_key.clone(), execution_mode.clone());
    count_builder.push(" AND e.status = ").push_bind("failed");
    let total: i64 = count_builder
        .build_query_scalar()
        .fetch_one(&state.db)
        .await?;

    let mut list_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        "SELECT id, template_key, template_name, status, execution_mode, replay_source_execution_id,
                actor, params, preflight, timeline, evidence, runtime_summary, remediation_hints,
                note, created_at, updated_at
         FROM ops_runbook_template_executions e
         WHERE e.created_at >= ",
    );
    list_builder.push_bind(start_at);
    append_analytics_filters(&mut list_builder, template_key.clone(), execution_mode.clone());
    list_builder
        .push(" AND e.status = ")
        .push_bind("failed")
        .push(" ORDER BY e.created_at DESC, e.id DESC LIMIT ")
        .push_bind(limit as i64)
        .push(" OFFSET ")
        .push_bind(offset as i64);

    let rows: Vec<RunbookTemplateExecutionRow> =
        list_builder.build_query_as().fetch_all(&state.db).await?;
    let mut items = Vec::new();
    for row in rows {
        let execution = parse_execution_row(row)?;
        let failed_step = first_failed_timeline_step(&execution.timeline);
        let remediation_hint = failed_step
            .and_then(|step| step.remediation_hint.clone())
            .or_else(|| execution.remediation_hints.first().cloned());

        items.push(RunbookFailureFeedItem {
            id: execution.id,
            template_key: execution.template_key,
            template_name: execution.template_name,
            execution_mode: execution.execution_mode,
            replay_source_execution_id: execution.replay_source_execution_id,
            actor: execution.actor,
            failed_step_id: failed_step.map(|step| step.step_id.clone()),
            failed_output: failed_step.map(|step| step.output.clone()),
            remediation_hint,
            evidence_summary: execution.evidence.summary,
            runtime_summary: execution.runtime_summary,
            created_at: execution.created_at,
        });
    }

    Ok(Json(RunbookFailureFeedResponse {
        generated_at,
        window: RunbookAnalyticsWindow {
            start_at,
            end_at: generated_at,
            days,
        },
        filters: RunbookAnalyticsFilters {
            template_key,
            execution_mode,
        },
        total,
        limit,
        offset,
        items,
    }))
}

fn append_execution_filters(
    builder: &mut QueryBuilder<Postgres>,
    template_key: Option<String>,
    status: Option<String>,
) {
    if let Some(template_key) = template_key {
        builder
            .push(" AND e.template_key = ")
            .push_bind(template_key);
    }
    if let Some(status) = status {
        builder.push(" AND e.status = ").push_bind(status);
    }
}

fn append_preset_filters(builder: &mut QueryBuilder<Postgres>, template_key: Option<String>) {
    if let Some(template_key) = template_key {
        builder
            .push(" AND p.template_key = ")
            .push_bind(template_key);
    }
}

fn append_analytics_filters(
    builder: &mut QueryBuilder<Postgres>,
    template_key: Option<String>,
    execution_mode: Option<String>,
) {
    if let Some(template_key) = template_key {
        builder
            .push(" AND e.template_key = ")
            .push_bind(template_key);
    }
    if let Some(execution_mode) = execution_mode {
        builder
            .push(" AND e.execution_mode = ")
            .push_bind(execution_mode);
    }
}

fn parse_runbook_execution_preset_row(
    row: RunbookExecutionPresetRow,
) -> AppResult<RunbookExecutionPresetItem> {
    let preflight_confirmations = parse_preflight_confirmations(row.preflight_confirmations)?;
    let execution_mode = normalize_execution_mode(row.execution_mode)?;
    if !matches!(row.params, Value::Object(_)) {
        return Err(AppError::Validation(
            "runbook preset params must be a JSON object".to_string(),
        ));
    }

    Ok(RunbookExecutionPresetItem {
        id: row.id,
        template_key: row.template_key,
        template_name: row.template_name,
        name: row.name,
        description: row.description,
        execution_mode,
        params: row.params,
        preflight_confirmations,
        updated_by: row.updated_by,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

fn parse_execution_row(
    row: RunbookTemplateExecutionRow,
) -> AppResult<RunbookTemplateExecutionItem> {
    let timeline: Vec<RunbookStepTimelineEvent> =
        serde_json::from_value(row.timeline).map_err(|err| {
            AppError::Validation(format!("runbook execution timeline data is invalid: {err}"))
        })?;
    let evidence: RunbookEvidenceRecord = serde_json::from_value(row.evidence).map_err(|err| {
        AppError::Validation(format!("runbook execution evidence data is invalid: {err}"))
    })?;
    let remediation_hints: Vec<String> =
        serde_json::from_value(row.remediation_hints).map_err(|err| {
            AppError::Validation(format!(
                "runbook execution remediation_hints data is invalid: {err}"
            ))
        })?;

    Ok(RunbookTemplateExecutionItem {
        id: row.id,
        template_key: row.template_key,
        template_name: row.template_name,
        status: row.status,
        execution_mode: row.execution_mode,
        replay_source_execution_id: row.replay_source_execution_id,
        actor: row.actor,
        params: row.params,
        preflight: row.preflight,
        timeline,
        evidence,
        runtime_summary: row.runtime_summary,
        remediation_hints,
        note: row.note,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

#[derive(Debug)]
struct RunbookExecutionOutcome {
    status: String,
    timeline: Vec<RunbookStepTimelineEvent>,
    remediation_hints: Vec<String>,
    runtime_summary: Value,
}

#[derive(Debug)]
struct LiveProbeTarget {
    host: String,
    port: u16,
    raw: String,
}

async fn load_or_seed_execution_policy(state: &AppState) -> AppResult<RunbookExecutionPolicyRow> {
    let existing: Option<RunbookExecutionPolicyRow> = sqlx::query_as(
        "SELECT policy_key, mode, live_templates, max_live_step_timeout_seconds,
                allow_simulate_failure, note, updated_by, created_at, updated_at
         FROM ops_runbook_execution_policies
         WHERE policy_key = $1",
    )
    .bind(DEFAULT_EXECUTION_POLICY_KEY)
    .fetch_optional(&state.db)
    .await?;

    if let Some(row) = existing {
        return Ok(row);
    }

    let row: RunbookExecutionPolicyRow = sqlx::query_as(
        "INSERT INTO ops_runbook_execution_policies (
            policy_key, mode, live_templates, max_live_step_timeout_seconds,
            allow_simulate_failure, note, updated_by
         )
         VALUES ($1, $2, '[]'::jsonb, $3, $4, $5, $6)
         RETURNING policy_key, mode, live_templates, max_live_step_timeout_seconds,
                   allow_simulate_failure, note, updated_by, created_at, updated_at",
    )
    .bind(DEFAULT_EXECUTION_POLICY_KEY)
    .bind(EXECUTION_POLICY_MODE_SIMULATE_ONLY)
    .bind(DEFAULT_LIVE_STEP_TIMEOUT_SECONDS)
    .bind(true)
    .bind(Some("seeded default policy".to_string()))
    .bind("system")
    .fetch_one(&state.db)
    .await?;

    Ok(row)
}

fn parse_execution_policy_row(
    row: RunbookExecutionPolicyRow,
) -> AppResult<RunbookExecutionPolicyItem> {
    let mode = normalize_execution_policy_mode(row.mode)?;
    let live_templates: Vec<String> =
        serde_json::from_value(row.live_templates).map_err(|err| {
            AppError::Validation(format!(
                "runbook execution policy live_templates is invalid: {err}"
            ))
        })?;
    let live_templates = normalize_live_template_keys(live_templates)?;
    if !(MIN_LIVE_STEP_TIMEOUT_SECONDS..=MAX_LIVE_STEP_TIMEOUT_SECONDS)
        .contains(&row.max_live_step_timeout_seconds)
    {
        return Err(AppError::Validation(format!(
            "runbook execution policy timeout must be between {} and {} seconds",
            MIN_LIVE_STEP_TIMEOUT_SECONDS, MAX_LIVE_STEP_TIMEOUT_SECONDS
        )));
    }

    Ok(RunbookExecutionPolicyItem {
        policy_key: row.policy_key,
        mode,
        live_templates,
        max_live_step_timeout_seconds: row.max_live_step_timeout_seconds,
        allow_simulate_failure: row.allow_simulate_failure,
        note: row.note,
        updated_by: row.updated_by,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

async fn load_or_seed_analytics_policy(state: &AppState) -> AppResult<RunbookAnalyticsPolicyRow> {
    let existing: Option<RunbookAnalyticsPolicyRow> = sqlx::query_as(
        "SELECT policy_key, failure_rate_threshold_percent, minimum_sample_size,
                note, updated_by, created_at, updated_at
         FROM ops_runbook_analytics_policies
         WHERE policy_key = $1",
    )
    .bind(DEFAULT_ANALYTICS_POLICY_KEY)
    .fetch_optional(&state.db)
    .await?;

    if let Some(row) = existing {
        return Ok(row);
    }

    let row: RunbookAnalyticsPolicyRow = sqlx::query_as(
        "INSERT INTO ops_runbook_analytics_policies (
            policy_key,
            failure_rate_threshold_percent,
            minimum_sample_size,
            note,
            updated_by
         )
         VALUES ($1, $2, $3, $4, $5)
         RETURNING policy_key, failure_rate_threshold_percent, minimum_sample_size,
                   note, updated_by, created_at, updated_at",
    )
    .bind(DEFAULT_ANALYTICS_POLICY_KEY)
    .bind(DEFAULT_ANALYTICS_FAILURE_RATE_THRESHOLD_PERCENT)
    .bind(DEFAULT_ANALYTICS_MINIMUM_SAMPLE_SIZE)
    .bind(Some("seeded default analytics policy".to_string()))
    .bind("system")
    .fetch_one(&state.db)
    .await?;

    Ok(row)
}

fn parse_runbook_analytics_policy_row(
    row: RunbookAnalyticsPolicyRow,
) -> AppResult<RunbookAnalyticsPolicyItem> {
    if !(MIN_ANALYTICS_FAILURE_RATE_THRESHOLD_PERCENT..=MAX_ANALYTICS_FAILURE_RATE_THRESHOLD_PERCENT)
        .contains(&row.failure_rate_threshold_percent)
    {
        return Err(AppError::Validation(format!(
            "runbook analytics policy failure_rate_threshold_percent must be between {} and {}",
            MIN_ANALYTICS_FAILURE_RATE_THRESHOLD_PERCENT,
            MAX_ANALYTICS_FAILURE_RATE_THRESHOLD_PERCENT
        )));
    }
    if !(MIN_ANALYTICS_MINIMUM_SAMPLE_SIZE..=MAX_ANALYTICS_MINIMUM_SAMPLE_SIZE)
        .contains(&row.minimum_sample_size)
    {
        return Err(AppError::Validation(format!(
            "runbook analytics policy minimum_sample_size must be between {} and {}",
            MIN_ANALYTICS_MINIMUM_SAMPLE_SIZE, MAX_ANALYTICS_MINIMUM_SAMPLE_SIZE
        )));
    }

    Ok(RunbookAnalyticsPolicyItem {
        policy_key: row.policy_key,
        failure_rate_threshold_percent: row.failure_rate_threshold_percent,
        minimum_sample_size: row.minimum_sample_size,
        note: row.note,
        updated_by: row.updated_by,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

fn risk_severity_rank(severity: &str) -> u8 {
    match severity {
        "critical" => 2,
        "warning" => 1,
        _ => 0,
    }
}

fn normalize_execution_policy_mode(value: String) -> AppResult<String> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        EXECUTION_POLICY_MODE_SIMULATE_ONLY | EXECUTION_POLICY_MODE_HYBRID_LIVE => Ok(normalized),
        _ => Err(AppError::Validation(
            "mode must be one of: simulate_only, hybrid_live".to_string(),
        )),
    }
}

fn normalize_execution_mode(value: String) -> AppResult<String> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        EXECUTION_MODE_SIMULATE | EXECUTION_MODE_LIVE => Ok(normalized),
        _ => Err(AppError::Validation(
            "execution_mode must be one of: simulate, live".to_string(),
        )),
    }
}

fn normalize_preset_name(value: String) -> AppResult<String> {
    required_trimmed("preset name", value, MAX_PRESET_NAME_LEN)
}

fn enforce_template_supports_execution_mode(
    template: &RunbookTemplateDefinition,
    execution_mode: &str,
) -> AppResult<()> {
    if execution_mode == EXECUTION_MODE_LIVE && !template.supports_live {
        return Err(AppError::Validation(format!(
            "template '{}' does not support live execution",
            template.key
        )));
    }
    Ok(())
}

fn normalize_live_template_keys(raw: Vec<String>) -> AppResult<Vec<String>> {
    if raw.len() > MAX_LIVE_TEMPLATE_COUNT {
        return Err(AppError::Validation(format!(
            "live_templates count must be <= {}",
            MAX_LIVE_TEMPLATE_COUNT
        )));
    }

    let mut dedup = BTreeSet::new();
    for key in raw {
        dedup.insert(normalize_template_key(key)?);
    }

    let mut normalized = dedup.into_iter().collect::<Vec<_>>();
    normalized.sort();

    for key in &normalized {
        let template = resolve_template_by_key(key)?;
        if !template.supports_live {
            return Err(AppError::Validation(format!(
                "template '{}' does not support live execution",
                key
            )));
        }
    }

    Ok(normalized)
}

fn enforce_execution_mode_policy(
    policy: &RunbookExecutionPolicyItem,
    template: &RunbookTemplateDefinition,
    execution_mode: &str,
) -> AppResult<()> {
    if execution_mode == EXECUTION_MODE_SIMULATE {
        return Ok(());
    }

    if policy.mode != EXECUTION_POLICY_MODE_HYBRID_LIVE {
        return Err(AppError::Validation(
            "live execution is disabled by execution policy".to_string(),
        ));
    }
    if !template.supports_live {
        return Err(AppError::Validation(format!(
            "template '{}' does not support live execution",
            template.key
        )));
    }
    if !policy
        .live_templates
        .iter()
        .any(|item| item == template.key)
    {
        return Err(AppError::Validation(format!(
            "template '{}' is not allowlisted for live execution",
            template.key
        )));
    }

    Ok(())
}

fn runbook_template_execution_modes(template: &RunbookTemplateDefinition) -> Vec<String> {
    if template.supports_live {
        vec![
            EXECUTION_MODE_SIMULATE.to_string(),
            EXECUTION_MODE_LIVE.to_string(),
        ]
    } else {
        vec![EXECUTION_MODE_SIMULATE.to_string()]
    }
}

fn execute_simulated_template(
    template: &RunbookTemplateDefinition,
    normalized_params: &JsonMap<String, Value>,
) -> RunbookExecutionOutcome {
    let now = Utc::now();
    let mut timeline = Vec::new();
    let mut remediation_hints = Vec::new();
    let mut final_status = "succeeded".to_string();

    for (idx, step) in template.steps.iter().enumerate() {
        let started_at = now + Duration::seconds(idx as i64);
        let finished_at = started_at + Duration::seconds(1);
        let failure_reason =
            evaluate_step_failure(template.key, step.step_id, normalized_params, idx);

        if let Some(reason) = failure_reason {
            final_status = "failed".to_string();
            remediation_hints.push(step.failure_hint.to_string());
            timeline.push(RunbookStepTimelineEvent {
                step_id: step.step_id.to_string(),
                name: step.name.to_string(),
                detail: step.detail.to_string(),
                status: "failed".to_string(),
                started_at,
                finished_at,
                output: reason,
                remediation_hint: Some(step.failure_hint.to_string()),
            });
            break;
        }

        timeline.push(RunbookStepTimelineEvent {
            step_id: step.step_id.to_string(),
            name: step.name.to_string(),
            detail: step.detail.to_string(),
            status: "succeeded".to_string(),
            started_at,
            finished_at,
            output: format!("step '{}' completed", step.name),
            remediation_hint: None,
        });
    }

    if final_status == "failed" && remediation_hints.is_empty() {
        remediation_hints.push(
            "Review failed step output, verify prerequisite checks, and rerun with guarded scope."
                .to_string(),
        );
    }

    let failed_step_id = timeline
        .iter()
        .find(|item| item.status == "failed")
        .map(|item| item.step_id.clone());
    let runtime_summary = json!({
        "mode": EXECUTION_MODE_SIMULATE,
        "total_steps": template.steps.len(),
        "executed_steps": timeline.len(),
        "failed_step_id": failed_step_id,
        "duration_ms": (timeline.len() as i64) * 1000
    });

    RunbookExecutionOutcome {
        status: final_status,
        timeline,
        remediation_hints,
        runtime_summary,
    }
}

async fn execute_live_template(
    template: &RunbookTemplateDefinition,
    normalized_params: &JsonMap<String, Value>,
    max_live_step_timeout_seconds: i32,
) -> AppResult<RunbookExecutionOutcome> {
    if template.key != "dependency-check" {
        return Err(AppError::Validation(format!(
            "template '{}' live execution adapter is not implemented",
            template.key
        )));
    }

    let dependency_target =
        string_param(normalized_params, "dependency_target").ok_or_else(|| {
            AppError::Validation("parameter 'dependency_target' is required".to_string())
        })?;
    let probe_target = parse_dependency_target(dependency_target.as_str())?;
    let configured_timeout_seconds = number_param(normalized_params, "probe_timeout_seconds")
        .unwrap_or(max_live_step_timeout_seconds as i64)
        .clamp(
            MIN_LIVE_STEP_TIMEOUT_SECONDS as i64,
            max_live_step_timeout_seconds as i64,
        ) as u64;

    let mut timeline = Vec::new();
    let mut remediation_hints = Vec::new();
    let mut status = "succeeded".to_string();
    let start_instant = Instant::now();
    let mut probe_latency_ms: Option<i64> = None;

    let validation_started_at = Utc::now();
    let validation_finished_at = validation_started_at + Duration::milliseconds(50);
    timeline.push(RunbookStepTimelineEvent {
        step_id: "scope_validation".to_string(),
        name: "Validate dependency scope".to_string(),
        detail: "Confirm dependency target syntax and authorized probing scope.".to_string(),
        status: "succeeded".to_string(),
        started_at: validation_started_at,
        finished_at: validation_finished_at,
        output: format!(
            "resolved target {}:{}",
            probe_target.host, probe_target.port
        ),
        remediation_hint: None,
    });

    let probe_started_at = Utc::now();
    let probe_clock = Instant::now();
    let probe_result = timeout(
        StdDuration::from_secs(configured_timeout_seconds),
        TcpStream::connect((probe_target.host.as_str(), probe_target.port)),
    )
    .await;
    let probe_finished_at = Utc::now();

    match probe_result {
        Ok(Ok(stream)) => {
            let elapsed_ms = probe_clock.elapsed().as_millis() as i64;
            probe_latency_ms = Some(elapsed_ms);
            drop(stream);
            timeline.push(RunbookStepTimelineEvent {
                step_id: "reachability_probe".to_string(),
                name: "Run dependency probe".to_string(),
                detail: "Execute probe and collect response timing.".to_string(),
                status: "succeeded".to_string(),
                started_at: probe_started_at,
                finished_at: probe_finished_at,
                output: format!(
                    "tcp probe succeeded target={}:{} latency_ms={}",
                    probe_target.host, probe_target.port, elapsed_ms
                ),
                remediation_hint: None,
            });
        }
        Ok(Err(err)) => {
            status = "failed".to_string();
            remediation_hints.push(
                "Check network ACL/DNS path and retry with dependency owner support.".to_string(),
            );
            timeline.push(RunbookStepTimelineEvent {
                step_id: "reachability_probe".to_string(),
                name: "Run dependency probe".to_string(),
                detail: "Execute probe and collect response timing.".to_string(),
                status: "failed".to_string(),
                started_at: probe_started_at,
                finished_at: probe_finished_at,
                output: format!(
                    "tcp probe failed target={}:{} error={}",
                    probe_target.host, probe_target.port, err
                ),
                remediation_hint: Some(
                    "Check network ACL/DNS path and retry with dependency owner support."
                        .to_string(),
                ),
            });
        }
        Err(_) => {
            status = "failed".to_string();
            remediation_hints.push(
                "Dependency probe timed out; validate firewall path and endpoint health."
                    .to_string(),
            );
            timeline.push(RunbookStepTimelineEvent {
                step_id: "reachability_probe".to_string(),
                name: "Run dependency probe".to_string(),
                detail: "Execute probe and collect response timing.".to_string(),
                status: "failed".to_string(),
                started_at: probe_started_at,
                finished_at: probe_finished_at,
                output: format!(
                    "tcp probe timeout target={}:{} timeout_seconds={}",
                    probe_target.host, probe_target.port, configured_timeout_seconds
                ),
                remediation_hint: Some(
                    "Dependency probe timed out; validate firewall path and endpoint health."
                        .to_string(),
                ),
            });
        }
    }

    if status == "succeeded" {
        let summary_started_at = Utc::now();
        let summary_finished_at = summary_started_at + Duration::milliseconds(25);
        timeline.push(RunbookStepTimelineEvent {
            step_id: "readiness_summary".to_string(),
            name: "Summarize dependency readiness".to_string(),
            detail: "Publish readiness result with latency and error context.".to_string(),
            status: "succeeded".to_string(),
            started_at: summary_started_at,
            finished_at: summary_finished_at,
            output: format!(
                "dependency target {}: {}",
                probe_target.raw,
                probe_latency_ms
                    .map(|value| format!("reachable latency_ms={value}"))
                    .unwrap_or_else(|| "reachable".to_string())
            ),
            remediation_hint: None,
        });
    }

    if status == "failed" && remediation_hints.is_empty() {
        remediation_hints.push(
            "Review failed live probe output and rerun after dependency path remediation."
                .to_string(),
        );
    }

    let failed_step_id = timeline
        .iter()
        .find(|item| item.status == "failed")
        .map(|item| item.step_id.clone());
    let runtime_summary = json!({
        "mode": EXECUTION_MODE_LIVE,
        "total_steps": template.steps.len(),
        "executed_steps": timeline.len(),
        "failed_step_id": failed_step_id,
        "duration_ms": start_instant.elapsed().as_millis() as i64,
        "probe_target": format!("{}:{}", probe_target.host, probe_target.port),
        "probe_latency_ms": probe_latency_ms,
        "probe_timeout_seconds": configured_timeout_seconds
    });

    Ok(RunbookExecutionOutcome {
        status,
        timeline,
        remediation_hints,
        runtime_summary,
    })
}

fn parse_dependency_target(raw: &str) -> AppResult<LiveProbeTarget> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(
            "dependency_target cannot be empty".to_string(),
        ));
    }

    if let Some(rest) = trimmed.strip_prefix("http://") {
        return parse_host_port_target(rest, 80, trimmed);
    }
    if let Some(rest) = trimmed.strip_prefix("https://") {
        return parse_host_port_target(rest, 443, trimmed);
    }

    parse_host_port_target(trimmed, 0, trimmed)
}

fn parse_params_object(value: Value) -> AppResult<JsonMap<String, Value>> {
    let Value::Object(params) = value else {
        return Err(AppError::Validation(
            "runbook preset params must be a JSON object".to_string(),
        ));
    };
    Ok(params)
}

fn parse_preflight_confirmations(value: Value) -> AppResult<Vec<String>> {
    let preflight_confirmations: Vec<String> = serde_json::from_value(value).map_err(|err| {
        AppError::Validation(format!(
            "runbook preset preflight_confirmations is invalid: {err}"
        ))
    })?;
    Ok(preflight_confirmations)
}

fn parse_preflight_snapshot_confirmed(value: Value) -> AppResult<Vec<String>> {
    let Value::Object(snapshot) = value else {
        return Err(AppError::Validation(
            "runbook execution preflight snapshot must be a JSON object".to_string(),
        ));
    };
    let confirmed = snapshot.get("confirmed").cloned().ok_or_else(|| {
        AppError::Validation(
            "runbook execution preflight snapshot missing confirmed field".to_string(),
        )
    })?;
    let confirmed: Vec<String> = serde_json::from_value(confirmed).map_err(|err| {
        AppError::Validation(format!(
            "runbook execution preflight snapshot confirmed field is invalid: {err}"
        ))
    })?;
    Ok(confirmed)
}

fn parse_host_port_target(
    authority_or_host: &str,
    default_port: u16,
    raw: &str,
) -> AppResult<LiveProbeTarget> {
    let authority = authority_or_host.split('/').next().unwrap_or("").trim();
    if authority.is_empty() {
        return Err(AppError::Validation(
            "dependency_target host is required".to_string(),
        ));
    }

    let (host, port) = if let Some((host, port)) = authority.rsplit_once(':') {
        let port = port.parse::<u16>().map_err(|_| {
            AppError::Validation("dependency_target port must be a valid integer".to_string())
        })?;
        (host.trim().to_string(), port)
    } else if default_port > 0 {
        (authority.to_string(), default_port)
    } else {
        return Err(AppError::Validation(
            "dependency_target must include ':<port>' or use http(s):// scheme".to_string(),
        ));
    };

    if host.is_empty() {
        return Err(AppError::Validation(
            "dependency_target host is required".to_string(),
        ));
    }
    if port == 0 {
        return Err(AppError::Validation(
            "dependency_target port must be > 0".to_string(),
        ));
    }

    Ok(LiveProbeTarget {
        host,
        port,
        raw: raw.to_string(),
    })
}

fn resolve_template_by_key(key: &str) -> AppResult<RunbookTemplateDefinition> {
    let normalized = normalize_template_key(key.to_string())?;
    built_in_runbook_templates()
        .into_iter()
        .find(|template| template.key == normalized)
        .ok_or_else(|| AppError::NotFound(format!("runbook template '{}' not found", normalized)))
}

fn runbook_template_to_catalog_item(
    template: &RunbookTemplateDefinition,
) -> RunbookTemplateCatalogItem {
    RunbookTemplateCatalogItem {
        key: template.key.to_string(),
        name: template.name.to_string(),
        description: template.description.to_string(),
        category: template.category.to_string(),
        execution_modes: runbook_template_execution_modes(template),
        params: template
            .params
            .iter()
            .map(|param| RunbookTemplateParamItem {
                key: param.key.to_string(),
                label: param.label.to_string(),
                field_type: param.field_type.to_string(),
                required: param.required,
                options: param
                    .options
                    .iter()
                    .map(|item| (*item).to_string())
                    .collect(),
                min_value: param.min_value,
                max_value: param.max_value,
                default_value: param.default_value.map(|item| item.to_string()),
                placeholder: param.placeholder.map(|item| item.to_string()),
            })
            .collect(),
        preflight: template
            .preflight
            .iter()
            .map(|item| RunbookTemplateChecklistItem {
                key: item.key.to_string(),
                label: item.label.to_string(),
                detail: item.detail.to_string(),
            })
            .collect(),
        steps: template
            .steps
            .iter()
            .map(|step| RunbookTemplateStepItem {
                step_id: step.step_id.to_string(),
                name: step.name.to_string(),
                detail: step.detail.to_string(),
            })
            .collect(),
    }
}

fn built_in_runbook_templates() -> Vec<RunbookTemplateDefinition> {
    vec![
        RunbookTemplateDefinition {
            key: "service-restart-safe",
            name: "Service restart (safe)",
            description: "Controlled service restart with preflight ownership and health confirmation.",
            category: "operations",
            supports_live: false,
            params: vec![
                RunbookTemplateParamDefinition {
                    key: "asset_ref",
                    label: "Asset reference",
                    field_type: "string",
                    required: true,
                    options: vec![],
                    min_value: None,
                    max_value: None,
                    default_value: None,
                    placeholder: Some("app-server-01"),
                },
                RunbookTemplateParamDefinition {
                    key: "service_name",
                    label: "Service name",
                    field_type: "string",
                    required: true,
                    options: vec![],
                    min_value: None,
                    max_value: None,
                    default_value: Some("nginx"),
                    placeholder: Some("nginx"),
                },
                RunbookTemplateParamDefinition {
                    key: "restart_scope",
                    label: "Restart scope",
                    field_type: "enum",
                    required: true,
                    options: vec!["single-node", "rolling"],
                    min_value: None,
                    max_value: None,
                    default_value: Some("rolling"),
                    placeholder: None,
                },
                RunbookTemplateParamDefinition {
                    key: "simulate_failure_step",
                    label: "Simulate failure step",
                    field_type: "string",
                    required: false,
                    options: vec![],
                    min_value: None,
                    max_value: None,
                    default_value: None,
                    placeholder: Some("post_health_validation"),
                },
            ],
            preflight: vec![
                RunbookTemplateChecklistDefinition {
                    key: "confirm_change_window",
                    label: "Change window is confirmed",
                    detail: "Reservation/change window exists and is currently valid.",
                },
                RunbookTemplateChecklistDefinition {
                    key: "confirm_owner_ack",
                    label: "Service owner is informed",
                    detail: "Owner/oncall acknowledgment completed before restart.",
                },
                RunbookTemplateChecklistDefinition {
                    key: "confirm_rollback_ready",
                    label: "Rollback plan is ready",
                    detail: "Rollback command and health rollback threshold are ready.",
                },
            ],
            steps: vec![
                RunbookTemplateStepDefinition {
                    step_id: "target_validation",
                    name: "Validate target service",
                    detail: "Verify service mapping and restart scope guardrails.",
                    failure_hint: "Verify service name, ownership, and restart scope before retry.",
                },
                RunbookTemplateStepDefinition {
                    step_id: "restart_execution",
                    name: "Execute safe restart",
                    detail: "Run controlled restart command with bounded impact.",
                    failure_hint: "Check process permissions and restart command policy allowlist.",
                },
                RunbookTemplateStepDefinition {
                    step_id: "post_health_validation",
                    name: "Validate post-restart health",
                    detail: "Confirm service endpoint and dependency readiness.",
                    failure_hint: "Run rollback and inspect health probes or dependency saturation before reattempt.",
                },
            ],
        },
        RunbookTemplateDefinition {
            key: "dependency-check",
            name: "Dependency health check",
            description: "Dependency reachability and readiness verification with remediation context.",
            category: "diagnostics",
            supports_live: true,
            params: vec![
                RunbookTemplateParamDefinition {
                    key: "asset_ref",
                    label: "Asset reference",
                    field_type: "string",
                    required: true,
                    options: vec![],
                    min_value: None,
                    max_value: None,
                    default_value: None,
                    placeholder: Some("app-server-01"),
                },
                RunbookTemplateParamDefinition {
                    key: "dependency_target",
                    label: "Dependency target",
                    field_type: "string",
                    required: true,
                    options: vec![],
                    min_value: None,
                    max_value: None,
                    default_value: None,
                    placeholder: Some("redis://cache-a:6379"),
                },
                RunbookTemplateParamDefinition {
                    key: "probe_timeout_seconds",
                    label: "Probe timeout (seconds)",
                    field_type: "number",
                    required: true,
                    options: vec![],
                    min_value: Some(1),
                    max_value: Some(300),
                    default_value: Some("10"),
                    placeholder: Some("10"),
                },
                RunbookTemplateParamDefinition {
                    key: "simulate_failure_step",
                    label: "Simulate failure step",
                    field_type: "string",
                    required: false,
                    options: vec![],
                    min_value: None,
                    max_value: None,
                    default_value: None,
                    placeholder: Some("reachability_probe"),
                },
            ],
            preflight: vec![
                RunbookTemplateChecklistDefinition {
                    key: "confirm_probe_source",
                    label: "Probe source scope confirmed",
                    detail: "Probe source host/site is approved for diagnostics.",
                },
                RunbookTemplateChecklistDefinition {
                    key: "confirm_dependency_owner",
                    label: "Dependency owner contact confirmed",
                    detail: "Owner/escalation route exists if dependency fails.",
                },
                RunbookTemplateChecklistDefinition {
                    key: "confirm_ticket_context",
                    label: "Ticket context linked",
                    detail: "Incident/change ticket linked for audit continuity.",
                },
            ],
            steps: vec![
                RunbookTemplateStepDefinition {
                    step_id: "scope_validation",
                    name: "Validate dependency scope",
                    detail: "Confirm dependency target syntax and authorized probing scope.",
                    failure_hint: "Correct target endpoint format and ensure scope authorization.",
                },
                RunbookTemplateStepDefinition {
                    step_id: "reachability_probe",
                    name: "Run dependency probe",
                    detail: "Execute probe and collect response timing.",
                    failure_hint: "Check network ACL/DNS path and retry with dependency owner support.",
                },
                RunbookTemplateStepDefinition {
                    step_id: "readiness_summary",
                    name: "Summarize dependency readiness",
                    detail: "Publish readiness result with latency and error context.",
                    failure_hint: "Capture probe logs and add mitigation note before closure.",
                },
            ],
        },
        RunbookTemplateDefinition {
            key: "backup-verify",
            name: "Backup verify closeout",
            description: "Backup restore-verification closeout with SLA and evidence linkage.",
            category: "continuity",
            supports_live: false,
            params: vec![
                RunbookTemplateParamDefinition {
                    key: "policy_id",
                    label: "Backup policy ID",
                    field_type: "number",
                    required: true,
                    options: vec![],
                    min_value: Some(1),
                    max_value: None,
                    default_value: None,
                    placeholder: Some("1"),
                },
                RunbookTemplateParamDefinition {
                    key: "evidence_ticket",
                    label: "Evidence ticket",
                    field_type: "string",
                    required: true,
                    options: vec![],
                    min_value: None,
                    max_value: None,
                    default_value: None,
                    placeholder: Some("TKT-1234"),
                },
                RunbookTemplateParamDefinition {
                    key: "expected_restore_minutes",
                    label: "Expected restore minutes",
                    field_type: "number",
                    required: true,
                    options: vec![],
                    min_value: Some(1),
                    max_value: Some(240),
                    default_value: Some("30"),
                    placeholder: Some("30"),
                },
                RunbookTemplateParamDefinition {
                    key: "simulate_failure_step",
                    label: "Simulate failure step",
                    field_type: "string",
                    required: false,
                    options: vec![],
                    min_value: None,
                    max_value: None,
                    default_value: None,
                    placeholder: Some("restore_sla_validation"),
                },
            ],
            preflight: vec![
                RunbookTemplateChecklistDefinition {
                    key: "confirm_latest_run",
                    label: "Latest backup run selected",
                    detail: "Backup/drill run reference is validated before verification.",
                },
                RunbookTemplateChecklistDefinition {
                    key: "confirm_restore_window",
                    label: "Restore window approved",
                    detail: "Restore verification was performed in approved window.",
                },
                RunbookTemplateChecklistDefinition {
                    key: "confirm_evidence_artifact",
                    label: "Evidence artifact prepared",
                    detail: "Artifact URL or ticket reference is ready for closure.",
                },
            ],
            steps: vec![
                RunbookTemplateStepDefinition {
                    step_id: "run_lookup",
                    name: "Lookup run context",
                    detail: "Load backup policy and latest run verification context.",
                    failure_hint: "Verify policy/run identifiers and rerun lookup.",
                },
                RunbookTemplateStepDefinition {
                    step_id: "restore_sla_validation",
                    name: "Validate restore SLA",
                    detail: "Compare observed restore signal with expected SLA budget.",
                    failure_hint: "Attach detailed restore metrics and escalate continuity review.",
                },
                RunbookTemplateStepDefinition {
                    step_id: "evidence_closeout",
                    name: "Close evidence record",
                    detail: "Persist evidence summary and close continuity verification.",
                    failure_hint: "Ensure ticket/artifact link exists and retry closeout action.",
                },
            ],
        },
        RunbookTemplateDefinition {
            key: "maintenance-closeout",
            name: "Maintenance closeout",
            description: "Finalize maintenance execution and publish handover summary.",
            category: "operations",
            supports_live: false,
            params: vec![
                RunbookTemplateParamDefinition {
                    key: "change_ticket",
                    label: "Change ticket",
                    field_type: "string",
                    required: true,
                    options: vec![],
                    min_value: None,
                    max_value: None,
                    default_value: None,
                    placeholder: Some("CHG-20260308-001"),
                },
                RunbookTemplateParamDefinition {
                    key: "change_summary",
                    label: "Change summary",
                    field_type: "string",
                    required: true,
                    options: vec![],
                    min_value: None,
                    max_value: None,
                    default_value: None,
                    placeholder: Some("patched gateway and rotated certs"),
                },
                RunbookTemplateParamDefinition {
                    key: "signoff_count",
                    label: "Signoff count",
                    field_type: "number",
                    required: true,
                    options: vec![],
                    min_value: Some(1),
                    max_value: Some(10),
                    default_value: Some("2"),
                    placeholder: Some("2"),
                },
                RunbookTemplateParamDefinition {
                    key: "simulate_failure_step",
                    label: "Simulate failure step",
                    field_type: "string",
                    required: false,
                    options: vec![],
                    min_value: None,
                    max_value: None,
                    default_value: None,
                    placeholder: Some("stakeholder_signoff"),
                },
            ],
            preflight: vec![
                RunbookTemplateChecklistDefinition {
                    key: "confirm_validation_complete",
                    label: "Validation checks complete",
                    detail: "Post-change validation checklist is complete.",
                },
                RunbookTemplateChecklistDefinition {
                    key: "confirm_alerts_stable",
                    label: "Alerts stable",
                    detail: "No unresolved critical alert remains after maintenance.",
                },
                RunbookTemplateChecklistDefinition {
                    key: "confirm_handover_ready",
                    label: "Handover summary ready",
                    detail: "Shift handover owner/action are prepared with next steps.",
                },
            ],
            steps: vec![
                RunbookTemplateStepDefinition {
                    step_id: "change_log_collection",
                    name: "Collect change log",
                    detail: "Collect affected services and validation result summary.",
                    failure_hint: "Complete validation checklist and attach missing logs.",
                },
                RunbookTemplateStepDefinition {
                    step_id: "stakeholder_signoff",
                    name: "Confirm stakeholder signoff",
                    detail: "Capture owner/signoff acknowledgements for closure.",
                    failure_hint: "Obtain at least two signoffs or document approved exception before closure.",
                },
                RunbookTemplateStepDefinition {
                    step_id: "handover_publish",
                    name: "Publish handover closeout",
                    detail: "Persist change summary and publish shift handover note.",
                    failure_hint: "Update handover owner/action fields and republish closeout summary.",
                },
            ],
        },
    ]
}

fn normalize_runbook_params(
    template: &RunbookTemplateDefinition,
    raw: Value,
) -> AppResult<JsonMap<String, Value>> {
    let Value::Object(input) = raw else {
        return Err(AppError::Validation(
            "params must be a JSON object".to_string(),
        ));
    };

    let allowed_keys = template
        .params
        .iter()
        .map(|param| param.key)
        .collect::<BTreeSet<_>>();
    for key in input.keys() {
        if !allowed_keys.contains(key.as_str()) {
            return Err(AppError::Validation(format!(
                "unknown runbook parameter '{}'",
                key
            )));
        }
    }

    let mut normalized = JsonMap::new();
    for param in &template.params {
        let value = input.get(param.key).cloned();
        let normalized_value = normalize_single_param(param, value)?;
        if let Some(value) = normalized_value {
            normalized.insert(param.key.to_string(), value);
        }
    }

    Ok(normalized)
}

fn normalize_single_param(
    definition: &RunbookTemplateParamDefinition,
    value: Option<Value>,
) -> AppResult<Option<Value>> {
    let value = match value {
        Some(Value::Null) => None,
        Some(value) => Some(value),
        None => definition
            .default_value
            .map(|default| Value::String(default.to_string())),
    };

    let Some(value) = value else {
        if definition.required {
            return Err(AppError::Validation(format!(
                "parameter '{}' is required",
                definition.key
            )));
        }
        return Ok(None);
    };

    match definition.field_type {
        "string" => {
            let Value::String(raw) = value else {
                return Err(AppError::Validation(format!(
                    "parameter '{}' must be a string",
                    definition.key
                )));
            };
            let trimmed = raw.trim();
            if definition.required && trimmed.is_empty() {
                return Err(AppError::Validation(format!(
                    "parameter '{}' cannot be empty",
                    definition.key
                )));
            }
            if trimmed.len() > MAX_TEXT_FIELD_LEN {
                return Err(AppError::Validation(format!(
                    "parameter '{}' length must be <= {}",
                    definition.key, MAX_TEXT_FIELD_LEN
                )));
            }
            Ok(Some(Value::String(trimmed.to_string())))
        }
        "enum" => {
            let Value::String(raw) = value else {
                return Err(AppError::Validation(format!(
                    "parameter '{}' must be a string",
                    definition.key
                )));
            };
            let trimmed = raw.trim();
            if definition.required && trimmed.is_empty() {
                return Err(AppError::Validation(format!(
                    "parameter '{}' cannot be empty",
                    definition.key
                )));
            }
            if !definition.options.is_empty()
                && !definition.options.iter().any(|option| *option == trimmed)
            {
                return Err(AppError::Validation(format!(
                    "parameter '{}' must be one of: {}",
                    definition.key,
                    definition.options.join(", ")
                )));
            }
            Ok(Some(Value::String(trimmed.to_string())))
        }
        "number" => {
            let number = match value {
                Value::Number(value) => value.as_i64().ok_or_else(|| {
                    AppError::Validation(format!(
                        "parameter '{}' must be an integer",
                        definition.key
                    ))
                })?,
                Value::String(value) => value.trim().parse::<i64>().map_err(|_| {
                    AppError::Validation(format!(
                        "parameter '{}' must be an integer",
                        definition.key
                    ))
                })?,
                _ => {
                    return Err(AppError::Validation(format!(
                        "parameter '{}' must be a number",
                        definition.key
                    )));
                }
            };

            if let Some(min_value) = definition.min_value {
                if number < min_value {
                    return Err(AppError::Validation(format!(
                        "parameter '{}' must be >= {}",
                        definition.key, min_value
                    )));
                }
            }
            if let Some(max_value) = definition.max_value {
                if number > max_value {
                    return Err(AppError::Validation(format!(
                        "parameter '{}' must be <= {}",
                        definition.key, max_value
                    )));
                }
            }
            Ok(Some(Value::Number(number.into())))
        }
        _ => Err(AppError::Validation(format!(
            "unsupported parameter field_type '{}'",
            definition.field_type
        ))),
    }
}

fn normalize_preflight_confirmations(
    template: &RunbookTemplateDefinition,
    raw: Vec<String>,
) -> AppResult<Vec<String>> {
    let mut normalized_set = BTreeSet::new();
    for item in raw {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            continue;
        }
        normalized_set.insert(trimmed.to_string());
    }

    let mut missing = Vec::new();
    let mut ordered = Vec::new();
    for required in &template.preflight {
        if normalized_set.contains(required.key) {
            ordered.push(required.key.to_string());
        } else {
            missing.push(required.key.to_string());
        }
    }

    if !missing.is_empty() {
        return Err(AppError::Validation(format!(
            "missing preflight confirmations: {}",
            missing.join(", ")
        )));
    }

    Ok(ordered)
}

fn normalize_runbook_evidence_input(raw: RunbookEvidenceInput) -> AppResult<RunbookEvidenceInput> {
    let summary = required_trimmed("evidence.summary", raw.summary, MAX_NOTE_LEN)?;
    let ticket_ref = trim_optional(raw.ticket_ref, MAX_TICKET_REF_LEN);
    let artifact_url = trim_optional(raw.artifact_url, MAX_ARTIFACT_URL_LEN);

    Ok(RunbookEvidenceInput {
        summary,
        ticket_ref,
        artifact_url,
    })
}

fn evaluate_step_failure(
    template_key: &str,
    step_id: &str,
    params: &JsonMap<String, Value>,
    step_index: usize,
) -> Option<String> {
    if let Some(simulated_step) = string_param(params, "simulate_failure_step") {
        if simulated_step == step_id {
            return Some(format!(
                "simulated failure requested for step '{}'",
                step_id
            ));
        }
    }

    match (template_key, step_id) {
        ("service-restart-safe", "post_health_validation") => {
            if let Some(service_name) = string_param(params, "service_name") {
                if service_name.contains("legacy") {
                    return Some(
                        "health probe failed: legacy service profile requires manual warmup"
                            .to_string(),
                    );
                }
            }
        }
        ("dependency-check", "reachability_probe") => {
            if let Some(target) = string_param(params, "dependency_target") {
                if target.contains("unstable") {
                    return Some(
                        "dependency probe timeout exceeded due to unstable endpoint".to_string(),
                    );
                }
            }
        }
        ("backup-verify", "restore_sla_validation") => {
            if let Some(minutes) = number_param(params, "expected_restore_minutes") {
                if minutes > 30 {
                    return Some(format!(
                        "restore SLA validation failed: expected_restore_minutes={} exceeds budget",
                        minutes
                    ));
                }
            }
        }
        ("maintenance-closeout", "stakeholder_signoff") => {
            if let Some(count) = number_param(params, "signoff_count") {
                if count < 2 {
                    return Some(format!(
                        "stakeholder signoff requirement not met: signoff_count={}",
                        count
                    ));
                }
            }
        }
        _ => {}
    }

    if step_index > 8 {
        return Some("guardrail abort: unexpected step depth".to_string());
    }

    None
}

fn string_param(params: &JsonMap<String, Value>, key: &str) -> Option<String> {
    params
        .get(key)
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
}

fn number_param(params: &JsonMap<String, Value>, key: &str) -> Option<i64> {
    params.get(key).and_then(|value| value.as_i64())
}

fn normalize_template_key(value: String) -> AppResult<String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Err(AppError::Validation("template key is required".to_string()));
    }
    if normalized.len() > MAX_TEMPLATE_KEY_LEN {
        return Err(AppError::Validation(format!(
            "template key length must be <= {}",
            MAX_TEMPLATE_KEY_LEN
        )));
    }
    if !normalized
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
    {
        return Err(AppError::Validation(
            "template key must only contain lowercase letters, digits, or '-'".to_string(),
        ));
    }
    Ok(normalized)
}

fn normalize_execution_status(value: String) -> AppResult<String> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "succeeded" | "failed" => Ok(normalized),
        _ => Err(AppError::Validation(
            "status must be one of: succeeded, failed".to_string(),
        )),
    }
}

fn normalize_ticket_lifecycle_status(value: String, field_name: &str) -> AppResult<String> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        TICKET_STATUS_OPEN
        | TICKET_STATUS_IN_PROGRESS
        | TICKET_STATUS_RESOLVED
        | TICKET_STATUS_CLOSED
        | TICKET_STATUS_CANCELLED => Ok(normalized),
        _ => Err(AppError::Validation(format!(
            "{field_name} must be one of: open, in_progress, resolved, closed, cancelled"
        ))),
    }
}

fn normalize_ticket_priority_for_link(value: String) -> AppResult<String> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        TICKET_PRIORITY_LOW
        | TICKET_PRIORITY_MEDIUM
        | TICKET_PRIORITY_HIGH
        | TICKET_PRIORITY_CRITICAL => Ok(normalized),
        _ => Err(AppError::Validation(
            "ticket priority must be one of: low, medium, high, critical".to_string(),
        )),
    }
}

fn normalize_owner_key(value: String) -> AppResult<String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Err(AppError::Validation("owner_key is required".to_string()));
    }
    if normalized.len() > MAX_OWNER_KEY_LEN {
        return Err(AppError::Validation(format!(
            "owner_key length must be <= {}",
            MAX_OWNER_KEY_LEN
        )));
    }
    if !normalized
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_')
    {
        return Err(AppError::Validation(
            "owner_key must only contain lowercase letters, digits, '-', or '_'".to_string(),
        ));
    }
    Ok(normalized)
}

fn normalize_owner_type_ref(value: String) -> AppResult<String> {
    let normalized = required_trimmed("owner_type", value, 32)?.to_ascii_lowercase();
    match normalized.as_str() {
        "team" | "user" | "group" | "external" => Ok(normalized),
        _ => Err(AppError::Validation(
            "owner_type must be one of: team, user, group, external".to_string(),
        )),
    }
}

fn normalize_risk_severity(value: String) -> AppResult<String> {
    let normalized = required_trimmed("severity", value, 32)?.to_ascii_lowercase();
    match normalized.as_str() {
        "warning" | "critical" => Ok(normalized),
        _ => Err(AppError::Validation(
            "severity must be one of: warning, critical".to_string(),
        )),
    }
}

fn build_runbook_risk_alert_source_key(
    template_key: &str,
    execution_mode: Option<&str>,
    days: u32,
) -> String {
    let mode = execution_mode.unwrap_or("all");
    format!("runbook-risk-alert:{template_key}:{mode}:{days}d")
}

fn ticket_priority_for_risk_severity(severity: &str) -> &'static str {
    if severity == "critical" {
        TICKET_PRIORITY_CRITICAL
    } else {
        TICKET_PRIORITY_HIGH
    }
}

fn normalize_analytics_days(value: Option<u32>) -> u32 {
    value
        .unwrap_or(DEFAULT_ANALYTICS_DAYS)
        .clamp(1, MAX_ANALYTICS_DAYS)
}

fn calculate_success_rate_percent(succeeded: usize, total: usize) -> f64 {
    if total == 0 {
        return 0.0;
    }
    ((succeeded as f64 * 1000.0) / total as f64).round() / 10.0
}

fn first_failed_timeline_step(
    timeline: &[RunbookStepTimelineEvent],
) -> Option<&RunbookStepTimelineEvent> {
    timeline.iter().find(|item| item.status == "failed")
}

fn required_trimmed(field: &str, value: String, max_len: usize) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(format!("{field} is required")));
    }
    if trimmed.len() > max_len {
        return Err(AppError::Validation(format!(
            "{field} length must be <= {max_len}"
        )));
    }
    Ok(trimmed.to_string())
}

fn trim_optional(value: Option<String>, max_len: usize) -> Option<String> {
    value.and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.chars().take(max_len).collect())
        }
    })
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use serde_json::json;

    use super::{
        RunbookRiskOwnerRoutingRuleRow,
        RunbookAnalyticsPolicyRow, RunbookRiskAlertNotificationDeliveryRow,
        RunbookRiskAlertTicketLinkRow,
        RunbookStepTimelineEvent, calculate_success_rate_percent,
        build_runbook_risk_alert_source_key, enforce_template_supports_execution_mode,
        evaluate_step_failure, first_failed_timeline_step, normalize_analytics_days,
        normalize_execution_mode, normalize_live_template_keys, normalize_owner_key,
        normalize_preflight_confirmations, normalize_preset_name,
        normalize_risk_severity, normalize_runbook_params,
        normalize_runbook_risk_alert_notification_status,
        parse_dependency_target, parse_preflight_snapshot_confirmed,
        parse_runbook_analytics_policy_row,
        parse_runbook_risk_alert_owner_route_from_ticket_metadata,
        parse_runbook_risk_alert_notification_delivery_row,
        parse_runbook_risk_alert_ticket_link_row, resolve_template_by_key,
        risk_severity_rank,
        select_runbook_risk_owner_routing_rule,
    };

    #[test]
    fn validates_runbook_param_guardrails() {
        let template = resolve_template_by_key("dependency-check").expect("template");
        let normalized = normalize_runbook_params(
            &template,
            json!({
                "asset_ref": "app-1",
                "dependency_target": "redis://cache-a:6379",
                "probe_timeout_seconds": 20
            }),
        )
        .expect("normalized");
        assert_eq!(
            normalized
                .get("probe_timeout_seconds")
                .and_then(|value| value.as_i64()),
            Some(20)
        );

        let err = normalize_runbook_params(
            &template,
            json!({
                "asset_ref": "app-1",
                "dependency_target": "redis://cache-a:6379",
                "probe_timeout_seconds": 500
            }),
        )
        .expect_err("out of range timeout should fail");
        assert!(format!("{}", err).contains("must be <= 300"));
    }

    #[test]
    fn requires_all_preflight_confirmations() {
        let template = resolve_template_by_key("service-restart-safe").expect("template");
        let err =
            normalize_preflight_confirmations(&template, vec!["confirm_change_window".to_string()])
                .expect_err("missing confirmations should fail");
        assert!(format!("{}", err).contains("missing preflight confirmations"));
    }

    #[test]
    fn provides_remediation_hint_for_failed_step() {
        let template = resolve_template_by_key("maintenance-closeout").expect("template");
        let params = normalize_runbook_params(
            &template,
            json!({
                "change_ticket": "CHG-001",
                "change_summary": "patched",
                "signoff_count": 1
            }),
        )
        .expect("params");

        let failure = evaluate_step_failure(template.key, "stakeholder_signoff", &params, 1);
        assert!(failure.is_some());
    }

    #[test]
    fn normalizes_live_template_allowlist() {
        let templates = normalize_live_template_keys(vec![
            "dependency-check".to_string(),
            "dependency-check".to_string(),
        ])
        .expect("live templates");
        assert_eq!(templates, vec!["dependency-check".to_string()]);

        let err = normalize_live_template_keys(vec!["backup-verify".to_string()])
            .expect_err("non-live template should fail");
        assert!(format!("{}", err).contains("does not support live execution"));
    }

    #[test]
    fn parses_dependency_target_with_scheme_and_port() {
        let target = parse_dependency_target("http://127.0.0.1:8080/health").expect("valid target");
        assert_eq!(target.host, "127.0.0.1");
        assert_eq!(target.port, 8080);

        let target = parse_dependency_target("https://example.local/api").expect("https target");
        assert_eq!(target.host, "example.local");
        assert_eq!(target.port, 443);

        let target = parse_dependency_target("redis.local:6379").expect("host:port target");
        assert_eq!(target.host, "redis.local");
        assert_eq!(target.port, 6379);
    }

    #[test]
    fn validates_execution_mode_values() {
        assert_eq!(
            normalize_execution_mode("simulate".to_string()).expect("simulate"),
            "simulate"
        );
        assert_eq!(
            normalize_execution_mode("live".to_string()).expect("live"),
            "live"
        );

        let err = normalize_execution_mode("invalid".to_string()).expect_err("should fail");
        assert!(format!("{}", err).contains("execution_mode must be one of"));
    }

    #[test]
    fn validates_preset_name_rules() {
        assert_eq!(
            normalize_preset_name("  dependency baseline  ".to_string()).expect("preset name"),
            "dependency baseline"
        );
        assert!(normalize_preset_name("".to_string()).is_err());
        assert!(normalize_preset_name("x".repeat(129)).is_err());
    }

    #[test]
    fn validates_template_execution_mode_support() {
        let live_template = resolve_template_by_key("dependency-check").expect("template");
        enforce_template_supports_execution_mode(&live_template, "live")
            .expect("live template supports live mode");

        let simulate_only_template = resolve_template_by_key("backup-verify").expect("template");
        let err = enforce_template_supports_execution_mode(&simulate_only_template, "live")
            .expect_err("should reject live mode");
        assert!(format!("{}", err).contains("does not support live execution"));
    }

    #[test]
    fn parses_preflight_snapshot_confirmed_values() {
        let confirmed = parse_preflight_snapshot_confirmed(json!({
            "confirmed": ["confirm_probe_source", "confirm_dependency_owner"],
            "total_required": 3
        }))
        .expect("confirmed list");
        assert_eq!(confirmed.len(), 2);
        assert_eq!(confirmed[0], "confirm_probe_source");
        assert_eq!(confirmed[1], "confirm_dependency_owner");

        let err = parse_preflight_snapshot_confirmed(json!({
            "total_required": 3
        }))
        .expect_err("missing confirmed should fail");
        assert!(format!("{}", err).contains("missing confirmed"));
    }

    #[test]
    fn normalizes_analytics_days_range() {
        assert_eq!(normalize_analytics_days(None), 14);
        assert_eq!(normalize_analytics_days(Some(0)), 1);
        assert_eq!(normalize_analytics_days(Some(14)), 14);
        assert_eq!(normalize_analytics_days(Some(365)), 90);
    }

    #[test]
    fn calculates_success_rate_percent_with_one_decimal_precision() {
        assert_eq!(calculate_success_rate_percent(0, 0), 0.0);
        assert_eq!(calculate_success_rate_percent(1, 3), 33.3);
        assert_eq!(calculate_success_rate_percent(3, 4), 75.0);
    }

    #[test]
    fn detects_first_failed_timeline_step() {
        let now = Utc::now();
        let timeline = vec![
            RunbookStepTimelineEvent {
                step_id: "scope_validation".to_string(),
                name: "Scope".to_string(),
                detail: "detail".to_string(),
                status: "succeeded".to_string(),
                started_at: now,
                finished_at: now,
                output: "ok".to_string(),
                remediation_hint: None,
            },
            RunbookStepTimelineEvent {
                step_id: "reachability_probe".to_string(),
                name: "Probe".to_string(),
                detail: "detail".to_string(),
                status: "failed".to_string(),
                started_at: now,
                finished_at: now,
                output: "timeout".to_string(),
                remediation_hint: Some("check network path".to_string()),
            },
        ];

        let failed = first_failed_timeline_step(&timeline).expect("failed step");
        assert_eq!(failed.step_id, "reachability_probe");
        assert_eq!(failed.output, "timeout");
    }

    #[test]
    fn validates_analytics_policy_ranges() {
        let now = Utc::now();
        let valid = parse_runbook_analytics_policy_row(RunbookAnalyticsPolicyRow {
            policy_key: "global".to_string(),
            failure_rate_threshold_percent: 20,
            minimum_sample_size: 5,
            note: None,
            updated_by: "admin".to_string(),
            created_at: now,
            updated_at: now,
        })
        .expect("valid policy");
        assert_eq!(valid.failure_rate_threshold_percent, 20);
        assert_eq!(valid.minimum_sample_size, 5);

        let err = parse_runbook_analytics_policy_row(RunbookAnalyticsPolicyRow {
            policy_key: "global".to_string(),
            failure_rate_threshold_percent: 0,
            minimum_sample_size: 5,
            note: None,
            updated_by: "admin".to_string(),
            created_at: now,
            updated_at: now,
        })
        .expect_err("threshold range should fail");
        assert!(format!("{}", err).contains("failure_rate_threshold_percent"));
    }

    #[test]
    fn builds_runbook_risk_alert_source_key() {
        assert_eq!(
            build_runbook_risk_alert_source_key("dependency-check", Some("simulate"), 14),
            "runbook-risk-alert:dependency-check:simulate:14d"
        );
        assert_eq!(
            build_runbook_risk_alert_source_key("dependency-check", None, 30),
            "runbook-risk-alert:dependency-check:all:30d"
        );
    }

    #[test]
    fn validates_runbook_risk_alert_ticket_link_row() {
        let now = Utc::now();
        let valid_row = RunbookRiskAlertTicketLinkRow {
            id: 1,
            template_key: "dependency-check".to_string(),
            execution_mode: Some("simulate".to_string()),
            window_days: 14,
            source_key: "runbook-risk-alert:dependency-check:simulate:14d".to_string(),
            status: "open".to_string(),
            ticket_id: 1001,
            ticket_no: "TKT-20260310-001001".to_string(),
            ticket_status: "in_progress".to_string(),
            ticket_priority: "high".to_string(),
            ticket_assignee: Some("dependency-owner".to_string()),
            ticket_metadata: json!({
                "runbook_risk_owner_route": {
                    "owner": "dependency-owner",
                    "source": "template_rule",
                    "reason": "Dependency probe failures route to the dependency owner for follow-up."
                }
            }),
            updated_at: now,
        };

        let (template_key, item) =
            parse_runbook_risk_alert_ticket_link_row(valid_row, Some("simulate"), 14)
                .expect("valid ticket link row");
        assert_eq!(template_key, "dependency-check");
        assert_eq!(item.ticket_status, "in_progress");
        assert_eq!(item.ticket_priority, "high");
        assert_eq!(item.ticket_assignee.as_deref(), Some("dependency-owner"));
        assert_eq!(
            item.owner_route.as_ref().map(|route| route.source.as_str()),
            Some("template_rule")
        );
        assert_eq!(item.status, "open");

        let invalid_source_key_row = RunbookRiskAlertTicketLinkRow {
            id: 2,
            template_key: "dependency-check".to_string(),
            execution_mode: Some("simulate".to_string()),
            window_days: 14,
            source_key: "runbook-risk-alert:dependency-check:live:14d".to_string(),
            status: "open".to_string(),
            ticket_id: 1002,
            ticket_no: "TKT-20260310-001002".to_string(),
            ticket_status: "open".to_string(),
            ticket_priority: "medium".to_string(),
            ticket_assignee: None,
            ticket_metadata: json!({}),
            updated_at: now,
        };
        let err = parse_runbook_risk_alert_ticket_link_row(
            invalid_source_key_row,
            Some("simulate"),
            14,
        )
        .expect_err("source key mismatch should fail");
        assert!(format!("{}", err).contains("source_key mismatch"));
    }

    #[test]
    fn ranks_risk_severity_levels() {
        assert!(risk_severity_rank("critical") > risk_severity_rank("warning"));
        assert!(risk_severity_rank("warning") > risk_severity_rank("unknown"));
    }

    #[test]
    fn parses_runbook_risk_owner_route_from_ticket_metadata() {
        let route = parse_runbook_risk_alert_owner_route_from_ticket_metadata(
            &json!({
                "runbook_risk_owner_route": {
                    "owner": "change-owner",
                    "owner_key": "change-primary",
                    "owner_label": "Change Primary",
                    "source": "template_rule",
                    "reason": "Maintenance closeout gaps route to the change owner for signoff and handover."
                }
            }),
            Some("change-owner"),
        )
        .expect("route")
        .expect("some route");
        assert_eq!(route.owner, "change-owner");
        assert_eq!(route.owner_key.as_deref(), Some("change-primary"));
        assert_eq!(route.owner_label.as_deref(), Some("Change Primary"));
        assert_eq!(route.source, "template_rule");

        let fallback = parse_runbook_risk_alert_owner_route_from_ticket_metadata(
            &json!({}),
            Some("ops-escalation"),
        )
        .expect("fallback")
        .expect("fallback route");
        assert_eq!(fallback.owner, "ops-escalation");
        assert_eq!(fallback.owner_key, None);
        assert_eq!(fallback.source, "ticket_assignee");
    }

    #[test]
    fn selects_most_specific_runbook_risk_owner_routing_rule() {
        let now = Utc::now();
        let rows = vec![
            RunbookRiskOwnerRoutingRuleRow {
                id: 1,
                template_key: "dependency-check".to_string(),
                execution_mode: None,
                severity: None,
                owner_key: "fallback-owner".to_string(),
                priority: 100,
                note: None,
                is_enabled: true,
                updated_by: "admin".to_string(),
                created_at: now,
                updated_at: now,
            },
            RunbookRiskOwnerRoutingRuleRow {
                id: 2,
                template_key: "dependency-check".to_string(),
                execution_mode: Some("simulate".to_string()),
                severity: Some("warning".to_string()),
                owner_key: "simulate-warning-owner".to_string(),
                priority: 50,
                note: None,
                is_enabled: true,
                updated_by: "admin".to_string(),
                created_at: now,
                updated_at: now,
            },
            RunbookRiskOwnerRoutingRuleRow {
                id: 3,
                template_key: "dependency-check".to_string(),
                execution_mode: None,
                severity: Some("warning".to_string()),
                owner_key: "warning-owner".to_string(),
                priority: 10,
                note: None,
                is_enabled: true,
                updated_by: "admin".to_string(),
                created_at: now,
                updated_at: now,
            },
        ];

        let selected = select_runbook_risk_owner_routing_rule(
            &rows,
            "dependency-check",
            Some("simulate"),
            "warning",
        )
        .expect("selected")
        .expect("rule");
        assert_eq!(selected.owner_key, "simulate-warning-owner");
    }

    #[test]
    fn normalizes_owner_config_fields() {
        assert_eq!(
            normalize_owner_key(" Team_A ".to_string()).expect("owner key"),
            "team_a"
        );
        assert_eq!(
            normalize_risk_severity("Critical".to_string()).expect("severity"),
            "critical"
        );
        assert!(normalize_owner_key("".to_string()).is_err());
        assert!(normalize_risk_severity("info".to_string()).is_err());
    }

    #[test]
    fn validates_runbook_risk_alert_notification_status_values() {
        assert_eq!(
            normalize_runbook_risk_alert_notification_status("queued")
                .expect("queued"),
            "queued"
        );
        assert_eq!(
            normalize_runbook_risk_alert_notification_status("delivered")
                .expect("delivered"),
            "delivered"
        );
        assert_eq!(
            normalize_runbook_risk_alert_notification_status("failed")
                .expect("failed"),
            "failed"
        );
        assert_eq!(
            normalize_runbook_risk_alert_notification_status("skipped")
                .expect("skipped"),
            "skipped"
        );

        let err = normalize_runbook_risk_alert_notification_status("unknown")
            .expect_err("invalid status should fail");
        assert!(format!("{}", err).contains("dispatch_status must be one of"));
    }

    #[test]
    fn validates_runbook_risk_alert_notification_delivery_row() {
        let now = Utc::now();
        let item = parse_runbook_risk_alert_notification_delivery_row(
            RunbookRiskAlertNotificationDeliveryRow {
                id: 1,
                source_key: "runbook-risk-alert:dependency-check:all:14d".to_string(),
                template_key: "dependency-check".to_string(),
                execution_mode: None,
                window_days: 14,
                ticket_id: 1001,
                ticket_no: "TKT-20260311-001001".to_string(),
                event_type: "runbook_risk.ticket_linked".to_string(),
                dispatch_status: "delivered".to_string(),
                subscription_id: Some(1),
                channel_id: Some(2),
                channel_type: Some("email".to_string()),
                target: "ops@example.com".to_string(),
                attempts: 1,
                response_code: Some(202),
                last_error: None,
                delivered_at: Some(now),
                created_at: now,
            },
            None,
            14,
        )
        .expect("valid notification row");
        assert_eq!(item.template_key, "dependency-check");
        assert_eq!(item.dispatch_status, "delivered");
        assert_eq!(item.channel_type.as_deref(), Some("email"));

        let err = parse_runbook_risk_alert_notification_delivery_row(
            RunbookRiskAlertNotificationDeliveryRow {
                id: 2,
                source_key: "runbook-risk-alert:dependency-check:simulate:14d".to_string(),
                template_key: "dependency-check".to_string(),
                execution_mode: None,
                window_days: 14,
                ticket_id: 1002,
                ticket_no: "TKT-20260311-001002".to_string(),
                event_type: "runbook_risk.ticket_linked".to_string(),
                dispatch_status: "failed".to_string(),
                subscription_id: None,
                channel_id: None,
                channel_type: None,
                target: "-".to_string(),
                attempts: 0,
                response_code: None,
                last_error: Some("no subscription".to_string()),
                delivered_at: None,
                created_at: now,
            },
            None,
            14,
        )
        .expect_err("source key mismatch should fail");
        assert!(format!("{}", err).contains("source_key mismatch"));
    }
}
