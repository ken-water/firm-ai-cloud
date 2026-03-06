import type { CSSProperties } from "react";
import { SectionCard } from "../components/layout";

type SetupCheckItem = {
  key: string;
  title: string;
  status: "pass" | "warn" | "fail";
  critical: boolean;
  message: string;
  remediation: string;
};

type SetupChecklistResponse = {
  generated_at: string;
  category: string;
  summary: {
    total: number;
    passed: number;
    warned: number;
    failed: number;
    critical_failed: number;
    ready: boolean;
  };
  checks: SetupCheckItem[];
};

type SetupTemplateSchemaField = {
  key: string;
  label: string;
  type: "string" | "enum";
  required?: boolean;
  options?: string[];
  default?: string;
  placeholder?: string;
  max_length?: number;
};

type SetupTemplateCatalogItem = {
  key: string;
  name: string;
  category: string;
  description: string | null;
  param_schema: {
    fields: SetupTemplateSchemaField[];
  };
  apply_plan: {
    actions?: string[];
  };
  rollback_hints: string[];
  is_enabled: boolean;
  is_system: boolean;
  updated_at: string;
};

type SetupTemplateValidationError = {
  field: string;
  message: string;
};

type SetupTemplatePreviewAction = {
  action_key: string;
  summary: string;
  outcome: string;
  detail: string;
};

type SetupTemplatePreviewResponse = {
  template: SetupTemplateCatalogItem;
  ready: boolean;
  validation_errors: SetupTemplateValidationError[];
  actions: SetupTemplatePreviewAction[];
  rollback_hints: string[];
};

type SetupTemplateApplyAction = {
  action_key: string;
  outcome: string;
  target_id: string | null;
  detail: string;
};

type SetupTemplateApplyResponse = {
  actor: string;
  template_key: string;
  status: string;
  applied_actions: SetupTemplateApplyAction[];
  rollback_hints: string[];
};

