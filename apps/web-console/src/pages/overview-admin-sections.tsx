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
    businessWorkspace,
    businessWorkspaceOptions,
    canAccessAdmin,
    canWriteCmdb,
    cockpitCriticalAssets,
    cockpitOperationalAssets,
    createSampleAsset,
    departmentWorkspace,
    departmentWorkspaceOptions,
    dailyCockpitDepartmentFilter,
    dailyCockpitNotice,
    dailyCockpitQueue,
    dailyCockpitSiteFilter,
    functionWorkspace,
    loadDailyCockpitQueue,
    loadAssets,
    loadAssetStats,
    loadFieldDefinitions,
    loadingDailyCockpit,
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
    runDailyCockpitAction,
    runningDailyCockpitActionKey,
    setBusinessWorkspace,
    setDailyCockpitDepartmentFilter,
    setDailyCockpitSiteFilter,
    setDepartmentWorkspace,
    setFunctionWorkspace,
    setMenuAxis,
    subSectionTitleStyle,
    t,
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
            <button onClick={() => void loadDailyCockpitQueue()} disabled={loadingDailyCockpit}>
              {loadingDailyCockpit ? t("cmdb.actions.loading") : t("cmdb.dailyCockpit.actions.refresh")}
            </button>
          )}
        >
          {dailyCockpitNotice && <p className="banner banner-success">{dailyCockpitNotice}</p>}
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
