import { useMemo, useState } from "react";
import { HorizontalFillBar } from "../components/chart-primitives";
import { SectionCard } from "../components/layout";

type DashboardWidgetKey =
  | "monitoring_health"
  | "daily_ops_risk"
  | "cmdb_capacity"
  | "ticket_escalation"
  | "topology_risk";

type DashboardWidgetLayout = {
  key: DashboardWidgetKey;
  enabled: boolean;
  order: number;
};

type DashboardLayoutPayload = {
  version: 1;
  widgets: DashboardWidgetLayout[];
};

const DASHBOARD_LAYOUT_STORAGE_KEY = "cloudops.dashboard_layout.v1";
const DASHBOARD_WIDGETS: Array<{ key: DashboardWidgetKey; title: string; description: string }> = [
  {
    key: "monitoring_health",
    title: "Monitoring health",
    description: "Reachability and critical source health."
  },
  {
    key: "daily_ops_risk",
    title: "Daily ops risk",
    description: "Overdue/blocked workload and escalation pressure."
  },
  {
    key: "cmdb_capacity",
    title: "CMDB capacity",
    description: "Asset volume and binding completeness."
  },
  {
    key: "ticket_escalation",
    title: "Ticket escalation",
    description: "Escalation queue pressure and pending items."
  },
  {
    key: "topology_risk",
    title: "Topology risk",
    description: "Topology map scale and diagnostics visibility."
  }
];

function buildDefaultDashboardLayout(): DashboardWidgetLayout[] {
  return DASHBOARD_WIDGETS.map((item, index) => ({
    key: item.key,
    enabled: true,
    order: index + 1
  }));
}

function parseDashboardLayout(raw: string | null): DashboardWidgetLayout[] {
  if (!raw) {
    return buildDefaultDashboardLayout();
  }
  try {
    const parsed = JSON.parse(raw) as DashboardLayoutPayload;
    if (parsed.version !== 1 || !Array.isArray(parsed.widgets)) {
      return buildDefaultDashboardLayout();
    }
    const byKey = new Map<DashboardWidgetKey, DashboardWidgetLayout>();
    for (const item of parsed.widgets) {
      if (!item || typeof item !== "object") {
        continue;
      }
      if (!DASHBOARD_WIDGETS.some((widget) => widget.key === item.key)) {
        continue;
      }
      byKey.set(item.key, {
        key: item.key,
        enabled: Boolean(item.enabled),
        order: Number.isFinite(item.order) ? Math.max(1, Math.floor(item.order)) : 999
      });
    }
    return DASHBOARD_WIDGETS.map((item, index) => {
      const existing = byKey.get(item.key);
      return (
        existing ?? {
          key: item.key,
          enabled: true,
          order: index + 1
        }
      );
    });
  } catch {
    return buildDefaultDashboardLayout();
  }
}

function persistDashboardLayout(layout: DashboardWidgetLayout[]) {
  if (typeof window === "undefined") {
    return;
  }
  const payload: DashboardLayoutPayload = {
    version: 1,
    widgets: layout
  };
  window.localStorage.setItem(DASHBOARD_LAYOUT_STORAGE_KEY, JSON.stringify(payload));
}

