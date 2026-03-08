import { HorizontalFillBar } from "../components/chart-primitives";
import { SectionCard } from "../components/layout";

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
    backupRestoreEvidence,
    backupRestoreEvidenceCoverage,
    backupRestoreEvidenceDraft,
    backupRestoreEvidenceMissingRunIds,
    backupRestoreRunStatusFilter,
    changeCalendar,
    changeCalendarConflictDraft,
    changeCalendarConflictResult,
    changeCalendarEndDate,
    changeCalendarNotice,
    changeCalendarStartDate,
    businessWorkspace,
    businessWorkspaceOptions,
    canAccessAdmin,
    canWriteCmdb,
    checkChangeCalendarConflicts,
    closeHandoverCarryoverItem,
    closingHandoverItemKey,
    cockpitCriticalAssets,
    cockpitOperationalAssets,
    completeOpsChecklistItem,
    createSampleAsset,
    departmentWorkspace,
    departmentWorkspaceOptions,
    dailyCockpitDepartmentFilter,
    dailyCockpitNotice,
    dailyCockpitQueue,
    dailyCockpitSiteFilter,
    nextBestActions,
    incidentCommandDetail,
    incidentCommandDraft,
    incidentCommandNotice,
    incidentCommands,
    exportingHandoverDigest,
    exportingWeeklyDigest,
    exportHandoverDigest,
    exportWeeklyDigest,
    handoverDigest,
    handoverDigestNotice,
    handoverDigestShiftDate,
    functionWorkspace,
    loadBackupPolicies,
    loadBackupPolicyRuns,
    loadBackupRestoreEvidence,
    loadChangeCalendar,
    loadHandoverDigest,
    loadIncidentCommandDetail,
    loadIncidentCommands,
    loadNextBestActions,
    loadWeeklyDigest,
    loadDailyCockpitSnapshot,
    loadOpsChecklist,
    loadAssets,
    loadAssetStats,
    loadFieldDefinitions,
    runBackupPolicy,
    runBackupSchedulerTick,
    closeBackupRestoreEvidence,
    runningBackupPolicyActionId,
    loadingDailyCockpit,
    loadingNextBestActions,
    loadingOpsChecklist,
    loadingIncidentCommandDetail,
    loadingIncidentCommands,
    loadingHandoverDigest,
    loadingBackupPolicies,
    loadingBackupPolicyRuns,
    loadingBackupRestoreEvidence,
    loadingChangeCalendar,
    checkingChangeCalendarConflict,
    loadingWeeklyDigest,
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
    savingBackupPolicy,
    saveBackupRestoreEvidence,
    savingBackupRestoreEvidence,
    setBusinessWorkspace,
    setBackupPolicyDraft,
    setBackupRestoreEvidenceDraft,
    setBackupRestoreRunStatusFilter,
    setChangeCalendarConflictDraft,
    setChangeCalendarEndDate,
    setChangeCalendarStartDate,
    setDailyCockpitDepartmentFilter,
    setDailyCockpitSiteFilter,
    setDepartmentWorkspace,
    setFunctionWorkspace,
    setHandoverDigestShiftDate,
    setIncidentCommandDraft,
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
    creatingSample
  } = rawProps as any;

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

      {visibleSections.has("section-daily-cockpit") && (
        <SectionCard
          id="section-daily-cockpit"
          title={t("cmdb.dailyCockpit.title")}
          actions={(
            <button
              onClick={() => void loadDailyCockpitSnapshot()}
              disabled={loadingDailyCockpit || loadingNextBestActions || loadingOpsChecklist}
            >
              {loadingDailyCockpit || loadingNextBestActions || loadingOpsChecklist
                ? t("cmdb.actions.loading")
                : t("cmdb.dailyCockpit.actions.refresh")}
            </button>
          )}
        >
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
                <button onClick={() => void checkChangeCalendarConflicts()} disabled={!canWriteCmdb || checkingChangeCalendarConflict}>
                  {checkingChangeCalendarConflict ? t("cmdb.actions.loading") : "Check conflict"}
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
                  onChange={(event) => setChangeCalendarConflictDraft((prev: any) => ({
                    ...prev,
                    operation_kind: event.target.value
                  }))}
                  placeholder="playbook.execute.restart-service-safe"
                />
              </label>
              <label className="control-field">
                <span>Risk level</span>
                <select
                  value={changeCalendarConflictDraft.risk_level}
                  onChange={(event) => setChangeCalendarConflictDraft((prev: any) => ({
                    ...prev,
                    risk_level: event.target.value
                  }))}
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
                  onChange={(event) => setChangeCalendarConflictDraft((prev: any) => ({
                    ...prev,
                    start_at_local: event.target.value
                  }))}
                />
              </label>
              <label className="control-field">
                <span>Slot end</span>
                <input
                  type="datetime-local"
                  value={changeCalendarConflictDraft.end_at_local}
                  onChange={(event) => setChangeCalendarConflictDraft((prev: any) => ({
                    ...prev,
                    end_at_local: event.target.value
                  }))}
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
                <button onClick={() => void exportHandoverDigest("csv")} disabled={exportingHandoverDigest}>
                  {exportingHandoverDigest ? t("cmdb.actions.loading") : "Export CSV"}
                </button>
                <button onClick={() => void exportHandoverDigest("json")} disabled={exportingHandoverDigest}>
                  {exportingHandoverDigest ? t("cmdb.actions.loading") : "Export JSON"}
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
                  <span className="status-chip status-chip-success">closed:{handoverDigest.metrics.closed_items}</span>
                </div>

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
                          <tr key={`handover-item-${item.item_key}`}>
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
