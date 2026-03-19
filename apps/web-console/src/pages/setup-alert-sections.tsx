import { useState, type CSSProperties } from "react";
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

type SetupActivationRecommendedAction = {
  action_key: string;
  label: string;
  description: string;
  action_type: "link";
  href?: string | null;
  requires_write: boolean;
  auto_applicable: boolean;
  profile_key?: string | null;
};

type SetupActivationItem = {
  item_key: string;
  title: string;
  status: "ready" | "warning" | "blocking";
  summary: string;
  reason: string;
  recommended_action?: SetupActivationRecommendedAction | null;
  evidence: Record<string, unknown>;
};

type SetupActivationResponse = {
  generated_at: string;
  overall_status: "ready" | "warning" | "blocking";
  recommended_next_step_key: string | null;
  recommended_profile_key: string | null;
  summary: {
    total: number;
    ready: number;
    warning: number;
    blocking: number;
  };
  items: SetupActivationItem[];
};

type SetupActivationStarterTemplateItem = {
  template_key: string;
  name: string;
  summary: string;
  target_scale: string;
  first_value_goal: string;
  recommended_when: string;
  profile_key: string;
  defaults: Record<string, unknown>;
};

type SetupActivationStarterTemplateCatalogResponse = {
  recommended_template_key: string;
  items: SetupActivationStarterTemplateItem[];
  total: number;
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

type SetupProfileCatalogItem = {
  key: string;
  name: string;
  description: string;
  target_scale: string;
  defaults: Record<string, unknown>;
};

type SetupProfileChangeSummary = {
  domain: string;
  before: string;
  after: string;
  changed: boolean;
};

type SetupProfilePreviewResponse = {
  profile: SetupProfileCatalogItem;
  ready: boolean;
  summary: SetupProfileChangeSummary[];
};

type SetupProfileApplyAction = {
  action_key: string;
  outcome: string;
  detail: string;
};

type SetupProfileApplyResponse = {
  run_id: number;
  actor: string;
  profile_key: string;
  status: string;
  actions: SetupProfileApplyAction[];
  history_hint: string;
};

type SetupProfileHistoryRecord = {
  id: number;
  profile_key: string;
  profile_name: string;
  actor: string;
  status: string;
  note: string | null;
  reverted_by: string | null;
  reverted_at: string | null;
  created_at: string;
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
    alertSuppressedFilter,
    alertStatusFilter,
    alertPolicies,
    alertPolicyDraft,
    alertPolicyNotice,
    alertPolicyPreview,
    alerts,
    alertsTotal,
    applySetupProfile,
    applySetupTemplate,
    createAlertPolicy,
    canWriteCmdb,
    closeAlert,
    completeSetupWizard,
    creatingAlertPolicy,
    loadingAlertDetail,
    loadingAlerts,
    loadingAlertPolicies,
    loadingSetupActivation,
    loadingSetupActivationStarterTemplates,
    loadingSetupChecklist,
    loadingSetupPreflight,
    loadingSetupProfileHistory,
    loadingSetupProfiles,
    loadingSetupTemplates,
    previewSetupProfile,
    previewSetupTemplate,
    previewAlertPolicy,
    previewingAlertPolicy,
    refreshAlerts,
    refreshAlertPolicies,
    refreshSetupWizard,
    loadSetupActivation,
    loadSetupActivationFeedbackLoop,
    submitSetupActivationFeedback,
    updateSetupActivationFeedbackClosure,
    runningSetupActivationFeedbackKey,
    updatingSetupActivationFeedbackId,
    runningSetupTemplateApply,
    runningSetupTemplatePreview,
    runningSetupProfileApply,
    runningSetupProfilePreview,
    runningSetupProfileRevertId,
    selectedAlertId,
    selectedAlertIds,
    selectedSetupProfileKey,
    selectedSetupTemplateKey,
    setAlertPolicyDraft,
    setAlertQueryFilter,
    setAlertSeverityFilter,
    setAlertSiteFilter,
    setAlertSuppressedFilter,
    setAlertStatusFilter,
    setSelectedSetupTemplateKey,
    setSelectedSetupProfileKey,
    setSetupProfileNote,
    setSetupStep,
    setSetupTemplateNote,
    setSetupTemplateParam,
    setupChecklist,
    setupCompleted,
    setupActivation,
    setupActivationFeedbackLoop,
    setupActivationNotice,
    setupActivationStarterTemplates,
    setupNotice,
    setupProfileApplyResult,
    setupProfileHistory,
    setupProfileNote,
    setupProfileNotice,
    setupProfilePreview,
    setupProfiles,
    setupTemplateApplyResult,
    setupTemplateNote,
    setupTemplateNotice,
    setupTemplateParamsDraft,
    setupTemplatePreview,
    setupPreflight,
    setupStep,
    setupTemplates,
    loadingSetupActivationFeedbackLoop,
    t,
    revertSetupProfileRun,
    toggleAlertPolicyEnabled,
    toggleAlertSelection,
    toggleSelectAllAlerts,
    triggerAlertRemediation,
    triggerBulkAcknowledge,
    triggerBulkClose,
    triggerSingleAcknowledge,
    updatingAlertPolicyId,
    visibleSections
  } = rawProps as any;

  const preflight = setupPreflight as SetupChecklistResponse | null;
  const checklist = setupChecklist as SetupChecklistResponse | null;
  const activation = setupActivation as SetupActivationResponse | null;
  const activationFeedbackLoop = setupActivationFeedbackLoop as {
    summary: { total: number; open: number; in_progress: number; resolved: number };
    items: Array<{
      id: number;
      actor: string;
      step_key: string;
      template_key: string | null;
      feedback_kind: string;
      comment: string | null;
      closure_status: "open" | "in_progress" | "resolved";
      owner_ref: string | null;
      closure_note: string | null;
      closure_updated_at: string | null;
      created_at: string;
    }>;
  } | null;
  const starterTemplates = setupActivationStarterTemplates as SetupActivationStarterTemplateCatalogResponse | null;
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
  const profileItems = (setupProfiles as SetupProfileCatalogItem[]) ?? [];
  const selectedSetupProfile = profileItems.find((item) => item.key === selectedSetupProfileKey) ?? null;
  const profilePreview = setupProfilePreview as SetupProfilePreviewResponse | null;
  const profileApplyResult = setupProfileApplyResult as SetupProfileApplyResponse | null;
  const profileHistory = (setupProfileHistory as SetupProfileHistoryRecord[]) ?? [];
  const [activationFeedbackDrafts, setActivationFeedbackDrafts] = useState<Record<string, {
    feedback_kind: "blocked" | "confused" | "not_applicable";
    comment: string;
    template_key: string;
  }>>({});

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
  const activationItems = activation?.items ?? [];
  const activationRecommendedTemplateKey = starterTemplates?.recommended_template_key ?? activation?.recommended_profile_key ?? "";
  const setActivationFeedbackDraft = (
    stepKey: string,
    patch: Partial<{
      feedback_kind: "blocked" | "confused" | "not_applicable";
      comment: string;
      template_key: string;
    }>
  ) => {
    setActivationFeedbackDrafts((prev) => ({
      ...prev,
      [stepKey]: {
        feedback_kind: prev[stepKey]?.feedback_kind ?? "blocked",
        comment: prev[stepKey]?.comment ?? "",
        template_key: prev[stepKey]?.template_key ?? activationRecommendedTemplateKey,
        ...patch
      }
    }));
  };

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
          {setupActivationNotice && <p className="banner banner-success">{setupActivationNotice}</p>}

          <div className="detail-panel" style={{ marginBottom: "0.75rem" }}>
            <div className="toolbar-row" style={{ justifyContent: "space-between", alignItems: "center" }}>
              <div>
                <h3 style={{ marginTop: 0, marginBottom: "0.35rem", fontSize: "1rem" }}>First-value activation</h3>
                <p className="section-note" style={{ marginBottom: 0 }}>
                  Track whether this environment has reached the first usable SMB baseline and capture pilot friction directly from the setup path.
                </p>
              </div>
              <button
                onClick={() => {
                  void Promise.all([
                    loadSetupActivation(),
                    loadSetupActivationFeedbackLoop()
                  ]);
                }}
                disabled={loadingSetupActivation}
              >
                {loadingSetupActivation ? "Refreshing..." : "Refresh activation"}
              </button>
            </div>

            {loadingSetupActivation && !activation ? (
              <p>Loading activation status...</p>
            ) : !activation ? (
              <p>No activation status available.</p>
            ) : (
              <>
                <div className="toolbar-row" style={{ marginTop: "0.6rem", marginBottom: "0.6rem", gap: "0.75rem", flexWrap: "wrap" }}>
                  <span className={`status-chip ${statusChipClass(activation.overall_status)}`}>{activation.overall_status}</span>
                  <span className="section-meta">ready={activation.summary.ready}</span>
                  <span className="section-meta">warning={activation.summary.warning}</span>
                  <span className="section-meta">blocking={activation.summary.blocking}</span>
                  {activation.recommended_next_step_key && (
                    <span className="inline-note">recommended_next={activation.recommended_next_step_key}</span>
                  )}
                </div>

                {loadingSetupActivationStarterTemplates ? (
                  <p>Loading starter templates...</p>
                ) : starterTemplates && (starterTemplates.items ?? []).length > 0 ? (
                  <div style={{ display: "grid", gap: "0.55rem", marginBottom: "0.75rem" }}>
                    <strong>Starter templates</strong>
                    {(starterTemplates.items ?? []).map((item) => (
                      <div key={`starter-template-${item.template_key}`} className="detail-panel" style={{ marginBottom: 0 }}>
                        <div className="toolbar-row" style={{ justifyContent: "space-between", alignItems: "flex-start" }}>
                          <div>
                            <strong>{item.name}</strong>
                            <div className="inline-note">{item.template_key} | scale={item.target_scale}</div>
                            <div style={{ marginTop: "0.35rem" }}>{item.summary}</div>
                            <div className="inline-note" style={{ marginTop: "0.25rem" }}>
                              first_value_goal={item.first_value_goal}
                            </div>
                            <div className="inline-note">{item.recommended_when}</div>
                          </div>
                          <div style={{ display: "grid", gap: "0.35rem", justifyItems: "end" }}>
                            {starterTemplates.recommended_template_key === item.template_key && (
                              <span className="status-chip status-chip-success">recommended</span>
                            )}
                            <button
                              onClick={() => {
                                setSelectedSetupProfileKey(item.profile_key);
                                setSetupStep(3);
                              }}
                            >
                              Use this profile
                            </button>
                          </div>
                        </div>
                      </div>
                    ))}
                  </div>
                ) : null}

                <div style={{ display: "grid", gap: "0.55rem" }}>
                  {activationItems.map((item) => {
                    const draft = activationFeedbackDrafts[item.item_key] ?? {
                      feedback_kind: "blocked" as const,
                      comment: "",
                      template_key: activationRecommendedTemplateKey
                    };
                    const runningFeedback = runningSetupActivationFeedbackKey === item.item_key;
                    const isNext = activation.recommended_next_step_key === item.item_key;
                    return (
                      <div key={`activation-item-${item.item_key}`} className="detail-panel">
                        <div className="toolbar-row" style={{ justifyContent: "space-between", alignItems: "flex-start" }}>
                          <div>
                            <strong>{item.title}</strong>
                            <div className="inline-note">{item.item_key}</div>
                            {isNext && <div className="inline-note">next recommended step</div>}
                          </div>
                          <span className={`status-chip ${statusChipClass(item.status)}`}>{item.status}</span>
                        </div>
                        <p style={{ marginTop: "0.45rem", marginBottom: "0.35rem" }}>{item.summary}</p>
                        <p className="section-note" style={{ marginTop: 0 }}>{item.reason}</p>
                        {item.recommended_action?.href && (
                          <div className="toolbar-row" style={{ marginBottom: "0.5rem" }}>
                            <a
                              href={item.recommended_action.href}
                              onClick={() => {
                                if (item.recommended_action?.profile_key) {
                                  setSelectedSetupProfileKey(item.recommended_action.profile_key);
                                  setSetupStep(3);
                                }
                              }}
                            >
                              {item.recommended_action.label}
                            </a>
                          </div>
                        )}
                        <div style={{ overflowX: "auto", marginBottom: "0.5rem" }}>
                          <pre style={{ margin: 0, whiteSpace: "pre-wrap", fontSize: "0.8rem" }}>
                            {JSON.stringify(item.evidence ?? {}, null, 2)}
                          </pre>
                        </div>
                        <div className="filter-grid">
                          <label className="control-field">
                            <span>Feedback</span>
                            <select
                              value={draft.feedback_kind}
                              onChange={(event) => setActivationFeedbackDraft(item.item_key, {
                                feedback_kind: event.target.value as "blocked" | "confused" | "not_applicable"
                              })}
                            >
                              <option value="blocked">blocked</option>
                              <option value="confused">confused</option>
                              <option value="not_applicable">not_applicable</option>
                            </select>
                          </label>
                          <label className="control-field">
                            <span>Starter template</span>
                            <select
                              value={draft.template_key}
                              onChange={(event) => setActivationFeedbackDraft(item.item_key, {
                                template_key: event.target.value
                              })}
                            >
                              <option value="">none</option>
                              {(starterTemplates?.items ?? []).map((template) => (
                                <option key={`feedback-template-${item.item_key}-${template.template_key}`} value={template.template_key}>
                                  {template.name}
                                </option>
                              ))}
                            </select>
                          </label>
                        </div>
                        <label className="control-field" style={{ marginTop: "0.5rem" }}>
                          <span>Comment</span>
                          <input
                            value={draft.comment}
                            onChange={(event) => setActivationFeedbackDraft(item.item_key, {
                              comment: event.target.value
                            })}
                            placeholder="what blocked or confused this step"
                          />
                        </label>
                        <div className="toolbar-row" style={{ marginTop: "0.5rem" }}>
                          <button
                            onClick={() => void submitSetupActivationFeedback(
                              item.item_key,
                              draft.feedback_kind,
                              draft.comment,
                              draft.template_key || null,
                              {
                                category: "activation_pilot",
                                severity: draft.feedback_kind === "blocked"
                                  ? "high"
                                  : draft.feedback_kind === "confused"
                                    ? "medium"
                                    : "low",
                                module: "setup_activation",
                                expected_value: item.recommended_action?.label ?? item.summary,
                                item_status: item.status,
                                item_title: item.title
                              }
                            ).then((result: any) => {
                              if (result) {
                                setActivationFeedbackDraft(item.item_key, { comment: "" });
                              }
                            })}
                            disabled={runningFeedback}
                          >
                            {runningFeedback ? "Submitting..." : "Submit feedback"}
                          </button>
                        </div>
                      </div>
                    );
                  })}
                </div>
                <div className="detail-panel" style={{ marginTop: "0.75rem", marginBottom: 0 }}>
                  <div className="toolbar-row" style={{ justifyContent: "space-between", alignItems: "center" }}>
                    <div>
                      <h3 style={{ marginTop: 0, marginBottom: "0.35rem", fontSize: "1rem" }}>Pilot feedback loop</h3>
                      <p className="section-note" style={{ marginBottom: 0 }}>
                        Structured feedback queue with closure status tracking for follow-up.
                      </p>
                    </div>
                    <button
                      onClick={() => void loadSetupActivationFeedbackLoop()}
                      disabled={loadingSetupActivationFeedbackLoop}
                    >
                      {loadingSetupActivationFeedbackLoop ? "Refreshing..." : "Refresh loop"}
                    </button>
                  </div>
                  {!activationFeedbackLoop ? (
                    <p className="inline-note" style={{ marginTop: "0.6rem" }}>
                      {loadingSetupActivationFeedbackLoop ? "Loading feedback loop..." : "No feedback loop snapshot loaded yet."}
                    </p>
                  ) : (
                    <>
                      <div className="toolbar-row" style={{ marginTop: "0.6rem", marginBottom: "0.6rem", gap: "0.7rem", flexWrap: "wrap" }}>
                        <span className="status-chip">total={activationFeedbackLoop.summary.total}</span>
                        <span className="status-chip status-chip-danger">open={activationFeedbackLoop.summary.open}</span>
                        <span className="status-chip status-chip-warn">in_progress={activationFeedbackLoop.summary.in_progress}</span>
                        <span className="status-chip status-chip-success">resolved={activationFeedbackLoop.summary.resolved}</span>
                      </div>
                      <div style={{ display: "grid", gap: "0.5rem" }}>
                        {(activationFeedbackLoop.items ?? []).slice(0, 12).map((entry) => (
                          <div key={`activation-feedback-loop-${entry.id}`} className="detail-panel" style={{ marginBottom: 0 }}>
                            <div className="toolbar-row" style={{ justifyContent: "space-between", alignItems: "flex-start" }}>
                              <div>
                                <strong>#{entry.id} {entry.step_key}</strong>
                                <div className="inline-note">
                                  {entry.feedback_kind} | actor={entry.actor} | created={entry.created_at}
                                </div>
                              </div>
                              <span
                                className={`status-chip ${
                                  entry.closure_status === "resolved"
                                    ? "status-chip-success"
                                    : entry.closure_status === "in_progress"
                                      ? "status-chip-warn"
                                      : "status-chip-danger"
                                }`}
                              >
                                {entry.closure_status}
                              </span>
                            </div>
                            {entry.comment && <p className="section-note" style={{ margin: "0.45rem 0" }}>{entry.comment}</p>}
                            <div className="toolbar-row" style={{ gap: "0.45rem", flexWrap: "wrap" }}>
                              <button
                                onClick={() => void updateSetupActivationFeedbackClosure(entry.id, "in_progress")}
                                disabled={updatingSetupActivationFeedbackId === entry.id || entry.closure_status === "in_progress"}
                              >
                                In progress
                              </button>
                              <button
                                onClick={() => void updateSetupActivationFeedbackClosure(entry.id, "resolved")}
                                disabled={updatingSetupActivationFeedbackId === entry.id || entry.closure_status === "resolved"}
                              >
                                Mark resolved
                              </button>
                              <button
                                onClick={() => void updateSetupActivationFeedbackClosure(entry.id, "open")}
                                disabled={updatingSetupActivationFeedbackId === entry.id || entry.closure_status === "open"}
                              >
                                Reopen
                              </button>
                            </div>
                          </div>
                        ))}
                      </div>
                    </>
                  )}
                </div>
              </>
            )}
          </div>

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

          <div className="detail-panel" style={{ marginBottom: "0.75rem" }}>
            <h3 style={{ marginTop: 0, marginBottom: "0.5rem", fontSize: "1rem" }}>Operator profile presets</h3>
            <p className="section-note" style={{ marginBottom: "0.6rem" }}>
              Select a profile to auto-fill identity, alerting, escalation, and backup defaults without manual JSON edits.
            </p>
            {setupProfileNotice && <p className="banner banner-success">{setupProfileNotice}</p>}
            {!canWriteCmdb && (
              <p className="inline-note">
                Read-only mode: preview and history are available, apply/revert requires operator or admin role.
              </p>
            )}
            {loadingSetupProfiles ? (
              <p>Loading setup profiles...</p>
            ) : profileItems.length === 0 ? (
              <p>No setup profile preset available.</p>
            ) : (
              <>
                <label className="control-field" style={{ marginBottom: "0.6rem" }}>
                  <span>Profile</span>
                  <select
                    value={selectedSetupProfileKey}
                    onChange={(event) => setSelectedSetupProfileKey(event.target.value)}
                  >
                    {profileItems.map((item) => (
                      <option key={item.key} value={item.key}>
                        {item.name}
                      </option>
                    ))}
                  </select>
                </label>

                {selectedSetupProfile && (
                  <p className="section-note" style={{ marginBottom: "0.5rem" }}>
                    {selectedSetupProfile.description} | target_scale={selectedSetupProfile.target_scale}
                  </p>
                )}

                <label className="control-field" style={{ marginTop: "0.5rem" }}>
                  <span>Note</span>
                  <input
                    value={setupProfileNote}
                    onChange={(event) => setSetupProfileNote(event.target.value)}
                    placeholder="change reason / rollout context"
                  />
                </label>

                <div className="toolbar-row" style={{ marginTop: "0.65rem" }}>
                  <button
                    onClick={() => void previewSetupProfile()}
                    disabled={!selectedSetupProfileKey || runningSetupProfilePreview || runningSetupProfileApply}
                  >
                    {runningSetupProfilePreview ? "Previewing..." : "Preview profile"}
                  </button>
                  <button
                    onClick={() => void applySetupProfile()}
                    disabled={!canWriteCmdb || !selectedSetupProfileKey || runningSetupProfileApply || runningSetupProfilePreview}
                  >
                    {runningSetupProfileApply ? "Applying..." : "Apply profile"}
                  </button>
                </div>

                {profilePreview && (
                  <div className="hint-list" style={{ marginTop: "0.75rem" }}>
                    <div className="hint-row" style={{ fontWeight: 600 }}>
                      Preview summary
                      {" "}
                      <span className={`status-chip ${profilePreview.ready ? "status-chip-success" : "status-chip-warn"}`}>
                        {profilePreview.ready ? "ready" : "blocked"}
                      </span>
                    </div>
                    {profilePreview.summary.map((item) => (
                      <div key={`profile-preview-${item.domain}`} className="hint-row">
                        <strong>{item.domain}</strong> [{item.changed ? "changed" : "unchanged"}]
                        {" | "}before={item.before}
                        {" | "}after={item.after}
                      </div>
                    ))}
                  </div>
                )}

                {profileApplyResult && (
                  <div className="hint-list" style={{ marginTop: "0.75rem" }}>
                    <div className="hint-row" style={{ fontWeight: 600 }}>
                      Applied profile={profileApplyResult.profile_key} by {profileApplyResult.actor} (run #{profileApplyResult.run_id})
                    </div>
                    {profileApplyResult.actions.map((action) => (
                      <div key={`profile-action-${action.action_key}`} className="hint-row">
                        <strong>{action.action_key}</strong> [{action.outcome}] {action.detail}
                      </div>
                    ))}
                    <div className="hint-row">{profileApplyResult.history_hint}</div>
                  </div>
                )}
              </>
            )}

            <h4 style={{ marginBottom: "0.4rem", marginTop: "0.8rem" }}>Profile apply history</h4>
            {loadingSetupProfileHistory ? (
              <p>Loading setup profile history...</p>
            ) : profileHistory.length === 0 ? (
              <p>No setup profile history yet.</p>
            ) : (
              <div style={{ overflowX: "auto" }}>
                <table style={{ borderCollapse: "collapse", minWidth: "1000px", width: "100%" }}>
                  <thead>
                    <tr>
                      <th style={cellStyle}>Run</th>
                      <th style={cellStyle}>Profile</th>
                      <th style={cellStyle}>Actor</th>
                      <th style={cellStyle}>Status</th>
                      <th style={cellStyle}>Time</th>
                      <th style={cellStyle}>Actions</th>
                    </tr>
                  </thead>
                  <tbody>
                    {profileHistory.map((item) => {
                      const running = runningSetupProfileRevertId === item.id;
                      const reverted = item.status === "reverted" || item.reverted_at !== null;
                      return (
                        <tr key={`setup-profile-history-${item.id}`}>
                          <td style={cellStyle}>#{item.id}</td>
                          <td style={cellStyle}>
                            {item.profile_name}
                            <div className="inline-note">{item.profile_key}</div>
                          </td>
                          <td style={cellStyle}>
                            {item.actor}
                            {item.reverted_by ? <div className="inline-note">reverted_by={item.reverted_by}</div> : null}
                          </td>
                          <td style={cellStyle}>
                            <span className={`status-chip ${reverted ? "status-chip-warn" : "status-chip-success"}`}>
                              {item.status}
                            </span>
                          </td>
                          <td style={cellStyle}>
                            {new Date(item.created_at).toLocaleString()}
                            {item.reverted_at ? (
                              <div className="inline-note">
                                reverted_at={new Date(item.reverted_at).toLocaleString()}
                              </div>
                            ) : null}
                          </td>
                          <td style={cellStyle}>
                            <button
                              onClick={() => void revertSetupProfileRun(item.id)}
                              disabled={!canWriteCmdb || reverted || running || runningSetupProfileRevertId !== null}
                            >
                              {running ? "Reverting..." : reverted ? "Reverted" : "Revert run"}
                            </button>
                          </td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>
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
              <span>Suppression</span>
              <select
                value={alertSuppressedFilter}
                onChange={(event) => setAlertSuppressedFilter(event.target.value as "all" | "true" | "false")}
              >
                <option value="all">All alerts</option>
                <option value="true">Suppressed only</option>
                <option value="false">Non-suppressed only</option>
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
                  <p className="inline-note">
                    dedup_events={alertDetail.governance.dedup_event_count}
                    {" | "}
                    suppressed_count={alertDetail.governance.suppressed_count}
                    {" | "}
                    latest_unsuppressed={
                      alertDetail.governance.latest_unsuppressed_event_at
                        ? new Date(alertDetail.governance.latest_unsuppressed_event_at).toLocaleString()
                        : "-"
                    }
                  </p>
                  {alertDetail.governance.latest_suppression_reason && (
                    <p className="inline-note">
                      latest_suppression_reason={alertDetail.governance.latest_suppression_reason}
                    </p>
                  )}
                  {alertDetail.governance.latest_unsuppressed_policy_key && (
                    <p className="inline-note">
                      latest_unsuppressed_policy={alertDetail.governance.latest_unsuppressed_policy_key}
                    </p>
                  )}
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

          <div className="detail-panel" style={{ marginTop: "0.85rem" }}>
            <div className="toolbar-row" style={{ justifyContent: "space-between" }}>
              <h3 style={{ margin: 0, fontSize: "1rem" }}>Alert policy governance</h3>
              <button onClick={() => void refreshAlertPolicies()} disabled={loadingAlertPolicies}>
                {loadingAlertPolicies ? t("alertsCenter.actions.refreshing") : "Refresh policies"}
              </button>
            </div>
            {alertPolicyNotice && <p className="banner banner-success">{alertPolicyNotice}</p>}
            <p className="section-note">
              Configure dedup/suppression safely with form fields and preview before creating policy.
            </p>
            <div className="form-grid">
              <label className="control-field">
                <span>Policy key</span>
                <input
                  value={alertPolicyDraft.policy_key}
                  onChange={(event) => setAlertPolicyDraft((prev: any) => ({ ...prev, policy_key: event.target.value }))}
                  placeholder="repeated-failure-followup"
                />
              </label>
              <label className="control-field">
                <span>Name</span>
                <input
                  value={alertPolicyDraft.name}
                  onChange={(event) => setAlertPolicyDraft((prev: any) => ({ ...prev, name: event.target.value }))}
                  placeholder="Repeated Failure Follow-up"
                />
              </label>
              <label className="control-field">
                <span>Source</span>
                <input
                  value={alertPolicyDraft.match_source}
                  onChange={(event) => setAlertPolicyDraft((prev: any) => ({ ...prev, match_source: event.target.value }))}
                  placeholder="monitoring_sync"
                />
              </label>
              <label className="control-field">
                <span>Severity</span>
                <select
                  value={alertPolicyDraft.match_severity}
                  onChange={(event) => setAlertPolicyDraft((prev: any) => ({ ...prev, match_severity: event.target.value }))}
                >
                  <option value="all">all</option>
                  <option value="critical">critical</option>
                  <option value="warning">warning</option>
                  <option value="info">info</option>
                </select>
              </label>
              <label className="control-field">
                <span>Status</span>
                <select
                  value={alertPolicyDraft.match_status}
                  onChange={(event) => setAlertPolicyDraft((prev: any) => ({ ...prev, match_status: event.target.value }))}
                >
                  <option value="all">all</option>
                  <option value="open">open</option>
                  <option value="acknowledged">acknowledged</option>
                  <option value="closed">closed</option>
                </select>
              </label>
              <label className="control-field">
                <span>Dedup window (seconds)</span>
                <input
                  type="number"
                  min={30}
                  max={604800}
                  value={alertPolicyDraft.dedup_window_seconds}
                  onChange={(event) => setAlertPolicyDraft((prev: any) => ({ ...prev, dedup_window_seconds: event.target.value }))}
                />
              </label>
              <label className="control-field">
                <span>Ticket priority</span>
                <select
                  value={alertPolicyDraft.ticket_priority}
                  onChange={(event) => setAlertPolicyDraft((prev: any) => ({ ...prev, ticket_priority: event.target.value }))}
                >
                  <option value="low">low</option>
                  <option value="medium">medium</option>
                  <option value="high">high</option>
                  <option value="critical">critical</option>
                </select>
              </label>
              <label className="control-field">
                <span>Ticket category</span>
                <input
                  value={alertPolicyDraft.ticket_category}
                  onChange={(event) => setAlertPolicyDraft((prev: any) => ({ ...prev, ticket_category: event.target.value }))}
                  placeholder="incident"
                />
              </label>
              <label className="control-field" style={{ gridColumn: "1 / -1" }}>
                <span>Description</span>
                <textarea
                  rows={2}
                  value={alertPolicyDraft.description}
                  onChange={(event) => setAlertPolicyDraft((prev: any) => ({ ...prev, description: event.target.value }))}
                />
              </label>
              <label className="control-field">
                <span>Enabled</span>
                <select
                  value={alertPolicyDraft.is_enabled ? "true" : "false"}
                  onChange={(event) => setAlertPolicyDraft((prev: any) => ({ ...prev, is_enabled: event.target.value === "true" }))}
                >
                  <option value="true">true</option>
                  <option value="false">false</option>
                </select>
              </label>
            </div>
            <div className="toolbar-row" style={{ marginTop: "0.6rem" }}>
              <button onClick={() => void previewAlertPolicy()} disabled={!canWriteCmdb || previewingAlertPolicy}>
                {previewingAlertPolicy ? t("alertsCenter.actions.processing") : "Preview policy"}
              </button>
              <button onClick={() => void createAlertPolicy()} disabled={!canWriteCmdb || creatingAlertPolicy}>
                {creatingAlertPolicy ? t("alertsCenter.actions.processing") : "Create policy"}
              </button>
            </div>

            {alertPolicyPreview && (
              <div className="detail-panel" style={{ marginTop: "0.6rem" }}>
                <p className="section-note" style={{ margin: 0 }}>
                  {alertPolicyPreview.summary}
                </p>
                <p className="inline-note" style={{ marginTop: "0.4rem" }}>
                  generated_at={new Date(alertPolicyPreview.generated_at).toLocaleString()}
                  {" | "}
                  matched={alertPolicyPreview.matched_alert_count}
                  {" | "}
                  potentially_suppressed={alertPolicyPreview.potentially_suppressed_count}
                </p>
                {alertPolicyPreview.sample_alerts.length > 0 && (
                  <div style={{ display: "grid", gap: "0.25rem" }}>
                    {alertPolicyPreview.sample_alerts.map((item: any) => (
                      <div key={`alert-policy-preview-${item.alert_id}`} className="hint-row">
                        #{item.alert_id} | {item.alert_source} | {item.severity} | {item.status} | {item.title}
                      </div>
                    ))}
                  </div>
                )}
              </div>
            )}

            {loadingAlertPolicies && (alertPolicies as any[]).length === 0 ? (
              <p>{t("alertsCenter.messages.loading")}</p>
            ) : (alertPolicies as any[]).length === 0 ? (
              <p>No policy configured.</p>
            ) : (
              <div style={{ overflowX: "auto", marginTop: "0.6rem" }}>
                <table style={{ borderCollapse: "collapse", minWidth: "980px", width: "100%" }}>
                  <thead>
                    <tr>
                      <th style={cellStyle}>Key</th>
                      <th style={cellStyle}>Name</th>
                      <th style={cellStyle}>Match</th>
                      <th style={cellStyle}>Dedup(s)</th>
                      <th style={cellStyle}>Ticket</th>
                      <th style={cellStyle}>Enabled</th>
                      <th style={cellStyle}>Actions</th>
                    </tr>
                  </thead>
                  <tbody>
                    {(alertPolicies as any[]).map((policy) => (
                      <tr key={`alert-policy-row-${policy.id}`}>
                        <td style={cellStyle}>{policy.policy_key}</td>
                        <td style={cellStyle}>{policy.name}</td>
                        <td style={cellStyle}>
                          source={policy.match_source ?? "*"} | severity={policy.match_severity ?? "*"} | status={policy.match_status ?? "*"}
                        </td>
                        <td style={cellStyle}>{policy.dedup_window_seconds}</td>
                        <td style={cellStyle}>
                          {policy.ticket_priority}/{policy.ticket_category}
                        </td>
                        <td style={cellStyle}>{policy.is_enabled ? "true" : "false"}</td>
                        <td style={cellStyle}>
                          <button
                            onClick={() => void toggleAlertPolicyEnabled(policy)}
                            disabled={!canWriteCmdb || updatingAlertPolicyId === policy.id}
                          >
                            {updatingAlertPolicyId === policy.id
                              ? t("alertsCenter.actions.processing")
                              : policy.is_enabled
                                ? "Disable"
                                : "Enable"}
                          </button>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </div>
        </SectionCard>
      )}
    </>
  );
}

function statusChipClass(status: string): string {
  if (status === "pass" || status === "open" || status === "ready") {
    return "status-chip-success";
  }
  if (status === "warn" || status === "acknowledged" || status === "warning") {
    return "status-chip-warn";
  }
  if (status === "fail" || status === "critical" || status === "closed" || status === "blocking") {
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