export function OverviewAdminSections(rawProps: Record<string, unknown>) {
  const {
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
    checkChangeCalendarConflicts,
    createChangeCalendarReservation,
    closeHandoverCarryoverItem,
    closingHandoverItemKey,
    cockpitCriticalAssets,
    cockpitOperationalAssets,
    completeOpsChecklistItem,
    createSampleAsset,
    dailyOpsBriefing,
    dailyOpsClosureContinuity,
    dailyOpsNotice,
    departmentWorkspace,
    departmentWorkspaceOptions,
    dailyCockpitDepartmentFilter,
    dailyCockpitNotice,
    dailyCockpitQueue,
    dailyCockpitSiteFilter,
    nextBestActions,
    runbookTemplates,
    runbookExecutions,
    runbookPresets,
    runbookExecutionPolicy,
    runbookExecutionPolicyDraft,
    runbookAnalyticsPolicy,
    runbookAnalyticsPolicyDraft,
    runbookAnalyticsFilterDraft,
    runbookAnalyticsSummary,
    runbookFailureFeed,
    runbookRiskAlerts,
    runbookRiskOwnerDirectory,
    runbookRiskOwnerRoutingRules,
    runbookRiskOwnerReadiness,
    runbookRiskOwnerRepairPlan,
    integrationBootstrapCatalog,
    integrationBootstrapDrafts,
    goLiveReadiness,
    runbookExecutionMode,
    selectedRunbookTemplateKey,
    selectedRunbookPresetId,
    runbookParamDraft,
    runbookPreflightDraft,
    runbookPresetDraft,
    runbookEvidenceDraft,
    runbookNotice,
    incidentCommandDetail,
    incidentCommandDraft,
    incidentCommandNotice,
    incidentCommands,
    exportingHandoverDigest,
    exportingHandoverReminders,
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
    functionWorkspace,
    loadBackupPolicies,
    loadRunbookTemplates,
    loadRunbookExecutionPolicy,
    loadRunbookAnalyticsPolicy,
    loadRunbookTemplateExecutions,
    loadRunbookExecutionPresets,
    loadRunbookAnalyticsSummary,
    loadRunbookFailureFeed,
    loadRunbookRiskAlerts,
    loadRunbookRiskOwnerDirectory,
    loadRunbookRiskOwnerRoutingRules,
    loadRunbookRiskOwnerReadiness,
    loadRunbookRiskOwnerRepairPlan,
    loadGoLiveReadiness,
    loadIntegrationBootstrapCatalog,
    applyGoLiveAction,
    applyIntegrationBootstrap,
    applyRunbookRiskOwnerReadinessRepair,
    createRunbookRiskAlertTicket,
    loadBackupPolicyRuns,
    loadBackupRestoreEvidence,
    loadBackupEvidenceCompliancePolicy,
    loadBackupEvidenceComplianceScorecard,
    loadChangeCalendar,
    loadChangeCalendarReservations,
    loadChangeCalendarSlotRecommendations,
    loadDailyOpsClosureContinuity,
    loadDailyOpsBriefing,
    loadHandoverDigest,
    loadHandoverReminders,
    loadIncidentCommandDetail,
    loadIncidentCommands,
    loadNextBestActions,
    loadWeeklyDigest,
    applyDailyOpsFollowUpAction,
    applyDailyOpsOwnerAssignment,
    applyDailyOpsEscalationAction,
    loadDailyCockpitSnapshot,
    loadBusinessOverview,
    loadMonitoringOverview,
    loadOpsChecklist,
    loadAssets,
    loadAssetStats,
    loadFieldDefinitions,
    runBackupPolicy,
    runBackupSchedulerTick,
    executeRunbookTemplate,
    applyRunbookExecutionPreset,
    createRunbookExecutionPreset,
    deleteRunbookExecutionPreset,
    replayRunbookTemplateExecution,
    saveRunbookExecutionPolicy,
    saveRunbookAnalyticsPolicy,
    saveRunbookRiskOwnerDirectory,
    saveRunbookRiskOwnerRoutingRules,
    closeBackupRestoreEvidence,
    runningBackupPolicyActionId,
    loadingDailyOpsBriefing,
    loadingDailyOpsClosureContinuity,
    loadingDailyCockpit,
    loadingNextBestActions,
    loadingOpsChecklist,
    loadingIncidentCommandDetail,
    loadingIncidentCommands,
    loadingRunbookTemplates,
    loadingRunbookExecutions,
    loadingRunbookPresets,
    loadingRunbookExecutionPolicy,
    loadingRunbookAnalyticsPolicy,
    loadingRunbookAnalyticsSummary,
    loadingRunbookFailureFeed,
    loadingRunbookRiskAlerts,
    loadingRunbookRiskOwnerDirectory,
    loadingRunbookRiskOwnerRoutingRules,
    loadingRunbookRiskOwnerReadiness,
    loadingRunbookRiskOwnerRepairPlan,
    loadingIntegrationBootstrapCatalog,
    loadingGoLiveReadiness,
    runningRunbookRiskTicketTemplateKey,
    runningRunbookRiskOwnerRepairKey,
    runningGoLiveActionKey,
    runningIntegrationBootstrapKey,
    executingRunbookTemplate,
    savingRunbookPreset,
    savingRunbookExecutionPolicy,
    savingRunbookAnalyticsPolicy,
    savingRunbookRiskOwnerDirectory,
    savingRunbookRiskOwnerRoutingRules,
    replayingRunbookExecutionId,
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
    loadingAssetStats,
    loadingAssets,
    loadingFields,
    loadingMonitoringOverview,
    businessOverview,
    loadingBusinessOverview,
    menuAxis,
    monitoringOverview,
    monitoringSources,
    ticketEscalationQueue,
    opsChecklist,
    opsChecklistDate,
    opsChecklistNotice,
    perspectiveScopeLabel,
    runningDailyOpsFollowUpActionKey,
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
    setSelectedRunbookPresetId,
    setRunbookExecutionMode,
    setRunbookExecutionPolicyDraft,
    setRunbookAnalyticsPolicyDraft,
    setRunbookAnalyticsFilterDraft,
    setRunbookParamDraft,
    setRunbookPreflightDraft,
    setRunbookPresetDraft,
    setRunbookEvidenceDraft,
    setIntegrationBootstrapDrafts,
    setRunbookRiskOwnerDirectory,
    setRunbookRiskOwnerRoutingRules,
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
    visibleSections,
    creatingSample,
    topologyMap
  } = rawProps as any;
  const selectedRunbookTemplate =
    (runbookTemplates as any[]).find((item) => item.key === selectedRunbookTemplateKey) ?? null;
  const runbookRiskOwnerRepairPlanByKey = new Map(
    ((runbookRiskOwnerRepairPlan?.items ?? []) as any[]).map((item) => [
      `${item.template_key}::${item.owner_key ?? "none"}::${item.readiness_status}`,
      item
    ])
  );
  const [dashboardLayout, setDashboardLayout] = useState<DashboardWidgetLayout[]>(() => {
    if (typeof window === "undefined") {
      return buildDefaultDashboardLayout();
    }
    return parseDashboardLayout(window.localStorage.getItem(DASHBOARD_LAYOUT_STORAGE_KEY));
  });
  const dashboardWidgets = useMemo(() => {
    const detailByKey = new Map(DASHBOARD_WIDGETS.map((item) => [item.key, item]));
    return [...dashboardLayout]
      .sort((left, right) => left.order - right.order)
      .map((item) => ({
        ...item,
        detail: detailByKey.get(item.key)
      }))
      .filter((item) => item.enabled);
  }, [dashboardLayout]);
  const moduleStatusCards = useMemo(() => {
    const monitoringUnreachable = Number(monitoringOverview?.summary?.source_unreachable_total ?? 0);
    const monitoringCritical = Number(monitoringOverview?.layers?.reduce((sum: number, layer: any) => {
      return sum + Number(layer?.health?.critical ?? 0);
    }, 0) ?? 0);
    const dailyBlocked = Number(dailyOpsBriefing?.summary?.blocked ?? 0);
    const dailyOverdue = Number(dailyOpsBriefing?.summary?.overdue ?? 0);
    const escalationQueue = Number((ticketEscalationQueue ?? []).length);
    const cmdbAssets = Number(assetStats?.total_assets ?? 0);
    const cmdbUnbound = Number(assetStats?.unbound?.department_assets ?? 0) + Number(assetStats?.unbound?.business_service_assets ?? 0);
    const topologyNodes = Number(topologyMap?.nodes?.length ?? 0);
    return [
      {
        key: "monitoring",
        label: "Monitoring",
        risk: monitoringUnreachable + monitoringCritical,
        summary: `unreachable=${monitoringUnreachable}, critical=${monitoringCritical}`,
        href: "#/monitoring"
      },
      {
        key: "cmdb",
        label: "CMDB",
        risk: cmdbUnbound,
        summary: `assets=${cmdbAssets}, unbound=${cmdbUnbound}`,
        href: "#/cmdb"
      },
      {
        key: "topology",
        label: "Topology",
        risk: 0,
        summary: `nodes=${topologyNodes}`,
        href: "#/topology"
      },
      {
        key: "workflow",
        label: "Workflow",
        risk: Number(runbookRiskAlerts?.items?.length ?? 0),
        summary: `risk_alerts=${Number(runbookRiskAlerts?.items?.length ?? 0)}`,
        href: "#/workflow"
      },
      {
        key: "tickets",
        label: "Tickets",
        risk: escalationQueue,
        summary: `escalation_queue=${escalationQueue}`,
        href: "#/tickets"
      },
      {
        key: "overview",
        label: "Daily Ops",
        risk: dailyBlocked + dailyOverdue,
        summary: `overdue=${dailyOverdue}, blocked=${dailyBlocked}`,
        href: "#/overview"
      }
    ];
  }, [assetStats, dailyOpsBriefing, monitoringOverview, runbookRiskAlerts, ticketEscalationQueue, topologyMap]);

  const updateDashboardLayout = (updater: (prev: DashboardWidgetLayout[]) => DashboardWidgetLayout[]) => {
    setDashboardLayout((prev) => {
      const next = updater(prev);
      persistDashboardLayout(next);
      return next;
    });
  };
  const toggleWidget = (key: DashboardWidgetKey) => {
    updateDashboardLayout((prev) =>
      prev.map((item) => (item.key === key ? { ...item, enabled: !item.enabled } : item))
    );
  };
  const moveWidget = (key: DashboardWidgetKey, direction: -1 | 1) => {
    updateDashboardLayout((prev) => {
      const sorted = [...prev].sort((left, right) => left.order - right.order);
      const index = sorted.findIndex((item) => item.key === key);
      if (index < 0) {
        return prev;
      }
      const target = index + direction;
      if (target < 0 || target >= sorted.length) {
        return prev;
      }
      const current = sorted[index];
      sorted[index] = sorted[target];
      sorted[target] = current;
      return sorted.map((item, order) => ({ ...item, order: order + 1 }));
    });
  };
  const resetDashboardLayout = () => {
    const next = buildDefaultDashboardLayout();
    persistDashboardLayout(next);
    setDashboardLayout(next);
  };

  return (
    <>
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
              <select value={menuAxis} onChange={(event) => setMenuAxis(event.target.value as any)}>
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
                  onChange={(event) => setFunctionWorkspace(event.target.value as any)}
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
                  {departmentWorkspaceOptions.map((item: any) => (
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
                  {businessWorkspaceOptions.map((item: any) => (
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

      {activePage === "overview" && visibleSections.has("section-module-cockpit") && (
        <SectionCard id="section-module-cockpit" title="Module operation cockpit">
          <p className="section-note" style={{ marginTop: 0 }}>
            Module-first entry with configurable widgets for quick risk/owner/action visibility.
          </p>
          <div className="toolbar-row" style={{ marginBottom: "0.65rem", flexWrap: "wrap" }}>
            <button
              onClick={() => void Promise.all([
                loadMonitoringOverview(),
                loadBusinessOverview({
                  department: menuAxis === "department" ? departmentWorkspace : undefined,
                  business_service: menuAxis === "business" ? businessWorkspace : undefined
                }),
                loadDailyCockpitSnapshot(),
                loadAssets(),
                loadAssetStats()
              ])}
            >
              Refresh cockpit
            </button>
            <button onClick={resetDashboardLayout}>Reset widget layout</button>
          </div>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))", gap: "0.65rem", marginBottom: "0.85rem" }}>
            {moduleStatusCards.map((card) => {
              const severityClass = card.risk > 0 ? "status-chip status-chip-danger" : "status-chip status-chip-success";
              return (
                <div key={`module-card-${card.key}`} className="detail-panel">
                  <div className="toolbar-row" style={{ justifyContent: "space-between" }}>
                    <strong>{card.label}</strong>
                    <span className={severityClass}>risk={card.risk}</span>
                  </div>
                  <p className="inline-note">{card.summary}</p>
                  <button
                    onClick={() => {
                      if (typeof window !== "undefined") {
                        window.location.hash = card.href;
                      }
                    }}
                  >
                    Open {card.label}
                  </button>
                </div>
              );
            })}
          </div>

          <div className="detail-panel" style={{ marginBottom: "0.75rem" }}>
            <div className="toolbar-row" style={{ justifyContent: "space-between" }}>
              <h3 style={{ ...subSectionTitleStyle, marginTop: 0, marginBottom: 0 }}>Business risk and resource baseline</h3>
              <button
                onClick={() => void loadBusinessOverview({
                  department: menuAxis === "department" ? departmentWorkspace : undefined,
                  business_service: menuAxis === "business" ? businessWorkspace : undefined
                })}
                disabled={loadingBusinessOverview}
              >
                {loadingBusinessOverview ? "Loading..." : "Refresh business overview"}
              </button>
            </div>
            {!businessOverview ? (
              <p className="inline-note">
                {loadingBusinessOverview ? "Loading business scope analytics..." : "Business overview is not loaded yet."}
              </p>
            ) : (
              <>
                <div className="toolbar-row" style={{ flexWrap: "wrap", marginBottom: "0.6rem" }}>
                  <span className="status-chip">services={businessOverview.summary.business_service_total}</span>
                  <span className="status-chip">assets={businessOverview.summary.asset_total}</span>
                  <span className="status-chip">active={businessOverview.summary.active_asset_total}</span>
                  <span className="status-chip status-chip-warn">idle={businessOverview.summary.idle_asset_total}</span>
                  <span className="status-chip">open_alerts={businessOverview.summary.open_alert_total}</span>
                  <span className="status-chip status-chip-danger">critical_alerts={businessOverview.summary.critical_alert_total}</span>
                  <span className="status-chip">open_tickets={businessOverview.summary.open_ticket_total}</span>
                  <span className="status-chip status-chip-danger">
                    escalation_tickets={businessOverview.summary.escalation_ticket_total}
                  </span>
                  <span className="inline-note">generated_at={new Date(businessOverview.generated_at).toLocaleString()}</span>
                </div>
                {businessOverview.items.length === 0 ? (
                  <p className="inline-note">No business service binding found for current scope.</p>
                ) : (
                  <div style={{ overflowX: "auto" }}>
                    <table style={{ borderCollapse: "collapse", minWidth: "1100px", width: "100%" }}>
                      <thead>
                        <tr>
                          <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Business service</th>
                          <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Risk score</th>
                          <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Assets (active / idle)</th>
                          <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Alerts</th>
                          <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Tickets</th>
                          <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Scope hints</th>
                        </tr>
                      </thead>
                      <tbody>
                        {businessOverview.items.map((item: any) => (
                          <tr key={`business-overview-${item.business_service}`}>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              {item.business_service}
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              <span className={`status-chip ${item.risk_score >= 180 ? "status-chip-danger" : item.risk_score >= 80 ? "status-chip-warn" : "status-chip-success"}`}>
                                {item.risk_score}
                              </span>
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              <div>total={item.asset_total}</div>
                              <div className="inline-note">active={item.active_asset_total} / idle={item.idle_asset_total}</div>
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              <div>open={item.open_alert_total}</div>
                              <div className="inline-note">critical={item.critical_alert_total}</div>
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              <div>open={item.open_ticket_total}</div>
                              <div className="inline-note">escalation={item.escalation_ticket_total}</div>
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              <div className="inline-note">departments={(item.top_departments ?? []).join(", ") || "-"}</div>
                              <div className="inline-note">sites={(item.top_sites ?? []).join(", ") || "-"}</div>
                            </td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                )}
              </>
            )}
          </div>

          <div className="detail-panel" style={{ marginBottom: "0.75rem" }}>
            <h3 style={{ ...subSectionTitleStyle, marginTop: 0, marginBottom: "0.45rem" }}>Widget layout contract (v1)</h3>
            <p className="inline-note">Storage key: {DASHBOARD_LAYOUT_STORAGE_KEY} | scope: current user browser profile</p>
            <div style={{ overflowX: "auto" }}>
              <table style={{ borderCollapse: "collapse", minWidth: "920px", width: "100%" }}>
                <thead>
                  <tr>
                    <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Widget</th>
                    <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Description</th>
                    <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Enabled</th>
                    <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Order</th>
                    <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Actions</th>
                  </tr>
                </thead>
                <tbody>
                  {[...dashboardLayout].sort((left, right) => left.order - right.order).map((item) => {
                    const detail = DASHBOARD_WIDGETS.find((widget) => widget.key === item.key);
                    return (
                      <tr key={`dashboard-widget-row-${item.key}`}>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>{detail?.title ?? item.key}</td>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>{detail?.description ?? "-"}</td>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>
                          <span className={`status-chip ${item.enabled ? "status-chip-success" : ""}`}>
                            {item.enabled ? "enabled" : "disabled"}
                          </span>
                        </td>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>{item.order}</td>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>
                          <div className="toolbar-row" style={{ flexWrap: "wrap" }}>
                            <button onClick={() => toggleWidget(item.key)}>
                              {item.enabled ? "Disable" : "Enable"}
                            </button>
                            <button onClick={() => moveWidget(item.key, -1)} disabled={item.order <= 1}>
                              Move up
                            </button>
                            <button onClick={() => moveWidget(item.key, 1)} disabled={item.order >= dashboardLayout.length}>
                              Move down
                            </button>
                          </div>
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          </div>

          {dashboardWidgets.length > 0 && (
            <div className="detail-panel">
              <h3 style={{ ...subSectionTitleStyle, marginTop: 0, marginBottom: "0.45rem" }}>Widget preview</h3>
              <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(260px, 1fr))", gap: "0.65rem" }}>
                {dashboardWidgets.map((item) => (
                  <div key={`dashboard-widget-preview-${item.key}`} className="detail-panel">
                    <strong>{item.detail?.title ?? item.key}</strong>
                    <p className="inline-note">{item.detail?.description ?? "-"}</p>
                  </div>
                ))}
              </div>
            </div>
          )}
        </SectionCard>
      )}

      {visibleSections.has("section-daily-cockpit") && (
        <SectionCard
          id="section-daily-cockpit"
          title={t("cmdb.dailyCockpit.title")}
          actions={(
            <button
              onClick={() => void loadDailyCockpitSnapshot()}
              disabled={
                loadingDailyOpsBriefing ||
                loadingDailyOpsClosureContinuity ||
                loadingDailyCockpit ||
                loadingNextBestActions ||
                loadingOpsChecklist
              }
            >
              {loadingDailyOpsBriefing ||
              loadingDailyOpsClosureContinuity ||
              loadingDailyCockpit ||
              loadingNextBestActions ||
              loadingOpsChecklist
                ? t("cmdb.actions.loading")
                : t("cmdb.dailyCockpit.actions.refresh")}
            </button>
          )}
        >
          {dailyOpsNotice && <p className="banner banner-success">{dailyOpsNotice}</p>}
          {dailyCockpitNotice && <p className="banner banner-success">{dailyCockpitNotice}</p>}
          {opsChecklistNotice && <p className="banner banner-success">{opsChecklistNotice}</p>}
          {!canWriteCmdb && <p className="inline-note">{t("cmdb.dailyCockpit.messages.readOnlyHint")}</p>}

          <div className="filter-grid" style={{ marginBottom: "0.75rem" }}>
            <label className="control-field">
              <span>{t("cmdb.dailyCockpit.filters.site")}</span>
              <input
                value={dailyCockpitSiteFilter}
                onChange={(event) => setDailyCockpitSiteFilter(event.target.value)}
                placeholder="dc-a"
              />
            </label>
            <label className="control-field">
              <span>{t("cmdb.dailyCockpit.filters.department")}</span>
              <input
                value={dailyCockpitDepartmentFilter}
                onChange={(event) => setDailyCockpitDepartmentFilter(event.target.value)}
                placeholder="platform"
              />
            </label>
          </div>

          <div className="detail-panel" style={{ marginBottom: "0.85rem" }}>
            <div className="toolbar-row" style={{ justifyContent: "space-between" }}>
              <h3 style={{ ...subSectionTitleStyle, marginTop: 0, marginBottom: 0 }}>
                Ownership closure continuity
              </h3>
              <button
                onClick={() => void loadDailyOpsClosureContinuity()}
                disabled={loadingDailyOpsClosureContinuity}
              >
                {loadingDailyOpsClosureContinuity ? t("cmdb.actions.loading") : "Refresh closure continuity"}
              </button>
            </div>
            <p className="section-note">
              Carryover and escalation signals linked to daily follow-up task keys.
            </p>
            {!dailyOpsClosureContinuity ? (
              <p>{loadingDailyOpsClosureContinuity ? t("cmdb.actions.loading") : "No closure continuity snapshot yet."}</p>
            ) : (
              <>
                <div className="toolbar-row" style={{ flexWrap: "wrap", marginBottom: "0.65rem" }}>
                  <span className="status-chip status-chip-danger">carryover={dailyOpsClosureContinuity.summary.carryover_total}</span>
                  <span className="status-chip status-chip-warn">owner_gap={dailyOpsClosureContinuity.summary.owner_gap_total}</span>
                  <span className="status-chip">overdue={dailyOpsClosureContinuity.summary.overdue_total}</span>
                  <span className="status-chip">blocked={dailyOpsClosureContinuity.summary.blocked_total}</span>
                  <span className="status-chip status-chip-danger">
                    escalation_candidates={dailyOpsClosureContinuity.summary.escalation_candidate_total}
                  </span>
                  <span className="inline-note">
                    generated_at={new Date(dailyOpsClosureContinuity.generated_at).toLocaleString()}
                  </span>
                </div>
                {dailyOpsClosureContinuity.carryover_items.length === 0 ? (
                  <p className="inline-note">No carryover item remains for current scope.</p>
                ) : (
                  <div style={{ overflowX: "auto", marginBottom: "0.6rem" }}>
                    <table style={{ borderCollapse: "collapse", minWidth: "1040px", width: "100%" }}>
                      <thead>
                        <tr>
                          <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Task</th>
                          <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Owner</th>
                          <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>State</th>
                          <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Due / Escalate</th>
                        </tr>
                      </thead>
                      <tbody>
                        {dailyOpsClosureContinuity.carryover_items.map((item: any) => (
                          <tr key={`daily-ops-carryover-${item.task_key}`}>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              <div>{item.task_key}</div>
                              <div className="inline-note">{item.summary}</div>
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              <div>{item.owner?.owner_ref ?? "-"}</div>
                              <div className="inline-note">{item.owner?.owner_state ?? "-"}</div>
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              <span className={`status-chip ${item.status === "blocked" ? "status-chip-warn" : item.status === "overdue" ? "status-chip-danger" : ""}`}>
                                {item.status}
                              </span>
                              <div className="inline-note">priority={item.priority}</div>
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              <div>due={item.due_at ? new Date(item.due_at).toLocaleString() : "-"}</div>
                              <div>escalate={item.escalate_at ? new Date(item.escalate_at).toLocaleString() : "-"}</div>
                            </td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                )}
                {dailyOpsClosureContinuity.escalation_candidates.length > 0 && (
                  <div style={{ overflowX: "auto" }}>
                    <table style={{ borderCollapse: "collapse", minWidth: "980px", width: "100%" }}>
                      <thead>
                        <tr>
                          <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Escalation task</th>
                          <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Owner</th>
                          <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Trigger</th>
                          <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Policy</th>
                          <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Action</th>
                        </tr>
                      </thead>
                      <tbody>
                        {dailyOpsClosureContinuity.escalation_candidates.map((item: any) => (
                          <tr key={`daily-ops-escalation-${item.task_key}`}>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              <div>{item.task_key}</div>
                              <div className="inline-note">{item.status} / {item.priority}</div>
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              {item.owner_ref}
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              <div>{item.trigger_state}</div>
                              <div className="inline-note">{item.trigger_reason}</div>
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              <div>{item.due_policy?.policy_key ?? "-"}</div>
                              <div className="inline-note">
                                due={item.due_policy?.due_window_minutes ?? "-"}m / escalation={item.due_policy?.escalation_window_minutes ?? "-"}m
                              </div>
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              {(() => {
                                const running = runningDailyOpsFollowUpActionKey === `${item.task_key}:escalation-action`;
                                return (
                                  <>
                                    <button
                                      onClick={() => void applyDailyOpsEscalationAction(item)}
                                      disabled={!canWriteCmdb || running || runningDailyOpsFollowUpActionKey !== null}
                                    >
                                      {running ? t("cmdb.actions.loading") : "Escalate now"}
                                    </button>
                                    {!canWriteCmdb && <div className="inline-note">read-only</div>}
                                  </>
                                );
                              })()}
                            </td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                )}
              </>
            )}
          </div>

          <div className="detail-panel" style={{ marginBottom: "0.85rem" }}>
            <div className="toolbar-row" style={{ justifyContent: "space-between" }}>
              <h3 style={{ ...subSectionTitleStyle, marginTop: 0, marginBottom: 0 }}>Daily ops return loop</h3>
              <button
                onClick={() => void Promise.all([loadDailyOpsBriefing(), loadDailyOpsClosureContinuity()])}
                disabled={loadingDailyOpsBriefing || loadingDailyOpsClosureContinuity}
              >
                {loadingDailyOpsBriefing || loadingDailyOpsClosureContinuity
                  ? t("cmdb.actions.loading")
                  : "Refresh daily ops"}
              </button>
            </div>
            <p className="section-note">
              One prioritized queue for today, overdue work, activation leftovers, and go-live drift.
            </p>

            {!dailyOpsBriefing ? (
              <p>{loadingDailyOpsBriefing ? t("cmdb.actions.loading") : "No daily ops briefing yet."}</p>
            ) : dailyOpsBriefing.items.length === 0 ? (
              <p>No active daily follow-up item is visible in the current scope.</p>
            ) : (
              <>
                <div className="toolbar-row" style={{ flexWrap: "wrap", marginBottom: "0.65rem" }}>
                  <span className="status-chip status-chip-danger">overdue={dailyOpsBriefing.summary.overdue}</span>
                  <span className="status-chip status-chip-warn">blocked={dailyOpsBriefing.summary.blocked}</span>
                  <span className="status-chip">due_today={dailyOpsBriefing.summary.due_today}</span>
                  <span className="status-chip status-chip-success">completed={dailyOpsBriefing.summary.completed}</span>
                  <span className="status-chip">deferred={dailyOpsBriefing.summary.deferred}</span>
                  <span className="inline-note">
                    recommended_next={dailyOpsBriefing.recommended_next_task_key ?? "none"}
                  </span>
                </div>
                <div className="toolbar-row" style={{ flexWrap: "wrap", marginBottom: "0.65rem" }}>
                  <span className="inline-note">critical={dailyOpsBriefing.summary.critical}</span>
                  <span className="inline-note">high={dailyOpsBriefing.summary.high}</span>
                  <span className="inline-note">medium={dailyOpsBriefing.summary.medium}</span>
                  <span className="inline-note">low={dailyOpsBriefing.summary.low}</span>
                  <span className="inline-note">acknowledged={dailyOpsBriefing.summary.acknowledged}</span>
                  <span className="inline-note">
                    generated_at={new Date(dailyOpsBriefing.generated_at).toLocaleString()}
                  </span>
                </div>
                <div style={{ overflowX: "auto" }}>
                  <table style={{ borderCollapse: "collapse", minWidth: "1380px", width: "100%" }}>
                    <thead>
                      <tr>
                        <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Task</th>
                        <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Status</th>
                        <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Priority</th>
                        <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Owner</th>
                        <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Reason</th>
                        <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Timing</th>
                        <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Recommended</th>
                        <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Follow-up</th>
                      </tr>
                    </thead>
                    <tbody>
                      {dailyOpsBriefing.items.map((item: any) => {
                        const recommended = item.recommended_action;
                        const statusClass = item.status === "overdue"
                          ? "status-chip-danger"
                          : item.status === "blocked"
                            ? "status-chip-warn"
                            : item.status === "completed"
                              ? "status-chip-success"
                              : "";
                        return (
                          <tr key={`daily-ops-${item.task_key}`}>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              <strong>{item.summary}</strong>
                              <div className="inline-note">task_key={item.task_key}</div>
                              <div className="inline-note">domain={item.domain} / type={item.item_type}</div>
                              <div className="inline-note">follow_up_state={item.follow_up_state}</div>
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              <span className={`status-chip ${statusClass}`}>{item.status}</span>
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              <span className={`status-chip ${item.priority === "critical" ? "status-chip-danger" : item.priority === "high" ? "status-chip-warn" : ""}`}>
                                {item.priority}
                              </span>
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              <div>{item.owner?.owner_ref ?? "-"}</div>
                              <div className="inline-note">state={item.owner?.owner_state ?? "unknown"}</div>
                              <div className="inline-note">source={item.owner?.source ?? "-"}</div>
                              <div className="inline-note">{item.owner?.reason ?? "-"}</div>
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              {item.reason}
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              <div>observed={new Date(item.observed_at).toLocaleString()}</div>
                              <div>due={item.due_at ? new Date(item.due_at).toLocaleString() : "-"}</div>
                              <div>escalate={item.escalate_at ? new Date(item.escalate_at).toLocaleString() : "-"}</div>
                              <div className="inline-note">
                                due_policy={item.due_policy?.policy_key ?? "-"} ({item.due_policy?.due_window_minutes ?? "-"}m/{item.due_policy?.escalation_window_minutes ?? "-"}m)
                              </div>
                              <div>deferred_until={item.deferred_until ? new Date(item.deferred_until).toLocaleString() : "-"}</div>
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              {recommended ? (
                                <>
                                  <div>{recommended.description}</div>
                                  {recommended.href ? (
                                    <button
                                      style={{ marginTop: "0.45rem" }}
                                      onClick={() => {
                                        if (typeof window !== "undefined") {
                                          window.location.hash = recommended.href;
                                        }
                                      }}
                                    >
                                      {recommended.label}
                                    </button>
                                  ) : (
                                    <span className="inline-note">{recommended.label}</span>
                                  )}
                                </>
                              ) : (
                                <span className="inline-note">No recommended action</span>
                              )}
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              <div className="toolbar-row" style={{ flexWrap: "wrap" }}>
                                {(item.available_actions ?? []).map((action: any) => {
                                  const running = runningDailyOpsFollowUpActionKey === `${item.task_key}:${action.action_key}`;
                                  return (
                                    <button
                                      key={`daily-ops-action-${item.task_key}-${action.action_key}`}
                                      onClick={() => void applyDailyOpsFollowUpAction(item, action)}
                                      disabled={!canWriteCmdb || running || runningDailyOpsFollowUpActionKey !== null}
                                    >
                                      {running ? t("cmdb.actions.loading") : action.label}
                                    </button>
                                  );
                                })}
                                <button
                                  onClick={() => void applyDailyOpsOwnerAssignment(item)}
                                  disabled={
                                    !canWriteCmdb ||
                                    runningDailyOpsFollowUpActionKey !== null
                                  }
                                >
                                  Update owner
                                </button>
                                {!canWriteCmdb && <span className="inline-note">read-only</span>}
                              </div>
                            </td>
                          </tr>
                        );
                      })}
                    </tbody>
                  </table>
                </div>
              </>
            )}
          </div>

          <div className="detail-panel" style={{ marginBottom: "0.85rem" }}>
            <div className="toolbar-row" style={{ justifyContent: "space-between" }}>
              <h3 style={{ ...subSectionTitleStyle, marginTop: 0, marginBottom: 0 }}>Next best actions</h3>
              <button onClick={() => void loadNextBestActions()} disabled={loadingNextBestActions}>
                {loadingNextBestActions ? t("cmdb.actions.loading") : "Refresh next actions"}
              </button>
            </div>
            <p className="section-note">
              Deterministic suggestions for unresolved incident/escalation/handover risks.
            </p>
            {!nextBestActions ? (
              <p>{loadingNextBestActions ? t("cmdb.actions.loading") : "No suggestions yet."}</p>
            ) : nextBestActions.items.length === 0 ? (
              <p>No unresolved risk item needs follow-up in current scope.</p>
            ) : (
              <>
                <p className="section-note">
                  generated_at={new Date(nextBestActions.generated_at).toLocaleString()} | shift_date={nextBestActions.shift_date} | total={nextBestActions.total}
                </p>
                <div style={{ overflowX: "auto" }}>
                  <table style={{ borderCollapse: "collapse", minWidth: "1220px", width: "100%" }}>
                    <thead>
                      <tr>
                        <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Suggestion</th>
                        <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Priority</th>
                        <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Reason</th>
                        <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Source signal</th>
                        <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Observed</th>
                        <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Action</th>
                      </tr>
                    </thead>
                    <tbody>
                      {nextBestActions.items.slice(0, 40).map((item: any) => {
                        const actionKey = `${item.suggestion_key}:${item.action.key}`;
                        const running = runningDailyCockpitActionKey === actionKey;
                        const disabled = running || (item.action.requires_write && !canWriteCmdb);
                        return (
                          <tr key={`next-best-action-${item.suggestion_key}`}>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              <strong>{item.suggestion_key}</strong>
                              <div className="inline-note">domain={item.domain}</div>
                              <div className="inline-note">risk={item.risk_level}</div>
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              <span className={`status-chip ${item.risk_level === "critical" ? "status-chip-danger" : item.risk_level === "high" ? "status-chip-warn" : ""}`}>
                                {item.priority_score}
                              </span>
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              {item.reason}
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              {item.source_signal}
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              {new Date(item.observed_at).toLocaleString()}
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              <button
                                onClick={() => void runDailyCockpitAction(item.suggestion_key, item.action)}
                                disabled={disabled}
                              >
                                {running ? t("cmdb.actions.loading") : item.action.label}
                              </button>
                            </td>
                          </tr>
                        );
                      })}
                    </tbody>
                  </table>
                </div>
              </>
            )}
          </div>

          <div className="detail-panel" style={{ marginBottom: "0.85rem" }}>
            <h3 style={{ ...subSectionTitleStyle, marginTop: 0 }}>{t("cmdb.dailyCockpit.checklist.title")}</h3>
            <p className="section-note">{t("cmdb.dailyCockpit.checklist.description")}</p>
            <div className="toolbar-row" style={{ marginBottom: "0.65rem" }}>
              <label className="control-field" style={{ minWidth: "220px" }}>
                <span>{t("cmdb.dailyCockpit.checklist.fields.date")}</span>
                <input
                  type="date"
                  value={opsChecklistDate}
                  onChange={(event) => setOpsChecklistDate(event.target.value)}
                />
              </label>
              <button onClick={() => void loadOpsChecklist()} disabled={loadingOpsChecklist}>
                {loadingOpsChecklist
                  ? t("cmdb.actions.loading")
                  : t("cmdb.dailyCockpit.checklist.actions.refresh")}
              </button>
            </div>

            {!opsChecklist ? (
              <p>
                {loadingOpsChecklist
                  ? t("cmdb.dailyCockpit.checklist.messages.loading")
                  : t("cmdb.dailyCockpit.checklist.messages.noData")}
              </p>
            ) : opsChecklist.items.length === 0 ? (
              <p>{t("cmdb.dailyCockpit.checklist.messages.empty")}</p>
            ) : (
              <>
                <p className="section-note">
                  {t("cmdb.dailyCockpit.checklist.summary", {
                    total: opsChecklist.summary.total,
                    completed: opsChecklist.summary.completed,
                    pending: opsChecklist.summary.pending,
                    skipped: opsChecklist.summary.skipped,
                    overdue: opsChecklist.summary.overdue
                  })}
                </p>
                <div className="toolbar-row" style={{ marginBottom: "0.5rem" }}>
                  <span className="status-chip status-chip-success">
                    {t("cmdb.dailyCockpit.checklist.cards.completed", { value: opsChecklist.summary.completed })}
                  </span>
                  <span className="status-chip">
                    {t("cmdb.dailyCockpit.checklist.cards.pending", { value: opsChecklist.summary.pending })}
                  </span>
                  <span className="status-chip status-chip-warn">
                    {t("cmdb.dailyCockpit.checklist.cards.skipped", { value: opsChecklist.summary.skipped })}
                  </span>
                  <span className="status-chip status-chip-danger">
                    {t("cmdb.dailyCockpit.checklist.cards.overdue", { value: opsChecklist.summary.overdue })}
                  </span>
                </div>
                <div style={{ overflowX: "auto" }}>
                  <table style={{ borderCollapse: "collapse", minWidth: "1120px", width: "100%" }}>
                    <thead>
                      <tr>
                        <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>
                          {t("cmdb.dailyCockpit.checklist.table.item")}
                        </th>
                        <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>
                          {t("cmdb.dailyCockpit.checklist.table.frequency")}
                        </th>
                        <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>
                          {t("cmdb.dailyCockpit.checklist.table.status")}
                        </th>
                        <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>
                          {t("cmdb.dailyCockpit.checklist.table.note")}
                        </th>
                        <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>
                          {t("cmdb.dailyCockpit.checklist.table.updatedAt")}
                        </th>
                        <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>
                          {t("cmdb.dailyCockpit.checklist.table.actions")}
                        </th>
                      </tr>
                    </thead>
                    <tbody>
                      {opsChecklist.items.map((item: any) => {
                        const statusClass = item.status === "completed"
                          ? "status-chip-success"
                          : item.status === "skipped"
                            ? "status-chip-warn"
                            : item.overdue
                              ? "status-chip-danger"
                              : "";
                        const runningComplete = runningOpsChecklistActionKey === `${item.template_key}:complete`;
                        const runningException = runningOpsChecklistActionKey === `${item.template_key}:exception`;
                        return (
                          <tr key={`ops-checklist-${item.template_key}`}>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              <p style={{ margin: 0 }}>{item.title}</p>
                              {item.description && <p className="section-note" style={{ marginTop: "0.25rem" }}>{item.description}</p>}
                              {item.guidance && <p className="inline-note" style={{ marginTop: "0.2rem" }}>{item.guidance}</p>}
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              {item.frequency}
                              {item.frequency === "weekly" && Number.isFinite(item.due_weekday)
                                ? ` / ${t("cmdb.dailyCockpit.checklist.weekday", { day: item.due_weekday })}`
                                : ""}
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              <span className={`status-chip ${statusClass}`}>
                                {item.status === "completed"
                                  ? t("cmdb.dailyCockpit.checklist.status.completed")
                                  : item.status === "skipped"
                                    ? t("cmdb.dailyCockpit.checklist.status.skipped")
                                    : item.overdue
                                      ? t("cmdb.dailyCockpit.checklist.status.overdue")
                                      : t("cmdb.dailyCockpit.checklist.status.pending")}
                              </span>
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              {item.exception_note ?? "-"}
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              {item.updated_at ? new Date(item.updated_at).toLocaleString() : "-"}
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              <div className="toolbar-row">
                                <button
                                  onClick={() => void completeOpsChecklistItem(item.template_key)}
                                  disabled={!canWriteCmdb || runningComplete || runningOpsChecklistActionKey !== null}
                                >
                                  {runningComplete
                                    ? t("cmdb.actions.loading")
                                    : t("cmdb.dailyCockpit.checklist.actions.complete")}
                                </button>
                                <button
                                  onClick={() => void recordOpsChecklistException(item.template_key)}
                                  disabled={!canWriteCmdb || runningException || runningOpsChecklistActionKey !== null}
                                >
                                  {runningException
                                    ? t("cmdb.actions.loading")
                                    : t("cmdb.dailyCockpit.checklist.actions.exception")}
                                </button>
                              </div>
                            </td>
                          </tr>
                        );
                      })}
                    </tbody>
                  </table>
                </div>
              </>
            )}
          </div>

          <div className="detail-panel" style={{ marginBottom: "0.85rem" }}>
            <div className="toolbar-row" style={{ justifyContent: "space-between" }}>
              <h3 style={{ ...subSectionTitleStyle, marginTop: 0, marginBottom: 0 }}>Incident command</h3>
              <div className="toolbar-row">
                <button onClick={() => void loadIncidentCommands()} disabled={loadingIncidentCommands}>
                  {loadingIncidentCommands ? t("cmdb.actions.loading") : "Refresh incidents"}
                </button>
                <button
                  onClick={() => {
                    if (selectedIncidentAlertId.trim().length > 0) {
                      void loadIncidentCommandDetail(selectedIncidentAlertId);
                    }
                  }}
                  disabled={loadingIncidentCommandDetail || selectedIncidentAlertId.trim().length === 0}
                >
                  {loadingIncidentCommandDetail ? t("cmdb.actions.loading") : "Refresh timeline"}
                </button>
              </div>
            </div>
            {incidentCommandNotice && <p className="banner banner-success">{incidentCommandNotice}</p>}
            <p className="section-note">
              Track incident owner, ETA, blocker, and status transitions with auditable timeline.
            </p>

            <div className="form-grid">
              <label className="control-field">
                <span>Alert</span>
                <select
                  value={selectedIncidentAlertId}
                  onChange={(event) => {
                    const value = event.target.value;
                    setSelectedIncidentAlertId(value);
                    const selected = (incidentCommands as any[]).find((item) => String(item.alert_id) === value);
                    if (!selected) {
                      return;
                    }
                    setIncidentCommandDraft((prev: any) => ({
                      ...prev,
                      alert_id: String(selected.alert_id),
                      status: selected.command_status,
                      owner: selected.command_owner ?? "",
                      eta_at: selected.eta_at ?? "",
                      blocker: selected.blocker ?? "",
                      summary: selected.summary ?? ""
                    }));
                  }}
                >
                  {(incidentCommands as any[]).map((item) => (
                    <option key={`incident-alert-${item.alert_id}`} value={String(item.alert_id)}>
                      #{item.alert_id} [{item.severity}] {item.title}
                    </option>
                  ))}
                </select>
              </label>
              <label className="control-field">
                <span>Status</span>
                <select
                  value={incidentCommandDraft.status}
                  onChange={(event) => setIncidentCommandDraft((prev: any) => ({ ...prev, status: event.target.value }))}
                >
                  <option value="triage">triage</option>
                  <option value="in_progress">in_progress</option>
                  <option value="blocked">blocked</option>
                  <option value="mitigated">mitigated</option>
                  <option value="postmortem">postmortem</option>
                </select>
              </label>
              <label className="control-field">
                <span>Owner</span>
                <input
                  value={incidentCommandDraft.owner}
                  onChange={(event) => setIncidentCommandDraft((prev: any) => ({ ...prev, owner: event.target.value }))}
                  placeholder="operator-id"
                />
              </label>
              <label className="control-field">
                <span>ETA (RFC3339)</span>
                <input
                  value={incidentCommandDraft.eta_at}
                  onChange={(event) => setIncidentCommandDraft((prev: any) => ({ ...prev, eta_at: event.target.value }))}
                  placeholder="2026-03-07T10:00:00Z"
                />
              </label>
              <label className="control-field" style={{ gridColumn: "1 / -1" }}>
                <span>Summary</span>
                <input
                  value={incidentCommandDraft.summary}
                  onChange={(event) => setIncidentCommandDraft((prev: any) => ({ ...prev, summary: event.target.value }))}
                  placeholder="Current incident summary"
                />
              </label>
              <label className="control-field" style={{ gridColumn: "1 / -1" }}>
                <span>Blocker</span>
                <input
                  value={incidentCommandDraft.blocker}
                  onChange={(event) => setIncidentCommandDraft((prev: any) => ({ ...prev, blocker: event.target.value }))}
                  placeholder="Blocking dependency"
                />
              </label>
              <label className="control-field" style={{ gridColumn: "1 / -1" }}>
                <span>Note</span>
                <input
                  value={incidentCommandDraft.note}
                  onChange={(event) => setIncidentCommandDraft((prev: any) => ({ ...prev, note: event.target.value }))}
                  placeholder="State change note"
                />
              </label>
            </div>

            <div className="toolbar-row" style={{ marginTop: "0.65rem" }}>
              <button onClick={() => void saveIncidentCommand()} disabled={!canWriteCmdb || savingIncidentCommand}>
                {savingIncidentCommand ? t("cmdb.actions.loading") : "Save incident command"}
              </button>
            </div>

            {(incidentCommands as any[]).length > 0 && (
              <div style={{ overflowX: "auto", marginTop: "0.65rem" }}>
                <table style={{ borderCollapse: "collapse", minWidth: "1050px", width: "100%" }}>
                  <thead>
                    <tr>
                      <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Alert</th>
                      <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Command</th>
                      <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Owner/ETA</th>
                      <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Scope</th>
                      <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Updated</th>
                    </tr>
                  </thead>
                  <tbody>
                    {(incidentCommands as any[]).slice(0, 20).map((item) => (
                      <tr key={`incident-command-${item.alert_id}`}>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                          #{item.alert_id} [{item.severity}] {item.title}
                        </td>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                          {item.command_status}
                          {item.summary ? ` | ${item.summary}` : ""}
                          {item.blocker ? ` | blocker=${item.blocker}` : ""}
                        </td>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                          {item.command_owner}
                          {item.eta_at ? ` @ ${new Date(item.eta_at).toLocaleString()}` : ""}
                        </td>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                          {item.site ?? "-"} / {item.department ?? "-"}
                        </td>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                          {new Date(item.updated_at).toLocaleString()}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}

            <h4 style={{ marginBottom: "0.4rem", marginTop: "0.8rem" }}>Incident timeline</h4>
            {loadingIncidentCommandDetail ? (
              <p>{t("cmdb.actions.loading")}</p>
            ) : !incidentCommandDetail ? (
              <p>No incident timeline selected.</p>
            ) : (incidentCommandDetail.timeline ?? []).length === 0 ? (
              <p>No timeline event yet.</p>
            ) : (
              <div style={{ overflowX: "auto" }}>
                <table style={{ borderCollapse: "collapse", minWidth: "980px", width: "100%" }}>
                  <thead>
                    <tr>
                      <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Event</th>
                      <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Status</th>
                      <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Owner/ETA</th>
                      <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Note</th>
                      <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Time</th>
                    </tr>
                  </thead>
                  <tbody>
                    {(incidentCommandDetail.timeline ?? []).slice(0, 20).map((event: any) => (
                      <tr key={`incident-event-${event.id}`}>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                          {event.event_type} by {event.actor}
                        </td>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                          {event.from_status ? `${event.from_status} -> ${event.to_status}` : event.to_status}
                        </td>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                          {event.command_owner}
                          {event.eta_at ? ` @ ${new Date(event.eta_at).toLocaleString()}` : ""}
                        </td>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                          {event.note ?? event.summary ?? event.blocker ?? "-"}
                        </td>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                          {new Date(event.created_at).toLocaleString()}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </div>

          <div className="detail-panel" style={{ marginBottom: "0.85rem" }}>
            <div className="toolbar-row" style={{ justifyContent: "space-between" }}>
              <h3 style={{ ...subSectionTitleStyle, marginTop: 0, marginBottom: 0 }}>One-click runbook templates</h3>
              <div className="toolbar-row">
                <button onClick={() => void loadRunbookTemplates()} disabled={loadingRunbookTemplates}>
                  {loadingRunbookTemplates ? t("cmdb.actions.loading") : "Refresh templates"}
                </button>
                <button onClick={() => void loadRunbookExecutionPolicy()} disabled={loadingRunbookExecutionPolicy}>
                  {loadingRunbookExecutionPolicy ? t("cmdb.actions.loading") : "Refresh execution policy"}
                </button>
                <button onClick={() => void loadRunbookAnalyticsPolicy()} disabled={loadingRunbookAnalyticsPolicy}>
                  {loadingRunbookAnalyticsPolicy ? t("cmdb.actions.loading") : "Refresh risk policy"}
                </button>
                <button onClick={() => void loadRunbookTemplateExecutions()} disabled={loadingRunbookExecutions}>
                  {loadingRunbookExecutions ? t("cmdb.actions.loading") : "Refresh executions"}
                </button>
                <button onClick={() => void loadRunbookExecutionPresets()} disabled={loadingRunbookPresets}>
                  {loadingRunbookPresets ? t("cmdb.actions.loading") : "Refresh presets"}
                </button>
                <button onClick={() => void loadRunbookAnalyticsSummary()} disabled={loadingRunbookAnalyticsSummary}>
                  {loadingRunbookAnalyticsSummary ? t("cmdb.actions.loading") : "Refresh analytics"}
                </button>
                <button onClick={() => void loadRunbookRiskAlerts()} disabled={loadingRunbookRiskAlerts}>
                  {loadingRunbookRiskAlerts ? t("cmdb.actions.loading") : "Refresh risk alerts"}
                </button>
                <button onClick={() => void loadRunbookFailureFeed()} disabled={loadingRunbookFailureFeed}>
                  {loadingRunbookFailureFeed ? t("cmdb.actions.loading") : "Refresh failure feed"}
                </button>
              </div>
            </div>

            {runbookNotice && <p className="banner banner-success">{runbookNotice}</p>}
            <p className="section-note">
              Use guided runbook templates with required preflight checklist and evidence closure.
            </p>
            <div className="detail-panel" style={{ marginBottom: "0.55rem" }}>
              <p className="section-note" style={{ marginTop: 0, marginBottom: "0.35rem" }}>
                execution policy={runbookExecutionPolicy?.mode ?? "not-loaded"}
                {" | "}live_templates={(runbookExecutionPolicy?.live_templates ?? []).length}
                {" | "}updated_by={runbookExecutionPolicy?.updated_by ?? "-"}
              </p>
              <div className="form-grid">
                <label className="control-field">
                  <span>Policy mode</span>
                  <select
                    value={runbookExecutionPolicyDraft.mode}
                    onChange={(event) => setRunbookExecutionPolicyDraft((prev: any) => ({
                      ...prev,
                      mode: event.target.value
                    }))}
                    disabled={!canWriteCmdb}
                  >
                    <option value="simulate_only">simulate_only</option>
                    <option value="hybrid_live">hybrid_live</option>
                  </select>
                </label>
                <label className="control-field">
                  <span>Live templates (csv)</span>
                  <input
                    value={runbookExecutionPolicyDraft.live_templates_csv}
                    onChange={(event) => setRunbookExecutionPolicyDraft((prev: any) => ({
                      ...prev,
                      live_templates_csv: event.target.value
                    }))}
                    disabled={!canWriteCmdb}
                    placeholder="dependency-check"
                  />
                </label>
                <label className="control-field">
                  <span>Max live step timeout (seconds)</span>
                  <input
                    type="number"
                    min={1}
                    max={120}
                    value={runbookExecutionPolicyDraft.max_live_step_timeout_seconds}
                    onChange={(event) => setRunbookExecutionPolicyDraft((prev: any) => ({
                      ...prev,
                      max_live_step_timeout_seconds: event.target.value
                    }))}
                    disabled={!canWriteCmdb}
                  />
                </label>
                <label className="control-field">
                  <span>Allow simulate_failure_step</span>
                  <select
                    value={runbookExecutionPolicyDraft.allow_simulate_failure ? "true" : "false"}
                    onChange={(event) => setRunbookExecutionPolicyDraft((prev: any) => ({
                      ...prev,
                      allow_simulate_failure: event.target.value === "true"
                    }))}
                    disabled={!canWriteCmdb}
                  >
                    <option value="true">true</option>
                    <option value="false">false</option>
                  </select>
                </label>
                <label className="control-field" style={{ gridColumn: "1 / -1" }}>
                  <span>Policy note</span>
                  <input
                    value={runbookExecutionPolicyDraft.note}
                    onChange={(event) => setRunbookExecutionPolicyDraft((prev: any) => ({
                      ...prev,
                      note: event.target.value
                    }))}
                    disabled={!canWriteCmdb}
                    placeholder="policy change context"
                  />
                </label>
              </div>
              <div className="toolbar-row" style={{ marginTop: "0.45rem" }}>
                <button onClick={() => void saveRunbookExecutionPolicy()} disabled={!canWriteCmdb || savingRunbookExecutionPolicy}>
                  {savingRunbookExecutionPolicy ? t("cmdb.actions.loading") : "Save execution policy"}
                </button>
              </div>
            </div>

            <div className="detail-panel" style={{ marginBottom: "0.55rem" }}>
              <p className="section-note" style={{ marginTop: 0, marginBottom: "0.35rem" }}>
                risk policy={runbookAnalyticsPolicy?.policy_key ?? "not-loaded"}
                {" | "}failure_rate_threshold={runbookAnalyticsPolicy?.failure_rate_threshold_percent ?? "-"}%
                {" | "}minimum_sample_size={runbookAnalyticsPolicy?.minimum_sample_size ?? "-"}
                {" | "}updated_by={runbookAnalyticsPolicy?.updated_by ?? "-"}
              </p>
              <div className="form-grid">
                <label className="control-field">
                  <span>Failure rate threshold (%)</span>
                  <input
                    type="number"
                    min={1}
                    max={100}
                    value={runbookAnalyticsPolicyDraft.failure_rate_threshold_percent}
                    onChange={(event) => setRunbookAnalyticsPolicyDraft((prev: any) => ({
                      ...prev,
                      failure_rate_threshold_percent: event.target.value
                    }))}
                    disabled={!canWriteCmdb}
                  />
                </label>
                <label className="control-field">
                  <span>Minimum sample size</span>
                  <input
                    type="number"
                    min={1}
                    max={500}
                    value={runbookAnalyticsPolicyDraft.minimum_sample_size}
                    onChange={(event) => setRunbookAnalyticsPolicyDraft((prev: any) => ({
                      ...prev,
                      minimum_sample_size: event.target.value
                    }))}
                    disabled={!canWriteCmdb}
                  />
                </label>
                <label className="control-field" style={{ gridColumn: "1 / -1" }}>
                  <span>Risk policy note</span>
                  <input
                    value={runbookAnalyticsPolicyDraft.note}
                    onChange={(event) => setRunbookAnalyticsPolicyDraft((prev: any) => ({
                      ...prev,
                      note: event.target.value
                    }))}
                    disabled={!canWriteCmdb}
                    placeholder="analytics risk policy context"
                  />
                </label>
              </div>
              <div className="toolbar-row" style={{ marginTop: "0.45rem" }}>
                <button onClick={() => void saveRunbookAnalyticsPolicy()} disabled={!canWriteCmdb || savingRunbookAnalyticsPolicy}>
                  {savingRunbookAnalyticsPolicy ? t("cmdb.actions.loading") : "Save risk policy"}
                </button>
              </div>
            </div>

            <div className="detail-panel" style={{ marginBottom: "0.55rem" }}>
              <p className="section-note" style={{ marginTop: 0, marginBottom: "0.35rem" }}>
                analytics filter window_days={runbookAnalyticsFilterDraft.days}
                {" | "}mode={runbookAnalyticsFilterDraft.execution_mode}
                {" | "}template={selectedRunbookTemplateKey || "all"}
              </p>
              <div className="form-grid">
                <label className="control-field">
                  <span>Analytics days (1-90)</span>
                  <input
                    type="number"
                    min={1}
                    max={90}
                    value={runbookAnalyticsFilterDraft.days}
                    onChange={(event) => setRunbookAnalyticsFilterDraft((prev: any) => ({
                      ...prev,
                      days: event.target.value
                    }))}
                  />
                </label>
                <label className="control-field">
                  <span>Analytics mode filter</span>
                  <select
                    value={runbookAnalyticsFilterDraft.execution_mode}
                    onChange={(event) => setRunbookAnalyticsFilterDraft((prev: any) => ({
                      ...prev,
                      execution_mode: event.target.value
                    }))}
                  >
                    <option value="all">all</option>
                    <option value="simulate">simulate</option>
                    <option value="live">live</option>
                  </select>
                </label>
              </div>
            </div>

            {(runbookTemplates as any[]).length === 0 ? (
              <p>No runbook template loaded.</p>
            ) : (
              <>
                <div className="form-grid">
                  <label className="control-field">
                    <span>Template</span>
                    <select
                      value={selectedRunbookTemplateKey}
                      onChange={(event) => {
                        const nextKey = event.target.value;
                        setSelectedRunbookTemplateKey(nextKey);
                      }}
                    >
                      {(runbookTemplates as any[]).map((item) => (
                        <option key={`runbook-template-${item.key}`} value={item.key}>
                          {item.name}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label className="control-field">
                    <span>Category</span>
                    <input value={selectedRunbookTemplate?.category ?? "-"} readOnly />
                  </label>
                  <label className="control-field">
                    <span>Supported modes</span>
                    <input value={(selectedRunbookTemplate?.execution_modes ?? []).join(", ") || "simulate"} readOnly />
                  </label>
                  <label className="control-field">
                    <span>Execution mode</span>
                    <select
                      value={runbookExecutionMode}
                      onChange={(event) => setRunbookExecutionMode(event.target.value)}
                      disabled={!canWriteCmdb}
                    >
                      {((selectedRunbookTemplate?.execution_modes ?? ["simulate"]) as string[]).map((mode: string) => (
                        <option key={`runbook-execution-mode-${mode}`} value={mode}>
                          {mode}
                        </option>
                      ))}
                    </select>
                  </label>
                </div>

                <div className="detail-panel" style={{ marginTop: "0.55rem", marginBottom: "0.55rem" }}>
                  <p className="section-note" style={{ marginTop: 0, marginBottom: "0.35rem" }}>
                    Execution presets (template scoped): {(runbookPresets as any[]).length}
                  </p>
                  <div className="form-grid">
                    <label className="control-field">
                      <span>Preset</span>
                      <select
                        value={selectedRunbookPresetId}
                        onChange={(event) => setSelectedRunbookPresetId(event.target.value)}
                      >
                        <option value="">-- select preset --</option>
                        {(runbookPresets as any[]).map((item) => (
                          <option key={`runbook-preset-${item.id}`} value={String(item.id)}>
                            {item.name}
                          </option>
                        ))}
                      </select>
                    </label>
                    <label className="control-field">
                      <span>Preset name</span>
                      <input
                        value={runbookPresetDraft.name}
                        onChange={(event) => setRunbookPresetDraft((prev: any) => ({
                          ...prev,
                          name: event.target.value
                        }))}
                        placeholder="dependency-baseline"
                        disabled={!canWriteCmdb}
                      />
                    </label>
                    <label className="control-field" style={{ gridColumn: "1 / -1" }}>
                      <span>Preset description</span>
                      <input
                        value={runbookPresetDraft.description}
                        onChange={(event) => setRunbookPresetDraft((prev: any) => ({
                          ...prev,
                          description: event.target.value
                        }))}
                        placeholder="validated by ops oncall"
                        disabled={!canWriteCmdb}
                      />
                    </label>
                  </div>
                  <div className="toolbar-row" style={{ marginTop: "0.45rem" }}>
                    <button
                      onClick={() => {
                        const preset = (runbookPresets as any[]).find((item) => String(item.id) === selectedRunbookPresetId) ?? null;
                        if (preset) {
                          applyRunbookExecutionPreset(preset);
                        }
                      }}
                      disabled={selectedRunbookPresetId.length === 0}
                    >
                      Apply preset
                    </button>
                    <button onClick={() => void createRunbookExecutionPreset()} disabled={!canWriteCmdb || savingRunbookPreset}>
                      {savingRunbookPreset ? t("cmdb.actions.loading") : "Save current as preset"}
                    </button>
                    <button onClick={() => void deleteRunbookExecutionPreset()} disabled={!canWriteCmdb || savingRunbookPreset || selectedRunbookPresetId.length === 0}>
                      {savingRunbookPreset ? t("cmdb.actions.loading") : "Delete preset"}
                    </button>
                  </div>
                </div>

                {selectedRunbookTemplate && (
                  <>
                    <p className="section-note">{selectedRunbookTemplate.description}</p>

                    <div className="form-grid">
                      {(selectedRunbookTemplate.params ?? []).map((field: any) => (
                        <label key={`runbook-param-${field.key}`} className="control-field">
                          <span>{field.label}{field.required ? " *" : ""}</span>
                          {field.field_type === "enum" ? (
                            <select
                              value={runbookParamDraft[field.key] ?? ""}
                              onChange={(event) => setRunbookParamDraft((prev: any) => ({
                                ...prev,
                                [field.key]: event.target.value
                              }))}
                            >
                              <option value="">-- select --</option>
                              {(field.options ?? []).map((option: any) => (
                                <option key={`runbook-param-option-${field.key}-${option}`} value={option}>
                                  {option}
                                </option>
                              ))}
                            </select>
                          ) : (
                            <input
                              type={field.field_type === "number" ? "number" : "text"}
                              min={field.min_value ?? undefined}
                              max={field.max_value ?? undefined}
                              value={runbookParamDraft[field.key] ?? ""}
                              onChange={(event) => setRunbookParamDraft((prev: any) => ({
                                ...prev,
                                [field.key]: event.target.value
                              }))}
                              placeholder={field.placeholder ?? ""}
                            />
                          )}
                        </label>
                      ))}
                    </div>

                    <div className="detail-panel" style={{ marginBottom: "0.55rem" }}>
                      <p className="section-note" style={{ marginTop: 0, marginBottom: "0.35rem" }}>
                        preflight checklist
                      </p>
                      {(selectedRunbookTemplate.preflight ?? []).map((item: any) => (
                        <label key={`runbook-preflight-${item.key}`} className="inline-note" style={{ display: "block" }}>
                          <input
                            type="checkbox"
                            checked={runbookPreflightDraft[item.key] ?? false}
                            onChange={(event) => setRunbookPreflightDraft((prev: any) => ({
                              ...prev,
                              [item.key]: event.target.checked
                            }))}
                          />
                          {" "}
                          {item.label} - {item.detail}
                        </label>
                      ))}
                    </div>

                    <div className="form-grid">
                      <label className="control-field" style={{ gridColumn: "1 / -1" }}>
                        <span>Evidence summary *</span>
                        <input
                          value={runbookEvidenceDraft.summary}
                          onChange={(event) => setRunbookEvidenceDraft((prev: any) => ({
                            ...prev,
                            summary: event.target.value
                          }))}
                          placeholder="what was executed and what is verified"
                        />
                      </label>
                      <label className="control-field">
                        <span>Evidence ticket</span>
                        <input
                          value={runbookEvidenceDraft.ticket_ref}
                          onChange={(event) => setRunbookEvidenceDraft((prev: any) => ({
                            ...prev,
                            ticket_ref: event.target.value
                          }))}
                          placeholder="TKT-..."
                        />
                      </label>
                      <label className="control-field">
                        <span>Evidence artifact URL</span>
                        <input
                          value={runbookEvidenceDraft.artifact_url}
                          onChange={(event) => setRunbookEvidenceDraft((prev: any) => ({
                            ...prev,
                            artifact_url: event.target.value
                          }))}
                          placeholder="https://artifact.example/runbook-proof"
                        />
                      </label>
                      <label className="control-field" style={{ gridColumn: "1 / -1" }}>
                        <span>Execution note</span>
                        <input
                          value={runbookEvidenceDraft.note}
                          onChange={(event) => setRunbookEvidenceDraft((prev: any) => ({
                            ...prev,
                            note: event.target.value
                          }))}
                          placeholder="operator note"
                        />
                      </label>
                    </div>

                    <div className="toolbar-row" style={{ marginTop: "0.55rem" }}>
                      <button onClick={() => void executeRunbookTemplate()} disabled={!canWriteCmdb || executingRunbookTemplate}>
                        {executingRunbookTemplate ? t("cmdb.actions.loading") : "Execute runbook"}
                      </button>
                    </div>
                  </>
                )}

                <div className="detail-panel" style={{ marginTop: "0.7rem", marginBottom: "0.55rem" }}>
                  <h4 style={{ marginTop: 0, marginBottom: "0.35rem" }}>Runbook analytics summary</h4>
                  {loadingRunbookAnalyticsSummary ? (
                    <p>{t("cmdb.actions.loading")}</p>
                  ) : !runbookAnalyticsSummary ? (
                    <p>No analytics summary available.</p>
                  ) : (
                    <>
                      <p className="section-note" style={{ marginTop: 0, marginBottom: "0.35rem" }}>
                        window={runbookAnalyticsSummary.window?.days ?? "-"}d
                        {" | "}generated={runbookAnalyticsSummary.generated_at ? new Date(runbookAnalyticsSummary.generated_at).toLocaleString() : "-"}
                        {" | "}sampled={runbookAnalyticsSummary.totals?.sampled_rows ?? 0}
                        {runbookAnalyticsSummary.totals?.truncated ? " (truncated)" : ""}
                      </p>
                      <div className="form-grid">
                        <label className="control-field">
                          <span>Total executions</span>
                          <input value={String(runbookAnalyticsSummary.totals?.executions ?? 0)} readOnly />
                        </label>
                        <label className="control-field">
                          <span>Succeeded / Failed</span>
                          <input
                            value={`${runbookAnalyticsSummary.totals?.succeeded ?? 0} / ${runbookAnalyticsSummary.totals?.failed ?? 0}`}
                            readOnly
                          />
                        </label>
                        <label className="control-field">
                          <span>Success rate</span>
                          <input value={`${runbookAnalyticsSummary.totals?.success_rate_percent ?? 0}%`} readOnly />
                        </label>
                        <label className="control-field">
                          <span>Replay usage</span>
                          <input value={String(runbookAnalyticsSummary.totals?.replayed ?? 0)} readOnly />
                        </label>
                      </div>
                      {(runbookAnalyticsSummary.failed_steps ?? []).length > 0 ? (
                        <p className="section-note" style={{ marginTop: "0.45rem", marginBottom: 0 }}>
                          failed hotspots={runbookAnalyticsSummary.failed_steps
                            .map((item: any) => `${item.template_key}:${item.step_id}(${item.failures})`)
                            .join(" | ")}
                        </p>
                      ) : (
                        <p className="section-note" style={{ marginTop: "0.45rem", marginBottom: 0 }}>
                          No failed step hotspot in current filter window.
                        </p>
                      )}
                    </>
                  )}
                </div>

                <div className="detail-panel" style={{ marginBottom: "0.55rem" }}>
                  <h4 style={{ marginTop: 0, marginBottom: "0.35rem" }}>Go-live readiness workspace</h4>
                  <div className="toolbar-row" style={{ marginBottom: "0.45rem" }}>
                    <button onClick={() => void loadGoLiveReadiness()}>
                      {loadingGoLiveReadiness ? t("cmdb.actions.loading") : "Refresh go-live readiness"}
                    </button>
                    {goLiveReadiness?.overall_status && (
                      <span className="inline-note">overall_status={goLiveReadiness.overall_status}</span>
                    )}
                    {goLiveReadiness?.recommended_next_domain && (
                      <span className="inline-note">
                        recommended_next={goLiveReadiness.recommended_next_domain}
                      </span>
                    )}
                  </div>
                  {loadingGoLiveReadiness ? (
                    <p>{t("cmdb.actions.loading")}</p>
                  ) : !goLiveReadiness || (goLiveReadiness.domains ?? []).length === 0 ? (
                    <p>No go-live readiness data available.</p>
                  ) : (
                    <>
                      <div className="toolbar-row" style={{ marginBottom: "0.45rem", gap: "0.75rem", flexWrap: "wrap" }}>
                        <span className="inline-note">domains={goLiveReadiness.summary?.total ?? 0}</span>
                        <span className="inline-note">ready={goLiveReadiness.summary?.ready ?? 0}</span>
                        <span className="inline-note">warning={goLiveReadiness.summary?.warning ?? 0}</span>
                        <span className="inline-note">blocking={goLiveReadiness.summary?.blocking ?? 0}</span>
                      </div>
                      <div style={{ overflowX: "auto" }}>
                        <table style={{ borderCollapse: "collapse", minWidth: "1540px", width: "100%" }}>
                          <thead>
                            <tr>
                              <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Domain</th>
                              <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Status</th>
                              <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Summary</th>
                              <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Reason</th>
                              <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Recommended action</th>
                              <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Evidence</th>
                            </tr>
                          </thead>
                          <tbody>
                            {(goLiveReadiness.domains ?? []).map((item: any) => {
                              const action = item.recommended_action;
                              const runningAction = runningGoLiveActionKey === action?.action_key;
                              const isNext = goLiveReadiness.recommended_next_domain === item.domain_key;
                              return (
                                <tr key={`go-live-domain-${item.domain_key}`}>
                                  <td style={{ border: "1px solid #ddd", padding: "0.5rem", verticalAlign: "top" }}>
                                    <strong>{item.name}</strong>
                                    <div className="inline-note">{item.domain_key}</div>
                                    {isNext && <div className="inline-note">next recommended domain</div>}
                                  </td>
                                  <td style={{ border: "1px solid #ddd", padding: "0.5rem", verticalAlign: "top" }}>
                                    {item.status}
                                  </td>
                                  <td style={{ border: "1px solid #ddd", padding: "0.5rem", verticalAlign: "top" }}>
                                    {item.summary}
                                  </td>
                                  <td style={{ border: "1px solid #ddd", padding: "0.5rem", verticalAlign: "top" }}>
                                    {item.reason}
                                  </td>
                                  <td style={{ border: "1px solid #ddd", padding: "0.5rem", verticalAlign: "top", minWidth: "300px" }}>
                                    {action ? (
                                      <div style={{ display: "grid", gap: "0.35rem" }}>
                                        <strong>{action.label}</strong>
                                        <span>{action.description}</span>
                                        <span className="inline-note">
                                          type={action.action_type} auto={String(action.auto_applicable)} write={String(action.requires_write)}
                                        </span>
                                        {action.blocked_reason && (
                                          <span className="inline-note">blocked={action.blocked_reason}</span>
                                        )}
                                        {action.action_type === "api" && action.auto_applicable ? (
                                          canWriteCmdb ? (
                                            <button
                                              onClick={() => void applyGoLiveAction(item.domain_key, action)}
                                              disabled={runningAction}
                                            >
                                              {runningAction ? t("cmdb.actions.loading") : action.label}
                                            </button>
                                          ) : (
                                            <span>read-only</span>
                                          )
                                        ) : action.href ? (
                                          <a href={action.href}>{action.label}</a>
                                        ) : (
                                          <span>manual</span>
                                        )}
                                      </div>
                                    ) : (
                                      "-"
                                    )}
                                  </td>
                                  <td style={{ border: "1px solid #ddd", padding: "0.5rem", verticalAlign: "top" }}>
                                    <pre style={{ margin: 0, whiteSpace: "pre-wrap", fontSize: "0.8rem" }}>
                                      {JSON.stringify(item.evidence ?? {}, null, 2)}
                                    </pre>
                                  </td>
                                </tr>
                              );
                            })}
                          </tbody>
                        </table>
                      </div>
                    </>
                  )}
                </div>

                <div className="detail-panel" style={{ marginBottom: "0.55rem" }}>
                  <h4 style={{ marginTop: 0, marginBottom: "0.35rem" }}>Integration bootstrap workspace</h4>
                  <div className="toolbar-row" style={{ marginBottom: "0.45rem" }}>
                    <button onClick={() => void loadIntegrationBootstrapCatalog()}>
                      {loadingIntegrationBootstrapCatalog ? t("cmdb.actions.loading") : "Refresh integration catalog"}
                    </button>
                    {integrationBootstrapCatalog?.recommended_next_key && (
                      <span className="inline-note">
                        recommended_next={integrationBootstrapCatalog.recommended_next_key}
                      </span>
                    )}
                  </div>
                  {loadingIntegrationBootstrapCatalog ? (
                    <p>{t("cmdb.actions.loading")}</p>
                  ) : !integrationBootstrapCatalog || (integrationBootstrapCatalog.items ?? []).length === 0 ? (
                    <p>No integration bootstrap data available.</p>
                  ) : (
                    <div style={{ overflowX: "auto" }}>
                      <table style={{ borderCollapse: "collapse", minWidth: "1420px", width: "100%" }}>
                        <thead>
                          <tr>
                            <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Integration</th>
                            <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Status</th>
                            <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Gap reason</th>
                            <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Suggested inputs</th>
                            <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Evidence</th>
                            <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Action</th>
                          </tr>
                        </thead>
                        <tbody>
                          {(integrationBootstrapCatalog.items ?? []).map((item: any) => {
                            const draft = integrationBootstrapDrafts?.[item.integration_key] ?? {};
                            const running = runningIntegrationBootstrapKey === item.integration_key;
                            return (
                              <tr key={`integration-bootstrap-${item.integration_key}`}>
                                <td style={{ border: "1px solid #ddd", padding: "0.5rem", verticalAlign: "top" }}>
                                  <strong>{item.name}</strong>
                                  <div className="inline-note">{item.integration_key}</div>
                                  <div className="inline-note">order={item.recommended_apply_order}</div>
                                  <div className="inline-note">{item.summary}</div>
                                </td>
                                <td style={{ border: "1px solid #ddd", padding: "0.5rem", verticalAlign: "top" }}>
                                  {item.status}
                                  <div className="inline-note">auto_applicable={String(item.auto_applicable)}</div>
                                </td>
                                <td style={{ border: "1px solid #ddd", padding: "0.5rem", verticalAlign: "top" }}>
                                  {item.gap_reason}
                                  <div className="inline-note">
                                    required_inputs={(item.required_inputs ?? []).join(",") || "none"}
                                  </div>
                                </td>
                                <td style={{ border: "1px solid #ddd", padding: "0.5rem", verticalAlign: "top", minWidth: "320px" }}>
                                  {item.integration_key === "operator_notifications" ? (
                                    <div style={{ display: "grid", gap: "0.35rem" }}>
                                      <input
                                        value={draft.channel_name ?? ""}
                                        readOnly={!canWriteCmdb}
                                        placeholder="channel name"
                                        onChange={(event) => setIntegrationBootstrapDrafts((prev: any) => ({
                                          ...prev,
                                          [item.integration_key]: {
                                            ...(prev[item.integration_key] ?? {}),
                                            channel_name: event.target.value
                                          }
                                        }))}
                                      />
                                      <select
                                        value={draft.channel_type ?? "email"}
                                        disabled={!canWriteCmdb}
                                        onChange={(event) => setIntegrationBootstrapDrafts((prev: any) => ({
                                          ...prev,
                                          [item.integration_key]: {
                                            ...(prev[item.integration_key] ?? {}),
                                            channel_type: event.target.value
                                          }
                                        }))}
                                      >
                                        <option value="email">email</option>
                                        <option value="webhook">webhook</option>
                                      </select>
                                      <input
                                        value={draft.target ?? ""}
                                        readOnly={!canWriteCmdb}
                                        placeholder="ops@example.com / https://..."
                                        onChange={(event) => setIntegrationBootstrapDrafts((prev: any) => ({
                                          ...prev,
                                          [item.integration_key]: {
                                            ...(prev[item.integration_key] ?? {}),
                                            target: event.target.value
                                          }
                                        }))}
                                      />
                                    </div>
                                  ) : (
                                    <input
                                      value={draft.escalation_owner ?? ""}
                                      readOnly={!canWriteCmdb}
                                      placeholder="escalation owner"
                                      onChange={(event) => setIntegrationBootstrapDrafts((prev: any) => ({
                                        ...prev,
                                        [item.integration_key]: {
                                          ...(prev[item.integration_key] ?? {}),
                                          escalation_owner: event.target.value
                                        }
                                      }))}
                                    />
                                  )}
                                </td>
                                <td style={{ border: "1px solid #ddd", padding: "0.5rem", verticalAlign: "top" }}>
                                  <pre style={{ margin: 0, whiteSpace: "pre-wrap", fontSize: "0.8rem" }}>
                                    {JSON.stringify(item.evidence ?? {}, null, 2)}
                                  </pre>
                                </td>
                                <td style={{ border: "1px solid #ddd", padding: "0.5rem", verticalAlign: "top" }}>
                                  {canWriteCmdb ? (
                                    <button onClick={() => void applyIntegrationBootstrap(item.integration_key)} disabled={running}>
                                      {running ? t("cmdb.actions.loading") : "Apply bootstrap"}
                                    </button>
                                  ) : (
                                    <span>read-only</span>
                                  )}
                                </td>
                              </tr>
                            );
                          })}
                        </tbody>
                      </table>
                    </div>
                  )}
                </div>

                <div className="detail-panel" style={{ marginBottom: "0.55rem" }}>
                  <h4 style={{ marginTop: 0, marginBottom: "0.35rem" }}>Runbook risk owner directory</h4>
                  <div className="toolbar-row" style={{ marginBottom: "0.45rem" }}>
                    <button
                      onClick={() => void Promise.all([
                        loadRunbookRiskOwnerDirectory(),
                        loadRunbookRiskOwnerRoutingRules(),
                        loadRunbookRiskOwnerReadiness(),
                        loadRunbookRiskOwnerRepairPlan()
                      ])}
                    >
                      {loadingRunbookRiskOwnerDirectory
                        || loadingRunbookRiskOwnerRoutingRules
                        || loadingRunbookRiskOwnerReadiness
                        || loadingRunbookRiskOwnerRepairPlan
                        ? t("cmdb.actions.loading")
                        : "Refresh owner readiness"}
                    </button>
                    {canWriteCmdb && (
                      <>
                        <button
                          onClick={() => setRunbookRiskOwnerDirectory((prev: any[]) => ([
                            ...prev,
                            {
                              owner_key: `owner_${prev.length + 1}`,
                              display_name: "",
                              owner_type: "team",
                              owner_ref: "",
                              notification_target: "",
                              note: "",
                              is_enabled: true,
                              updated_by: "draft",
                              created_at: "",
                              updated_at: ""
                            }
                          ]))}
                        >
                          Add owner
                        </button>
                        <button onClick={() => void saveRunbookRiskOwnerDirectory()} disabled={savingRunbookRiskOwnerDirectory}>
                          {savingRunbookRiskOwnerDirectory ? t("cmdb.actions.loading") : "Save owners"}
                        </button>
                      </>
                    )}
                  </div>
                  {loadingRunbookRiskOwnerDirectory ? (
                    <p>{t("cmdb.actions.loading")}</p>
                  ) : (
                    <div style={{ overflowX: "auto" }}>
                      <table style={{ borderCollapse: "collapse", minWidth: "1080px", width: "100%" }}>
                        <thead>
                          <tr>
                            <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Owner key</th>
                            <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Display name</th>
                            <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Type</th>
                            <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Owner ref</th>
                            <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Notification target</th>
                            <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Enabled</th>
                            <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Note</th>
                            <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Action</th>
                          </tr>
                        </thead>
                        <tbody>
                          {(runbookRiskOwnerDirectory ?? []).map((item: any, index: number) => (
                            <tr key={`runbook-risk-owner-${item.owner_key}-${index}`}>
                              <td style={{ border: "1px solid #ddd", padding: "0.5rem" }}>
                                <input
                                  value={item.owner_key ?? ""}
                                  readOnly={!canWriteCmdb}
                                  onChange={(event) => setRunbookRiskOwnerDirectory((prev: any[]) => prev.map((entry, entryIndex) => (
                                    entryIndex === index ? { ...entry, owner_key: event.target.value } : entry
                                  )))}
                                />
                              </td>
                              <td style={{ border: "1px solid #ddd", padding: "0.5rem" }}>
                                <input
                                  value={item.display_name ?? ""}
                                  readOnly={!canWriteCmdb}
                                  onChange={(event) => setRunbookRiskOwnerDirectory((prev: any[]) => prev.map((entry, entryIndex) => (
                                    entryIndex === index ? { ...entry, display_name: event.target.value } : entry
                                  )))}
                                />
                              </td>
                              <td style={{ border: "1px solid #ddd", padding: "0.5rem" }}>
                                <select
                                  value={item.owner_type ?? "team"}
                                  disabled={!canWriteCmdb}
                                  onChange={(event) => setRunbookRiskOwnerDirectory((prev: any[]) => prev.map((entry, entryIndex) => (
                                    entryIndex === index ? { ...entry, owner_type: event.target.value } : entry
                                  )))}
                                >
                                  <option value="team">team</option>
                                  <option value="user">user</option>
                                  <option value="group">group</option>
                                  <option value="external">external</option>
                                </select>
                              </td>
                              <td style={{ border: "1px solid #ddd", padding: "0.5rem" }}>
                                <input
                                  value={item.owner_ref ?? ""}
                                  readOnly={!canWriteCmdb}
                                  onChange={(event) => setRunbookRiskOwnerDirectory((prev: any[]) => prev.map((entry, entryIndex) => (
                                    entryIndex === index ? { ...entry, owner_ref: event.target.value } : entry
                                  )))}
                                />
                              </td>
                              <td style={{ border: "1px solid #ddd", padding: "0.5rem" }}>
                                <input
                                  value={item.notification_target ?? ""}
                                  readOnly={!canWriteCmdb}
                                  onChange={(event) => setRunbookRiskOwnerDirectory((prev: any[]) => prev.map((entry, entryIndex) => (
                                    entryIndex === index ? { ...entry, notification_target: event.target.value } : entry
                                  )))}
                                  placeholder="ops@example.com / webhook target"
                                />
                              </td>
                              <td style={{ border: "1px solid #ddd", padding: "0.5rem" }}>
                                <label style={{ display: "inline-flex", gap: "0.35rem", alignItems: "center" }}>
                                  <input
                                    type="checkbox"
                                    checked={Boolean(item.is_enabled)}
                                    disabled={!canWriteCmdb}
                                    onChange={(event) => setRunbookRiskOwnerDirectory((prev: any[]) => prev.map((entry, entryIndex) => (
                                      entryIndex === index ? { ...entry, is_enabled: event.target.checked } : entry
                                    )))}
                                  />
                                  enabled
                                </label>
                              </td>
                              <td style={{ border: "1px solid #ddd", padding: "0.5rem" }}>
                                <input
                                  value={item.note ?? ""}
                                  readOnly={!canWriteCmdb}
                                  onChange={(event) => setRunbookRiskOwnerDirectory((prev: any[]) => prev.map((entry, entryIndex) => (
                                    entryIndex === index ? { ...entry, note: event.target.value } : entry
                                  )))}
                                />
                              </td>
                              <td style={{ border: "1px solid #ddd", padding: "0.5rem" }}>
                                {canWriteCmdb ? (
                                  <button onClick={() => setRunbookRiskOwnerDirectory((prev: any[]) => prev.filter((_, entryIndex) => entryIndex !== index))}>
                                    Remove
                                  </button>
                                ) : (
                                  <span>-</span>
                                )}
                              </td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </div>
                  )}
                </div>

                <div className="detail-panel" style={{ marginBottom: "0.55rem" }}>
                  <h4 style={{ marginTop: 0, marginBottom: "0.35rem" }}>Runbook risk routing rules</h4>
                  <div className="toolbar-row" style={{ marginBottom: "0.45rem" }}>
                    {canWriteCmdb && (
                      <>
                        <button
                          onClick={() => setRunbookRiskOwnerRoutingRules((prev: any[]) => ([
                            ...prev,
                            {
                              rule_id: Date.now(),
                              template_key: selectedRunbookTemplateKey || "dependency-check",
                              execution_mode: null,
                              severity: null,
                              owner_key: runbookRiskOwnerDirectory[0]?.owner_key ?? "",
                              owner_label: null,
                              owner_ref: null,
                              priority: 100,
                              note: "",
                              is_enabled: true,
                              updated_by: "draft",
                              created_at: "",
                              updated_at: ""
                            }
                          ]))}
                        >
                          Add rule
                        </button>
                        <button onClick={() => void saveRunbookRiskOwnerRoutingRules()} disabled={savingRunbookRiskOwnerRoutingRules}>
                          {savingRunbookRiskOwnerRoutingRules ? t("cmdb.actions.loading") : "Save rules"}
                        </button>
                      </>
                    )}
                  </div>
                  {loadingRunbookRiskOwnerRoutingRules ? (
                    <p>{t("cmdb.actions.loading")}</p>
                  ) : (
                    <div style={{ overflowX: "auto" }}>
                      <table style={{ borderCollapse: "collapse", minWidth: "980px", width: "100%" }}>
                        <thead>
                          <tr>
                            <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Template</th>
                            <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Mode</th>
                            <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Severity</th>
                            <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Owner key</th>
                            <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Priority</th>
                            <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Enabled</th>
                            <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Note</th>
                            <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Action</th>
                          </tr>
                        </thead>
                        <tbody>
                          {(runbookRiskOwnerRoutingRules ?? []).map((item: any, index: number) => (
                            <tr key={`runbook-risk-rule-${item.rule_id}-${index}`}>
                              <td style={{ border: "1px solid #ddd", padding: "0.5rem" }}>
                                <input
                                  value={item.template_key ?? ""}
                                  readOnly={!canWriteCmdb}
                                  onChange={(event) => setRunbookRiskOwnerRoutingRules((prev: any[]) => prev.map((entry, entryIndex) => (
                                    entryIndex === index ? { ...entry, template_key: event.target.value } : entry
                                  )))}
                                />
                              </td>
                              <td style={{ border: "1px solid #ddd", padding: "0.5rem" }}>
                                <select
                                  value={item.execution_mode ?? "all"}
                                  disabled={!canWriteCmdb}
                                  onChange={(event) => setRunbookRiskOwnerRoutingRules((prev: any[]) => prev.map((entry, entryIndex) => (
                                    entryIndex === index ? { ...entry, execution_mode: event.target.value === "all" ? null : event.target.value } : entry
                                  )))}
                                >
                                  <option value="all">all</option>
                                  <option value="simulate">simulate</option>
                                  <option value="live">live</option>
                                </select>
                              </td>
                              <td style={{ border: "1px solid #ddd", padding: "0.5rem" }}>
                                <select
                                  value={item.severity ?? "all"}
                                  disabled={!canWriteCmdb}
                                  onChange={(event) => setRunbookRiskOwnerRoutingRules((prev: any[]) => prev.map((entry, entryIndex) => (
                                    entryIndex === index ? { ...entry, severity: event.target.value === "all" ? null : event.target.value } : entry
                                  )))}
                                >
                                  <option value="all">all</option>
                                  <option value="warning">warning</option>
                                  <option value="critical">critical</option>
                                </select>
                              </td>
                              <td style={{ border: "1px solid #ddd", padding: "0.5rem" }}>
                                <input
                                  value={item.owner_key ?? ""}
                                  readOnly={!canWriteCmdb}
                                  onChange={(event) => setRunbookRiskOwnerRoutingRules((prev: any[]) => prev.map((entry, entryIndex) => (
                                    entryIndex === index ? { ...entry, owner_key: event.target.value } : entry
                                  )))}
                                />
                              </td>
                              <td style={{ border: "1px solid #ddd", padding: "0.5rem" }}>
                                <input
                                  type="number"
                                  value={item.priority ?? 100}
                                  readOnly={!canWriteCmdb}
                                  onChange={(event) => setRunbookRiskOwnerRoutingRules((prev: any[]) => prev.map((entry, entryIndex) => (
                                    entryIndex === index ? { ...entry, priority: Number.parseInt(event.target.value, 10) || 100 } : entry
                                  )))}
                                />
                              </td>
                              <td style={{ border: "1px solid #ddd", padding: "0.5rem" }}>
                                <label style={{ display: "inline-flex", gap: "0.35rem", alignItems: "center" }}>
                                  <input
                                    type="checkbox"
                                    checked={Boolean(item.is_enabled)}
                                    disabled={!canWriteCmdb}
                                    onChange={(event) => setRunbookRiskOwnerRoutingRules((prev: any[]) => prev.map((entry, entryIndex) => (
                                      entryIndex === index ? { ...entry, is_enabled: event.target.checked } : entry
                                    )))}
                                  />
                                  enabled
                                </label>
                              </td>
                              <td style={{ border: "1px solid #ddd", padding: "0.5rem" }}>
                                <input
                                  value={item.note ?? ""}
                                  readOnly={!canWriteCmdb}
                                  onChange={(event) => setRunbookRiskOwnerRoutingRules((prev: any[]) => prev.map((entry, entryIndex) => (
                                    entryIndex === index ? { ...entry, note: event.target.value } : entry
                                  )))}
                                />
                              </td>
                              <td style={{ border: "1px solid #ddd", padding: "0.5rem" }}>
                                {canWriteCmdb ? (
                                  <button onClick={() => setRunbookRiskOwnerRoutingRules((prev: any[]) => prev.filter((_, entryIndex) => entryIndex !== index))}>
                                    Remove
                                  </button>
                                ) : (
                                  <span>-</span>
                                )}
                              </td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </div>
                  )}
                </div>

                <div className="detail-panel" style={{ marginBottom: "0.55rem" }}>
                  <h4 style={{ marginTop: 0, marginBottom: "0.35rem" }}>Runbook risk owner readiness</h4>
                  {loadingRunbookRiskOwnerReadiness || loadingRunbookRiskOwnerRepairPlan ? (
                    <p>{t("cmdb.actions.loading")}</p>
                  ) : !runbookRiskOwnerReadiness || (runbookRiskOwnerReadiness.items ?? []).length === 0 ? (
                    <p>No readiness data available.</p>
                  ) : (
                    <div style={{ overflowX: "auto" }}>
                      <table style={{ borderCollapse: "collapse", minWidth: "1500px", width: "100%" }}>
                        <thead>
                          <tr>
                            <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Template</th>
                            <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Owner</th>
                            <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Status</th>
                            <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Target</th>
                            <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Coverage</th>
                            <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Gap reason</th>
                            <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Repair plan</th>
                            <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Action</th>
                          </tr>
                        </thead>
                        <tbody>
                          {(runbookRiskOwnerReadiness.items ?? []).map((item: any) => {
                            const repairPlan = runbookRiskOwnerRepairPlanByKey.get(
                              `${item.template_key}::${item.owner_key ?? "none"}::${item.readiness_status}`
                            );
                            const runningRepair =
                              runningRunbookRiskOwnerRepairKey === `${item.template_key}::${item.owner_key ?? "none"}::apply`;
                            return (
                            <tr key={`runbook-risk-readiness-${item.template_key}-${item.owner_key ?? "none"}-${item.readiness_status}`}>
                              <td style={{ border: "1px solid #ddd", padding: "0.5rem" }}>
                                {item.template_name}
                                <div className="inline-note">{item.template_key}</div>
                              </td>
                              <td style={{ border: "1px solid #ddd", padding: "0.5rem" }}>
                                {item.owner_label ?? item.owner_ref ?? "-"}
                                <div className="inline-note">owner_key={item.owner_key ?? "-"}</div>
                              </td>
                              <td style={{ border: "1px solid #ddd", padding: "0.5rem" }}>{item.readiness_status}</td>
                              <td style={{ border: "1px solid #ddd", padding: "0.5rem" }}>{item.notification_target ?? "-"}</td>
                              <td style={{ border: "1px solid #ddd", padding: "0.5rem" }}>
                                channels={item.matched_channel_count} subscriptions={item.matched_subscription_count}
                                <div className="inline-note">template_enabled={String(item.notification_template_enabled)}</div>
                              </td>
                              <td style={{ border: "1px solid #ddd", padding: "0.5rem" }}>{item.gap_reason}</td>
                              <td style={{ border: "1px solid #ddd", padding: "0.5rem" }}>
                                {repairPlan ? (
                                  <>
                                    <div>{repairPlan.action.description}</div>
                                    <div className="inline-note">
                                      {repairPlan.action.resource_kind}/{repairPlan.action.operation}
                                    </div>
                                    {repairPlan.action.proposed_name && (
                                      <div className="inline-note">name={repairPlan.action.proposed_name}</div>
                                    )}
                                    {repairPlan.action.proposed_target && (
                                      <div className="inline-note">target={repairPlan.action.proposed_target}</div>
                                    )}
                                    {repairPlan.action.proposed_channel_type && (
                                      <div className="inline-note">channel={repairPlan.action.proposed_channel_type}</div>
                                    )}
                                    {repairPlan.action.reuse_hint && (
                                      <div className="inline-note">{repairPlan.action.reuse_hint}</div>
                                    )}
                                  </>
                                ) : (
                                  "-"
                                )}
                              </td>
                              <td style={{ border: "1px solid #ddd", padding: "0.5rem" }}>
                                {repairPlan?.action.auto_applicable && canWriteCmdb ? (
                                  <button
                                    onClick={() => void applyRunbookRiskOwnerReadinessRepair(item.template_key, item.owner_key ?? null)}
                                    disabled={runningRepair}
                                  >
                                    {runningRepair ? t("cmdb.actions.loading") : "Apply repair"}
                                  </button>
                                ) : repairPlan?.action.auto_applicable ? (
                                  <span>read-only</span>
                                ) : (
                                  <span>manual</span>
                                )}
                              </td>
                            </tr>
                          )})}
                        </tbody>
                      </table>
                    </div>
                  )}
                </div>

                <div className="detail-panel" style={{ marginBottom: "0.55rem" }}>
                  <h4 style={{ marginTop: 0, marginBottom: "0.35rem" }}>Runbook risk alerts</h4>
                  {loadingRunbookRiskAlerts ? (
                    <p>{t("cmdb.actions.loading")}</p>
                  ) : !runbookRiskAlerts || (runbookRiskAlerts.items ?? []).length === 0 ? (
                    <p>No runbook risk alert in current filter window.</p>
                  ) : (
                    <>
                      <p className="section-note" style={{ marginTop: 0, marginBottom: "0.35rem" }}>
                        window={runbookRiskAlerts.window?.days ?? "-"}d
                        {" | "}threshold={runbookRiskAlerts.policy?.failure_rate_threshold_percent ?? "-"}%
                        {" | "}minimum_sample_size={runbookRiskAlerts.policy?.minimum_sample_size ?? "-"}
                        {" | "}total={runbookRiskAlerts.total ?? 0}
                      </p>
                      <div style={{ overflowX: "auto" }}>
                        <table style={{ borderCollapse: "collapse", minWidth: "1480px", width: "100%" }}>
                          <thead>
                            <tr>
                              <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Template</th>
                              <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Severity</th>
                              <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Failure rate</th>
                              <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Failure context</th>
                              <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Owner route</th>
                              <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Dispatch</th>
                              <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Recommended action</th>
                              <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Ticket link</th>
                              <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Action</th>
                            </tr>
                          </thead>
                          <tbody>
                            {(runbookRiskAlerts.items ?? []).map((item: any) => {
                              const ticketLink = item.ticket_link;
                              const ownerRoute = ticketLink?.owner_route;
                              const notificationSummary = item.notification_summary;
                              const rowRunning = runningRunbookRiskTicketTemplateKey === item.template_key;
                              return (
                                <tr key={`runbook-risk-alert-${item.template_key}`}>
                                  <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                                    {item.template_name}
                                    <div className="inline-note">{item.template_key}</div>
                                  </td>
                                  <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                                    {item.severity}
                                  </td>
                                  <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                                    {item.failure_rate_percent}%
                                    <div className="inline-note">{item.failed} / {item.executions}</div>
                                  </td>
                                  <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                                    top_failed_step={item.top_failed_step_id ?? "-"}
                                    <div className="inline-note">latest_failed_execution=#{item.latest_failed_execution_id ?? "-"}</div>
                                    <div className="inline-note">
                                      latest_failed_at={item.latest_failed_at ? new Date(item.latest_failed_at).toLocaleString() : "-"}
                                    </div>
                                  </td>
                                  <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                                    {ownerRoute || ticketLink?.ticket_assignee ? (
                                      <>
                                        <div>{ownerRoute?.owner_label ?? ownerRoute?.owner ?? ticketLink?.ticket_assignee ?? "-"}</div>
                                        <div className="inline-note">owner={ownerRoute?.owner ?? ticketLink?.ticket_assignee ?? "-"}</div>
                                        <div className="inline-note">source={ownerRoute?.source ?? "ticket_assignee"}</div>
                                        <div className="inline-note">ticket_assignee={ticketLink?.ticket_assignee ?? "-"}</div>
                                        <div className="inline-note">{ownerRoute?.reason ?? "Existing ticket assignee retained."}</div>
                                      </>
                                    ) : (
                                      <span>-</span>
                                    )}
                                  </td>
                                  <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                                    {notificationSummary ? (
                                      <>
                                        <div>{notificationSummary.latest_status}</div>
                                        <div className="inline-note">
                                          delivered={notificationSummary.delivered} failed={notificationSummary.failed} skipped={notificationSummary.skipped}
                                        </div>
                                        <div className="inline-note">
                                          latest_target={notificationSummary.latest_target || "-"}
                                        </div>
                                        <div className="inline-note">
                                          channel={notificationSummary.latest_channel_type ?? "-"}
                                        </div>
                                        <div className="inline-note">
                                          {notificationSummary.latest_delivered_at
                                            ? new Date(notificationSummary.latest_delivered_at).toLocaleString()
                                            : notificationSummary.latest_created_at
                                              ? new Date(notificationSummary.latest_created_at).toLocaleString()
                                              : "-"}
                                        </div>
                                      </>
                                    ) : (
                                      <span>-</span>
                                    )}
                                  </td>
                                  <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                                    {item.recommended_action}
                                  </td>
                                  <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                                    {ticketLink ? (
                                      <>
                                        <div>{ticketLink.ticket_no}</div>
                                        <div className="inline-note">status={ticketLink.ticket_status}</div>
                                        <div className="inline-note">priority={ticketLink.ticket_priority}</div>
                                        <div className="inline-note">link_status={ticketLink.status}</div>
                                        <div className="inline-note">assignee={ticketLink.ticket_assignee ?? "-"}</div>
                                        <div className="inline-note">
                                          {ticketLink.updated_at ? new Date(ticketLink.updated_at).toLocaleString() : "-"}
                                        </div>
                                      </>
                                    ) : (
                                      <span>-</span>
                                    )}
                                  </td>
                                  <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                                    <button
                                      onClick={() => void createRunbookRiskAlertTicket(item.template_key)}
                                      disabled={!canWriteCmdb || rowRunning}
                                    >
                                      {rowRunning ? t("cmdb.actions.loading") : "Create/reuse ticket"}
                                    </button>
                                  </td>
                                </tr>
                              );
                            })}
                          </tbody>
                        </table>
                      </div>
                    </>
                  )}
                </div>

                <div className="detail-panel" style={{ marginBottom: "0.55rem" }}>
                  <h4 style={{ marginTop: 0, marginBottom: "0.35rem" }}>Runbook failure hotspot feed</h4>
                  {loadingRunbookFailureFeed ? (
                    <p>{t("cmdb.actions.loading")}</p>
                  ) : !runbookFailureFeed || (runbookFailureFeed.items ?? []).length === 0 ? (
                    <p>No failed runbook record in current window.</p>
                  ) : (
                    <div style={{ overflowX: "auto" }}>
                      <table style={{ borderCollapse: "collapse", minWidth: "980px", width: "100%" }}>
                        <thead>
                          <tr>
                            <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Execution</th>
                            <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Failed step</th>
                            <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Diagnostics</th>
                            <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Evidence/Actor</th>
                          </tr>
                        </thead>
                        <tbody>
                          {(runbookFailureFeed.items ?? []).map((item: any) => (
                            <tr key={`runbook-failure-feed-${item.id}`}>
                              <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                                #{item.id} {item.template_name}
                                <div className="inline-note">{item.template_key}</div>
                                <div className="inline-note">mode={item.execution_mode}</div>
                                {item.replay_source_execution_id ? (
                                  <div className="inline-note">replay_of=#{item.replay_source_execution_id}</div>
                                ) : null}
                              </td>
                              <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                                {item.failed_step_id ?? "-"}
                                <div className="inline-note">{item.failed_output ?? "-"}</div>
                              </td>
                              <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                                {item.remediation_hint ?? "-"}
                                <div className="inline-note">runtime={JSON.stringify(item.runtime_summary ?? {})}</div>
                              </td>
                              <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                                {item.evidence_summary ?? "-"}
                                <div className="inline-note">actor={item.actor}</div>
                                <div className="inline-note">{item.created_at ? new Date(item.created_at).toLocaleString() : "-"}</div>
                              </td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </div>
                  )}
                </div>

                <h4 style={{ marginTop: "0.8rem", marginBottom: "0.4rem" }}>Runbook execution timeline</h4>
                {loadingRunbookExecutions ? (
                  <p>{t("cmdb.actions.loading")}</p>
                ) : (runbookExecutions as any[]).length === 0 ? (
                  <p>No runbook execution record yet.</p>
                ) : (
                  <div style={{ overflowX: "auto" }}>
                    <table style={{ borderCollapse: "collapse", minWidth: "1120px", width: "100%" }}>
                      <thead>
                        <tr>
                          <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Execution</th>
                          <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Status/Mode</th>
                          <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Remediation hints</th>
                          <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Evidence</th>
                          <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Actor/Time</th>
                          <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Actions</th>
                        </tr>
                      </thead>
                      <tbody>
                        {(runbookExecutions as any[]).slice(0, 20).map((item) => (
                          <tr key={`runbook-execution-${item.id}`}>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              #{item.id} {item.template_name}
                              <div className="inline-note">{item.template_key}</div>
                              <div className="inline-note">
                                timeline={(item.timeline ?? []).map((step: any) => `${step.step_id}:${step.status}`).join(", ")}
                              </div>
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              {item.status}
                              <div className="inline-note">mode={item.execution_mode ?? "simulate"}</div>
                              {item.replay_source_execution_id ? (
                                <div className="inline-note">replay_of=#{item.replay_source_execution_id}</div>
                              ) : null}
                              <div className="inline-note">runtime={JSON.stringify(item.runtime_summary ?? {})}</div>
                              <div className="inline-note">note={item.note ?? "-"}</div>
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              {(item.remediation_hints ?? []).length > 0
                                ? (item.remediation_hints ?? []).join(" | ")
                                : "-"}
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              {item.evidence?.summary ?? "-"}
                              <div className="inline-note">ticket={item.evidence?.ticket_ref ?? "-"}</div>
                              <div className="inline-note">artifact={item.evidence?.artifact_url ?? "-"}</div>
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              {item.actor}
                              <div className="inline-note">{new Date(item.created_at).toLocaleString()}</div>
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              <button
                                onClick={() => void replayRunbookTemplateExecution(item.id)}
                                disabled={!canWriteCmdb || replayingRunbookExecutionId === item.id}
                              >
                                {replayingRunbookExecutionId === item.id ? t("cmdb.actions.loading") : "Replay"}
                              </button>
                            </td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                )}
              </>
            )}
          </div>

          <div className="detail-panel" style={{ marginBottom: "0.85rem" }}>
            <div className="toolbar-row" style={{ justifyContent: "space-between" }}>
              <h3 style={{ ...subSectionTitleStyle, marginTop: 0, marginBottom: 0 }}>Backup & DR policy</h3>
              <div className="toolbar-row">
                <button onClick={() => void loadBackupPolicies()} disabled={loadingBackupPolicies}>
                  {loadingBackupPolicies ? t("cmdb.actions.loading") : "Refresh policies"}
                </button>
                <button onClick={() => void loadBackupPolicyRuns()} disabled={loadingBackupPolicyRuns}>
                  {loadingBackupPolicyRuns ? t("cmdb.actions.loading") : "Refresh runs"}
                </button>
                <button onClick={() => void loadBackupRestoreEvidence()} disabled={loadingBackupRestoreEvidence}>
                  {loadingBackupRestoreEvidence ? t("cmdb.actions.loading") : "Refresh evidence"}
                </button>
              </div>
            </div>
            {backupPolicyNotice && <p className="banner banner-success">{backupPolicyNotice}</p>}
            <p className="section-note">
              Configure backup frequency/retention/destination with no-code form and schedule drill orchestration.
            </p>

            <div className="form-grid">
              <label className="control-field">
                <span>Policy</span>
                <select
                  value={backupPolicyDraft.policy_id}
                  onChange={(event) => {
                    const nextId = event.target.value;
                    const next = (backupPolicies as any[]).find((item) => String(item.id) === nextId);
                    if (!next) {
                      return;
                    }
                    setBackupPolicyDraft((prev: any) => ({
                      ...prev,
                      policy_id: String(next.id),
                      policy_key: next.policy_key,
                      name: next.name,
                      frequency: next.frequency,
                      schedule_time_utc: next.schedule_time_utc,
                      schedule_weekday: String(next.schedule_weekday ?? 1),
                      retention_days: String(next.retention_days),
                      destination_type: next.destination_type,
                      destination_uri: next.destination_uri,
                      drill_enabled: next.drill_enabled,
                      drill_frequency: next.drill_frequency,
                      drill_weekday: String(next.drill_weekday ?? 3),
                      drill_time_utc: next.drill_time_utc
                    }));
                  }}
                >
                  {(backupPolicies as any[]).map((item) => (
                    <option key={`backup-policy-${item.id}`} value={String(item.id)}>
                      {item.policy_key}
                    </option>
                  ))}
                </select>
              </label>
              <label className="control-field">
                <span>Policy key</span>
                <input
                  value={backupPolicyDraft.policy_key}
                  onChange={(event) => setBackupPolicyDraft((prev: any) => ({ ...prev, policy_key: event.target.value }))}
                />
              </label>
              <label className="control-field">
                <span>Name</span>
                <input
                  value={backupPolicyDraft.name}
                  onChange={(event) => setBackupPolicyDraft((prev: any) => ({ ...prev, name: event.target.value }))}
                />
              </label>
              <label className="control-field">
                <span>Frequency</span>
                <select
                  value={backupPolicyDraft.frequency}
                  onChange={(event) => setBackupPolicyDraft((prev: any) => ({ ...prev, frequency: event.target.value }))}
                >
                  <option value="daily">daily</option>
                  <option value="weekly">weekly</option>
                </select>
              </label>
              <label className="control-field">
                <span>Schedule time (UTC)</span>
                <input
                  value={backupPolicyDraft.schedule_time_utc}
                  onChange={(event) => setBackupPolicyDraft((prev: any) => ({ ...prev, schedule_time_utc: event.target.value }))}
                  placeholder="01:30"
                />
              </label>
              <label className="control-field">
                <span>Schedule weekday</span>
                <input
                  type="number"
                  min={1}
                  max={7}
                  value={backupPolicyDraft.schedule_weekday}
                  onChange={(event) => setBackupPolicyDraft((prev: any) => ({ ...prev, schedule_weekday: event.target.value }))}
                  disabled={backupPolicyDraft.frequency !== "weekly"}
                />
              </label>
              <label className="control-field">
                <span>Retention days</span>
                <input
                  type="number"
                  min={1}
                  max={3650}
                  value={backupPolicyDraft.retention_days}
                  onChange={(event) => setBackupPolicyDraft((prev: any) => ({ ...prev, retention_days: event.target.value }))}
                />
              </label>
              <label className="control-field">
                <span>Destination type</span>
                <select
                  value={backupPolicyDraft.destination_type}
                  onChange={(event) => setBackupPolicyDraft((prev: any) => ({ ...prev, destination_type: event.target.value }))}
                >
                  <option value="local">local</option>
                  <option value="s3">s3</option>
                  <option value="nfs">nfs</option>
                </select>
              </label>
              <label className="control-field">
                <span>Destination URI</span>
                <input
                  value={backupPolicyDraft.destination_uri}
                  onChange={(event) => setBackupPolicyDraft((prev: any) => ({ ...prev, destination_uri: event.target.value }))}
                />
              </label>
              <label className="control-field">
                <span>Drill enabled</span>
                <select
                  value={backupPolicyDraft.drill_enabled ? "true" : "false"}
                  onChange={(event) => setBackupPolicyDraft((prev: any) => ({ ...prev, drill_enabled: event.target.value === "true" }))}
                >
                  <option value="true">true</option>
                  <option value="false">false</option>
                </select>
              </label>
              <label className="control-field">
                <span>Drill frequency</span>
                <select
                  value={backupPolicyDraft.drill_frequency}
                  onChange={(event) => setBackupPolicyDraft((prev: any) => ({ ...prev, drill_frequency: event.target.value }))}
                  disabled={!backupPolicyDraft.drill_enabled}
                >
                  <option value="weekly">weekly</option>
                  <option value="monthly">monthly</option>
                  <option value="quarterly">quarterly</option>
                </select>
              </label>
              <label className="control-field">
                <span>Drill weekday</span>
                <input
                  type="number"
                  min={1}
                  max={7}
                  value={backupPolicyDraft.drill_weekday}
                  onChange={(event) => setBackupPolicyDraft((prev: any) => ({ ...prev, drill_weekday: event.target.value }))}
                  disabled={!backupPolicyDraft.drill_enabled || backupPolicyDraft.drill_frequency !== "weekly"}
                />
              </label>
              <label className="control-field">
                <span>Drill time (UTC)</span>
                <input
                  value={backupPolicyDraft.drill_time_utc}
                  onChange={(event) => setBackupPolicyDraft((prev: any) => ({ ...prev, drill_time_utc: event.target.value }))}
                  disabled={!backupPolicyDraft.drill_enabled}
                />
              </label>
              <label className="control-field" style={{ gridColumn: "1 / -1" }}>
                <span>Note</span>
                <input
                  value={backupPolicyDraft.note}
                  onChange={(event) => setBackupPolicyDraft((prev: any) => ({ ...prev, note: event.target.value }))}
                  placeholder="Change reason or run note"
                />
              </label>
            </div>

            <div className="toolbar-row" style={{ marginTop: "0.65rem" }}>
              <button onClick={() => void saveBackupPolicy()} disabled={!canWriteCmdb || savingBackupPolicy}>
                {savingBackupPolicy ? t("cmdb.actions.loading") : "Save policy"}
              </button>
              <button
                onClick={() => {
                  const id = Number.parseInt(backupPolicyDraft.policy_id, 10);
                  if (Number.isFinite(id) && id > 0) {
                    void runBackupPolicy(id, "backup", false);
                  }
                }}
                disabled={
                  !canWriteCmdb
                  || runningBackupPolicyActionId === `${backupPolicyDraft.policy_id}:backup:run`
                }
              >
                {runningBackupPolicyActionId === `${backupPolicyDraft.policy_id}:backup:run`
                  ? t("cmdb.actions.loading")
                  : "Run backup now"}
              </button>
              <button
                onClick={() => {
                  const id = Number.parseInt(backupPolicyDraft.policy_id, 10);
                  if (Number.isFinite(id) && id > 0) {
                    void runBackupPolicy(id, "drill", false);
                  }
                }}
                disabled={
                  !canWriteCmdb
                  || runningBackupPolicyActionId === `${backupPolicyDraft.policy_id}:drill:run`
                }
              >
                {runningBackupPolicyActionId === `${backupPolicyDraft.policy_id}:drill:run`
                  ? t("cmdb.actions.loading")
                  : "Run drill now"}
              </button>
              <button
                onClick={() => {
                  const id = Number.parseInt(backupPolicyDraft.policy_id, 10);
                  if (Number.isFinite(id) && id > 0) {
                    void runBackupPolicy(id, "backup", true);
                  }
                }}
                disabled={
                  !canWriteCmdb
                  || runningBackupPolicyActionId === `${backupPolicyDraft.policy_id}:backup:fail`
                }
              >
                {runningBackupPolicyActionId === `${backupPolicyDraft.policy_id}:backup:fail`
                  ? t("cmdb.actions.loading")
                  : "Simulate backup failure"}
              </button>
              <button onClick={() => void runBackupSchedulerTick()} disabled={!canWriteCmdb || tickingBackupScheduler}>
                {tickingBackupScheduler ? t("cmdb.actions.loading") : "Run scheduler tick"}
              </button>
            </div>

            {(backupPolicies as any[]).length > 0 && (
              <div style={{ overflowX: "auto", marginTop: "0.65rem" }}>
                <table style={{ borderCollapse: "collapse", minWidth: "1120px", width: "100%" }}>
                  <thead>
                    <tr>
                      <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Policy</th>
                      <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Backup status</th>
                      <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Drill status</th>
                      <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Next schedule</th>
                      <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Destination</th>
                    </tr>
                  </thead>
                  <tbody>
                    {(backupPolicies as any[]).map((item) => (
                      <tr key={`backup-policy-status-${item.id}`}>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                          {item.policy_key} ({item.frequency})
                        </td>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                          {item.last_backup_status}
                          {item.last_backup_at ? ` @ ${new Date(item.last_backup_at).toLocaleString()}` : ""}
                          {item.last_backup_error ? ` | ${item.last_backup_error}` : ""}
                        </td>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                          {item.last_drill_status}
                          {item.last_drill_at ? ` @ ${new Date(item.last_drill_at).toLocaleString()}` : ""}
                          {item.last_drill_error ? ` | ${item.last_drill_error}` : ""}
                        </td>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                          backup={item.next_backup_at ? new Date(item.next_backup_at).toLocaleString() : "-"}
                          <br />
                          drill={item.next_drill_at ? new Date(item.next_drill_at).toLocaleString() : "-"}
                        </td>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                          {item.destination_type}:{item.destination_uri}
                          {item.destination_validated ? " (validated)" : " (invalid)"}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}

            <h4 style={{ marginBottom: "0.4rem", marginTop: "0.8rem" }}>Recent backup/drill runs</h4>
            {loadingBackupPolicyRuns ? (
              <p>{t("cmdb.actions.loading")}</p>
            ) : (backupPolicyRuns as any[]).length === 0 ? (
              <p>No run record yet.</p>
            ) : (
              <div style={{ overflowX: "auto" }}>
                <table style={{ borderCollapse: "collapse", minWidth: "980px", width: "100%" }}>
                  <thead>
                    <tr>
                      <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Run</th>
                      <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Status</th>
                      <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Restore evidence</th>
                      <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Trigger</th>
                      <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Hint</th>
                      <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Time</th>
                    </tr>
                  </thead>
                  <tbody>
                    {(backupPolicyRuns as any[]).slice(0, 20).map((run) => (
                      <tr key={`backup-run-row-${run.id}`}>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                          #{run.id} / policy #{run.policy_id} / {run.run_type}
                        </td>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                          {run.status}
                          {run.error_message ? ` | ${run.error_message}` : ""}
                        </td>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                          count={run.restore_evidence_count}
                          {run.latest_restore_closure_status ? ` | ${run.latest_restore_closure_status}` : ""}
                          {run.latest_restore_verified_at ? ` @ ${new Date(run.latest_restore_verified_at).toLocaleString()}` : ""}
                          {(run.status === "failed" || run.run_type === "drill") && run.restore_evidence_count === 0
                            ? " | missing"
                            : ""}
                        </td>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                          {run.triggered_by}
                          {run.triggered_by_scheduler ? " (scheduler)" : ""}
                        </td>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                          {run.remediation_hint ?? "-"}
                        </td>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                          {new Date(run.started_at).toLocaleString()}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}

            <h4 style={{ marginBottom: "0.4rem", marginTop: "0.8rem" }}>Restore verification evidence</h4>
            <div className="toolbar-row" style={{ marginBottom: "0.45rem" }}>
              <label className="control-field" style={{ minWidth: "220px" }}>
                <span>Run status filter</span>
                <select
                  value={backupRestoreRunStatusFilter}
                  onChange={(event) => setBackupRestoreRunStatusFilter(event.target.value)}
                >
                  <option value="all">all</option>
                  <option value="succeeded">succeeded</option>
                  <option value="failed">failed</option>
                </select>
              </label>
              <button onClick={() => void loadBackupRestoreEvidence()} disabled={loadingBackupRestoreEvidence}>
                {loadingBackupRestoreEvidence ? t("cmdb.actions.loading") : "Apply evidence filter"}
              </button>
            </div>
            <p className="section-note">
              required_runs={backupRestoreEvidenceCoverage.required_runs} | covered_runs={backupRestoreEvidenceCoverage.covered_runs} | missing_runs={backupRestoreEvidenceCoverage.missing_runs}
              {(backupRestoreEvidenceMissingRunIds as number[]).length > 0
                ? ` | missing_run_ids=${(backupRestoreEvidenceMissingRunIds as number[]).slice(0, 12).join(",")}`
                : ""}
            </p>

            <div className="detail-panel" style={{ marginBottom: "0.55rem" }}>
              <div className="toolbar-row" style={{ justifyContent: "space-between" }}>
                <h4 style={{ marginTop: 0, marginBottom: 0 }}>Restore evidence SLA compliance</h4>
                <div className="toolbar-row">
                  <button onClick={() => void loadBackupEvidenceCompliancePolicy()} disabled={loadingBackupEvidenceCompliancePolicy}>
                    {loadingBackupEvidenceCompliancePolicy ? t("cmdb.actions.loading") : "Refresh policy"}
                  </button>
                  <button onClick={() => void loadBackupEvidenceComplianceScorecard()} disabled={loadingBackupEvidenceComplianceScorecard}>
                    {loadingBackupEvidenceComplianceScorecard ? t("cmdb.actions.loading") : "Refresh scorecard"}
                  </button>
                  <button
                    onClick={() => void exportBackupEvidenceComplianceScorecard("csv")}
                    disabled={exportingBackupEvidenceComplianceScorecard}
                  >
                    {exportingBackupEvidenceComplianceScorecard ? t("cmdb.actions.loading") : "Export scorecard CSV"}
                  </button>
                  <button
                    onClick={() => void exportBackupEvidenceComplianceScorecard("json")}
                    disabled={exportingBackupEvidenceComplianceScorecard}
                  >
                    {exportingBackupEvidenceComplianceScorecard ? t("cmdb.actions.loading") : "Export scorecard JSON"}
                  </button>
                </div>
              </div>

              <div className="form-grid" style={{ marginTop: "0.5rem" }}>
                <label className="control-field">
                  <span>Week start</span>
                  <input
                    type="date"
                    value={backupEvidenceComplianceWeekStart}
                    onChange={(event) => setBackupEvidenceComplianceWeekStart(event.target.value)}
                  />
                </label>
                <label className="control-field">
                  <span>Mode</span>
                  <select
                    value={backupEvidenceCompliancePolicyDraft.mode}
                    onChange={(event) => setBackupEvidenceCompliancePolicyDraft((prev: any) => ({
                      ...prev,
                      mode: event.target.value
                    }))}
                    disabled={!canWriteCmdb}
                  >
                    <option value="advisory">advisory</option>
                    <option value="enforced">enforced</option>
                  </select>
                </label>
                <label className="control-field">
                  <span>SLA hours</span>
                  <input
                    type="number"
                    min={1}
                    max={720}
                    value={backupEvidenceCompliancePolicyDraft.sla_hours}
                    onChange={(event) => setBackupEvidenceCompliancePolicyDraft((prev: any) => ({
                      ...prev,
                      sla_hours: event.target.value
                    }))}
                    disabled={!canWriteCmdb}
                  />
                </label>
                <label className="control-field">
                  <span>Scope</span>
                  <div className="toolbar-row">
                    <label className="inline-note">
                      <input
                        type="checkbox"
                        checked={backupEvidenceCompliancePolicyDraft.require_failed_runs}
                        onChange={(event) => setBackupEvidenceCompliancePolicyDraft((prev: any) => ({
                          ...prev,
                          require_failed_runs: event.target.checked
                        }))}
                        disabled={!canWriteCmdb}
                      />
                      failed runs
                    </label>
                    <label className="inline-note">
                      <input
                        type="checkbox"
                        checked={backupEvidenceCompliancePolicyDraft.require_drill_runs}
                        onChange={(event) => setBackupEvidenceCompliancePolicyDraft((prev: any) => ({
                          ...prev,
                          require_drill_runs: event.target.checked
                        }))}
                        disabled={!canWriteCmdb}
                      />
                      drill runs
                    </label>
                  </div>
                </label>
                <label className="control-field" style={{ gridColumn: "1 / -1" }}>
                  <span>Policy note</span>
                  <input
                    value={backupEvidenceCompliancePolicyDraft.note}
                    onChange={(event) => setBackupEvidenceCompliancePolicyDraft((prev: any) => ({
                      ...prev,
                      note: event.target.value
                    }))}
                    placeholder="why this SLA mode is selected"
                    disabled={!canWriteCmdb}
                  />
                </label>
              </div>

              <div className="toolbar-row" style={{ marginTop: "0.5rem" }}>
                <button onClick={() => void saveBackupEvidenceCompliancePolicy()} disabled={!canWriteCmdb || savingBackupEvidenceCompliancePolicy}>
                  {savingBackupEvidenceCompliancePolicy ? t("cmdb.actions.loading") : "Save evidence policy"}
                </button>
              </div>

              {backupEvidenceCompliancePolicy && (
                <p className="section-note">
                  policy={backupEvidenceCompliancePolicy.policy.policy_key} | mode={backupEvidenceCompliancePolicy.policy.mode}
                  {" | "}updated_by={backupEvidenceCompliancePolicy.policy.updated_by}
                  {" @ "}{new Date(backupEvidenceCompliancePolicy.policy.updated_at).toLocaleString()}
                </p>
              )}

              {backupEvidenceComplianceScorecard ? (
                <>
                  <div className="toolbar-row">
                    <span className="status-chip">required:{backupEvidenceComplianceScorecard.metrics.required_runs}</span>
                    <span className="status-chip status-chip-success">closed:{backupEvidenceComplianceScorecard.metrics.closed_runs}</span>
                    <span className="status-chip status-chip-warn">closed_within_sla:{backupEvidenceComplianceScorecard.metrics.closed_within_sla_runs}</span>
                    <span className="status-chip">open:{backupEvidenceComplianceScorecard.metrics.open_runs}</span>
                    <span className="status-chip status-chip-danger">overdue:{backupEvidenceComplianceScorecard.metrics.overdue_runs}</span>
                    <span className="status-chip status-chip-danger">overdue_open:{backupEvidenceComplianceScorecard.metrics.overdue_open_runs}</span>
                  </div>

                  {(backupEvidenceComplianceScorecard.timeline ?? []).length > 0 && (
                    <div className="toolbar-row" style={{ marginTop: "0.35rem", marginBottom: "0.35rem" }}>
                      {(backupEvidenceComplianceScorecard.timeline ?? []).map((point: any) => (
                        <span key={`evidence-timeline-${point.date}`} className="status-chip">
                          {point.date}: req={point.required_runs}, closed={point.closed_runs}, overdue={point.overdue_runs}
                        </span>
                      ))}
                    </div>
                  )}

                  {(backupEvidenceComplianceScorecard.overdue_items ?? []).length === 0 ? (
                    <p className="inline-note">No overdue evidence item in selected week.</p>
                  ) : (
                    <div style={{ overflowX: "auto", marginTop: "0.35rem" }}>
                      <table style={{ borderCollapse: "collapse", minWidth: "1120px", width: "100%" }}>
                        <thead>
                          <tr>
                            <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Run</th>
                            <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>SLA window</th>
                            <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Evidence state</th>
                            <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Reference</th>
                          </tr>
                        </thead>
                        <tbody>
                          {(backupEvidenceComplianceScorecard.overdue_items ?? []).slice(0, 30).map((item: any) => (
                            <tr key={`evidence-overdue-item-${item.run_id}`}>
                              <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                                run #{item.run_id} / policy #{item.policy_id}
                                <div className="inline-note">{item.run_type} / {item.run_status}</div>
                                <div className="inline-note">started={new Date(item.started_at).toLocaleString()}</div>
                              </td>
                              <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                                deadline={new Date(item.deadline_at).toLocaleString()}
                                <div className="inline-note">overdue_hours={item.overdue_hours}</div>
                              </td>
                              <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                                state={item.closure_state}
                                <div className="inline-note">evidence_total={item.evidence_total}</div>
                                <div className="inline-note">closed_evidence_count={item.closed_evidence_count}</div>
                                <div className="inline-note">
                                  latest={item.latest_evidence_id ? `#${item.latest_evidence_id}` : "-"}
                                  {item.latest_closure_status ? ` (${item.latest_closure_status})` : ""}
                                  {item.latest_evidence_at ? ` @ ${new Date(item.latest_evidence_at).toLocaleString()}` : ""}
                                </div>
                              </td>
                              <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                                <code>{item.run_ref}</code>
                              </td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </div>
                  )}
                </>
              ) : (
                <p className="inline-note">No compliance scorecard loaded.</p>
              )}
            </div>

            <div className="form-grid">
              <label className="control-field">
                <span>Run ID</span>
                <input
                  value={backupRestoreEvidenceDraft.run_id}
                  onChange={(event) => setBackupRestoreEvidenceDraft((prev: any) => ({ ...prev, run_id: event.target.value }))}
                  placeholder="backup run id"
                />
              </label>
              <label className="control-field">
                <span>Ticket</span>
                <input
                  value={backupRestoreEvidenceDraft.ticket_ref}
                  onChange={(event) => setBackupRestoreEvidenceDraft((prev: any) => ({ ...prev, ticket_ref: event.target.value }))}
                  placeholder="TKT-..."
                />
              </label>
              <label className="control-field">
                <span>Verifier</span>
                <input
                  value={backupRestoreEvidenceDraft.verifier}
                  onChange={(event) => setBackupRestoreEvidenceDraft((prev: any) => ({ ...prev, verifier: event.target.value }))}
                  placeholder="operator id"
                />
              </label>
              <label className="control-field">
                <span>Close evidence</span>
                <select
                  value={backupRestoreEvidenceDraft.close_evidence ? "true" : "false"}
                  onChange={(event) =>
                    setBackupRestoreEvidenceDraft((prev: any) => ({ ...prev, close_evidence: event.target.value === "true" }))
                  }
                >
                  <option value="true">true</option>
                  <option value="false">false</option>
                </select>
              </label>
              <label className="control-field" style={{ gridColumn: "1 / -1" }}>
                <span>Artifact URL</span>
                <input
                  value={backupRestoreEvidenceDraft.artifact_url}
                  onChange={(event) => setBackupRestoreEvidenceDraft((prev: any) => ({ ...prev, artifact_url: event.target.value }))}
                  placeholder="https://artifact.example/restore-proof"
                />
              </label>
              <label className="control-field" style={{ gridColumn: "1 / -1" }}>
                <span>Note</span>
                <input
                  value={backupRestoreEvidenceDraft.note}
                  onChange={(event) => setBackupRestoreEvidenceDraft((prev: any) => ({ ...prev, note: event.target.value }))}
                  placeholder="restore validation summary"
                />
              </label>
            </div>

            <div className="toolbar-row" style={{ marginTop: "0.5rem" }}>
              <button onClick={() => void saveBackupRestoreEvidence()} disabled={!canWriteCmdb || savingBackupRestoreEvidence}>
                {savingBackupRestoreEvidence ? t("cmdb.actions.loading") : "Attach evidence"}
              </button>
            </div>

            {loadingBackupRestoreEvidence ? (
              <p>{t("cmdb.actions.loading")}</p>
            ) : (backupRestoreEvidence as any[]).length === 0 ? (
              <p>No restore evidence yet.</p>
            ) : (
              <div style={{ overflowX: "auto", marginTop: "0.55rem" }}>
                <table style={{ borderCollapse: "collapse", minWidth: "1080px", width: "100%" }}>
                  <thead>
                    <tr>
                      <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Evidence</th>
                      <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Run</th>
                      <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Verifier</th>
                      <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Status</th>
                      <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Time</th>
                      <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Actions</th>
                    </tr>
                  </thead>
                  <tbody>
                    {(backupRestoreEvidence as any[]).slice(0, 30).map((item) => (
                      <tr key={`restore-evidence-row-${item.id}`}>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                          {item.ticket_ref ?? "-"}
                          <div className="inline-note" style={{ marginTop: "0.2rem" }}>
                            {item.artifact_url}
                          </div>
                          <div className="inline-note">{item.note ?? "-"}</div>
                        </td>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                          #{item.run_id} / policy #{item.policy_id} / {item.run_type} / {item.run_status}
                        </td>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                          {item.verifier}
                          {item.closed_by ? ` -> ${item.closed_by}` : ""}
                        </td>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                          {item.closure_status}
                          {item.closed_at ? ` @ ${new Date(item.closed_at).toLocaleString()}` : ""}
                        </td>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                          {new Date(item.created_at).toLocaleString()}
                        </td>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                          <div className="toolbar-row">
                            <button
                              onClick={() => {
                                setBackupRestoreEvidenceDraft((prev: any) => ({
                                  ...prev,
                                  run_id: String(item.run_id),
                                  ticket_ref: item.ticket_ref ?? "",
                                  artifact_url: item.artifact_url,
                                  note: item.note ?? "",
                                  verifier: item.verifier
                                }));
                              }}
                            >
                              Fill draft
                            </button>
                            <button
                              onClick={() => void closeBackupRestoreEvidence(item.id)}
                              disabled={!canWriteCmdb || item.closure_status === "closed" || savingBackupRestoreEvidence}
                            >
                              {item.closure_status === "closed" ? "Closed" : "Close"}
                            </button>
                          </div>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </div>

          <div className="detail-panel" style={{ marginBottom: "0.85rem" }}>
            <div className="toolbar-row" style={{ justifyContent: "space-between" }}>
              <h3 style={{ ...subSectionTitleStyle, marginTop: 0, marginBottom: 0 }}>Unified change calendar</h3>
              <div className="toolbar-row">
                <button onClick={() => void loadChangeCalendar()} disabled={loadingChangeCalendar}>
                  {loadingChangeCalendar ? t("cmdb.actions.loading") : "Refresh calendar"}
                </button>
                <button onClick={() => void loadChangeCalendarReservations()} disabled={loadingChangeCalendarReservations}>
                  {loadingChangeCalendarReservations ? t("cmdb.actions.loading") : "Refresh reservations"}
                </button>
                <button onClick={() => void checkChangeCalendarConflicts()} disabled={!canWriteCmdb || checkingChangeCalendarConflict}>
                  {checkingChangeCalendarConflict ? t("cmdb.actions.loading") : "Check conflict"}
                </button>
                <button onClick={() => void loadChangeCalendarSlotRecommendations()} disabled={!canWriteCmdb || loadingChangeCalendarRecommendations}>
                  {loadingChangeCalendarRecommendations ? t("cmdb.actions.loading") : "Auto suggest slots"}
                </button>
                <button onClick={() => void createChangeCalendarReservation()} disabled={!canWriteCmdb || creatingChangeCalendarReservation}>
                  {creatingChangeCalendarReservation ? t("cmdb.actions.loading") : "Reserve slot"}
                </button>
              </div>
            </div>
            {changeCalendarNotice && <p className="banner banner-success">{changeCalendarNotice}</p>}
            {!canWriteCmdb && <p className="inline-note">{t("cmdb.dailyCockpit.messages.readOnlyHint")}</p>}
            <div className="detail-grid" style={{ marginBottom: "0.65rem" }}>
              <label className="control-field">
                <span>Range start</span>
                <input
                  type="date"
                  value={changeCalendarStartDate}
                  onChange={(event) => setChangeCalendarStartDate(event.target.value)}
                />
              </label>
              <label className="control-field">
                <span>Range end</span>
                <input
                  type="date"
                  value={changeCalendarEndDate}
                  onChange={(event) => setChangeCalendarEndDate(event.target.value)}
                />
              </label>
              <label className="control-field">
                <span>Operation kind</span>
                <input
                  value={changeCalendarConflictDraft.operation_kind}
                  onChange={(event) => {
                    const value = event.target.value;
                    setChangeCalendarConflictDraft((prev: any) => ({
                      ...prev,
                      operation_kind: value
                    }));
                    setChangeCalendarReservationDraft((prev: any) => ({
                      ...prev,
                      operation_kind: value
                    }));
                  }}
                  placeholder="playbook.execute.restart-service-safe"
                />
              </label>
              <label className="control-field">
                <span>Risk level</span>
                <select
                  value={changeCalendarConflictDraft.risk_level}
                  onChange={(event) => {
                    const value = event.target.value;
                    setChangeCalendarConflictDraft((prev: any) => ({
                      ...prev,
                      risk_level: value
                    }));
                    setChangeCalendarReservationDraft((prev: any) => ({
                      ...prev,
                      risk_level: value
                    }));
                  }}
                >
                  <option value="low">low</option>
                  <option value="medium">medium</option>
                  <option value="high">high</option>
                  <option value="critical">critical</option>
                </select>
              </label>
              <label className="control-field">
                <span>Slot start</span>
                <input
                  type="datetime-local"
                  value={changeCalendarConflictDraft.start_at_local}
                  onChange={(event) => {
                    const value = event.target.value;
                    setChangeCalendarConflictDraft((prev: any) => ({
                      ...prev,
                      start_at_local: value
                    }));
                    setChangeCalendarReservationDraft((prev: any) => ({
                      ...prev,
                      start_at_local: value
                    }));
                  }}
                />
              </label>
              <label className="control-field">
                <span>Slot end</span>
                <input
                  type="datetime-local"
                  value={changeCalendarConflictDraft.end_at_local}
                  onChange={(event) => {
                    const value = event.target.value;
                    setChangeCalendarConflictDraft((prev: any) => ({
                      ...prev,
                      end_at_local: value
                    }));
                    setChangeCalendarReservationDraft((prev: any) => ({
                      ...prev,
                      end_at_local: value
                    }));
                  }}
                />
              </label>
              <label className="control-field">
                <span>Reservation owner</span>
                <input
                  value={changeCalendarReservationDraft.owner}
                  onChange={(event) => setChangeCalendarReservationDraft((prev: any) => ({
                    ...prev,
                    owner: event.target.value
                  }))}
                  placeholder="ops-oncall"
                />
              </label>
              <label className="control-field">
                <span>Reservation site</span>
                <input
                  value={changeCalendarReservationDraft.site}
                  onChange={(event) => setChangeCalendarReservationDraft((prev: any) => ({
                    ...prev,
                    site: event.target.value
                  }))}
                  placeholder="dc-a"
                />
              </label>
              <label className="control-field">
                <span>Reservation department</span>
                <input
                  value={changeCalendarReservationDraft.department}
                  onChange={(event) => setChangeCalendarReservationDraft((prev: any) => ({
                    ...prev,
                    department: event.target.value
                  }))}
                  placeholder="platform"
                />
              </label>
              <label className="control-field" style={{ gridColumn: "1 / -1" }}>
                <span>Reservation note</span>
                <input
                  value={changeCalendarReservationDraft.note}
                  onChange={(event) => setChangeCalendarReservationDraft((prev: any) => ({
                    ...prev,
                    note: event.target.value
                  }))}
                  placeholder="purpose/change ticket/context"
                />
              </label>
            </div>
            {changeCalendar ? (
              <p className="section-note">
                generated_at={new Date(changeCalendar.generated_at).toLocaleString()} | range={changeCalendar.range.start_date}..{changeCalendar.range.end_date} | total={changeCalendar.total}
              </p>
            ) : (
              <p className="inline-note">{loadingChangeCalendar ? t("cmdb.actions.loading") : "No calendar data loaded yet."}</p>
            )}
            {changeCalendarConflictResult && (
              <div className="detail-panel" style={{ marginBottom: "0.55rem" }}>
                <p className="section-note" style={{ marginTop: 0 }}>
                  decision={changeCalendarConflictResult.decision_reason}
                  {changeCalendarConflictResult.recommended_slot
                    ? ` | recommended_slot=${new Date(changeCalendarConflictResult.recommended_slot).toLocaleString()}`
                    : ""}
                </p>
                <div className="toolbar-row" style={{ marginBottom: "0.35rem" }}>
                  <span className={`status-chip ${changeCalendarConflictResult.has_conflict ? "status-chip-danger" : "status-chip-success"}`}>
                    {changeCalendarConflictResult.has_conflict ? "conflict" : "clear"}
                  </span>
                  <span className="status-chip">risk:{changeCalendarConflictResult.slot.risk_level}</span>
                  <span className="status-chip">operation:{changeCalendarConflictResult.slot.operation_kind}</span>
                </div>
                {(changeCalendarConflictResult.conflicts ?? []).length > 0 && (
                  <ul style={{ marginTop: "0.35rem", marginBottom: 0 }}>
                    {(changeCalendarConflictResult.conflicts ?? []).map((item: any) => (
                      <li key={`change-calendar-conflict-${item.code}`}>
                        [{item.severity}] {item.title}: {item.detail}
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            )}
            {changeCalendarSlotRecommendations && (
              <div className="detail-panel" style={{ marginBottom: "0.55rem" }}>
                <p className="section-note" style={{ marginTop: 0 }}>
                  recommendations={changeCalendarSlotRecommendations.total}
                  {" | "}duration={changeCalendarSlotRecommendations.duration_minutes}m
                  {" | "}workload(incidents={changeCalendarSlotRecommendations.pending_risky_workload.unresolved_incidents},
                  tickets={changeCalendarSlotRecommendations.pending_risky_workload.high_priority_tickets},
                  approvals={changeCalendarSlotRecommendations.pending_risky_workload.pending_approvals})
                </p>
                {(changeCalendarSlotRecommendations.items ?? []).length === 0 ? (
                  <p className="inline-note">No conflict-free slot found in current window.</p>
                ) : (
                  <div style={{ overflowX: "auto" }}>
                    <table style={{ borderCollapse: "collapse", minWidth: "980px", width: "100%" }}>
                      <thead>
                        <tr>
                          <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Rank</th>
                          <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Slot</th>
                          <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Score</th>
                          <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Rationale</th>
                          <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Actions</th>
                        </tr>
                      </thead>
                      <tbody>
                        {(changeCalendarSlotRecommendations.items ?? []).map((item: any) => (
                          <tr key={`calendar-recommendation-${item.rank}-${item.start_at}`}>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              #{item.rank}
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              {new Date(item.start_at).toLocaleString()}
                              <div className="inline-note">to {new Date(item.end_at).toLocaleString()}</div>
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              {item.score}
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              {(item.rationale ?? []).join(" | ")}
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              <button
                                onClick={() => {
                                  const start = new Date(item.start_at);
                                  const end = new Date(item.end_at);
                                  const startLocal = `${start.getFullYear()}-${String(start.getMonth() + 1).padStart(2, "0")}-${String(start.getDate()).padStart(2, "0")}T${String(start.getHours()).padStart(2, "0")}:${String(start.getMinutes()).padStart(2, "0")}`;
                                  const endLocal = `${end.getFullYear()}-${String(end.getMonth() + 1).padStart(2, "0")}-${String(end.getDate()).padStart(2, "0")}T${String(end.getHours()).padStart(2, "0")}:${String(end.getMinutes()).padStart(2, "0")}`;
                                  setChangeCalendarConflictDraft((prev: any) => ({
                                    ...prev,
                                    start_at_local: startLocal,
                                    end_at_local: endLocal
                                  }));
                                  setChangeCalendarReservationDraft((prev: any) => ({
                                    ...prev,
                                    start_at_local: startLocal,
                                    end_at_local: endLocal
                                  }));
                                }}
                              >
                                Use slot
                              </button>
                            </td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                )}
              </div>
            )}
            {(changeCalendarReservations ?? []).length > 0 && (
              <div className="detail-panel" style={{ marginBottom: "0.55rem" }}>
                <p className="section-note" style={{ marginTop: 0 }}>
                  reserved_slots={(changeCalendarReservations ?? []).length}
                </p>
                <div style={{ overflowX: "auto" }}>
                  <table style={{ borderCollapse: "collapse", minWidth: "1020px", width: "100%" }}>
                    <thead>
                      <tr>
                        <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Reservation</th>
                        <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Slot</th>
                        <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Scope</th>
                        <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Owner</th>
                      </tr>
                    </thead>
                    <tbody>
                      {(changeCalendarReservations ?? []).slice(0, 30).map((item: any) => (
                        <tr key={`calendar-reservation-${item.id}`}>
                          <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                            #{item.id} {item.operation_kind}
                            <div className="inline-note">risk={item.risk_level} status={item.status}</div>
                            <div className="inline-note">{item.note ?? "-"}</div>
                          </td>
                          <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                            {new Date(item.start_at).toLocaleString()}
                            <div className="inline-note">to {new Date(item.end_at).toLocaleString()}</div>
                          </td>
                          <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                            {item.site ?? "-"} / {item.department ?? "-"}
                          </td>
                          <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                            {item.owner}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </div>
            )}
            {!changeCalendar || (changeCalendar.items ?? []).length === 0 ? (
              <p className="inline-note">No upcoming maintenance/freeze/approval overlay items in selected range.</p>
            ) : (
              <div style={{ overflowX: "auto" }}>
                <table style={{ borderCollapse: "collapse", minWidth: "1180px", width: "100%" }}>
                  <thead>
                    <tr>
                      <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Event</th>
                      <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Type</th>
                      <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Severity</th>
                      <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Time</th>
                      <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Source</th>
                    </tr>
                  </thead>
                  <tbody>
                    {(changeCalendar.items ?? []).slice(0, 40).map((item: any) => (
                      <tr key={`change-calendar-item-${item.event_key}`}>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                          <strong>{item.title}</strong>
                          <div className="inline-note">{item.event_key}</div>
                          <div className="inline-note">{item.details}</div>
                        </td>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>{item.event_type}</td>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                          <span className={`status-chip ${item.severity === "critical" ? "status-chip-danger" : item.severity === "high" ? "status-chip-warn" : ""}`}>
                            {item.severity}
                          </span>
                        </td>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                          {new Date(item.starts_at).toLocaleString()}
                          <div className="inline-note">to {new Date(item.ends_at).toLocaleString()}</div>
                        </td>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                          {item.source_type}#{item.source_id}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </div>

          <div className="detail-panel" style={{ marginBottom: "0.85rem" }}>
            <div className="toolbar-row" style={{ justifyContent: "space-between" }}>
              <h3 style={{ ...subSectionTitleStyle, marginTop: 0, marginBottom: 0 }}>Weekly operations digest</h3>
              <div className="toolbar-row">
                <button onClick={() => void loadWeeklyDigest()} disabled={loadingWeeklyDigest}>
                  {loadingWeeklyDigest ? t("cmdb.actions.loading") : "Generate digest"}
                </button>
                <button onClick={() => void exportWeeklyDigest("csv")} disabled={exportingWeeklyDigest}>
                  {exportingWeeklyDigest ? t("cmdb.actions.loading") : "Export CSV"}
                </button>
                <button onClick={() => void exportWeeklyDigest("json")} disabled={exportingWeeklyDigest}>
                  {exportingWeeklyDigest ? t("cmdb.actions.loading") : "Export JSON"}
                </button>
              </div>
            </div>
            {weeklyDigestNotice && <p className="banner banner-success">{weeklyDigestNotice}</p>}
            <div className="toolbar-row" style={{ marginBottom: "0.55rem" }}>
              <label className="control-field" style={{ minWidth: "220px" }}>
                <span>Week start (Monday)</span>
                <input
                  type="date"
                  value={weeklyDigestWeekStart}
                  onChange={(event) => setWeeklyDigestWeekStart(event.target.value)}
                />
              </label>
            </div>

            {!weeklyDigest ? (
              <p>{loadingWeeklyDigest ? t("cmdb.actions.loading") : "No weekly digest generated yet."}</p>
            ) : (
              <>
                <p className="section-note">
                  digest_key={weeklyDigest.digest_key} | generated_at={new Date(weeklyDigest.generated_at).toLocaleString()} | week=
                  {weeklyDigest.week_start}..{weeklyDigest.week_end}
                </p>
                <div className="detail-grid">
                  <div className="detail-panel">
                    <strong>Alert and ticket health</strong>
                    <p className="inline-note">
                      critical={weeklyDigest.metrics.open_critical_alerts} | warning={weeklyDigest.metrics.open_warning_alerts}
                    </p>
                    <p className="inline-note">
                      suppressed_threads={weeklyDigest.metrics.suppressed_alert_threads} | stale_tickets={weeklyDigest.metrics.stale_open_tickets}
                    </p>
                  </div>
                  <div className="detail-panel">
                    <strong>Execution and continuity</strong>
                    <p className="inline-note">
                      workflow_approval={weeklyDigest.metrics.workflow_approval_backlog} | playbook_approval={weeklyDigest.metrics.playbook_approval_backlog}
                    </p>
                    <p className="inline-note">
                      backup_failed={weeklyDigest.metrics.backup_failed_policies} | drill_failed={weeklyDigest.metrics.drill_failed_policies}
                    </p>
                    <p className="inline-note">
                      evidence_required={weeklyDigest.metrics.continuity_runs_requiring_evidence}
                      {" | "}evidence_covered={weeklyDigest.metrics.continuity_runs_with_evidence}
                      {" | "}evidence_missing={weeklyDigest.metrics.continuity_runs_missing_evidence}
                    </p>
                  </div>
                  <div className="detail-panel">
                    <strong>Auth risk signals</strong>
                    <p className="inline-note">
                      locked_accounts={weeklyDigest.metrics.locked_local_accounts}
                    </p>
                    <p className="inline-note">
                      local_without_mfa={weeklyDigest.metrics.local_accounts_without_mfa}
                    </p>
                  </div>
                </div>
                <div className="detail-grid" style={{ marginTop: "0.5rem" }}>
                  <div className="detail-panel">
                    <strong>Top risks</strong>
                    <ul style={{ marginTop: "0.35rem", marginBottom: 0 }}>
                      {(weeklyDigest.top_risks ?? []).map((item: string, idx: number) => (
                        <li key={`weekly-risk-${idx}`}>{item}</li>
                      ))}
                    </ul>
                  </div>
                  <div className="detail-panel">
                    <strong>Unresolved items</strong>
                    <ul style={{ marginTop: "0.35rem", marginBottom: 0 }}>
                      {(weeklyDigest.unresolved_items ?? []).map((item: string, idx: number) => (
                        <li key={`weekly-unresolved-${idx}`}>{item}</li>
                      ))}
                    </ul>
                  </div>
                  <div className="detail-panel">
                    <strong>Recommended next actions</strong>
                    <ul style={{ marginTop: "0.35rem", marginBottom: 0 }}>
                      {(weeklyDigest.recommended_actions ?? []).map((item: string, idx: number) => (
                        <li key={`weekly-action-${idx}`}>{item}</li>
                      ))}
                    </ul>
                  </div>
                </div>
              </>
            )}
          </div>

          <div className="detail-panel" style={{ marginBottom: "0.85rem" }}>
            <div className="toolbar-row" style={{ justifyContent: "space-between" }}>
              <h3 style={{ ...subSectionTitleStyle, marginTop: 0, marginBottom: 0 }}>Shift handover digest</h3>
              <div className="toolbar-row">
                <button onClick={() => void loadHandoverDigest()} disabled={loadingHandoverDigest}>
                  {loadingHandoverDigest ? t("cmdb.actions.loading") : "Generate handover"}
                </button>
                <button onClick={() => void loadHandoverReminders()} disabled={loadingHandoverReminders}>
                  {loadingHandoverReminders ? t("cmdb.actions.loading") : "Refresh reminders"}
                </button>
                <button onClick={() => void exportHandoverDigest("csv")} disabled={exportingHandoverDigest}>
                  {exportingHandoverDigest ? t("cmdb.actions.loading") : "Export CSV"}
                </button>
                <button onClick={() => void exportHandoverDigest("json")} disabled={exportingHandoverDigest}>
                  {exportingHandoverDigest ? t("cmdb.actions.loading") : "Export JSON"}
                </button>
                <button onClick={() => void exportHandoverReminders("csv")} disabled={exportingHandoverReminders}>
                  {exportingHandoverReminders ? t("cmdb.actions.loading") : "Export reminders CSV"}
                </button>
                <button onClick={() => void exportHandoverReminders("json")} disabled={exportingHandoverReminders}>
                  {exportingHandoverReminders ? t("cmdb.actions.loading") : "Export reminders JSON"}
                </button>
              </div>
            </div>
            {handoverDigestNotice && <p className="banner banner-success">{handoverDigestNotice}</p>}

            <div className="toolbar-row" style={{ marginBottom: "0.55rem" }}>
              <label className="control-field" style={{ minWidth: "220px" }}>
                <span>Shift date</span>
                <input
                  type="date"
                  value={handoverDigestShiftDate}
                  onChange={(event) => setHandoverDigestShiftDate(event.target.value)}
                />
              </label>
            </div>

            {!handoverDigest ? (
              <p>{loadingHandoverDigest ? t("cmdb.actions.loading") : "No handover digest generated yet."}</p>
            ) : (
              <>
                <p className="section-note">
                  digest_key={handoverDigest.digest_key} | generated_at={new Date(handoverDigest.generated_at).toLocaleString()} | shift_date={handoverDigest.shift_date}
                </p>
                <div className="toolbar-row" style={{ marginBottom: "0.45rem" }}>
                  <span className="status-chip status-chip-danger">incidents:{handoverDigest.metrics.unresolved_incidents}</span>
                  <span className="status-chip">ticket_backlog:{handoverDigest.metrics.escalation_backlog}</span>
                  <span className="status-chip status-chip-warn">failed_runs:{handoverDigest.metrics.failed_continuity_runs}</span>
                  <span className="status-chip">pending_approvals:{handoverDigest.metrics.pending_approvals}</span>
                  <span className="status-chip status-chip-danger">evidence_gap:{handoverDigest.metrics.restore_evidence_missing_runs}</span>
                  <span className="status-chip status-chip-danger">overdue:{handoverDigest.metrics.overdue_open_items}</span>
                  <span className="status-chip status-chip-warn">ownership_gap:{handoverDigest.metrics.ownership_gap_items}</span>
                  <span className="status-chip status-chip-success">closed:{handoverDigest.metrics.closed_items}</span>
                </div>
                {(handoverDigest.overdue_trend ?? []).length > 0 && (
                  <div className="detail-panel" style={{ marginBottom: "0.55rem" }}>
                    <p className="section-note" style={{ marginTop: 0 }}>overdue trend by shift date</p>
                    <div className="toolbar-row">
                      {(handoverDigest.overdue_trend ?? []).map((point: any) => (
                        <span key={`handover-trend-${point.shift_date}`} className="status-chip">
                          {point.shift_date}: open={point.open_items}, overdue={point.overdue_items}
                        </span>
                      ))}
                    </div>
                  </div>
                )}

                {handoverReminders && (
                  <div className="detail-panel" style={{ marginBottom: "0.55rem" }}>
                    <p className="section-note" style={{ marginTop: 0 }}>
                      reminder_total={handoverReminders.total} | digest_key={handoverReminders.digest_key}
                    </p>
                    {(handoverReminders.items ?? []).length === 0 ? (
                      <p className="inline-note">No overdue or ownership-gap reminder item.</p>
                    ) : (
                      <div style={{ overflowX: "auto" }}>
                        <table style={{ borderCollapse: "collapse", minWidth: "1020px", width: "100%" }}>
                          <thead>
                            <tr>
                              <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Item</th>
                              <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Owner/Action</th>
                              <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Reminder reason</th>
                            </tr>
                          </thead>
                          <tbody>
                            {(handoverReminders.items ?? []).slice(0, 30).map((item: any) => (
                              <tr key={`handover-reminder-${item.item_key}`}>
                                <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                                  {item.item_key}
                                  <div className="inline-note">{item.source_type}#{item.source_id}</div>
                                </td>
                                <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                                  {item.next_owner}
                                  <div className="inline-note">{item.next_action}</div>
                                </td>
                                <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                                  {item.overdue ? `overdue by ${item.overdue_days} day(s)` : "not overdue"}
                                  <div className="inline-note">
                                    violations={(item.ownership_violations ?? []).length > 0
                                      ? item.ownership_violations.join("|")
                                      : "-"}
                                  </div>
                                </td>
                              </tr>
                            ))}
                          </tbody>
                        </table>
                      </div>
                    )}
                  </div>
                )}

                {(handoverDigest.items ?? []).length === 0 ? (
                  <p>No carryover item.</p>
                ) : (
                  <div style={{ overflowX: "auto" }}>
                    <table style={{ borderCollapse: "collapse", minWidth: "1280px", width: "100%" }}>
                      <thead>
                        <tr>
                          <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Item</th>
                          <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Owner</th>
                          <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Carryover</th>
                          <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Risk</th>
                          <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Observed</th>
                          <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>Actions</th>
                        </tr>
                      </thead>
                      <tbody>
                        {(handoverDigest.items ?? []).slice(0, 40).map((item: any) => (
                          <tr
                            key={`handover-item-${item.item_key}`}
                            style={item.overdue || (item.ownership_violations ?? []).length > 0
                              ? { backgroundColor: "#fff7ed" }
                              : undefined}
                          >
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              <strong>{item.item_key}</strong>
                              <div>{item.title}</div>
                              <div className="inline-note">{item.source_type}#{item.source_id}</div>
                              <div className="inline-note">{item.source_ref}</div>
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              owner={item.owner}
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              next_owner={item.next_owner}
                              <div className="inline-note">{item.next_action}</div>
                              <div className="inline-note">status={item.status}</div>
                              <div className="inline-note">overdue={item.overdue ? `yes(+${item.overdue_days}d)` : "no"}</div>
                              <div className="inline-note">
                                ownership_violations={(item.ownership_violations ?? []).length > 0
                                  ? item.ownership_violations.join("|")
                                  : "-"}
                              </div>
                              <div className="inline-note">note={item.note ?? "-"}</div>
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              <span className={`status-chip ${item.risk_level === "critical" ? "status-chip-danger" : item.risk_level === "high" ? "status-chip-warn" : ""}`}>
                                {item.risk_level}
                              </span>
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              {new Date(item.observed_at).toLocaleString()}
                            </td>
                            <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                              <button
                                onClick={() => void closeHandoverCarryoverItem(item)}
                                disabled={!canWriteCmdb || item.status === "closed" || closingHandoverItemKey === item.item_key}
                              >
                                {closingHandoverItemKey === item.item_key
                                  ? t("cmdb.actions.loading")
                                  : item.status === "closed"
                                    ? "Closed"
                                    : "Close item"}
                              </button>
                            </td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                )}
              </>
            )}
          </div>

          {!dailyCockpitQueue ? (
            <p>{loadingDailyCockpit ? t("cmdb.dailyCockpit.messages.loading") : t("cmdb.dailyCockpit.messages.noData")}</p>
          ) : dailyCockpitQueue.items.length === 0 ? (
            <p>{t("cmdb.dailyCockpit.messages.empty")}</p>
          ) : (
            <>
              <p className="section-note">
                {t("cmdb.dailyCockpit.summary", {
                  total: dailyCockpitQueue.window.total,
                  visible: dailyCockpitQueue.items.length
                })}
              </p>
              <div style={{ overflowX: "auto" }}>
                <table style={{ borderCollapse: "collapse", minWidth: "1200px", width: "100%" }}>
                  <thead>
                    <tr>
                      <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>{t("cmdb.dailyCockpit.table.type")}</th>
                      <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>{t("cmdb.dailyCockpit.table.priority")}</th>
                      <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>{t("cmdb.dailyCockpit.table.rationale")}</th>
                      <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>{t("cmdb.dailyCockpit.table.scope")}</th>
                      <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>{t("cmdb.dailyCockpit.table.time")}</th>
                      <th style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left" }}>{t("cmdb.dailyCockpit.table.actions")}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {dailyCockpitQueue.items.map((item: any) => (
                      <tr key={item.queue_key}>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                          <span className="status-chip">{item.item_type}</span>
                        </td>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                          <span className={`status-chip ${item.priority_level === "critical" ? "status-chip-danger" : ""}`}>
                            {item.priority_level} ({item.priority_score})
                          </span>
                        </td>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                          <p style={{ margin: 0 }}>{item.rationale}</p>
                          {Array.isArray(item.rationale_details) && item.rationale_details.length > 0 && (
                            <p className="section-note" style={{ marginTop: "0.25rem" }}>
                              {item.rationale_details.join(" | ")}
                            </p>
                          )}
                        </td>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                          {item.site ?? "-"} / {item.department ?? "-"}
                        </td>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                          {new Date(item.observed_at).toLocaleString()}
                        </td>
                        <td style={{ border: "1px solid #ddd", padding: "0.5rem", textAlign: "left", verticalAlign: "top" }}>
                          <div className="toolbar-row">
                            {(item.actions ?? []).map((action: any) => {
                              const actionKey = `${item.queue_key}:${action.key}`;
                              const running = runningDailyCockpitActionKey === actionKey;
                              const disabled = running || (action.requires_write && !canWriteCmdb);
                              return (
                                <button
                                  key={actionKey}
                                  onClick={() => void runDailyCockpitAction(item.queue_key, action)}
                                  disabled={disabled}
                                >
                                  {running ? t("cmdb.actions.loading") : action.label}
                                </button>
                              );
                            })}
                          </div>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </>
          )}
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
                assetStatsStatusBuckets.slice(0, 6).map((bucket: any) => (
                  <div key={`cockpit-status-${bucket.key}`} style={{ display: "grid", gridTemplateColumns: "120px 1fr auto", gap: "0.5rem", marginBottom: "0.35rem", alignItems: "center" }}>
                    <span>{bucket.label}</span>
                    <div style={{ background: "#e2e8f0", height: "8px", borderRadius: "999px", overflow: "hidden" }}>
                      <HorizontalFillBar width={bucketBarWidth(bucket.asset_total, assetStatsStatusMax)} color="#1d4ed8" />
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
                assetStatsDepartmentBuckets.slice(0, 6).map((bucket: any) => (
                  <div key={`cockpit-dept-${bucket.key}`} style={{ display: "grid", gridTemplateColumns: "120px 1fr auto", gap: "0.5rem", marginBottom: "0.35rem", alignItems: "center" }}>
                    <span>{bucket.label}</span>
                    <div style={{ background: "#e2e8f0", height: "8px", borderRadius: "999px", overflow: "hidden" }}>
                      <HorizontalFillBar width={bucketBarWidth(bucket.asset_total, assetStatsDepartmentMax)} color="#0f766e" />
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
                assetStatsBusinessServiceBuckets.slice(0, 6).map((bucket: any) => (
                  <div key={`cockpit-biz-${bucket.key}`} style={{ display: "grid", gridTemplateColumns: "120px 1fr auto", gap: "0.5rem", marginBottom: "0.35rem", alignItems: "center" }}>
                    <span title={bucket.label} style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{bucket.label}</span>
                    <div style={{ background: "#e2e8f0", height: "8px", borderRadius: "999px", overflow: "hidden" }}>
                      <HorizontalFillBar width={bucketBarWidth(bucket.asset_total, assetStatsBusinessServiceMax)} color="#be123c" />
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
    </>
  );
}