export function SetupAlertSections(rawProps: Record<string, unknown>) {
  const {
    alertActionRunningId,
    alertBulkActionRunning,
    alertDetail,
    alertNotice,
    alertQueryFilter,
    alertSeverityFilter,
    alertSiteFilter,
    alertStatusFilter,
    alerts,
    alertsTotal,
    applySetupTemplate,
    canWriteCmdb,
    closeAlert,
    completeSetupWizard,
    loadingAlertDetail,
    loadingAlerts,
    loadingSetupChecklist,
    loadingSetupPreflight,
    loadingSetupTemplates,
    previewSetupTemplate,
    refreshAlerts,
    refreshSetupWizard,
    runningSetupTemplateApply,
    runningSetupTemplatePreview,
    selectedAlertId,
    selectedAlertIds,
    selectedSetupTemplateKey,
    setAlertQueryFilter,
    setAlertSeverityFilter,
    setAlertSiteFilter,
    setAlertStatusFilter,
    setSelectedSetupTemplateKey,
    setSetupStep,
    setSetupTemplateNote,
    setSetupTemplateParam,
    setupChecklist,
    setupCompleted,
    setupNotice,
    setupTemplateApplyResult,
    setupTemplateNote,
    setupTemplateNotice,
    setupTemplateParamsDraft,
    setupTemplatePreview,
    setupPreflight,
    setupStep,
    setupTemplates,
    t,
    toggleAlertSelection,
    toggleSelectAllAlerts,
    triggerAlertRemediation,
    triggerBulkAcknowledge,
    triggerBulkClose,
    triggerSingleAcknowledge,
    visibleSections
  } = rawProps as any;

  const preflight = setupPreflight as SetupChecklistResponse | null;
  const checklist = setupChecklist as SetupChecklistResponse | null;
  const checklistByKey = new Map<string, SetupCheckItem>(
    (checklist?.checks ?? []).map((item) => [item.key, item])
  );
  const preflightBlockingChecks = (preflight?.checks ?? []).filter((item) => item.critical && item.status === "fail");
  const checklistBlockingChecks = (checklist?.checks ?? []).filter((item) => item.critical && item.status === "fail");
  const blockingChecks = [...preflightBlockingChecks, ...checklistBlockingChecks];
  const setupCanComplete = blockingChecks.length === 0;
  const templateItems = (setupTemplates as SetupTemplateCatalogItem[]) ?? [];
  const selectedSetupTemplate = templateItems.find((item) => item.key === selectedSetupTemplateKey) ?? null;
  const selectedTemplateFields = selectedSetupTemplate?.param_schema.fields ?? [];
  const templateDraft = (setupTemplateParamsDraft as Record<string, string>) ?? {};
  const templatePreview = setupTemplatePreview as SetupTemplatePreviewResponse | null;
  const templateApplyResult = setupTemplateApplyResult as SetupTemplateApplyResponse | null;

  const setupSteps = [
    {
      key: "environment",
      title: t("setupWizard.steps.environment.title"),
      description: t("setupWizard.steps.environment.description"),
      passed: preflight ? preflight.summary.critical_failed === 0 : false,
      checks: preflight?.checks ?? []
    },
    {
      key: "account",
      title: t("setupWizard.steps.account.title"),
      description: t("setupWizard.steps.account.description"),
      passed: preflight ? preflight.summary.failed === 0 : false,
      checks: [
        checklistByKey.get("rbac"),
        checklistByKey.get("workflow-policy"),
        checklistByKey.get("monitoring-secret-key")
      ].filter(Boolean) as SetupCheckItem[]
    },
    {
      key: "monitoring",
      title: t("setupWizard.steps.monitoring.title"),
      description: t("setupWizard.steps.monitoring.description"),
      passed: checklistByKey.get("monitoring-source-seed")?.status === "pass",
      checks: [
        checklistByKey.get("monitoring-source-seed"),
        checklistByKey.get("zabbix-server"),
        checklistByKey.get("api-port")
      ].filter(Boolean) as SetupCheckItem[]
    },
    {
      key: "validation",
      title: t("setupWizard.steps.validation.title"),
      description: t("setupWizard.steps.validation.description"),
      passed: checklist ? checklist.summary.critical_failed === 0 : false,
      checks: [
        checklistByKey.get("setup-template-baseline"),
        checklistByKey.get("alert-policy-templates"),
        checklistByKey.get("database"),
        checklistByKey.get("web-console"),
        checklistByKey.get("redis"),
        checklistByKey.get("opensearch"),
        checklistByKey.get("minio")
      ].filter(Boolean) as SetupCheckItem[]
    }
  ];

  const currentSetupStep = Math.max(0, Math.min(setupStep, setupSteps.length - 1));
  const setupCurrent = setupSteps[currentSetupStep];
  const setupReadyLabel = setupCanComplete
    ? t("setupWizard.summary.ready")
    : t("setupWizard.summary.blocked", { count: blockingChecks.length });

  const allAlertIds = (alerts as Array<{ id: number }>).map((item) => item.id);
  const allAlertsSelected = allAlertIds.length > 0 && allAlertIds.every((id) => selectedAlertIds.includes(id));

  return (
    <>
      {visibleSections.has("section-setup-wizard") && (
        <SectionCard
          id="section-setup-wizard"
          title={t("setupWizard.title")}
          actions={(
            <button onClick={() => void refreshSetupWizard()} disabled={loadingSetupPreflight || loadingSetupChecklist}>
              {loadingSetupPreflight || loadingSetupChecklist
                ? t("setupWizard.actions.refreshing")
                : t("setupWizard.actions.refresh")}
            </button>
          )}
        >
          <p className="section-note">{t("setupWizard.summary.description")}</p>
          <div className="toolbar-row" style={{ marginBottom: "0.75rem" }}>
            <span className={`status-chip ${setupCanComplete ? "status-chip-success" : "status-chip-danger"}`}>
              {setupReadyLabel}
            </span>
            {preflight && (
              <span className="section-meta">
                {t("setupWizard.summary.preflight", {
                  passed: preflight.summary.passed,
                  warned: preflight.summary.warned,
                  failed: preflight.summary.failed
                })}
              </span>
            )}
            {checklist && (
              <span className="section-meta">
                {t("setupWizard.summary.checklist", {
                  passed: checklist.summary.passed,
                  warned: checklist.summary.warned,
                  failed: checklist.summary.failed
                })}
              </span>
            )}
          </div>

          {setupNotice && <p className="banner banner-success">{setupNotice}</p>}

          <div className="toolbar-row" style={{ marginBottom: "0.75rem", alignItems: "stretch" }}>
            {setupSteps.map((step, index) => (
              <button
                key={step.key}
                className={index === currentSetupStep ? "is-active" : undefined}
                onClick={() => setSetupStep(index)}
                style={{
                  flex: "1 1 180px",
                  textAlign: "left",
                  borderColor: index === currentSetupStep ? "var(--brand)" : "var(--border-color)",
                  background:
                    step.passed
                      ? "rgba(10, 111, 75, 0.08)"
                      : index === currentSetupStep
                        ? "#edf6ff"
                        : "#f7fbff"
                }}
              >
                <div style={{ fontWeight: 600 }}>
                  {index + 1}. {step.title}
                </div>
                <div style={{ fontSize: "0.85rem", color: "var(--text-muted)" }}>{step.description}</div>
              </button>
            ))}
          </div>

          {(loadingSetupPreflight || loadingSetupChecklist) && !preflight && !checklist ? (
            <p>{t("setupWizard.messages.loading")}</p>
          ) : (
            <>
              <h3 style={{ marginTop: 0, marginBottom: "0.5rem", fontSize: "1rem" }}>{setupCurrent.title}</h3>
              <p className="section-note">{setupCurrent.description}</p>
              {setupCurrent.checks.length === 0 ? (
                <p className="inline-note">{t("setupWizard.messages.noChecks")}</p>
              ) : (
                <div style={{ display: "grid", gap: "0.5rem", marginBottom: "0.75rem" }}>
                  {setupCurrent.checks.map((item) => (
                    <div key={`${setupCurrent.key}-${item.key}`} className="detail-panel">
                      <div className="toolbar-row" style={{ justifyContent: "space-between" }}>
                        <strong>{item.title}</strong>
                        <span className={`status-chip ${statusChipClass(item.status)}`}>{t(`setupWizard.status.${item.status}`)}</span>
                      </div>
                      <p style={{ marginTop: "0.45rem", marginBottom: "0.35rem" }}>{item.message}</p>
                      <p style={{ margin: 0, color: "var(--text-muted)" }}>
                        {t("setupWizard.labels.remediation")} {item.remediation}
                      </p>
                    </div>
                  ))}
                </div>
              )}
            </>
          )}

          <div className="detail-panel" style={{ marginBottom: "0.75rem" }}>
            <h3 style={{ marginTop: 0, marginBottom: "0.5rem", fontSize: "1rem" }}>{t("setupWizard.templates.title")}</h3>
            <p className="section-note" style={{ marginBottom: "0.6rem" }}>{t("setupWizard.templates.description")}</p>
            {setupTemplateNotice && <p className="banner banner-success">{setupTemplateNotice}</p>}
            {!canWriteCmdb && <p className="inline-note">{t("setupWizard.templates.readOnlyHint")}</p>}
            {loadingSetupTemplates ? (
              <p>{t("setupWizard.templates.messages.loading")}</p>
            ) : templateItems.length === 0 ? (
              <p>{t("setupWizard.templates.messages.empty")}</p>
            ) : (
              <>
                <label className="control-field" style={{ marginBottom: "0.6rem" }}>
                  <span>{t("setupWizard.templates.selector")}</span>
                  <select
                    value={selectedSetupTemplateKey}
                    onChange={(event) => setSelectedSetupTemplateKey(event.target.value)}
                  >
                    {templateItems.map((item) => (
                      <option key={item.key} value={item.key}>
                        {item.name}
                      </option>
                    ))}
                  </select>
                </label>

                {selectedSetupTemplate && (
                  <>
                    {selectedSetupTemplate.description && (
                      <p className="section-note" style={{ marginBottom: "0.5rem" }}>
                        {selectedSetupTemplate.description}
                      </p>
                    )}
                    <div className="filter-grid">
                      {selectedTemplateFields.map((field) => {
                        const value = templateDraft[field.key] ?? "";
                        if (field.type === "enum") {
                          return (
                            <label className="control-field" key={field.key}>
                              <span>{field.label}</span>
                              <select
                                value={value}
                                onChange={(event) => setSetupTemplateParam(field.key, event.target.value)}
                              >
                                {!field.required && <option value="">{t("setupWizard.templates.optionalValue")}</option>}
                                {(field.options ?? []).map((option) => (
                                  <option key={option} value={option}>
                                    {option}
                                  </option>
                                ))}
                              </select>
                            </label>
                          );
                        }

                        return (
                          <label className="control-field" key={field.key}>
                            <span>{field.label}</span>
                            <input
                              value={value}
                              onChange={(event) => setSetupTemplateParam(field.key, event.target.value)}
                              placeholder={field.placeholder ?? ""}
                            />
                          </label>
                        );
                      })}
                    </div>

                    <label className="control-field" style={{ marginTop: "0.5rem" }}>
                      <span>{t("setupWizard.templates.note")}</span>
                      <input
                        value={setupTemplateNote}
                        onChange={(event) => setSetupTemplateNote(event.target.value)}
                        placeholder={t("setupWizard.templates.notePlaceholder")}
                      />
                    </label>

                    <div className="toolbar-row" style={{ marginTop: "0.65rem" }}>
                      <button
                        onClick={() => void previewSetupTemplate()}
                        disabled={runningSetupTemplatePreview || runningSetupTemplateApply}
                      >
                        {runningSetupTemplatePreview
                          ? t("setupWizard.templates.actions.previewing")
                          : t("setupWizard.templates.actions.preview")}
                      </button>
                      <button
                        onClick={() => void applySetupTemplate()}
                        disabled={!canWriteCmdb || runningSetupTemplateApply || runningSetupTemplatePreview}
                      >
                        {runningSetupTemplateApply
                          ? t("setupWizard.templates.actions.applying")
                          : t("setupWizard.templates.actions.apply")}
                      </button>
                    </div>
                  </>
                )}

                {templatePreview && (
                  <div className="hint-list" style={{ marginTop: "0.75rem" }}>
                    <div className="hint-row" style={{ fontWeight: 600 }}>
                      {t("setupWizard.templates.previewTitle")}
                      {" "}
                      <span className={`status-chip ${templatePreview.ready ? "status-chip-success" : "status-chip-warn"}`}>
                        {templatePreview.ready
                          ? t("setupWizard.templates.previewReady")
                          : t("setupWizard.templates.previewBlocked")}
                      </span>
                    </div>
                    {templatePreview.validation_errors.length > 0 ? (
                      templatePreview.validation_errors.map((item) => (
                        <div key={`${item.field}-${item.message}`} className="hint-row">
                          <strong>{item.field}</strong>: {item.message}
                        </div>
                      ))
                    ) : (
                      templatePreview.actions.map((action) => (
                        <div key={action.action_key} className="hint-row">
                          <strong>{action.summary}</strong> [{action.outcome}] {action.detail}
                        </div>
                      ))
                    )}
                    {templatePreview.rollback_hints.map((hint) => (
                      <div key={hint} className="hint-row">
                        {t("setupWizard.templates.rollbackPrefix")} {hint}
                      </div>
                    ))}
                  </div>
                )}

                {templateApplyResult && (
                  <div className="hint-list" style={{ marginTop: "0.75rem" }}>
                    <div className="hint-row" style={{ fontWeight: 600 }}>
                      {t("setupWizard.templates.applyTitle", {
                        actor: templateApplyResult.actor,
                        key: templateApplyResult.template_key
                      })}
                    </div>
                    {templateApplyResult.applied_actions.map((action) => (
                      <div key={`${action.action_key}-${action.target_id ?? "-"}`} className="hint-row">
                        <strong>{action.action_key}</strong> [{action.outcome}] {action.detail}
                      </div>
                    ))}
                  </div>
                )}
              </>
            )}
          </div>

          {currentSetupStep === 2 && (
            <div className="toolbar-row" style={{ marginBottom: "0.75rem" }}>
              <button
                onClick={() => {
                  if (typeof window !== "undefined") {
                    window.location.hash = "#/monitoring";
                  }
                }}
              >
                {t("setupWizard.actions.goMonitoring")}
              </button>
            </div>
          )}

          {blockingChecks.length > 0 && (
            <div className="hint-list" style={{ marginBottom: "0.75rem" }}>
              <div className="hint-row" style={{ fontWeight: 600 }}>
                {t("setupWizard.messages.blockingTitle")}
              </div>
              {blockingChecks.map((item) => (
                <div key={`blocking-${item.key}`} className="hint-row">
                  <strong>{item.title}:</strong> {item.remediation}
                </div>
              ))}
            </div>
          )}

          {setupCompleted && <p className="banner banner-success">{t("setupWizard.messages.completed")}</p>}

          <div className="toolbar-row">
            <button onClick={() => setSetupStep(Math.max(0, currentSetupStep - 1))} disabled={currentSetupStep === 0}>
              {t("setupWizard.actions.previous")}
            </button>
            <button
              onClick={() => setSetupStep(Math.min(setupSteps.length - 1, currentSetupStep + 1))}
              disabled={currentSetupStep === setupSteps.length - 1}
            >
              {t("setupWizard.actions.next")}
            </button>
            <button onClick={() => void completeSetupWizard()} disabled={!setupCanComplete}>
              {t("setupWizard.actions.complete")}
            </button>
          </div>
        </SectionCard>
      )}

      {visibleSections.has("section-alert-center") && (
        <SectionCard
          id="section-alert-center"
          title={t("alertsCenter.title")}
          actions={(
            <button onClick={() => void refreshAlerts()} disabled={loadingAlerts}>
              {loadingAlerts ? t("alertsCenter.actions.refreshing") : t("alertsCenter.actions.refresh")}
            </button>
          )}
        >
          <p className="section-note">
            {t("alertsCenter.summary", { total: alertsTotal, selected: selectedAlertIds.length })}
          </p>
          {!canWriteCmdb && <p className="inline-note">{t("alertsCenter.messages.readOnlyHint")}</p>}
          {alertNotice && <p className="banner banner-success">{alertNotice}</p>}

          <div className="filter-grid">
            <label className="control-field">
              <span>{t("alertsCenter.filters.statusLabel")}</span>
              <select
                value={alertStatusFilter}
                onChange={(event) => setAlertStatusFilter(event.target.value as "all" | "open" | "acknowledged" | "closed")}
              >
                <option value="all">{t("alertsCenter.filters.statusAll")}</option>
                <option value="open">{t("alertsCenter.status.open")}</option>
                <option value="acknowledged">{t("alertsCenter.status.acknowledged")}</option>
                <option value="closed">{t("alertsCenter.status.closed")}</option>
              </select>
            </label>
            <label className="control-field">
              <span>{t("alertsCenter.filters.severityLabel")}</span>
              <select
                value={alertSeverityFilter}
                onChange={(event) => setAlertSeverityFilter(event.target.value as "all" | "critical" | "warning" | "info")}
              >
                <option value="all">{t("alertsCenter.filters.severityAll")}</option>
                <option value="critical">{t("alertsCenter.severity.critical")}</option>
                <option value="warning">{t("alertsCenter.severity.warning")}</option>
                <option value="info">{t("alertsCenter.severity.info")}</option>
              </select>
            </label>
            <label className="control-field">
              <span>{t("alertsCenter.filters.siteLabel")}</span>
              <input
                value={alertSiteFilter}
                onChange={(event) => setAlertSiteFilter(event.target.value)}
                placeholder={t("alertsCenter.filters.sitePlaceholder")}
              />
            </label>
            <label className="control-field">
              <span>{t("alertsCenter.filters.queryLabel")}</span>
              <input
                value={alertQueryFilter}
                onChange={(event) => setAlertQueryFilter(event.target.value)}
                placeholder={t("alertsCenter.filters.queryPlaceholder")}
              />
            </label>
          </div>

          <div className="toolbar-row" style={{ marginBottom: "0.75rem" }}>
            <button
              onClick={() => void triggerBulkAcknowledge()}
              disabled={!canWriteCmdb || selectedAlertIds.length === 0 || alertBulkActionRunning !== null}
            >
              {alertBulkActionRunning === "ack" ? t("alertsCenter.actions.acking") : t("alertsCenter.actions.bulkAck")}
            </button>
            <button
              onClick={() => void triggerBulkClose()}
              disabled={!canWriteCmdb || selectedAlertIds.length === 0 || alertBulkActionRunning !== null}
            >
              {alertBulkActionRunning === "close" ? t("alertsCenter.actions.closing") : t("alertsCenter.actions.bulkClose")}
            </button>
          </div>

          {loadingAlerts && (alerts as any[]).length === 0 ? (
            <p>{t("alertsCenter.messages.loading")}</p>
          ) : (alerts as any[]).length === 0 ? (
            <p>{t("alertsCenter.messages.empty")}</p>
          ) : (
            <div style={{ overflowX: "auto", marginBottom: "1rem" }}>
              <table style={{ borderCollapse: "collapse", width: "100%", minWidth: "1080px" }}>
                <thead>
                  <tr>
                    <th style={cellStyle}>
                      <input type="checkbox" checked={allAlertsSelected} onChange={() => toggleSelectAllAlerts()} />
                    </th>
                    <th style={cellStyle}>{t("alertsCenter.table.id")}</th>
                    <th style={cellStyle}>{t("alertsCenter.table.title")}</th>
                    <th style={cellStyle}>{t("alertsCenter.table.severity")}</th>
                    <th style={cellStyle}>{t("alertsCenter.table.status")}</th>
                    <th style={cellStyle}>{t("alertsCenter.table.scope")}</th>
                    <th style={cellStyle}>{t("alertsCenter.table.source")}</th>
                    <th style={cellStyle}>{t("alertsCenter.table.lastSeen")}</th>
                    <th style={cellStyle}>{t("alertsCenter.table.actions")}</th>
                  </tr>
                </thead>
                <tbody>
                  {(alerts as any[]).map((alert) => (
                    <tr key={alert.id}>
                      <td style={cellStyle}>
                        <input
                          type="checkbox"
                          checked={selectedAlertIds.includes(alert.id)}
                          onChange={() => toggleAlertSelection(alert.id)}
                        />
                      </td>
                      <td style={cellStyle}>
                        <button
                          onClick={() => toggleAlertSelection(alert.id, true)}
                          style={{ border: "none", background: "transparent", color: "var(--brand)", padding: 0 }}
                        >
                          #{alert.id}
                        </button>
                      </td>
                      <td style={cellStyle}>{alert.title}</td>
                      <td style={cellStyle}>
                        <span className={`status-chip ${severityChipClass(alert.severity)}`}>
                          {t(`alertsCenter.severity.${alert.severity}`)}
                        </span>
                      </td>
                      <td style={cellStyle}>
                        <span className={`status-chip ${statusChipClass(alert.status)}`}>
                          {t(`alertsCenter.status.${alert.status}`)}
                        </span>
                      </td>
                      <td style={cellStyle}>
                        {(alert.site ?? "-")} / {(alert.department ?? "-")}
                      </td>
                      <td style={cellStyle}>{alert.alert_source}</td>
                      <td style={cellStyle}>{new Date(alert.last_seen_at).toLocaleString()}</td>
                      <td style={cellStyle}>
                        <div className="toolbar-row">
                          <button
                            onClick={() => void triggerSingleAcknowledge(alert.id)}
                            disabled={!canWriteCmdb || alert.status === "closed" || alertActionRunningId === alert.id}
                          >
                            {alertActionRunningId === alert.id ? t("alertsCenter.actions.processing") : t("alertsCenter.actions.ack")}
                          </button>
                          <button
                            onClick={() => void closeAlert(alert.id)}
                            disabled={!canWriteCmdb || alert.status === "closed" || alertActionRunningId === alert.id}
                          >
                            {alertActionRunningId === alert.id ? t("alertsCenter.actions.processing") : t("alertsCenter.actions.close")}
                          </button>
                        </div>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}

          <div className="detail-grid">
            <div className="detail-panel">
              <h3 style={{ marginTop: 0, marginBottom: "0.5rem", fontSize: "1rem" }}>
                {t("alertsCenter.detail.title")}
              </h3>
              {loadingAlertDetail ? (
                <p>{t("alertsCenter.messages.loadingDetail")}</p>
              ) : !alertDetail ? (
                <p>{t("alertsCenter.messages.detailEmpty")}</p>
              ) : (
                <>
                  <p className="section-meta">
                    #{alertDetail.alert.id} | {alertDetail.alert.alert_source} | {alertDetail.alert.alert_key}
                  </p>
                  <p style={{ marginTop: 0 }}>{alertDetail.alert.title}</p>
                  <p className="inline-note">
                    {t("alertsCenter.detail.scope")} {(alertDetail.alert.site ?? "-")} / {(alertDetail.alert.department ?? "-")}
                  </p>
                  <p className="inline-note">
                    {t("alertsCenter.detail.links")}{" "}
                    <a href="#/cmdb">{t("alertsCenter.detail.openCmdb")}</a>
                    {" | "}
                    <a href="#/tickets">{t("alertsCenter.detail.openTickets")}</a>
                  </p>
                  <h4 style={{ marginBottom: "0.4rem" }}>{t("alertsCenter.detail.timelineTitle")}</h4>
                  {alertDetail.timeline.length === 0 ? (
                    <p>{t("alertsCenter.detail.timelineEmpty")}</p>
                  ) : (
                    <div style={{ display: "grid", gap: "0.35rem", marginBottom: "0.75rem" }}>
                      {alertDetail.timeline.slice(0, 12).map((item: any) => (
                        <div key={item.id} className="hint-row">
                          {new Date(item.created_at).toLocaleString()} | {item.event_type} | {item.actor}
                          {item.message ? ` | ${item.message}` : ""}
                        </div>
                      ))}
                    </div>
                  )}
                  <h4 style={{ marginBottom: "0.4rem" }}>{t("alertsCenter.detail.linkedTicketsTitle")}</h4>
                  {alertDetail.linked_tickets.length === 0 ? (
                    <p>{t("alertsCenter.detail.linkedTicketsEmpty")}</p>
                  ) : (
                    <div style={{ display: "grid", gap: "0.35rem" }}>
                      {alertDetail.linked_tickets.map((ticket: any) => (
                        <div key={ticket.id} className="hint-row">
                          <a href="#/tickets">{ticket.ticket_no}</a> | {ticket.status} | {ticket.priority}
                        </div>
                      ))}
                    </div>
                  )}
                </>
              )}
            </div>

            <div className="detail-panel">
              <h3 style={{ marginTop: 0, marginBottom: "0.5rem", fontSize: "1rem" }}>
                {t("alertsCenter.detail.quickActionsTitle")}
              </h3>
              <p className="inline-note">{t("alertsCenter.detail.quickActionsDescription")}</p>
              <div className="toolbar-row">
                <button
                  onClick={() => void triggerSingleAcknowledge(Number.parseInt(selectedAlertId, 10))}
                  disabled={
                    !canWriteCmdb
                    || !selectedAlertId
                    || !Number.isFinite(Number.parseInt(selectedAlertId, 10))
                    || alertActionRunningId !== null
                  }
                >
                  {t("alertsCenter.actions.ackSelected")}
                </button>
                <button
                  onClick={() => void closeAlert(Number.parseInt(selectedAlertId, 10))}
                  disabled={
                    !canWriteCmdb
                    || !selectedAlertId
                    || !Number.isFinite(Number.parseInt(selectedAlertId, 10))
                    || alertActionRunningId !== null
                  }
                >
                  {t("alertsCenter.actions.closeSelected")}
                </button>
                <button
                  onClick={() => void triggerAlertRemediation(Number.parseInt(selectedAlertId, 10))}
                  disabled={
                    !canWriteCmdb
                    || !selectedAlertId
                    || !Number.isFinite(Number.parseInt(selectedAlertId, 10))
                    || alertActionRunningId !== null
                  }
                >
                  {alertActionRunningId !== null
                    ? t("alertsCenter.actions.remediating")
                    : t("alertsCenter.actions.remediate")}
                </button>
              </div>
              <p className="inline-note" style={{ marginTop: "0.6rem" }}>
                {t("alertsCenter.detail.selection", { id: selectedAlertId || "-" })}
              </p>
            </div>
          </div>
        </SectionCard>
      )}
    </>
  );
}

function statusChipClass(status: string): string {
  if (status === "pass" || status === "open") {
    return "status-chip-success";
  }
  if (status === "warn" || status === "acknowledged") {
    return "status-chip-warn";
  }
  if (status === "fail" || status === "critical" || status === "closed") {
    return "status-chip-danger";
  }
  return "";
}

function severityChipClass(severity: string): string {
  if (severity === "critical") {
    return "status-chip-danger";
  }
  if (severity === "warning") {
    return "status-chip-warn";
  }
  if (severity === "info") {
    return "status-chip-success";
  }
  return "";
}

const cellStyle: CSSProperties = {
  border: "1px solid #ddd",
  padding: "0.5rem",
  textAlign: "left",
  whiteSpace: "nowrap",
  verticalAlign: "top"
};
