import { SectionCard } from "../components/layout";

const HEALTH_COLORS: Record<string, string> = {
  healthy: "#059669",
  warning: "#d97706",
  critical: "#b91c1c",
  unknown: "#334155"
};

export function TopologyWorkspaceSections(rawProps: Record<string, unknown>) {
  const {
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
  } = rawProps as any;

  if (!visibleSections.has("section-topology-workspace")) {
    return null;
  }

  return (
    <SectionCard
      id="section-topology-workspace"
      title={t("topology.workspace.title")}
      actions={(
        <button onClick={() => void loadTopologyMap()} disabled={loadingTopologyMap}>
          {loadingTopologyMap ? t("cmdb.actions.loading") : t("topology.workspace.actions.refresh")}
        </button>
      )}
    >
      {!canWriteCmdb && <p className="inline-note">{t("topology.workspace.messages.readOnlyHint")}</p>}
      {topologyMapNotice && <p className="banner banner-success">{topologyMapNotice}</p>}

      <div className="filter-grid" style={{ marginBottom: "0.75rem" }}>
        <label className="control-field">
          <span>{t("topology.workspace.filters.scope")}</span>
          <input
            value={topologyScopeInput}
            onChange={(event) => setTopologyScopeInput(event.target.value)}
            placeholder="global | site:dc-a | department:platform"
          />
        </label>
        <label className="control-field">
          <span>{t("topology.workspace.filters.site")}</span>
          <input
            value={topologySiteFilter}
            onChange={(event) => setTopologySiteFilter(event.target.value)}
            placeholder="dc-a"
          />
        </label>
        <label className="control-field">
          <span>{t("topology.workspace.filters.department")}</span>
          <input
            value={topologyDepartmentFilter}
            onChange={(event) => setTopologyDepartmentFilter(event.target.value)}
            placeholder="platform"
          />
        </label>
        <label className="control-field">
          <span>{t("topology.workspace.filters.limit")}</span>
          <input value={topologyWindowLimit} onChange={(event) => setTopologyWindowLimit(event.target.value)} />
        </label>
        <label className="control-field">
          <span>{t("topology.workspace.filters.offset")}</span>
          <input value={topologyWindowOffset} onChange={(event) => setTopologyWindowOffset(event.target.value)} />
        </label>
      </div>

      {!topologyMap ? (
        <p>{loadingTopologyMap ? t("topology.workspace.messages.loading") : t("topology.workspace.messages.noData")}</p>
      ) : topologyMap.empty ? (
        <p>{t("topology.workspace.messages.emptyScope")}</p>
      ) : (
        <>
          <p className="section-note">
            {t("topology.workspace.summary", {
              scope: topologyMap.scope.scope_key,
              total: topologyMap.stats.total_nodes,
              windowNodes: topologyMap.stats.window_nodes,
              windowEdges: topologyMap.stats.window_edges,
              limit: topologyMap.window.limit,
              offset: topologyMap.window.offset
            })}
          </p>

          <div className="toolbar-row" style={{ marginBottom: "0.5rem" }}>
            {Object.entries(HEALTH_COLORS).map(([key, color]) => (
              <span key={key} className="status-chip" style={{ borderColor: color, color }}>
                {key}
              </span>
            ))}
          </div>

          <div style={{ overflowX: "auto", border: "1px solid #e2e8f0", borderRadius: "12px", padding: "0.5rem" }}>
            <svg
              viewBox="0 0 1080 620"
              style={{
                width: "100%",
                minWidth: "860px",
                height: "620px",
                display: "block",
                background: "linear-gradient(180deg, #f8fafc 0%, #e0f2fe 100%)",
                borderRadius: "8px"
              }}
            >
              {topologyMapEdgesForRender.map((edge: any) => {
                const src = topologyMapNodePositions.get(edge.src_asset_id);
                const dst = topologyMapNodePositions.get(edge.dst_asset_id);
                if (!src || !dst) {
                  return null;
                }
                const meta = topologyMapEdgeRenderMeta.get(topologyEdgeKey(edge)) ?? { index: 0, total: 1 };
                const path = buildTopologyEdgePath(src, dst, meta.index, meta.total);
                const selected = selectedTopologyMapEdgeKey === topologyEdgeKey(edge);
                return (
                  <path
                    key={topologyEdgeKey(edge)}
                    d={path}
                    fill="none"
                    stroke={relationTypeColor(edge.relation_type)}
                    strokeWidth={selected ? 3.2 : 1.6}
                    opacity={selected ? 1 : 0.7}
                    style={{ cursor: "pointer" }}
                    onClick={() => setSelectedTopologyMapEdgeKey(topologyEdgeKey(edge))}
                  />
                );
              })}

              {topologyMap.nodes.map((node: any) => {
                const pos = topologyMapNodePositions.get(node.id);
                if (!pos) {
                  return null;
                }
                const selected = Number.parseInt(selectedTopologyMapNodeId, 10) === node.id;
                const healthColor = HEALTH_COLORS[node.health] ?? HEALTH_COLORS.unknown;
                return (
                  <g
                    key={`topology-map-node-${node.id}`}
                    style={{ cursor: "pointer" }}
                    onClick={() => setSelectedTopologyMapNodeId(String(node.id))}
                  >
                    <circle
                      cx={pos.x}
                      cy={pos.y}
                      r={selected ? 18 : 14}
                      fill={healthColor}
                      stroke={selected ? "#1d4ed8" : "#0f172a"}
                      strokeWidth={selected ? 3 : 1.3}
                    />
                    <text
                      x={pos.x}
                      y={pos.y + 4}
                      textAnchor="middle"
                      fill="#ffffff"
                      style={{ fontSize: "10px", fontWeight: 700 }}
                    >
                      {node.id}
                    </text>
                    <text
                      x={pos.x}
                      y={pos.y + 28}
                      textAnchor="middle"
                      fill="#0f172a"
                      style={{ fontSize: "10px", fontWeight: selected ? 700 : 500 }}
                    >
                      {truncateTopologyLabel(node.name, 24)}
                    </text>
                  </g>
                );
              })}
            </svg>
          </div>

          <div className="detail-grid" style={{ marginTop: "0.75rem" }}>
            <div className="detail-panel">
              <h3 style={{ marginTop: 0 }}>{t("topology.workspace.drawer.nodeTitle")}</h3>
              {!selectedTopologyMapNode ? (
                <p className="inline-note">{t("topology.workspace.messages.selectNode")}</p>
              ) : (
                <>
                  <p className="section-note">
                    {t("topology.workspace.drawer.nodeSummary", {
                      id: selectedTopologyMapNode.id,
                      class: selectedTopologyMapNode.asset_class,
                      status: selectedTopologyMapNode.status,
                      health: selectedTopologyMapNode.health
                    })}
                  </p>
                  <p className="section-note">
                    {t("topology.workspace.drawer.nodeScope", {
                      site: selectedTopologyMapNode.site ?? "-",
                      department: selectedTopologyMapNode.department ?? "-"
                    })}
                  </p>
                  <p className="section-note">
                    {t("topology.workspace.drawer.nodeEdges", {
                      outbound: topologyMapNodeEdgeSummary.outbound,
                      inbound: topologyMapNodeEdgeSummary.inbound,
                      total: topologyMapNodeEdgeSummary.total
                    })}
                  </p>
                  <div className="toolbar-row">
                    <button onClick={() => setTopologySiteFilter(selectedTopologyMapNode.site ?? "")}>
                      {t("topology.workspace.actions.useNodeSite")}
                    </button>
                    <button onClick={() => setTopologyDepartmentFilter(selectedTopologyMapNode.department ?? "")}>
                      {t("topology.workspace.actions.useNodeDepartment")}
                    </button>
                    <button
                      onClick={() => {
                        if (selectedTopologyMapNode.site) {
                          setTopologyScopeInput(`site:${selectedTopologyMapNode.site}`);
                        } else if (selectedTopologyMapNode.department) {
                          setTopologyScopeInput(`department:${selectedTopologyMapNode.department}`);
                        }
                      }}
                    >
                      {t("topology.workspace.actions.useNodeScope")}
                    </button>
                  </div>
                </>
              )}
            </div>

            <div className="detail-panel">
              <h3 style={{ marginTop: 0 }}>{t("topology.workspace.drawer.edgeTitle")}</h3>
              {!selectedTopologyMapEdge ? (
                <p className="inline-note">{t("topology.workspace.messages.selectEdge")}</p>
              ) : (
                <>
                  <p className="section-note">
                    {t("topology.workspace.drawer.edgeSummary", {
                      id: selectedTopologyMapEdge.id,
                      src: selectedTopologyMapEdge.src_asset_id,
                      dst: selectedTopologyMapEdge.dst_asset_id,
                      relationType: selectedTopologyMapEdge.relation_type,
                      source: selectedTopologyMapEdge.source
                    })}
                  </p>
                  <div className="toolbar-row">
                    <button onClick={() => setSelectedTopologyMapNodeId(String(selectedTopologyMapEdge.src_asset_id))}>
                      {t("topology.workspace.actions.focusSource")}
                    </button>
                    <button onClick={() => setSelectedTopologyMapNodeId(String(selectedTopologyMapEdge.dst_asset_id))}>
                      {t("topology.workspace.actions.focusTarget")}
                    </button>
                  </div>

                  <div className="filter-grid" style={{ marginTop: "0.6rem" }}>
                    <label className="control-field">
                      <span>{t("topology.workspace.diagnostics.windowLabel")}</span>
                      <input
                        value={topologyDiagnosticsWindowMinutes}
                        onChange={(event) => setTopologyDiagnosticsWindowMinutes(event.target.value)}
                      />
                    </label>
                    <div className="toolbar-row" style={{ alignSelf: "end" }}>
                      <button
                        onClick={() => void loadTopologyEdgeDiagnostics(selectedTopologyMapEdge.id)}
                        disabled={loadingTopologyDiagnostics}
                      >
                        {loadingTopologyDiagnostics
                          ? t("cmdb.actions.loading")
                          : t("topology.workspace.diagnostics.refresh")}
                      </button>
                    </div>
                  </div>

                  {topologyDiagnosticsNotice && <p className="inline-note">{topologyDiagnosticsNotice}</p>}

                  {!topologyDiagnostics ? (
                    <p className="inline-note">
                      {loadingTopologyDiagnostics
                        ? t("topology.workspace.diagnostics.loading")
                        : t("topology.workspace.diagnostics.empty")}
                    </p>
                  ) : (
                    <>
                      <p className="section-note">
                        {t("topology.workspace.diagnostics.summary", {
                          trend: topologyDiagnostics.trend.length,
                          alerts: topologyDiagnostics.alerts.length,
                          changes: topologyDiagnostics.recent_changes.length
                        })}
                      </p>

                      <h4 style={{ marginBottom: "0.35rem" }}>{t("topology.workspace.diagnostics.trendTitle")}</h4>
                      {topologyDiagnostics.trend.length === 0 ? (
                        <p className="inline-note">{t("topology.workspace.diagnostics.noTrend")}</p>
                      ) : (
                        <div style={{ maxHeight: "140px", overflowY: "auto" }}>
                          {topologyDiagnostics.trend.slice(0, 8).map((point: any) => (
                            <div key={`diag-trend-${point.bucket_at}`} className="section-note">
                              {new Date(point.bucket_at).toLocaleString()} | total={point.total_jobs} | failed={point.failed_jobs}
                            </div>
                          ))}
                        </div>
                      )}

                      <h4 style={{ marginBottom: "0.35rem", marginTop: "0.6rem" }}>{t("topology.workspace.diagnostics.alertTitle")}</h4>
                      {topologyDiagnostics.alerts.length === 0 ? (
                        <p className="inline-note">{t("topology.workspace.diagnostics.noAlerts")}</p>
                      ) : (
                        <div style={{ maxHeight: "140px", overflowY: "auto" }}>
                          {topologyDiagnostics.alerts.slice(0, 8).map((alert: any) => (
                            <div key={`diag-alert-${alert.id}`} className="section-note">
                              #{alert.id} [{alert.severity}] {truncateTopologyLabel(alert.title, 44)}
                            </div>
                          ))}
                        </div>
                      )}

                      <h4 style={{ marginBottom: "0.35rem", marginTop: "0.6rem" }}>{t("topology.workspace.diagnostics.checklistTitle")}</h4>
                      {topologyDiagnostics.checklist.length === 0 ? (
                        <p className="inline-note">{t("topology.workspace.diagnostics.noChecklist")}</p>
                      ) : (
                        <ul style={{ margin: 0, paddingLeft: "1.2rem" }}>
                          {topologyDiagnostics.checklist.map((step: any) => (
                            <li key={`diag-step-${step.key}`} style={{ marginBottom: "0.25rem" }}>
                              {step.done ? "[done]" : "[todo]"} {step.title} ({step.hint})
                            </li>
                          ))}
                        </ul>
                      )}

                      <h4 style={{ marginBottom: "0.35rem", marginTop: "0.6rem" }}>{t("topology.workspace.diagnostics.actionsTitle")}</h4>
                      <div className="toolbar-row">
                        {(topologyDiagnostics.quick_actions ?? []).map((action: any) => {
                          const running = runningTopologyDiagnosticsActionKey === action.key;
                          const disabled = running || (action.requires_write && !canWriteCmdb);
                          return (
                            <button
                              key={`diag-action-${action.key}`}
                              onClick={() => void runTopologyDiagnosticsAction(action)}
                              disabled={disabled}
                            >
                              {running ? t("cmdb.actions.loading") : action.label}
                            </button>
                          );
                        })}
                      </div>
                    </>
                  )}
                </>
              )}
            </div>
          </div>
        </>
      )}
    </SectionCard>
  );
}
