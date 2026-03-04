import { MetricSparkline } from "../components/chart-primitives";
import { SectionCard } from "../components/layout";

export function IntegrationMonitoringSections(rawProps: Record<string, unknown>) {
  const {
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
  visibleSections,
  } = rawProps as any;

  return (
    <>
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
                {discoveryJobs.map((job: any) => (
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
                {discoveryCandidates.map((candidate: any) => (
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
                setMonitoringSourceFilters((prev: any) => ({
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
                setMonitoringSourceFilters((prev: any) => ({
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
                setMonitoringSourceFilters((prev: any) => ({
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
                setMonitoringSourceFilters((prev: any) => ({
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
                  setNewMonitoringSource((prev: any) => ({
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
                  setNewMonitoringSource((prev: any) => ({
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
                  setNewMonitoringSource((prev: any) => ({
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
                  setNewMonitoringSource((prev: any) => ({
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
                  setNewMonitoringSource((prev: any) => ({
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
                  setNewMonitoringSource((prev: any) => ({
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
                  setNewMonitoringSource((prev: any) => ({
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
                  setNewMonitoringSource((prev: any) => ({
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
                  setNewMonitoringSource((prev: any) => ({
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
                  setNewMonitoringSource((prev: any) => ({
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
                {monitoringSources.map((source: any) => (
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
              {assets.map((asset: any) => (
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
              {monitoringMetrics.series.map((series: any) => (
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
                      <MetricSparkline
                        ariaLabel={series.label}
                        points={buildMetricPolylinePoints(series.points, 320, 120, 12)}
                        stroke="#2563eb"
                      />
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
                setNewNotificationChannel((prev: any) => ({
                  ...prev,
                  name: event.target.value
                }))
              }
              placeholder={t("cmdb.notifications.form.channelName")}
            />
            <select
              value={newNotificationChannel.channel_type}
              onChange={(event) =>
                setNewNotificationChannel((prev: any) => ({
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
                setNewNotificationChannel((prev: any) => ({
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
                setNewNotificationChannel((prev: any) => ({
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
                {notificationChannels.map((channel: any) => (
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
                setNewNotificationTemplate((prev: any) => ({
                  ...prev,
                  event_type: event.target.value
                }))
              }
              placeholder={t("cmdb.notifications.form.eventType")}
            />
            <input
              value={newNotificationTemplate.title_template}
              onChange={(event) =>
                setNewNotificationTemplate((prev: any) => ({
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
                setNewNotificationTemplate((prev: any) => ({
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
                {notificationTemplates.map((template: any) => (
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
                setNewNotificationSubscription((prev: any) => ({
                  ...prev,
                  channel_id: event.target.value
                }))
              }
            >
              <option value="">{t("cmdb.notifications.form.selectChannel")}</option>
              {notificationChannels.map((channel: any) => (
                <option key={channel.id} value={channel.id}>
                  #{channel.id} {channel.name} ({channel.channel_type})
                </option>
              ))}
            </select>
            <input
              value={newNotificationSubscription.event_type}
              onChange={(event) =>
                setNewNotificationSubscription((prev: any) => ({
                  ...prev,
                  event_type: event.target.value
                }))
              }
              placeholder={t("cmdb.notifications.form.eventType")}
            />
            <input
              value={newNotificationSubscription.site}
              onChange={(event) =>
                setNewNotificationSubscription((prev: any) => ({
                  ...prev,
                  site: event.target.value
                }))
              }
              placeholder={t("cmdb.notifications.form.siteOptional")}
            />
            <input
              value={newNotificationSubscription.department}
              onChange={(event) =>
                setNewNotificationSubscription((prev: any) => ({
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
                {notificationSubscriptions.map((subscription: any) => {
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
    </>
  );
}
