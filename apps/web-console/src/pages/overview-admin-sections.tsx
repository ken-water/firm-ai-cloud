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
    businessWorkspace,
    businessWorkspaceOptions,
    canAccessAdmin,
    canWriteCmdb,
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
    exportingWeeklyDigest,
    exportWeeklyDigest,
    functionWorkspace,
    loadBackupPolicies,
    loadBackupPolicyRuns,
    loadWeeklyDigest,
    loadDailyCockpitSnapshot,
    loadOpsChecklist,
    loadAssets,
    loadAssetStats,
    loadFieldDefinitions,
    runBackupPolicy,
    runBackupSchedulerTick,
    runningBackupPolicyActionId,
    loadingDailyCockpit,
    loadingOpsChecklist,
    loadingBackupPolicies,
    loadingBackupPolicyRuns,
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
    saveBackupPolicy,
    savingBackupPolicy,
    setBusinessWorkspace,
    setBackupPolicyDraft,
    setDailyCockpitDepartmentFilter,
    setDailyCockpitSiteFilter,
    setDepartmentWorkspace,
    setFunctionWorkspace,
    setMenuAxis,
    setOpsChecklistDate,
    setWeeklyDigestWeekStart,
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
              disabled={loadingDailyCockpit || loadingOpsChecklist}
            >
              {loadingDailyCockpit || loadingOpsChecklist
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
              <h3 style={{ ...subSectionTitleStyle, marginTop: 0, marginBottom: 0 }}>Backup & DR policy</h3>
              <div className="toolbar-row">
                <button onClick={() => void loadBackupPolicies()} disabled={loadingBackupPolicies}>
                  {loadingBackupPolicies ? t("cmdb.actions.loading") : "Refresh policies"}
                </button>
                <button onClick={() => void loadBackupPolicyRuns()} disabled={loadingBackupPolicyRuns}>
                  {loadingBackupPolicyRuns ? t("cmdb.actions.loading") : "Refresh runs"}
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
                                  onClick={() => void runDailyCockpitAction(item, action)}
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
