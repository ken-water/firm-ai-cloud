import { SectionCard } from "../components/layout";
import { HorizontalFillBar } from "../components/chart-primitives";

type WorkflowStepKind = "approval" | "script" | "manual";
type NewTicketForm = {
  priority: "low" | "medium" | "high" | "critical";
};

export function WorkflowTicketSections(rawProps: Record<string, unknown>) {
  const {

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
  loadPlaybookCatalog,
  loadPlaybookApprovalRequests,
  loadPlaybookExecutionPolicy,
  loadPlaybookExecutions,
  loadWorkflowLogs,
  loadWorkflowRequests,
  loadWorkflowTemplates,
  loadingPlaybookCatalog,
  loadingPlaybookApprovals,
  loadingPlaybookExecutions,
  loadingPlaybookPolicy,
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
  playbookAssetRef,
  playbookCatalog,
  playbookCategoryFilter,
  playbookCategoryOptions,
  playbookApprovalDecisionNote,
  playbookApprovalRequestNote,
  playbookApprovalRequests,
  playbookApprovalToken,
  playbookConfirmationToken,
  playbookDryRunResponse,
  playbookExecutionPolicy,
  playbookExecutionResult,
  approvePlaybookApprovalRequest,
  approvingPlaybookApprovalId,
  playbookExecutions,
  playbookMaintenanceOverrideConfirmed,
  playbookMaintenanceOverrideReason,
  playbookNotice,
  playbookParamsDraft,
  playbookQuery,
  rejectPlaybookApprovalRequest,
  rejectWorkflowRequest,
  rejectingPlaybookApprovalId,
  rejectingWorkflowRequestId,
  removeWorkflowStepFromDraft,
  requestPlaybookApproval,
  runPlaybookDryRun,
  runPlaybookExecute,
  requestingPlaybookApproval,
  runningPlaybookDryRun,
  runningPlaybookExecute,
  selectedPlaybookApprovalId,
  selectedPlaybook,
  selectedPlaybookKey,
  selectedPlaybookParamFields,
  selectedTicketId,
  selectedTicketSummary,
  selectedWorkflowRequest,
  setPlaybookAssetRef,
  setPlaybookApprovalDecisionNote,
  setPlaybookApprovalRequestNote,
  setPlaybookApprovalToken,
  setPlaybookCategoryFilter,
  setPlaybookConfirmationToken,
  setPlaybookMaintenanceOverrideConfirmed,
  setPlaybookMaintenanceOverrideReason,
  setPlaybookParamsDraft,
  setPlaybookQuery,
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
  workflowTemplates,
  } = rawProps as any;

  const requiresPlaybookApproval = Boolean(
    selectedPlaybook
    && (
      selectedPlaybook.requires_confirmation
      || selectedPlaybook.risk_level === "high"
      || selectedPlaybook.risk_level === "critical"
    )
  );
  const selectedDryRunId = playbookDryRunResponse?.execution?.id ?? null;
  const approvalRows = (playbookApprovalRequests as any[]).filter((item) => {
    if (!selectedPlaybookKey) {
      return true;
    }
    return item.playbook_key === selectedPlaybookKey;
  });
  const selectedApproval = approvalRows.find((item) => String(item.id) === selectedPlaybookApprovalId) ?? null;

  return (
    <>
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
                workflowStatusBuckets.slice(0, 8).map((bucket: any) => (
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
                      <HorizontalFillBar width={bucketBarWidth(bucket.asset_total, workflowStatusMax)} color="#1d4ed8" />
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
                workflowTemplateUsageBuckets.slice(0, 8).map((bucket: any) => (
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
                      <HorizontalFillBar width={bucketBarWidth(bucket.asset_total, workflowTemplateUsageMax)} color="#0f766e" />
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
                workflowRequesterBuckets.slice(0, 8).map((bucket: any) => (
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
                      <HorizontalFillBar width={bucketBarWidth(bucket.asset_total, workflowRequesterMax)} color="#be123c" />
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
                  {workflowDailyTrend.map((point: any) => (
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
                {workflowReportStatusOptions.map((status: any) => (
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
                {workflowReportTemplateOptions.map((templateName: any) => (
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
                workflowReportStatusBuckets.map((bucket: any) => (
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
                      <HorizontalFillBar width={bucketBarWidth(bucket.asset_total, workflowReportStatusMax)} color="#0f766e" />
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
                workflowReportTemplateBuckets.slice(0, 10).map((bucket: any) => (
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
                      <HorizontalFillBar width={bucketBarWidth(bucket.asset_total, workflowReportTemplateMax)} color="#1d4ed8" />
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
                  {workflowReportDailyTrend.map((point: any) => (
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
                      {workflowTemplateTrendRanks.slice(0, 8).map((item: any) => (
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
                      {workflowRequesterTrendRanks.slice(0, 8).map((item: any) => (
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
                  {workflowReportRows.slice(0, 200).map((item: any) => (
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

      {visibleSections.has("section-playbook-library") && (
        <SectionCard
          id="section-playbook-library"
          title={t("cmdb.playbooks.title")}
          actions={(
            <div className="toolbar-row">
              <button onClick={() => void loadPlaybookCatalog()} disabled={loadingPlaybookCatalog}>
                {loadingPlaybookCatalog ? t("cmdb.actions.loading") : t("cmdb.playbooks.actions.refreshCatalog")}
              </button>
              <button onClick={() => void loadPlaybookExecutions()} disabled={loadingPlaybookExecutions}>
                {loadingPlaybookExecutions ? t("cmdb.actions.loading") : t("cmdb.playbooks.actions.refreshExecutions")}
              </button>
              <button onClick={() => void loadPlaybookApprovalRequests()} disabled={loadingPlaybookApprovals}>
                {loadingPlaybookApprovals ? t("cmdb.actions.loading") : "Refresh approvals"}
              </button>
            </div>
          )}
        >
          {playbookNotice && <p className="banner banner-success">{playbookNotice}</p>}
          <p className="section-note">
            {t("cmdb.playbooks.summary", {
              catalog: playbookCatalog.length,
              executions: playbookExecutions.length,
              selected: selectedPlaybookKey || "-"
            })}
          </p>
          {!canWriteCmdb && <p className="inline-note">{t("cmdb.playbooks.messages.readOnlyHint")}</p>}

          <div className="detail-panel" style={{ marginBottom: "0.75rem" }}>
            <div className="toolbar-row" style={{ justifyContent: "space-between" }}>
              <strong>Execution policy</strong>
              <button onClick={() => void loadPlaybookExecutionPolicy()} disabled={loadingPlaybookPolicy}>
                {loadingPlaybookPolicy ? t("cmdb.actions.loading") : "Refresh policy"}
              </button>
            </div>
            {!playbookExecutionPolicy ? (
              <p style={{ marginTop: "0.5rem", marginBottom: 0 }}>Policy not loaded.</p>
            ) : (
              <>
                <p className="section-note" style={{ marginTop: "0.4rem", marginBottom: "0.3rem" }}>
                  timezone={playbookExecutionPolicy.policy.timezone_name} | now=
                  {new Date(playbookExecutionPolicy.runtime.timezone_now).toLocaleString()} | freeze=
                  {playbookExecutionPolicy.policy.change_freeze_enabled ? "on" : "off"} | in_window=
                  {playbookExecutionPolicy.runtime.in_maintenance_window ? "yes" : "no"}
                </p>
                {playbookExecutionPolicy.runtime.blocked_reason && (
                  <p className="inline-note" style={{ marginTop: 0 }}>
                    blocked_reason={playbookExecutionPolicy.runtime.blocked_reason}
                    {playbookExecutionPolicy.runtime.next_allowed_at
                      ? ` | next_allowed_at=${new Date(playbookExecutionPolicy.runtime.next_allowed_at).toLocaleString()}`
                      : ""}
                  </p>
                )}
              </>
            )}
          </div>

          <div className="detail-panel" style={{ marginBottom: "0.75rem" }}>
            <div className="toolbar-row" style={{ justifyContent: "space-between" }}>
              <strong>High-risk approval queue</strong>
              <button onClick={() => void loadPlaybookApprovalRequests()} disabled={loadingPlaybookApprovals}>
                {loadingPlaybookApprovals ? t("cmdb.actions.loading") : "Refresh queue"}
              </button>
            </div>
            {approvalRows.length === 0 ? (
              <p style={{ marginTop: "0.5rem", marginBottom: 0 }}>No approval request.</p>
            ) : (
              <div style={{ overflowX: "auto", marginTop: "0.5rem" }}>
                <table style={{ borderCollapse: "collapse", minWidth: "860px", width: "100%" }}>
                  <thead>
                    <tr>
                      <th style={cellStyle}>ID</th>
                      <th style={cellStyle}>Dry-run</th>
                      <th style={cellStyle}>Status</th>
                      <th style={cellStyle}>Requester</th>
                      <th style={cellStyle}>Approver</th>
                      <th style={cellStyle}>Expires</th>
                      <th style={cellStyle}>Used</th>
                    </tr>
                  </thead>
                  <tbody>
                    {approvalRows.slice(0, 20).map((item: any) => (
                      <tr key={`playbook-approval-row-${item.id}`}>
                        <td style={cellStyle}>#{item.id}</td>
                        <td style={cellStyle}>#{item.dry_run_execution_id}</td>
                        <td style={cellStyle}>
                          <span className={statusChipClass(item.status)}>{item.status}</span>
                        </td>
                        <td style={cellStyle}>{item.requester}</td>
                        <td style={cellStyle}>{item.approver ?? "-"}</td>
                        <td style={cellStyle}>{new Date(item.expires_at).toLocaleString()}</td>
                        <td style={cellStyle}>{item.used_at ? new Date(item.used_at).toLocaleString() : "-"}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </div>

          <div className="filter-grid" style={{ marginBottom: "0.75rem" }}>
            <label className="control-field">
              <span>{t("cmdb.playbooks.filters.category")}</span>
              <select value={playbookCategoryFilter} onChange={(event) => setPlaybookCategoryFilter(event.target.value)}>
                <option value="all">{t("cmdb.playbooks.filters.allCategories")}</option>
                {playbookCategoryOptions.map((category: string) => (
                  <option key={`playbook-category-${category}`} value={category}>
                    {category}
                  </option>
                ))}
              </select>
            </label>
            <label className="control-field">
              <span>{t("cmdb.playbooks.filters.query")}</span>
              <input
                value={playbookQuery}
                onChange={(event) => setPlaybookQuery(event.target.value)}
                placeholder={t("cmdb.playbooks.filters.queryPlaceholder")}
              />
            </label>
            <label className="control-field">
              <span>{t("cmdb.playbooks.form.assetRef")}</span>
              <input
                value={playbookAssetRef}
                onChange={(event) => setPlaybookAssetRef(event.target.value)}
                placeholder="asset-101"
              />
            </label>
          </div>

          {loadingPlaybookCatalog && playbookCatalog.length === 0 ? (
            <p>{t("cmdb.playbooks.messages.loadingCatalog")}</p>
          ) : playbookCatalog.length === 0 ? (
            <p>{t("cmdb.playbooks.messages.noCatalog")}</p>
          ) : (
            <div style={{ overflowX: "auto", marginBottom: "1rem" }}>
              <table style={{ borderCollapse: "collapse", minWidth: "980px", width: "100%" }}>
                <thead>
                  <tr>
                    <th style={cellStyle}>{t("cmdb.playbooks.table.catalog.key")}</th>
                    <th style={cellStyle}>{t("cmdb.playbooks.table.catalog.name")}</th>
                    <th style={cellStyle}>{t("cmdb.playbooks.table.catalog.category")}</th>
                    <th style={cellStyle}>{t("cmdb.playbooks.table.catalog.risk")}</th>
                    <th style={cellStyle}>{t("cmdb.playbooks.table.catalog.confirmation")}</th>
                    <th style={cellStyle}>{t("cmdb.playbooks.table.catalog.actions")}</th>
                  </tr>
                </thead>
                <tbody>
                  {playbookCatalog.map((item: any) => (
                    <tr key={`playbook-catalog-${item.key}`}>
                      <td style={cellStyle}>{item.key}</td>
                      <td style={cellStyle}>{item.name}</td>
                      <td style={cellStyle}>{item.category}</td>
                      <td style={cellStyle}>
                        <span className={statusChipClass(item.risk_level)}>{item.risk_level}</span>
                      </td>
                      <td style={cellStyle}>{item.requires_confirmation ? "yes" : "no"}</td>
                      <td style={cellStyle}>
                        <button onClick={() => setSelectedPlaybookKey(item.key)}>
                          {selectedPlaybookKey === item.key
                            ? t("cmdb.playbooks.actions.selected")
                            : t("cmdb.playbooks.actions.select")}
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}

          {!selectedPlaybook ? (
            <p>{t("cmdb.playbooks.messages.selectPlaybook")}</p>
          ) : (
            <div className="detail-grid" style={{ marginBottom: "1rem" }}>
              <div className="detail-panel">
                <h3 style={subSectionTitleStyle}>{t("cmdb.playbooks.form.title")}</h3>
                <p className="section-note">
                  {t("cmdb.playbooks.form.playbookHint", {
                    key: selectedPlaybook.key,
                    risk: selectedPlaybook.risk_level
                  })}
                </p>
                {selectedPlaybookParamFields.length === 0 ? (
                  <p>{t("cmdb.playbooks.messages.noParams")}</p>
                ) : (
                  <div className="form-grid">
                    {selectedPlaybookParamFields.map((field: any) => (
                      <label key={`playbook-param-${field.key}`} className="control-field">
                        <span>
                          {field.key}
                          {field.required ? " *" : ""}
                        </span>
                        {field.type === "enum" ? (
                          <select
                            value={playbookParamsDraft[field.key] ?? ""}
                            onChange={(event) =>
                              setPlaybookParamsDraft((prev: any) => ({ ...prev, [field.key]: event.target.value }))
                            }
                          >
                            <option value="">{t("cmdb.playbooks.form.selectOption")}</option>
                            {(field.options ?? []).map((option: string) => (
                              <option key={`playbook-param-${field.key}-${option}`} value={option}>
                                {option}
                              </option>
                            ))}
                          </select>
                        ) : field.type === "boolean" ? (
                          <select
                            value={playbookParamsDraft[field.key] ?? ""}
                            onChange={(event) =>
                              setPlaybookParamsDraft((prev: any) => ({ ...prev, [field.key]: event.target.value }))
                            }
                          >
                            <option value="">{t("cmdb.playbooks.form.selectOption")}</option>
                            <option value="true">true</option>
                            <option value="false">false</option>
                          </select>
                        ) : (
                          <input
                            type={field.type === "integer" || field.type === "number" ? "number" : "text"}
                            value={playbookParamsDraft[field.key] ?? ""}
                            onChange={(event) =>
                              setPlaybookParamsDraft((prev: any) => ({ ...prev, [field.key]: event.target.value }))
                            }
                            placeholder={String(field.default ?? "")}
                          />
                        )}
                      </label>
                    ))}
                  </div>
                )}

                {requiresPlaybookApproval && (
                  <>
                    <div className="detail-panel" style={{ marginTop: "0.75rem" }}>
                      <strong>Approval flow</strong>
                      <p className="section-note" style={{ marginTop: "0.45rem", marginBottom: "0.35rem" }}>
                        Dry-run {selectedDryRunId ? `#${selectedDryRunId}` : "not ready"}.
                      </p>
                      <label className="control-field" style={{ marginTop: "0.45rem" }}>
                        <span>Approval request note</span>
                        <input
                          value={playbookApprovalRequestNote}
                          onChange={(event) => setPlaybookApprovalRequestNote(event.target.value)}
                          placeholder="Why this high-risk execution is needed"
                        />
                      </label>
                      <div className="toolbar-row" style={{ marginTop: "0.45rem" }}>
                        <button
                          onClick={() => void requestPlaybookApproval()}
                          disabled={!canWriteCmdb || requestingPlaybookApproval || !selectedDryRunId}
                        >
                          {requestingPlaybookApproval ? t("cmdb.actions.loading") : "Request approval"}
                        </button>
                      </div>
                      <label className="control-field" style={{ marginTop: "0.45rem" }}>
                        <span>Select approval request</span>
                        <select
                          value={selectedPlaybookApprovalId}
                          onChange={(event) => {
                            const nextId = event.target.value;
                            setSelectedPlaybookApprovalId(nextId);
                            const next = approvalRows.find((item: any) => String(item.id) === nextId);
                            setPlaybookApprovalToken(next?.approval_token ?? "");
                          }}
                        >
                          <option value="">Select request</option>
                          {approvalRows.map((item: any) => (
                            <option key={`playbook-approval-option-${item.id}`} value={String(item.id)}>
                              #{item.id} | dry-run #{item.dry_run_execution_id} | {item.status}
                            </option>
                          ))}
                        </select>
                      </label>
                      <label className="control-field" style={{ marginTop: "0.45rem" }}>
                        <span>Approval decision note</span>
                        <input
                          value={playbookApprovalDecisionNote}
                          onChange={(event) => setPlaybookApprovalDecisionNote(event.target.value)}
                          placeholder="Approval/rejection rationale"
                        />
                      </label>
                      <div className="toolbar-row" style={{ marginTop: "0.45rem" }}>
                        <button
                          onClick={() => selectedApproval && void approvePlaybookApprovalRequest(selectedApproval.id)}
                          disabled={
                            !canWriteCmdb
                            || !selectedApproval
                            || selectedApproval.status !== "pending"
                            || approvingPlaybookApprovalId === selectedApproval.id
                          }
                        >
                          {selectedApproval && approvingPlaybookApprovalId === selectedApproval.id
                            ? t("cmdb.actions.loading")
                            : "Approve"}
                        </button>
                        <button
                          onClick={() => selectedApproval && void rejectPlaybookApprovalRequest(selectedApproval.id)}
                          disabled={
                            !canWriteCmdb
                            || !selectedApproval
                            || selectedApproval.status !== "pending"
                            || rejectingPlaybookApprovalId === selectedApproval.id
                          }
                        >
                          {selectedApproval && rejectingPlaybookApprovalId === selectedApproval.id
                            ? t("cmdb.actions.loading")
                            : "Reject"}
                        </button>
                      </div>
                      {selectedApproval && (
                        <p className="inline-note" style={{ marginTop: "0.5rem", marginBottom: 0 }}>
                          status={selectedApproval.status}
                          {selectedApproval.approver ? ` | approver=${selectedApproval.approver}` : ""}
                          {selectedApproval.approved_at
                            ? ` | approved_at=${new Date(selectedApproval.approved_at).toLocaleString()}`
                            : ""}
                          {selectedApproval.expires_at
                            ? ` | expires_at=${new Date(selectedApproval.expires_at).toLocaleString()}`
                            : ""}
                        </p>
                      )}
                    </div>

                    <label className="control-field" style={{ marginTop: "0.75rem" }}>
                      <span>{t("cmdb.playbooks.form.confirmationToken")}</span>
                      <input
                        value={playbookConfirmationToken}
                        onChange={(event) => setPlaybookConfirmationToken(event.target.value)}
                        placeholder="PBK-XXXXXXX"
                      />
                    </label>
                    <label className="control-field" style={{ marginTop: "0.45rem" }}>
                      <span>Approval token</span>
                      <input
                        value={playbookApprovalToken}
                        onChange={(event) => setPlaybookApprovalToken(event.target.value)}
                        placeholder="approval token from approver"
                      />
                    </label>
                    <label className="control-field" style={{ marginTop: "0.45rem" }}>
                      <span>Maintenance override reason (required only when blocked)</span>
                      <input
                        value={playbookMaintenanceOverrideReason}
                        onChange={(event) => setPlaybookMaintenanceOverrideReason(event.target.value)}
                        placeholder="Emergency reason and risk acceptance"
                      />
                    </label>
                    <label className="control-field" style={{ marginTop: "0.45rem" }}>
                      <span>Maintenance override confirmation</span>
                      <select
                        value={playbookMaintenanceOverrideConfirmed ? "true" : "false"}
                        onChange={(event) => setPlaybookMaintenanceOverrideConfirmed(event.target.value === "true")}
                      >
                        <option value="false">false</option>
                        <option value="true">true</option>
                      </select>
                    </label>
                  </>
                )}

                <div className="toolbar-row" style={{ marginTop: "0.75rem" }}>
                  <button onClick={() => void runPlaybookDryRun()} disabled={runningPlaybookDryRun || !canWriteCmdb}>
                    {runningPlaybookDryRun ? t("cmdb.actions.loading") : t("cmdb.playbooks.actions.dryRun")}
                  </button>
                  <button onClick={() => void runPlaybookExecute()} disabled={runningPlaybookExecute || !canWriteCmdb}>
                    {runningPlaybookExecute ? t("cmdb.actions.loading") : t("cmdb.playbooks.actions.execute")}
                  </button>
                </div>
              </div>

              <div className="detail-panel">
                <h3 style={subSectionTitleStyle}>{t("cmdb.playbooks.results.title")}</h3>
                {!playbookDryRunResponse ? (
                  <p>{t("cmdb.playbooks.results.noDryRun")}</p>
                ) : (
                  <>
                    <p className="section-note">
                      {t("cmdb.playbooks.results.dryRunSummary", {
                        id: playbookDryRunResponse.execution.id,
                        risk: playbookDryRunResponse.risk_summary.risk_level
                      })}
                    </p>
                    <p className="section-note">{playbookDryRunResponse.risk_summary.summary}</p>
                    {playbookDryRunResponse.confirmation && (
                      <p className="inline-note">
                        {t("cmdb.playbooks.results.confirmationHint", {
                          token: playbookDryRunResponse.confirmation.token,
                          expiresAt: new Date(playbookDryRunResponse.confirmation.expires_at).toLocaleString()
                        })}
                      </p>
                    )}
                    <ol style={{ marginTop: "0.35rem" }}>
                      {(playbookDryRunResponse.execution.planned_steps ?? []).map((step: string, idx: number) => (
                        <li key={`playbook-dry-step-${idx}`}>{step}</li>
                      ))}
                    </ol>
                  </>
                )}
                {!playbookExecutionResult ? (
                  <p>{t("cmdb.playbooks.results.noExecution")}</p>
                ) : (
                  <>
                    <p className="section-note">
                      {t("cmdb.playbooks.results.executionSummary", {
                        id: playbookExecutionResult.id,
                        status: playbookExecutionResult.status
                      })}
                    </p>
                    <p className="section-note">
                      {t("cmdb.playbooks.results.executionActor", {
                        actor: playbookExecutionResult.actor,
                        finishedAt: playbookExecutionResult.finished_at
                          ? new Date(playbookExecutionResult.finished_at).toLocaleString()
                          : "-"
                      })}
                    </p>
                  </>
                )}
              </div>
            </div>
          )}

          <h3 style={subSectionTitleStyle}>{t("cmdb.playbooks.history.title")}</h3>
          {loadingPlaybookExecutions && playbookExecutions.length === 0 ? (
            <p>{t("cmdb.playbooks.messages.loadingExecutions")}</p>
          ) : playbookExecutions.length === 0 ? (
            <p>{t("cmdb.playbooks.messages.noExecutions")}</p>
          ) : (
            <div style={{ overflowX: "auto" }}>
              <table style={{ borderCollapse: "collapse", minWidth: "980px", width: "100%" }}>
                <thead>
                  <tr>
                    <th style={cellStyle}>{t("cmdb.playbooks.table.history.id")}</th>
                    <th style={cellStyle}>{t("cmdb.playbooks.table.history.playbook")}</th>
                    <th style={cellStyle}>{t("cmdb.playbooks.table.history.mode")}</th>
                    <th style={cellStyle}>{t("cmdb.playbooks.table.history.status")}</th>
                    <th style={cellStyle}>{t("cmdb.playbooks.table.history.actor")}</th>
                    <th style={cellStyle}>{t("cmdb.playbooks.table.history.asset")}</th>
                    <th style={cellStyle}>{t("cmdb.playbooks.table.history.createdAt")}</th>
                  </tr>
                </thead>
                <tbody>
                  {playbookExecutions.map((item: any) => (
                    <tr key={`playbook-execution-${item.id}`}>
                      <td style={cellStyle}>#{item.id}</td>
                      <td style={cellStyle}>{item.playbook_key}</td>
                      <td style={cellStyle}>{item.mode}</td>
                      <td style={cellStyle}>
                        <span className={statusChipClass(item.status)}>{item.status}</span>
                      </td>
                      <td style={cellStyle}>{item.actor}</td>
                      <td style={cellStyle}>{item.asset_ref ?? "-"}</td>
                      <td style={cellStyle}>{new Date(item.created_at).toLocaleString()}</td>
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
                      setNewWorkflowStep((prev: any) => ({
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
                      setNewWorkflowStep((prev: any) => ({
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
                      setNewWorkflowStep((prev: any) => ({
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
                      setNewWorkflowStep((prev: any) => ({
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
                      setNewWorkflowStep((prev: any) => ({
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
                      setNewWorkflowStep((prev: any) => ({
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
                      setNewWorkflowStep((prev: any) => ({
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
                  {newWorkflowTemplateSteps.map((step: any) => (
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
                  {workflowTemplates.map((template: any) => (
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
                    setNewWorkflowRequest((prev: any) => ({
                      ...prev,
                      template_id: event.target.value
                    }))
                  }
                >
                  <option value="">{t("cmdb.workflow.form.selectTemplate")}</option>
                  {workflowTemplates.map((template: any) => (
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
                    setNewWorkflowRequest((prev: any) => ({
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
                    setNewWorkflowRequest((prev: any) => ({
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
                  {workflowRequests.map((request: any) => (
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
                  {workflowLogs.map((log: any) => (
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

      {visibleSections.has("section-tickets") && (
        <SectionCard
          id="section-tickets"
          title={t("cmdb.tickets.title")}
          actions={(
            <div className="toolbar-row">
              <button onClick={() => void loadTickets()} disabled={loadingTickets}>
                {loadingTickets ? t("cmdb.actions.loading") : t("cmdb.tickets.actions.refresh")}
              </button>
              <button onClick={() => void loadTickets()} disabled={loadingTickets}>
                {t("cmdb.tickets.actions.applyFilters")}
              </button>
              <button
                onClick={() => {
                  setTicketStatusFilter("all");
                  setTicketPriorityFilter("all");
                  setTicketQueryFilter("");
                }}
              >
                {t("cmdb.tickets.actions.resetFilters")}
              </button>
            </div>
          )}
        >
          {ticketNotice && <p className="banner banner-success">{ticketNotice}</p>}
          <p className="section-note">
            {t("cmdb.tickets.summary", {
              total: tickets.length,
              selected: selectedTicketSummary?.ticket_no ?? "-"
            })}
          </p>
          {!canWriteCmdb && <p className="inline-note">{t("cmdb.tickets.messages.readOnlyHint")}</p>}

          <div className="filter-grid">
            <label className="control-field">
              <span>{t("cmdb.tickets.filters.status")}</span>
              <select value={ticketStatusFilter} onChange={(event) => setTicketStatusFilter(event.target.value)}>
                <option value="all">{t("cmdb.tickets.filters.all")}</option>
                <option value="open">open</option>
                <option value="in_progress">in_progress</option>
                <option value="resolved">resolved</option>
                <option value="closed">closed</option>
                <option value="cancelled">cancelled</option>
              </select>
            </label>
            <label className="control-field">
              <span>{t("cmdb.tickets.filters.priority")}</span>
              <select value={ticketPriorityFilter} onChange={(event) => setTicketPriorityFilter(event.target.value)}>
                <option value="all">{t("cmdb.tickets.filters.all")}</option>
                <option value="low">low</option>
                <option value="medium">medium</option>
                <option value="high">high</option>
                <option value="critical">critical</option>
              </select>
            </label>
            <label className="control-field">
              <span>{t("cmdb.tickets.filters.query")}</span>
              <input
                value={ticketQueryFilter}
                onChange={(event) => setTicketQueryFilter(event.target.value)}
                placeholder={t("cmdb.tickets.filters.queryPlaceholder")}
              />
            </label>
          </div>

          {canWriteCmdb && (
            <>
              <h3 style={subSectionTitleStyle}>{t("cmdb.tickets.createTitle")}</h3>
              <div className="form-grid">
                <label className="control-field">
                  <span>{t("cmdb.tickets.form.title")}</span>
                  <input
                    value={newTicket.title}
                    onChange={(event) => setNewTicket((prev: any) => ({ ...prev, title: event.target.value }))}
                    placeholder={t("cmdb.tickets.form.titlePlaceholder")}
                  />
                </label>
                <label className="control-field">
                  <span>{t("cmdb.tickets.form.priority")}</span>
                  <select
                    value={newTicket.priority}
                    onChange={(event) =>
                      setNewTicket((prev: any) => ({ ...prev, priority: event.target.value as NewTicketForm["priority"] }))
                    }
                  >
                    <option value="low">low</option>
                    <option value="medium">medium</option>
                    <option value="high">high</option>
                    <option value="critical">critical</option>
                  </select>
                </label>
                <label className="control-field">
                  <span>{t("cmdb.tickets.form.category")}</span>
                  <input
                    value={newTicket.category}
                    onChange={(event) => setNewTicket((prev: any) => ({ ...prev, category: event.target.value }))}
                    placeholder="incident"
                  />
                </label>
                <label className="control-field">
                  <span>{t("cmdb.tickets.form.assignee")}</span>
                  <input
                    value={newTicket.assignee}
                    onChange={(event) => setNewTicket((prev: any) => ({ ...prev, assignee: event.target.value }))}
                    placeholder="ops-oncall"
                  />
                </label>
              </div>
              <div className="form-grid" style={{ marginTop: "0.5rem" }}>
                <label className="control-field">
                  <span>{t("cmdb.tickets.form.assetIds")}</span>
                  <input
                    value={newTicket.asset_ids_csv}
                    onChange={(event) => setNewTicket((prev: any) => ({ ...prev, asset_ids_csv: event.target.value }))}
                    placeholder="1,2,3"
                  />
                </label>
                <label className="control-field">
                  <span>{t("cmdb.tickets.form.alertSource")}</span>
                  <input
                    value={newTicket.alert_source}
                    onChange={(event) => setNewTicket((prev: any) => ({ ...prev, alert_source: event.target.value }))}
                    placeholder="zabbix"
                  />
                </label>
                <label className="control-field">
                  <span>{t("cmdb.tickets.form.alertKey")}</span>
                  <input
                    value={newTicket.alert_key}
                    onChange={(event) => setNewTicket((prev: any) => ({ ...prev, alert_key: event.target.value }))}
                    placeholder="problemid:123456"
                  />
                </label>
                <label className="control-field">
                  <span>{t("cmdb.tickets.form.alertSeverity")}</span>
                  <input
                    value={newTicket.alert_severity}
                    onChange={(event) => setNewTicket((prev: any) => ({ ...prev, alert_severity: event.target.value }))}
                    placeholder="warning"
                  />
                </label>
                <label className="control-field">
                  <span>{t("cmdb.tickets.form.workflowTemplateId")}</span>
                  <input
                    value={newTicket.workflow_template_id}
                    onChange={(event) => setNewTicket((prev: any) => ({ ...prev, workflow_template_id: event.target.value }))}
                    placeholder="1"
                  />
                </label>
                <label className="control-field">
                  <span>{t("cmdb.tickets.form.triggerWorkflow")}</span>
                  <select
                    value={newTicket.trigger_workflow ? "true" : "false"}
                    onChange={(event) =>
                      setNewTicket((prev: any) => ({ ...prev, trigger_workflow: event.target.value === "true" }))
                    }
                  >
                    <option value="false">false</option>
                    <option value="true">true</option>
                  </select>
                </label>
              </div>
              <label className="control-field" style={{ marginTop: "0.5rem" }}>
                <span>{t("cmdb.tickets.form.alertTitle")}</span>
                <input
                  value={newTicket.alert_title}
                  onChange={(event) => setNewTicket((prev: any) => ({ ...prev, alert_title: event.target.value }))}
                  placeholder="Database CPU usage high"
                />
              </label>
              <label className="control-field" style={{ marginTop: "0.5rem" }}>
                <span>{t("cmdb.tickets.form.description")}</span>
                <textarea
                  rows={3}
                  value={newTicket.description}
                  onChange={(event) => setNewTicket((prev: any) => ({ ...prev, description: event.target.value }))}
                  placeholder={t("cmdb.tickets.form.descriptionPlaceholder")}
                />
              </label>
              <div className="toolbar-row" style={{ marginTop: "0.5rem" }}>
                <button onClick={() => void createTicket()} disabled={creatingTicket}>
                  {creatingTicket ? t("cmdb.actions.creating") : t("cmdb.tickets.actions.create")}
                </button>
              </div>
            </>
          )}

          <h3 style={{ ...subSectionTitleStyle, marginTop: "0.8rem" }}>{t("cmdb.tickets.listTitle")}</h3>
          {loadingTickets && tickets.length === 0 ? (
            <p>{t("cmdb.tickets.messages.loading")}</p>
          ) : tickets.length === 0 ? (
            <p>{t("cmdb.tickets.messages.noTickets")}</p>
          ) : (
            <div style={{ overflowX: "auto" }}>
              <table style={{ borderCollapse: "collapse", minWidth: "1120px", width: "100%" }}>
                <thead>
                  <tr>
                    <th style={cellStyle}>{t("cmdb.tickets.table.id")}</th>
                    <th style={cellStyle}>{t("cmdb.tickets.table.title")}</th>
                    <th style={cellStyle}>{t("cmdb.tickets.table.status")}</th>
                    <th style={cellStyle}>{t("cmdb.tickets.table.priority")}</th>
                    <th style={cellStyle}>{t("cmdb.tickets.table.requester")}</th>
                    <th style={cellStyle}>{t("cmdb.tickets.table.links")}</th>
                    <th style={cellStyle}>{t("cmdb.tickets.table.workflow")}</th>
                    <th style={cellStyle}>{t("cmdb.tickets.table.updatedAt")}</th>
                    <th style={cellStyle}>{t("cmdb.tickets.table.actions")}</th>
                  </tr>
                </thead>
                <tbody>
                  {tickets.map((ticket: any) => (
                    <tr key={`ticket-row-${ticket.id}`}>
                      <td style={cellStyle}>{ticket.ticket_no}</td>
                      <td style={cellStyle}>{ticket.title}</td>
                      <td style={cellStyle}>
                        <span className={statusChipClass(ticket.status)}>{ticket.status}</span>
                      </td>
                      <td style={cellStyle}>{ticket.priority}</td>
                      <td style={cellStyle}>{ticket.requester}</td>
                      <td style={cellStyle}>
                        assets:{ticket.asset_link_count} / alerts:{ticket.alert_link_count}
                      </td>
                      <td style={cellStyle}>
                        {ticket.workflow_request_id ? `request #${ticket.workflow_request_id}` : "-"}
                      </td>
                      <td style={cellStyle}>{new Date(ticket.updated_at).toLocaleString()}</td>
                      <td style={cellStyle}>
                        <button
                          onClick={() => {
                            setSelectedTicketId(String(ticket.id));
                            void loadTicketDetail(ticket.id);
                          }}
                        >
                          {t("cmdb.tickets.actions.viewDetail")}
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}

          <h3 style={{ ...subSectionTitleStyle, marginTop: "0.8rem" }}>{t("cmdb.tickets.detailTitle")}</h3>
          {!selectedTicketId ? (
            <p>{t("cmdb.tickets.messages.selectTicket")}</p>
          ) : loadingTicketDetail && !ticketDetail ? (
            <p>{t("cmdb.tickets.messages.loadingDetail")}</p>
          ) : !ticketDetail ? (
            <p>{t("cmdb.tickets.messages.detailEmpty")}</p>
          ) : (
            <div className="detail-grid">
              <div className="detail-panel">
                <h3 style={subSectionTitleStyle}>{ticketDetail.ticket.ticket_no}</h3>
                <p style={{ margin: "0.2rem 0 0.5rem 0" }}>{ticketDetail.ticket.title}</p>
                <p className="section-note">
                  {ticketDetail.ticket.description?.trim().length
                    ? ticketDetail.ticket.description
                    : t("cmdb.tickets.messages.noDescription")}
                </p>
                <div className="toolbar-row" style={{ marginTop: "0.5rem" }}>
                  <span className={statusChipClass(ticketDetail.ticket.status)}>{ticketDetail.ticket.status}</span>
                  <span className="status-chip">priority: {ticketDetail.ticket.priority}</span>
                  <span className="status-chip">category: {ticketDetail.ticket.category}</span>
                </div>
                <p className="inline-note" style={{ marginTop: "0.45rem" }}>
                  requester: {ticketDetail.ticket.requester} | assignee: {ticketDetail.ticket.assignee ?? "-"}
                </p>
                <p className="inline-note">
                  updated: {new Date(ticketDetail.ticket.updated_at).toLocaleString()}
                </p>
                {canWriteCmdb && (
                  <div className="toolbar-row" style={{ marginTop: "0.45rem" }}>
                    <select value={ticketStatusDraft} onChange={(event) => setTicketStatusDraft(event.target.value)}>
                      <option value="open">open</option>
                      <option value="in_progress">in_progress</option>
                      <option value="resolved">resolved</option>
                      <option value="closed">closed</option>
                      <option value="cancelled">cancelled</option>
                    </select>
                    <button
                      onClick={() => void updateTicketStatus(ticketDetail.ticket.id, ticketStatusDraft)}
                      disabled={updatingTicketStatusId === ticketDetail.ticket.id}
                    >
                      {updatingTicketStatusId === ticketDetail.ticket.id
                        ? t("cmdb.actions.loading")
                        : t("cmdb.tickets.actions.updateStatus")}
                    </button>
                  </div>
                )}
              </div>
              <div className="detail-panel">
                <h3 style={subSectionTitleStyle}>{t("cmdb.tickets.detail.assetLinks")}</h3>
                {ticketDetail.asset_links.length === 0 ? (
                  <p>{t("cmdb.tickets.messages.noAssetLinks")}</p>
                ) : (
                  ticketDetail.asset_links.map((link: any) => (
                    <div key={`ticket-asset-${link.asset_id}`} className="toolbar-row" style={{ justifyContent: "space-between" }}>
                      <span>#{link.asset_id} {link.asset_name ?? "-"}</span>
                      <span className="status-chip">{link.asset_class ?? "-"}</span>
                    </div>
                  ))
                )}
              </div>
              <div className="detail-panel">
                <h3 style={subSectionTitleStyle}>{t("cmdb.tickets.detail.alertLinks")}</h3>
                {ticketDetail.alert_links.length === 0 ? (
                  <p>{t("cmdb.tickets.messages.noAlertLinks")}</p>
                ) : (
                  ticketDetail.alert_links.map((link: any) => (
                    <div key={`ticket-alert-${link.alert_source}-${link.alert_key}`} style={{ marginBottom: "0.45rem" }}>
                      <div>
                        <strong>{link.alert_source}</strong> / {link.alert_key}
                      </div>
                      <div className="inline-note">
                        {link.alert_title ?? "-"} | severity: {link.severity ?? "-"}
                      </div>
                    </div>
                  ))
                )}
              </div>
            </div>
          )}
        </SectionCard>
      )}

    </>
  );
}
