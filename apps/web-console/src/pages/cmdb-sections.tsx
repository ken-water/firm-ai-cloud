import { SectionCard } from "../components/layout";
import { HorizontalFillBar } from "../components/chart-primitives";

type AssetSortMode = "updated_desc" | "name_asc" | "id_asc";
type ImpactDirection = "downstream" | "upstream" | "both";

export function CmdbSections(rawProps: Record<string, unknown>) {
  const {
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
  visibleSections,
  } = rawProps as any;

  return (
    <>
      {visibleSections.has("section-fields") && (
        <SectionCard id="section-fields" title={t("cmdb.fields.title")}>
        {canWriteCmdb ? (
          <div style={{ display: "flex", gap: "0.5rem", flexWrap: "wrap", marginBottom: "0.75rem" }}>
            <input
              value={newField.field_key}
              onChange={(event) => setNewField((prev: any) => ({ ...prev, field_key: event.target.value }))}
              placeholder={t("cmdb.fields.form.fieldKey")}
            />
            <input
              value={newField.name}
              onChange={(event) => setNewField((prev: any) => ({ ...prev, name: event.target.value }))}
              placeholder={t("cmdb.fields.form.name")}
            />
            <select
              value={newField.field_type}
              onChange={(event) => setNewField((prev: any) => ({ ...prev, field_type: event.target.value }))}
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
              onChange={(event) => setNewField((prev: any) => ({ ...prev, max_length: event.target.value }))}
              placeholder={t("cmdb.fields.form.maxLength")}
              style={{ width: "140px" }}
            />
            {newField.field_type === "enum" && (
              <input
                value={newField.options_csv}
                onChange={(event) => setNewField((prev: any) => ({ ...prev, options_csv: event.target.value }))}
                placeholder={t("cmdb.fields.form.enumOptions")}
                style={{ minWidth: "250px" }}
              />
            )}
            <label>
              <input
                type="checkbox"
                checked={newField.required}
                onChange={(event) => setNewField((prev: any) => ({ ...prev, required: event.target.checked }))}
              />{" "}
              {t("cmdb.fields.form.required")}
            </label>
            <label>
              <input
                type="checkbox"
                checked={newField.scanner_enabled}
                onChange={(event) => setNewField((prev: any) => ({ ...prev, scanner_enabled: event.target.checked }))}
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
                {fieldDefinitions.map((item: any) => (
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
                {assets.map((asset: any) => (
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
                  onChange={(event) => setNewRelation((prev: any) => ({ ...prev, dst_asset_id: event.target.value }))}
                >
                  <option value="">{t("cmdb.relations.form.selectTarget")}</option>
                  {assets.map((asset: any) => (
                    <option key={asset.id} value={asset.id}>
                      #{asset.id} {asset.name}
                    </option>
                  ))}
                </select>

                <input
                  value={newRelation.relation_type}
                  onChange={(event) => setNewRelation((prev: any) => ({ ...prev, relation_type: event.target.value }))}
                  placeholder={t("cmdb.relations.form.relationType")}
                />

                <select
                  value={newRelation.source}
                  onChange={(event) => setNewRelation((prev: any) => ({ ...prev, source: event.target.value }))}
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
                    {relations.map((relation: any) => (
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
                  const relationTypes = parseImpactRelationTypesInput(impactRelationTypesInput, defaultImpactRelationTypes);
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
                            .map((item: any) => t(`cmdb.assetDetail.readiness.missing.${item}`))
                            .join(", ")
                        })}
                      </p>
                    )}

                    <div className="toolbar-row">
                      {lifecycleStatuses.map((status: any) => (
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
                    {bindingOwnerDrafts.map((owner: any) => (
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
                        {hierarchyHintEdges.map((edge: any) => (
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
                        value: assetImpact.affected_business_services.map((item: any) => item.name).join(", ") || "-"
                      })}
                    </p>
                    <p className="section-note">
                      {t("cmdb.assetDetail.impact.affectedOwners", {
                        value: assetImpact.affected_owners.map((item: any) => item.name).join(", ") || "-"
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
                  {assets.map((asset: any) => (
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
                    {assetImpact.edges.map((edge: any) => {
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

                    {assetImpact.nodes.map((node: any) => {
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
                  {assetImpact.relation_types.map((relationType: any) => (
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
                    {assetStatsStatusBuckets.map((bucket: any) => (
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
                          <HorizontalFillBar width={bucketBarWidth(bucket.asset_total, assetStatsStatusMax)} color="#2563eb" />
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
                    {assetStatsDepartmentBuckets.map((bucket: any) => (
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
                          <HorizontalFillBar width={bucketBarWidth(bucket.asset_total, assetStatsDepartmentMax)} color="#0f766e" />
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
                    {assetStatsBusinessServiceBuckets.map((bucket: any) => (
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
                          <HorizontalFillBar width={bucketBarWidth(bucket.asset_total, assetStatsBusinessServiceMax)} color="#ea580c" />
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
              {assetStatusOptions.map((status: any) => (
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
              {assetClassOptions.map((assetClass: any) => (
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
              {assetSiteOptions.map((site: any) => (
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
                {filteredAssets.map((asset: any) => (
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
    </>
  );
}
