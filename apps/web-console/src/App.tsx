import { useCallback, useEffect, useMemo, useState } from "react";
import type { CSSProperties } from "react";
import { useTranslation } from "react-i18next";
import { AppShell, AuthGate, SectionCard } from "./components/layout";

type Asset = {
  id: number;
  asset_class: string;
  name: string;
  hostname: string | null;
  ip: string | null;
  status: string;
  site: string | null;
  department: string | null;
  owner: string | null;
  qr_code: string | null;
  barcode: string | null;
  custom_fields: Record<string, unknown>;
  created_at: string;
  updated_at: string;
};

type AssetListResponse = {
  items: Asset[];
  total: number;
  limit: number;
  offset: number;
};

type FieldDefinition = {
  id: number;
  field_key: string;
  name: string;
  field_type: string;
  max_length: number | null;
  required: boolean;
  options: string[] | null;
  scanner_enabled: boolean;
  is_enabled: boolean;
  created_at: string;
  updated_at: string;
};

type AssetRelation = {
  id: number;
  src_asset_id: number;
  dst_asset_id: number;
  relation_type: string;
  source: string;
  created_at: string;
  updated_at: string;
};

type AssetOwnerBinding = {
  owner_type: string;
  owner_ref: string;
};

type AssetOperationalReadiness = {
  department_count: number;
  business_service_count: number;
  owner_count: number;
  can_transition_operational: boolean;
  missing: string[];
};

type AssetBindingsResponse = {
  asset_id: number;
  departments: string[];
  business_services: string[];
  owners: AssetOwnerBinding[];
  readiness: AssetOperationalReadiness;
};

type MonitoringBindingRecord = {
  id: number;
  asset_id: number;
  source_system: string;
  source_id: number | null;
  external_host_id: string | null;
  last_sync_status: string;
  last_sync_message: string | null;
  last_sync_at: string | null;
  mapping: Record<string, unknown>;
  created_at: string;
  updated_at: string;
};

type MonitoringSyncJobRecord = {
  id: number;
  asset_id: number;
  trigger_source: string;
  status: string;
  attempt: number;
  max_attempts: number;
  run_after: string;
  requested_by: string | null;
  requested_at: string;
  started_at: string | null;
  completed_at: string | null;
  last_error: string | null;
  payload: Record<string, unknown>;
  created_at: string;
  updated_at: string;
};

type AssetMonitoringBindingResponse = {
  asset_id: number;
  binding: MonitoringBindingRecord | null;
  latest_job: MonitoringSyncJobRecord | null;
};

type AssetLifecycleTransitionResponse = {
  asset_id: number;
  previous_status: string;
  status: string;
  readiness: AssetOperationalReadiness;
};

type ImpactNode = {
  id: number;
  name: string;
  asset_class: string;
  status: string;
  depth: number;
};

type ImpactEdge = {
  id: number;
  src_asset_id: number;
  dst_asset_id: number;
  relation_type: string;
  source: string;
  direction: string;
  depth: number;
};

type AssetImpactResponse = {
  root_asset_id: number;
  direction: string;
  depth_limit: number;
  relation_types: string[];
  nodes: ImpactNode[];
  edges: ImpactEdge[];
  affected_business_services: ImpactNode[];
  affected_owners: ImpactNode[];
};

type DiscoveryJob = {
  id: number;
  name: string;
  source_type: string;
  scope: Record<string, unknown>;
  schedule: string | null;
  status: string;
  is_enabled: boolean;
  last_run_at: string | null;
  last_run_status: string | null;
  last_error: string | null;
  created_at: string;
  updated_at: string;
};

type DiscoveryCandidate = {
  id: number;
  job_id: number | null;
  fingerprint: string;
  payload: Record<string, unknown>;
  review_status: string;
  discovered_at: string;
  reviewed_by: string | null;
  reviewed_at: string | null;
  created_at: string;
  updated_at: string;
};

type NotificationChannel = {
  id: number;
  name: string;
  channel_type: string;
  target: string;
  config: Record<string, unknown>;
  is_enabled: boolean;
  created_at: string;
  updated_at: string;
};

type NotificationTemplate = {
  id: number;
  event_type: string;
  title_template: string;
  body_template: string;
  is_enabled: boolean;
  created_at: string;
  updated_at: string;
};

type NotificationSubscription = {
  id: number;
  channel_id: number;
  event_type: string;
  site: string | null;
  department: string | null;
  is_enabled: boolean;
  created_at: string;
  updated_at: string;
};

type NewFieldForm = {
  field_key: string;
  name: string;
  field_type: string;
  max_length: string;
  options_csv: string;
  required: boolean;
  scanner_enabled: boolean;
};

type NewRelationForm = {
  dst_asset_id: string;
  relation_type: string;
  source: string;
};

type NewNotificationChannelForm = {
  name: string;
  channel_type: "email" | "webhook";
  target: string;
  config_json: string;
};

type NewNotificationTemplateForm = {
  event_type: string;
  title_template: string;
  body_template: string;
};

type NewNotificationSubscriptionForm = {
  channel_id: string;
  event_type: string;
  site: string;
  department: string;
};

type AssetSortMode = "updated_desc" | "name_asc" | "id_asc";
type LifecycleStatus = "idle" | "onboarding" | "operational" | "maintenance" | "retired";
type ImpactDirection = "downstream" | "upstream" | "both";
type OwnerType = "team" | "user" | "group" | "external";

type OwnerDraft = {
  key: string;
  owner_type: OwnerType;
  owner_ref: string;
};

const DEFAULT_API_BASE_URL =
  typeof window !== "undefined"
    ? `${window.location.protocol}//${window.location.hostname}:8080`
    : "http://127.0.0.1:8080";
const API_BASE_URL = import.meta.env.VITE_API_BASE_URL?.trim() || DEFAULT_API_BASE_URL;
const API_AUTH_USER = (import.meta.env.VITE_AUTH_USER ?? "admin").trim();
const API_AUTH_TOKEN = (import.meta.env.VITE_AUTH_TOKEN ?? "").trim();
const AUTH_SESSION_STORAGE_KEY = "cloudops.auth.session.v1";
const AUTH_SESSION_EXPIRED_EVENT = "cloudops.auth.session-expired";

type AuthMode = "header" | "bearer";

type AuthSession = {
  mode: AuthMode;
  principal: string;
  token: string | null;
};

type AuthIdentity = {
  user: {
    id: number;
    username: string;
    display_name: string | null;
    email: string | null;
  };
  roles: string[];
};

const runtimeDefaultSession = deriveDefaultAuthSession();
let runtimeAuthSession: AuthSession | null = loadStoredAuthSession() ?? runtimeDefaultSession;

const defaultFieldForm: NewFieldForm = {
  field_key: "",
  name: "",
  field_type: "text",
  max_length: "",
  options_csv: "",
  required: false,
  scanner_enabled: false
};

const defaultRelationForm: NewRelationForm = {
  dst_asset_id: "",
  relation_type: "depends_on",
  source: "manual"
};

const defaultNotificationChannelForm: NewNotificationChannelForm = {
  name: "",
  channel_type: "webhook",
  target: "",
  config_json: "{}"
};

const defaultNotificationTemplateForm: NewNotificationTemplateForm = {
  event_type: "asset.new_detected",
  title_template: "Discovery Event: {{event_type}}",
  body_template: "{{payload}}"
};

const defaultNotificationSubscriptionForm: NewNotificationSubscriptionForm = {
  channel_id: "",
  event_type: "asset.new_detected",
  site: "",
  department: ""
};

const lifecycleStatuses: LifecycleStatus[] = [
  "idle",
  "onboarding",
  "operational",
  "maintenance",
  "retired"
];

export function App() {
  const { t } = useTranslation();
  const [authSession, setAuthSession] = useState<AuthSession | null>(runtimeAuthSession);
  const [authIdentity, setAuthIdentity] = useState<AuthIdentity | null>(null);
  const [authLoading, setAuthLoading] = useState(false);
  const [authError, setAuthError] = useState<string | null>(null);
  const [authNotice, setAuthNotice] = useState<string | null>(null);
  const [loginMode, setLoginMode] = useState<AuthMode>(runtimeAuthSession?.mode ?? "header");
  const [loginPrincipal, setLoginPrincipal] = useState<string>(
    runtimeAuthSession?.mode === "header" ? runtimeAuthSession.principal : API_AUTH_USER
  );
  const [loginToken, setLoginToken] = useState<string>(
    runtimeAuthSession?.mode === "bearer" ? runtimeAuthSession.token ?? "" : API_AUTH_TOKEN
  );
  const [assets, setAssets] = useState<Asset[]>([]);
  const [fieldDefinitions, setFieldDefinitions] = useState<FieldDefinition[]>([]);
  const [loadingAssets, setLoadingAssets] = useState(false);
  const [loadingFields, setLoadingFields] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [creatingSample, setCreatingSample] = useState(false);
  const [creatingField, setCreatingField] = useState(false);
  const [newField, setNewField] = useState<NewFieldForm>(defaultFieldForm);
  const [scanCode, setScanCode] = useState("");
  const [scanMode, setScanMode] = useState<"auto" | "qr" | "barcode">("auto");
  const [scanning, setScanning] = useState(false);
  const [scanResult, setScanResult] = useState<Asset | null>(null);
  const [relations, setRelations] = useState<AssetRelation[]>([]);
  const [loadingRelations, setLoadingRelations] = useState(false);
  const [selectedAssetId, setSelectedAssetId] = useState<string>("");
  const [assetSearch, setAssetSearch] = useState("");
  const [assetStatusFilter, setAssetStatusFilter] = useState("");
  const [assetClassFilter, setAssetClassFilter] = useState("");
  const [assetSiteFilter, setAssetSiteFilter] = useState("");
  const [assetSortMode, setAssetSortMode] = useState<AssetSortMode>("updated_desc");
  const [relationNotice, setRelationNotice] = useState<string | null>(null);
  const [creatingRelation, setCreatingRelation] = useState(false);
  const [deletingRelationId, setDeletingRelationId] = useState<number | null>(null);
  const [newRelation, setNewRelation] = useState<NewRelationForm>(defaultRelationForm);
  const [assetBindings, setAssetBindings] = useState<AssetBindingsResponse | null>(null);
  const [loadingAssetBindings, setLoadingAssetBindings] = useState(false);
  const [updatingAssetBindings, setUpdatingAssetBindings] = useState(false);
  const [bindingNotice, setBindingNotice] = useState<string | null>(null);
  const [bindingDepartmentsInput, setBindingDepartmentsInput] = useState("");
  const [bindingBusinessServicesInput, setBindingBusinessServicesInput] = useState("");
  const [bindingOwnerDrafts, setBindingOwnerDrafts] = useState<OwnerDraft[]>([]);
  const [transitioningLifecycleStatus, setTransitioningLifecycleStatus] = useState<LifecycleStatus | null>(null);
  const [lifecycleNotice, setLifecycleNotice] = useState<string | null>(null);
  const [assetMonitoring, setAssetMonitoring] = useState<AssetMonitoringBindingResponse | null>(null);
  const [loadingAssetMonitoring, setLoadingAssetMonitoring] = useState(false);
  const [triggeringMonitoringSync, setTriggeringMonitoringSync] = useState(false);
  const [monitoringNotice, setMonitoringNotice] = useState<string | null>(null);
  const [impactDirection, setImpactDirection] = useState<ImpactDirection>("downstream");
  const [impactDepth, setImpactDepth] = useState("4");
  const [assetImpact, setAssetImpact] = useState<AssetImpactResponse | null>(null);
  const [loadingAssetImpact, setLoadingAssetImpact] = useState(false);
  const [impactNotice, setImpactNotice] = useState<string | null>(null);
  const [discoveryJobs, setDiscoveryJobs] = useState<DiscoveryJob[]>([]);
  const [discoveryCandidates, setDiscoveryCandidates] = useState<DiscoveryCandidate[]>([]);
  const [loadingDiscoveryJobs, setLoadingDiscoveryJobs] = useState(false);
  const [loadingDiscoveryCandidates, setLoadingDiscoveryCandidates] = useState(false);
  const [runningDiscoveryJobId, setRunningDiscoveryJobId] = useState<number | null>(null);
  const [reviewingCandidateId, setReviewingCandidateId] = useState<number | null>(null);
  const [discoveryNotice, setDiscoveryNotice] = useState<string | null>(null);
  const [notificationChannels, setNotificationChannels] = useState<NotificationChannel[]>([]);
  const [notificationTemplates, setNotificationTemplates] = useState<NotificationTemplate[]>([]);
  const [notificationSubscriptions, setNotificationSubscriptions] = useState<NotificationSubscription[]>([]);
  const [loadingNotificationChannels, setLoadingNotificationChannels] = useState(false);
  const [loadingNotificationTemplates, setLoadingNotificationTemplates] = useState(false);
  const [loadingNotificationSubscriptions, setLoadingNotificationSubscriptions] = useState(false);
  const [creatingNotificationChannel, setCreatingNotificationChannel] = useState(false);
  const [creatingNotificationTemplate, setCreatingNotificationTemplate] = useState(false);
  const [creatingNotificationSubscription, setCreatingNotificationSubscription] = useState(false);
  const [notificationNotice, setNotificationNotice] = useState<string | null>(null);
  const [newNotificationChannel, setNewNotificationChannel] =
    useState<NewNotificationChannelForm>(defaultNotificationChannelForm);
  const [newNotificationTemplate, setNewNotificationTemplate] =
    useState<NewNotificationTemplateForm>(defaultNotificationTemplateForm);
  const [newNotificationSubscription, setNewNotificationSubscription] =
    useState<NewNotificationSubscriptionForm>(defaultNotificationSubscriptionForm);
  const roleSet = useMemo(() => new Set(authIdentity?.roles ?? []), [authIdentity?.roles]);
  const canWriteCmdb = roleSet.has("admin") || roleSet.has("operator");
  const canAccessAdmin = roleSet.has("admin");
  const roleText = useMemo(() => (authIdentity?.roles.length ? authIdentity.roles.join(", ") : "-"), [authIdentity]);

  const applyAuthSession = useCallback((session: AuthSession | null) => {
    runtimeAuthSession = session;
    persistAuthSession(session);
    setAuthSession(session);
  }, []);

  const loadCurrentIdentity = useCallback(async () => {
    const response = await apiFetch(`${API_BASE_URL}/api/v1/auth/me`);
    if (!response.ok) {
      throw new Error(await readErrorMessage(response));
    }
    const payload: AuthIdentity = await response.json();
    return payload;
  }, []);

  const signIn = useCallback(async () => {
    const normalizedPrincipal = loginPrincipal.trim();
    const normalizedToken = loginToken.trim();
    setAuthError(null);
    setAuthNotice(null);

    if (loginMode === "header") {
      if (!normalizedPrincipal) {
        setAuthError(t("auth.messages.usernameRequired"));
        return;
      }
      applyAuthSession({
        mode: "header",
        principal: normalizedPrincipal,
        token: null
      });
      return;
    }

    if (!normalizedToken) {
      setAuthError(t("auth.messages.tokenRequired"));
      return;
    }

    applyAuthSession({
      mode: "bearer",
      principal: "oidc-session",
      token: normalizedToken
    });
  }, [applyAuthSession, loginMode, loginPrincipal, loginToken, t]);

  const signOut = useCallback(async () => {
    if (runtimeAuthSession?.mode === "bearer" && runtimeAuthSession.token) {
      try {
        await apiFetch(`${API_BASE_URL}/api/v1/auth/logout`, { method: "POST" });
      } catch {
        // Best-effort session revoke; local sign-out still proceeds.
      }
    }

    applyAuthSession(null);
    setAuthIdentity(null);
    setAuthError(null);
    setAuthNotice(t("auth.messages.signedOut"));
    setError(null);
  }, [applyAuthSession, t]);

  useEffect(() => {
    const onSessionExpired = () => {
      applyAuthSession(null);
      setAuthIdentity(null);
      setAuthError(null);
      setAuthNotice(t("auth.messages.sessionExpired"));
      setError(null);
    };

    window.addEventListener(AUTH_SESSION_EXPIRED_EVENT, onSessionExpired);
    return () => window.removeEventListener(AUTH_SESSION_EXPIRED_EVENT, onSessionExpired);
  }, [applyAuthSession, t]);

  useEffect(() => {
    let cancelled = false;

    if (!authSession) {
      setAuthLoading(false);
      setAuthIdentity(null);
      return;
    }

    setAuthLoading(true);
    setAuthError(null);
    void loadCurrentIdentity()
      .then((identity) => {
        if (cancelled) {
          return;
        }
        setAuthIdentity(identity);
        if (authSession.mode === "header" && authSession.principal !== identity.user.username) {
          applyAuthSession({
            ...authSession,
            principal: identity.user.username
          });
        }
      })
      .catch((err) => {
        if (cancelled) {
          return;
        }
        applyAuthSession(null);
        setAuthIdentity(null);
        setAuthError(err instanceof Error ? err.message : t("cmdb.messages.error"));
      })
      .finally(() => {
        if (!cancelled) {
          setAuthLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [applyAuthSession, authSession, loadCurrentIdentity, t]);

  const loadAssets = useCallback(async () => {
    setLoadingAssets(true);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/assets?limit=50`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: AssetListResponse = await response.json();
      setAssets(payload.items);
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setLoadingAssets(false);
    }
  }, []);

  const loadFieldDefinitions = useCallback(async () => {
    setLoadingFields(true);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/field-definitions`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: FieldDefinition[] = await response.json();
      setFieldDefinitions(payload);
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setLoadingFields(false);
    }
  }, []);

  const loadRelations = useCallback(async (assetId: number) => {
    setLoadingRelations(true);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/relations?asset_id=${assetId}`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: AssetRelation[] = await response.json();
      setRelations(payload);
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setLoadingRelations(false);
    }
  }, []);

  const applyBindingsToForm = useCallback((payload: AssetBindingsResponse) => {
    setBindingDepartmentsInput(payload.departments.join(", "));
    setBindingBusinessServicesInput(payload.business_services.join(", "));
    setBindingOwnerDrafts(
      payload.owners.map((owner, index) =>
        createOwnerDraft(normalizeOwnerType(owner.owner_type), owner.owner_ref, `${payload.asset_id}-${index}`)
      )
    );
  }, []);

  const loadAssetBindings = useCallback(async (assetId: number) => {
    setLoadingAssetBindings(true);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/assets/${assetId}/bindings`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: AssetBindingsResponse = await response.json();
      setAssetBindings(payload);
      applyBindingsToForm(payload);
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setAssetBindings(null);
      setBindingOwnerDrafts([]);
    } finally {
      setLoadingAssetBindings(false);
    }
  }, [applyBindingsToForm]);

  const loadAssetMonitoring = useCallback(async (assetId: number) => {
    setLoadingAssetMonitoring(true);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/assets/${assetId}/monitoring-binding`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: AssetMonitoringBindingResponse = await response.json();
      setAssetMonitoring(payload);
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setAssetMonitoring(null);
    } finally {
      setLoadingAssetMonitoring(false);
    }
  }, []);

  const loadAssetImpact = useCallback(async (assetId: number, direction: ImpactDirection, depth: number) => {
    setLoadingAssetImpact(true);
    setError(null);
    try {
      const params = new URLSearchParams({
        direction,
        depth: String(depth)
      });
      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/assets/${assetId}/impact?${params.toString()}`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: AssetImpactResponse = await response.json();
      setAssetImpact(payload);
      return payload;
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
      setAssetImpact(null);
      return null;
    } finally {
      setLoadingAssetImpact(false);
    }
  }, []);

  const loadDiscoveryJobs = useCallback(async () => {
    setLoadingDiscoveryJobs(true);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/discovery/jobs`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: DiscoveryJob[] = await response.json();
      setDiscoveryJobs(payload);
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setLoadingDiscoveryJobs(false);
    }
  }, []);

  const loadDiscoveryCandidates = useCallback(async () => {
    setLoadingDiscoveryCandidates(true);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/discovery/candidates?review_status=pending&limit=100`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: { items: DiscoveryCandidate[] } = await response.json();
      setDiscoveryCandidates(payload.items);
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setLoadingDiscoveryCandidates(false);
    }
  }, []);

  const loadNotificationChannels = useCallback(async () => {
    setLoadingNotificationChannels(true);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/discovery/notification-channels`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: NotificationChannel[] = await response.json();
      setNotificationChannels(payload);
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setLoadingNotificationChannels(false);
    }
  }, []);

  const loadNotificationTemplates = useCallback(async () => {
    setLoadingNotificationTemplates(true);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/discovery/notification-templates`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: NotificationTemplate[] = await response.json();
      setNotificationTemplates(payload);
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setLoadingNotificationTemplates(false);
    }
  }, []);

  const loadNotificationSubscriptions = useCallback(async () => {
    setLoadingNotificationSubscriptions(true);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/discovery/notification-subscriptions`);
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: NotificationSubscription[] = await response.json();
      setNotificationSubscriptions(payload);
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setLoadingNotificationSubscriptions(false);
    }
  }, []);

  const createSampleAsset = useCallback(async () => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }

    setCreatingSample(true);
    setError(null);
    try {
      const customFields: Record<string, unknown> = {};
      for (const definition of fieldDefinitions) {
        if (definition.required && definition.is_enabled) {
          customFields[definition.field_key] = sampleValueForField(definition);
        }
      }

      const stamp = Date.now();
      const body = {
        asset_class: "server",
        name: `sample-${stamp}`,
        hostname: `sample-${stamp}.local`,
        ip: "10.0.0.10",
        status: "idle",
        site: "dc-a",
        department: "platform",
        owner: "ops",
        qr_code: `QR-${stamp}`,
        barcode: `BC-${stamp}`,
        custom_fields: customFields
      };

      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/assets`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify(body)
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      await loadAssets();
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setCreatingSample(false);
    }
  }, [canWriteCmdb, fieldDefinitions, loadAssets, t]);

  const createFieldDefinition = useCallback(async () => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }

    setCreatingField(true);
    setError(null);
    try {
      const payload: Record<string, unknown> = {
        field_key: newField.field_key,
        name: newField.name,
        field_type: newField.field_type,
        required: newField.required,
        scanner_enabled: newField.scanner_enabled,
        is_enabled: true
      };

      if (newField.max_length.trim() !== "") {
        payload.max_length = Number(newField.max_length.trim());
      }

      if (newField.field_type === "enum") {
        payload.options = newField.options_csv
          .split(",")
          .map((item) => item.trim())
          .filter((item) => item.length > 0);
      }

      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/field-definitions`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify(payload)
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }

      setNewField(defaultFieldForm);
      await loadFieldDefinitions();
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setCreatingField(false);
    }
  }, [canWriteCmdb, newField, loadFieldDefinitions, t]);

  const createRelation = useCallback(async () => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }

    const srcAssetId = Number.parseInt(selectedAssetId, 10);
    const dstAssetId = Number.parseInt(newRelation.dst_asset_id, 10);
    if (!Number.isFinite(srcAssetId) || srcAssetId <= 0) {
      setError(t("cmdb.relations.messages.selectSource"));
      return;
    }
    if (!Number.isFinite(dstAssetId) || dstAssetId <= 0) {
      setError(t("cmdb.relations.messages.selectTarget"));
      return;
    }
    if (srcAssetId === dstAssetId) {
      setError(t("cmdb.relations.messages.sameAsset"));
      return;
    }

    setCreatingRelation(true);
    setRelationNotice(null);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/relations`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          src_asset_id: srcAssetId,
          dst_asset_id: dstAssetId,
          relation_type: newRelation.relation_type,
          source: newRelation.source
        })
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }

      setNewRelation((prev) => ({ ...prev, dst_asset_id: "" }));
      await loadRelations(srcAssetId);
      setRelationNotice(t("cmdb.relations.messages.created"));
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setCreatingRelation(false);
    }
  }, [canWriteCmdb, loadRelations, newRelation.dst_asset_id, newRelation.relation_type, newRelation.source, selectedAssetId, t]);

  const deleteRelation = useCallback(
    async (relationId: number) => {
      if (!canWriteCmdb) {
        setError(t("auth.messages.forbiddenAction"));
        return;
      }

      const srcAssetId = Number.parseInt(selectedAssetId, 10);
      if (!Number.isFinite(srcAssetId) || srcAssetId <= 0) {
        return;
      }

      if (typeof window !== "undefined") {
        const shouldDelete = window.confirm(t("cmdb.relations.messages.deleteConfirm", { id: relationId }));
        if (!shouldDelete) {
          return;
        }
      }

      setDeletingRelationId(relationId);
      setRelationNotice(null);
      setError(null);
      try {
        const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/relations/${relationId}`, {
          method: "DELETE"
        });
        if (!response.ok) {
          throw new Error(await readErrorMessage(response));
        }
        await loadRelations(srcAssetId);
        setRelationNotice(t("cmdb.relations.messages.deleted"));
      } catch (err) {
        setError(err instanceof Error ? err.message : "unknown error");
      } finally {
        setDeletingRelationId(null);
      }
    },
    [canWriteCmdb, loadRelations, selectedAssetId, t]
  );

  const addOwnerDraft = useCallback(() => {
    setBindingOwnerDrafts((prev) => [...prev, createOwnerDraft("team", "")]);
  }, []);

  const updateOwnerDraftType = useCallback((key: string, ownerType: OwnerType) => {
    setBindingOwnerDrafts((prev) =>
      prev.map((item) => (item.key === key ? { ...item, owner_type: ownerType } : item))
    );
  }, []);

  const updateOwnerDraftRef = useCallback((key: string, ownerRef: string) => {
    setBindingOwnerDrafts((prev) =>
      prev.map((item) => (item.key === key ? { ...item, owner_ref: ownerRef } : item))
    );
  }, []);

  const removeOwnerDraft = useCallback((key: string) => {
    setBindingOwnerDrafts((prev) => prev.filter((item) => item.key !== key));
  }, []);

  const saveAssetBindings = useCallback(async () => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }

    const assetId = Number.parseInt(selectedAssetId, 10);
    if (!Number.isFinite(assetId) || assetId <= 0) {
      setError(t("cmdb.assetDetail.messages.selectAsset"));
      return;
    }

    setUpdatingAssetBindings(true);
    setBindingNotice(null);
    setError(null);
    try {
      const owners = bindingOwnerDrafts
        .map((item) => ({
          owner_type: item.owner_type,
          owner_ref: item.owner_ref.trim()
        }))
        .filter((item) => item.owner_ref.length > 0);

      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/assets/${assetId}/bindings`, {
        method: "PUT",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          departments: parseBindingList(bindingDepartmentsInput),
          business_services: parseBindingList(bindingBusinessServicesInput),
          owners
        })
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: AssetBindingsResponse = await response.json();
      setAssetBindings(payload);
      applyBindingsToForm(payload);
      setBindingNotice(t("cmdb.assetDetail.messages.bindingsSaved"));
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setUpdatingAssetBindings(false);
    }
  }, [
    applyBindingsToForm,
    bindingBusinessServicesInput,
    bindingDepartmentsInput,
    bindingOwnerDrafts,
    canWriteCmdb,
    selectedAssetId,
    t
  ]);

  const transitionAssetLifecycle = useCallback(async (status: LifecycleStatus) => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }

    const assetId = Number.parseInt(selectedAssetId, 10);
    if (!Number.isFinite(assetId) || assetId <= 0) {
      setError(t("cmdb.assetDetail.messages.selectAsset"));
      return;
    }

    setTransitioningLifecycleStatus(status);
    setLifecycleNotice(null);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/assets/${assetId}/lifecycle`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({ status })
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }

      const payload: AssetLifecycleTransitionResponse = await response.json();
      setAssets((prev) =>
        prev.map((item) =>
          item.id === payload.asset_id
            ? {
                ...item,
                status: payload.status
              }
            : item
        )
      );
      setAssetBindings((prev) =>
        prev
          ? {
              ...prev,
              readiness: payload.readiness
            }
          : prev
      );
      setLifecycleNotice(t("cmdb.assetDetail.messages.lifecycleChanged", { status: payload.status }));
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setTransitioningLifecycleStatus(null);
    }
  }, [canWriteCmdb, selectedAssetId, t]);

  const triggerAssetMonitoringSync = useCallback(async () => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }

    const assetId = Number.parseInt(selectedAssetId, 10);
    if (!Number.isFinite(assetId) || assetId <= 0) {
      setError(t("cmdb.assetDetail.messages.selectAsset"));
      return;
    }

    setTriggeringMonitoringSync(true);
    setMonitoringNotice(null);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/assets/${assetId}/monitoring-sync`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          reason: "manual sync from readiness panel"
        })
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: { job_id: number } = await response.json();
      await loadAssetMonitoring(assetId);
      setMonitoringNotice(t("cmdb.assetDetail.monitoring.messages.syncQueued", { id: payload.job_id }));
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setTriggeringMonitoringSync(false);
    }
  }, [canWriteCmdb, loadAssetMonitoring, selectedAssetId, t]);

  const refreshImpact = useCallback(async () => {
    const assetId = Number.parseInt(selectedAssetId, 10);
    if (!Number.isFinite(assetId) || assetId <= 0) {
      setError(t("cmdb.assetDetail.messages.selectAsset"));
      return;
    }

    const depth = parseImpactDepth(impactDepth);
    if (depth === null) {
      setError(t("cmdb.assetDetail.impact.messages.invalidDepth"));
      return;
    }

    setImpactNotice(null);
    const payload = await loadAssetImpact(assetId, impactDirection, depth);
    if (payload) {
      setImpactNotice(
        t("cmdb.assetDetail.impact.messages.loaded", {
          nodes: payload.nodes.length,
          edges: payload.edges.length
        })
      );
    }
  }, [impactDepth, impactDirection, loadAssetImpact, selectedAssetId, t]);

  const runDiscoveryJob = useCallback(async (jobId: number) => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }

    setRunningDiscoveryJobId(jobId);
    setDiscoveryNotice(null);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/discovery/jobs/${jobId}/run`, {
        method: "POST"
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      await Promise.all([loadDiscoveryJobs(), loadDiscoveryCandidates(), loadAssets()]);
      setDiscoveryNotice(t("cmdb.discovery.messages.jobRunTriggered", { id: jobId }));
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setRunningDiscoveryJobId(null);
    }
  }, [canWriteCmdb, loadAssets, loadDiscoveryCandidates, loadDiscoveryJobs, t]);

  const reviewDiscoveryCandidate = useCallback(
    async (candidateId: number, action: "approve" | "reject") => {
      if (!canWriteCmdb) {
        setError(t("auth.messages.forbiddenAction"));
        return;
      }

      setReviewingCandidateId(candidateId);
      setDiscoveryNotice(null);
      setError(null);
      try {
        const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/discovery/candidates/${candidateId}/${action}`, {
          method: "POST",
          headers: {
            "Content-Type": "application/json"
          },
          body: JSON.stringify({ reviewed_by: "web-console" })
        });
        if (!response.ok) {
          throw new Error(await readErrorMessage(response));
        }
        await Promise.all([loadDiscoveryCandidates(), loadAssets()]);
        setDiscoveryNotice(
          action === "approve"
            ? t("cmdb.discovery.messages.candidateApproved", { id: candidateId })
            : t("cmdb.discovery.messages.candidateRejected", { id: candidateId })
        );
      } catch (err) {
        setError(err instanceof Error ? err.message : "unknown error");
      } finally {
        setReviewingCandidateId(null);
      }
    },
    [canWriteCmdb, loadAssets, loadDiscoveryCandidates, t]
  );

  const createNotificationChannel = useCallback(async () => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }

    const name = newNotificationChannel.name.trim();
    const target = newNotificationChannel.target.trim();
    if (!name) {
      setError(t("cmdb.notifications.validation.channelNameRequired"));
      return;
    }
    if (!target) {
      setError(t("cmdb.notifications.validation.targetRequired"));
      return;
    }

    let config: Record<string, unknown> = {};
    const configRaw = newNotificationChannel.config_json.trim();
    if (configRaw.length > 0) {
      try {
        const parsed = JSON.parse(configRaw) as unknown;
        if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
          setError(t("cmdb.notifications.validation.configMustBeObject"));
          return;
        }
        config = parsed as Record<string, unknown>;
      } catch {
        setError(t("cmdb.notifications.validation.configInvalidJson"));
        return;
      }
    }

    setCreatingNotificationChannel(true);
    setNotificationNotice(null);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/discovery/notification-channels`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          name,
          channel_type: newNotificationChannel.channel_type,
          target,
          config
        })
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      setNewNotificationChannel(defaultNotificationChannelForm);
      await loadNotificationChannels();
      setNotificationNotice(t("cmdb.notifications.messages.channelCreated", { name }));
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setCreatingNotificationChannel(false);
    }
  }, [canWriteCmdb, loadNotificationChannels, newNotificationChannel, t]);

  const createNotificationTemplate = useCallback(async () => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }

    const eventType = newNotificationTemplate.event_type.trim();
    const titleTemplate = newNotificationTemplate.title_template.trim();
    const bodyTemplate = newNotificationTemplate.body_template.trim();
    if (!eventType) {
      setError(t("cmdb.notifications.validation.eventTypeRequired"));
      return;
    }
    if (!titleTemplate) {
      setError(t("cmdb.notifications.validation.templateTitleRequired"));
      return;
    }
    if (!bodyTemplate) {
      setError(t("cmdb.notifications.validation.templateBodyRequired"));
      return;
    }

    setCreatingNotificationTemplate(true);
    setNotificationNotice(null);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/discovery/notification-templates`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          event_type: eventType,
          title_template: titleTemplate,
          body_template: bodyTemplate
        })
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      setNewNotificationTemplate((prev) => ({ ...prev, title_template: "", body_template: "" }));
      await loadNotificationTemplates();
      setNotificationNotice(t("cmdb.notifications.messages.templateCreated", { eventType }));
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setCreatingNotificationTemplate(false);
    }
  }, [canWriteCmdb, loadNotificationTemplates, newNotificationTemplate, t]);

  const createNotificationSubscription = useCallback(async () => {
    if (!canWriteCmdb) {
      setError(t("auth.messages.forbiddenAction"));
      return;
    }

    const channelId = Number.parseInt(newNotificationSubscription.channel_id, 10);
    const eventType = newNotificationSubscription.event_type.trim();
    if (!Number.isFinite(channelId) || channelId <= 0) {
      setError(t("cmdb.notifications.validation.channelRequired"));
      return;
    }
    if (!eventType) {
      setError(t("cmdb.notifications.validation.eventTypeRequired"));
      return;
    }

    setCreatingNotificationSubscription(true);
    setNotificationNotice(null);
    setError(null);
    try {
      const response = await apiFetch(`${API_BASE_URL}/api/v1/cmdb/discovery/notification-subscriptions`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify({
          channel_id: channelId,
          event_type: eventType,
          site: trimToNull(newNotificationSubscription.site),
          department: trimToNull(newNotificationSubscription.department)
        })
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      setNewNotificationSubscription((prev) => ({ ...prev, site: "", department: "" }));
      await loadNotificationSubscriptions();
      setNotificationNotice(t("cmdb.notifications.messages.subscriptionCreated", { eventType }));
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setCreatingNotificationSubscription(false);
    }
  }, [canWriteCmdb, loadNotificationSubscriptions, newNotificationSubscription, t]);

  const findAssetByCode = useCallback(async () => {
    const normalized = scanCode.trim();
    if (!normalized) {
      setError(t("cmdb.scan.codeRequired"));
      return;
    }

    setScanning(true);
    setError(null);
    setScanResult(null);
    try {
      const response = await apiFetch(
        `${API_BASE_URL}/api/v1/cmdb/assets/by-code/${encodeURIComponent(normalized)}?mode=${scanMode}`
      );
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      const payload: Asset = await response.json();
      setScanResult(payload);
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setScanning(false);
    }
  }, [scanCode, scanMode, t]);

  useEffect(() => {
    if (!authIdentity) {
      return;
    }
    void Promise.all([
      loadAssets(),
      loadFieldDefinitions(),
      loadDiscoveryJobs(),
      loadDiscoveryCandidates(),
      loadNotificationChannels(),
      loadNotificationTemplates(),
      loadNotificationSubscriptions()
    ]);
  }, [
    loadAssets,
    loadFieldDefinitions,
    loadDiscoveryCandidates,
    loadDiscoveryJobs,
    loadNotificationChannels,
    loadNotificationTemplates,
    loadNotificationSubscriptions,
    authIdentity
  ]);

  useEffect(() => {
    if (assets.length === 0 || selectedAssetId) {
      return;
    }
    const firstAssetId = String(assets[0].id);
    setSelectedAssetId(firstAssetId);
  }, [assets, selectedAssetId]);

  useEffect(() => {
    setRelationNotice(null);
    setBindingNotice(null);
    setLifecycleNotice(null);
    setMonitoringNotice(null);
    setImpactNotice(null);
  }, [selectedAssetId]);

  useEffect(() => {
    const assetId = Number.parseInt(selectedAssetId, 10);
    if (!Number.isFinite(assetId) || assetId <= 0) {
      setRelations([]);
      return;
    }
    void loadRelations(assetId);
  }, [loadRelations, selectedAssetId]);

  useEffect(() => {
    const assetId = Number.parseInt(selectedAssetId, 10);
    if (!Number.isFinite(assetId) || assetId <= 0) {
      setAssetBindings(null);
      setAssetMonitoring(null);
      setAssetImpact(null);
      setBindingOwnerDrafts([]);
      return;
    }

    const depth = parseImpactDepth(impactDepth) ?? 4;
    void Promise.all([
      loadAssetBindings(assetId),
      loadAssetMonitoring(assetId),
      loadAssetImpact(assetId, impactDirection, depth)
    ]);
  }, [loadAssetBindings, loadAssetImpact, loadAssetMonitoring, selectedAssetId]);

  const emptyState = useMemo(() => assets.length === 0, [assets]);
  const assetNameById = useMemo(() => {
    const map = new Map<number, string>();
    for (const asset of assets) {
      map.set(asset.id, asset.name);
    }
    return map;
  }, [assets]);
  const notificationChannelNameById = useMemo(() => {
    const map = new Map<number, string>();
    for (const channel of notificationChannels) {
      map.set(channel.id, channel.name);
    }
    return map;
  }, [notificationChannels]);
  const notificationChannelById = useMemo(() => {
    const map = new Map<number, NotificationChannel>();
    for (const channel of notificationChannels) {
      map.set(channel.id, channel);
    }
    return map;
  }, [notificationChannels]);
  const selectedAssetNumericId = useMemo(() => Number.parseInt(selectedAssetId, 10), [selectedAssetId]);
  const selectedAsset = useMemo(
    () => assets.find((item) => item.id === selectedAssetNumericId) ?? null,
    [assets, selectedAssetNumericId]
  );
  const relationSummary = useMemo(() => {
    if (!Number.isFinite(selectedAssetNumericId) || selectedAssetNumericId <= 0) {
      return { upstream: 0, downstream: 0 };
    }
    const upstream = relations.filter((item) => item.dst_asset_id === selectedAssetNumericId).length;
    const downstream = relations.filter((item) => item.src_asset_id === selectedAssetNumericId).length;
    return { upstream, downstream };
  }, [relations, selectedAssetNumericId]);
  const impactNodeNameById = useMemo(() => {
    const map = new Map<number, string>();
    for (const node of assetImpact?.nodes ?? []) {
      map.set(node.id, node.name);
    }
    for (const asset of assets) {
      if (!map.has(asset.id)) {
        map.set(asset.id, asset.name);
      }
    }
    return map;
  }, [assetImpact?.nodes, assets]);
  const hierarchyHintEdges = useMemo(
    () => (assetImpact?.edges ?? []).filter((edge) => edge.relation_type === "contains").slice(0, 10),
    [assetImpact?.edges]
  );
  const assetStatusOptions = useMemo(
    () => Array.from(new Set(assets.map((item) => item.status).filter((item) => item.trim().length > 0))).sort(),
    [assets]
  );
  const assetClassOptions = useMemo(
    () => Array.from(new Set(assets.map((item) => item.asset_class).filter((item) => item.trim().length > 0))).sort(),
    [assets]
  );
  const assetSiteOptions = useMemo(
    () => Array.from(new Set(assets.map((item) => item.site ?? "").filter((item) => item.trim().length > 0))).sort(),
    [assets]
  );
  const filteredAssets = useMemo(() => {
    const normalizedQuery = assetSearch.trim().toLowerCase();
    const filtered = assets.filter((asset) => {
      if (assetStatusFilter && asset.status !== assetStatusFilter) {
        return false;
      }
      if (assetClassFilter && asset.asset_class !== assetClassFilter) {
        return false;
      }
      if (assetSiteFilter && (asset.site ?? "") !== assetSiteFilter) {
        return false;
      }
      if (!normalizedQuery) {
        return true;
      }

      const haystack = [
        asset.name,
        asset.hostname ?? "",
        asset.ip ?? "",
        asset.owner ?? "",
        asset.site ?? "",
        asset.department ?? "",
        asset.asset_class,
        String(asset.id)
      ]
        .join(" ")
        .toLowerCase();
      return haystack.includes(normalizedQuery);
    });

    filtered.sort((left, right) => {
      if (assetSortMode === "name_asc") {
        return left.name.localeCompare(right.name);
      }
      if (assetSortMode === "id_asc") {
        return left.id - right.id;
      }
      return new Date(right.updated_at).getTime() - new Date(left.updated_at).getTime();
    });

    return filtered;
  }, [assetClassFilter, assetSearch, assetSiteFilter, assetSortMode, assetStatusFilter, assets]);
  const hasAssetFilter = useMemo(
    () =>
      assetSearch.trim().length > 0
      || assetStatusFilter.length > 0
      || assetClassFilter.length > 0
      || assetSiteFilter.length > 0
      || assetSortMode !== "updated_desc",
    [assetClassFilter, assetSearch, assetSiteFilter, assetSortMode, assetStatusFilter]
  );
  const resetAssetFilters = useCallback(() => {
    setAssetSearch("");
    setAssetStatusFilter("");
    setAssetClassFilter("");
    setAssetSiteFilter("");
    setAssetSortMode("updated_desc");
  }, []);
  const navigationItems = useMemo(
    () => [
      { href: "#section-scan", label: t("auth.navigation.scan") },
      { href: "#section-discovery", label: t("auth.navigation.discovery") },
      { href: "#section-notifications", label: t("auth.navigation.notifications") },
      { href: "#section-fields", label: t("auth.navigation.fields") },
      { href: "#section-relations", label: t("auth.navigation.relations") },
      { href: "#section-readiness", label: t("auth.navigation.readiness") },
      { href: "#section-assets", label: t("auth.navigation.assets") },
      ...(canAccessAdmin ? [{ href: "#section-admin", label: t("auth.navigation.admin") }] : [])
    ],
    [canAccessAdmin, t]
  );

  if (!authSession || !authIdentity) {
    return (
      <AuthGate
        title={t("app.title")}
        subtitle={t("app.subtitle")}
        notice={authNotice}
        error={authError}
      >
        <h2>{t("auth.title")}</h2>
        <p>{t("auth.subtitle")}</p>

        <div className="auth-form-row">
          <label>
            {t("auth.modeLabel")}{" "}
            <select value={loginMode} onChange={(event) => setLoginMode(event.target.value as AuthMode)}>
              <option value="header">{t("auth.modes.header")}</option>
              <option value="bearer">{t("auth.modes.bearer")}</option>
            </select>
          </label>

          {loginMode === "header" ? (
            <label>
              {t("auth.usernameLabel")}{" "}
              <input
                value={loginPrincipal}
                onChange={(event) => setLoginPrincipal(event.target.value)}
                placeholder={t("auth.usernamePlaceholder")}
              />
            </label>
          ) : (
            <label style={{ flex: "1 1 420px" }}>
              {t("auth.tokenLabel")}{" "}
              <input
                value={loginToken}
                onChange={(event) => setLoginToken(event.target.value)}
                placeholder={t("auth.tokenPlaceholder")}
                className="auth-token-input"
              />
            </label>
          )}

          <button onClick={() => void signIn()} disabled={authLoading}>
            {authLoading ? t("auth.signingIn") : t("auth.signIn")}
          </button>
        </div>
      </AuthGate>
    );
  }

  return (
    <AppShell
      title={t("app.title")}
      subtitle={t("app.subtitle")}
      statusText={t("auth.status", { username: authIdentity.user.username, roles: roleText })}
      modeText={t("auth.statusMode", { mode: authSession.mode })}
      signOutLabel={t("auth.signOut")}
      onSignOut={() => void signOut()}
      navigationItems={navigationItems}
      notice={authNotice}
      error={error ? `${t("cmdb.messages.error")}: ${error}` : null}
      warning={!canWriteCmdb ? t("auth.messages.readOnly") : null}
    >

      {canAccessAdmin && (
        <SectionCard id="section-admin" title={t("auth.adminPanel.title")}>
          <p style={{ marginTop: 0 }}>{t("auth.adminPanel.description")}</p>
        </SectionCard>
      )}

      <SectionCard>
        <div className="toolbar-row">
          <button onClick={() => void loadAssets()} disabled={loadingAssets}>
            {loadingAssets ? t("cmdb.actions.loading") : t("cmdb.actions.refreshAssets")}
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
                {discoveryJobs.map((job) => (
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
                {discoveryCandidates.map((candidate) => (
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
                setNewNotificationChannel((prev) => ({
                  ...prev,
                  name: event.target.value
                }))
              }
              placeholder={t("cmdb.notifications.form.channelName")}
            />
            <select
              value={newNotificationChannel.channel_type}
              onChange={(event) =>
                setNewNotificationChannel((prev) => ({
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
                setNewNotificationChannel((prev) => ({
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
                setNewNotificationChannel((prev) => ({
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
                {notificationChannels.map((channel) => (
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
                setNewNotificationTemplate((prev) => ({
                  ...prev,
                  event_type: event.target.value
                }))
              }
              placeholder={t("cmdb.notifications.form.eventType")}
            />
            <input
              value={newNotificationTemplate.title_template}
              onChange={(event) =>
                setNewNotificationTemplate((prev) => ({
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
                setNewNotificationTemplate((prev) => ({
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
                {notificationTemplates.map((template) => (
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
                setNewNotificationSubscription((prev) => ({
                  ...prev,
                  channel_id: event.target.value
                }))
              }
            >
              <option value="">{t("cmdb.notifications.form.selectChannel")}</option>
              {notificationChannels.map((channel) => (
                <option key={channel.id} value={channel.id}>
                  #{channel.id} {channel.name} ({channel.channel_type})
                </option>
              ))}
            </select>
            <input
              value={newNotificationSubscription.event_type}
              onChange={(event) =>
                setNewNotificationSubscription((prev) => ({
                  ...prev,
                  event_type: event.target.value
                }))
              }
              placeholder={t("cmdb.notifications.form.eventType")}
            />
            <input
              value={newNotificationSubscription.site}
              onChange={(event) =>
                setNewNotificationSubscription((prev) => ({
                  ...prev,
                  site: event.target.value
                }))
              }
              placeholder={t("cmdb.notifications.form.siteOptional")}
            />
            <input
              value={newNotificationSubscription.department}
              onChange={(event) =>
                setNewNotificationSubscription((prev) => ({
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
                {notificationSubscriptions.map((subscription) => {
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

      <SectionCard id="section-fields" title={t("cmdb.fields.title")}>
        {canWriteCmdb ? (
          <div style={{ display: "flex", gap: "0.5rem", flexWrap: "wrap", marginBottom: "0.75rem" }}>
            <input
              value={newField.field_key}
              onChange={(event) => setNewField((prev) => ({ ...prev, field_key: event.target.value }))}
              placeholder={t("cmdb.fields.form.fieldKey")}
            />
            <input
              value={newField.name}
              onChange={(event) => setNewField((prev) => ({ ...prev, name: event.target.value }))}
              placeholder={t("cmdb.fields.form.name")}
            />
            <select
              value={newField.field_type}
              onChange={(event) => setNewField((prev) => ({ ...prev, field_type: event.target.value }))}
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
              onChange={(event) => setNewField((prev) => ({ ...prev, max_length: event.target.value }))}
              placeholder={t("cmdb.fields.form.maxLength")}
              style={{ width: "140px" }}
            />
            {newField.field_type === "enum" && (
              <input
                value={newField.options_csv}
                onChange={(event) => setNewField((prev) => ({ ...prev, options_csv: event.target.value }))}
                placeholder={t("cmdb.fields.form.enumOptions")}
                style={{ minWidth: "250px" }}
              />
            )}
            <label>
              <input
                type="checkbox"
                checked={newField.required}
                onChange={(event) => setNewField((prev) => ({ ...prev, required: event.target.checked }))}
              />{" "}
              {t("cmdb.fields.form.required")}
            </label>
            <label>
              <input
                type="checkbox"
                checked={newField.scanner_enabled}
                onChange={(event) => setNewField((prev) => ({ ...prev, scanner_enabled: event.target.checked }))}
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
                {fieldDefinitions.map((item) => (
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
                {assets.map((asset) => (
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
                  onChange={(event) => setNewRelation((prev) => ({ ...prev, dst_asset_id: event.target.value }))}
                >
                  <option value="">{t("cmdb.relations.form.selectTarget")}</option>
                  {assets.map((asset) => (
                    <option key={asset.id} value={asset.id}>
                      #{asset.id} {asset.name}
                    </option>
                  ))}
                </select>

                <input
                  value={newRelation.relation_type}
                  onChange={(event) => setNewRelation((prev) => ({ ...prev, relation_type: event.target.value }))}
                  placeholder={t("cmdb.relations.form.relationType")}
                />

                <select
                  value={newRelation.source}
                  onChange={(event) => setNewRelation((prev) => ({ ...prev, source: event.target.value }))}
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
                    {relations.map((relation) => (
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
                  void Promise.all([
                    loadAssetBindings(assetId),
                    loadAssetMonitoring(assetId),
                    loadAssetImpact(assetId, impactDirection, depth)
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
                            .map((item) => t(`cmdb.assetDetail.readiness.missing.${item}`))
                            .join(", ")
                        })}
                      </p>
                    )}

                    <div className="toolbar-row">
                      {lifecycleStatuses.map((status) => (
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
                    {bindingOwnerDrafts.map((owner) => (
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
                        {hierarchyHintEdges.map((edge) => (
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
                        value: assetImpact.affected_business_services.map((item) => item.name).join(", ") || "-"
                      })}
                    </p>
                    <p className="section-note">
                      {t("cmdb.assetDetail.impact.affectedOwners", {
                        value: assetImpact.affected_owners.map((item) => item.name).join(", ") || "-"
                      })}
                    </p>
                  </>
                )}
              </div>
            </div>
          </>
        )}
      </SectionCard>

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
              {assetStatusOptions.map((status) => (
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
              {assetClassOptions.map((assetClass) => (
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
              {assetSiteOptions.map((site) => (
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
                {filteredAssets.map((asset) => (
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
    </AppShell>
  );
}

async function readErrorMessage(response: Response): Promise<string> {
  const message = await extractApiErrorMessage(response.clone());
  if (response.status === 403) {
    if (message && isSessionExpiredError(message)) {
      return "Session is invalid or expired. Please sign in again.";
    }
    if (message) {
      return `Unauthorized: ${message}`;
    }
    return "Unauthorized: your current role cannot perform this action.";
  }

  return message ?? `HTTP ${response.status}`;
}

async function apiFetch(input: string, init?: RequestInit): Promise<Response> {
  const headers = new Headers(init?.headers ?? undefined);
  if (runtimeAuthSession?.mode === "header") {
    const principal = runtimeAuthSession.principal.trim();
    if (principal.length > 0) {
      headers.set("x-auth-user", principal);
    }
  }
  if (runtimeAuthSession?.mode === "bearer") {
    const token = runtimeAuthSession.token?.trim() ?? "";
    if (token.length > 0) {
      headers.set("Authorization", `Bearer ${token}`);
    }
  }

  const response = await fetch(input, {
    ...init,
    headers
  });

  if (response.status === 403 && runtimeAuthSession?.mode === "bearer") {
    const message = await extractApiErrorMessage(response.clone());
    if (message && isSessionExpiredError(message)) {
      runtimeAuthSession = null;
      persistAuthSession(null);
      if (typeof window !== "undefined") {
        window.dispatchEvent(new Event(AUTH_SESSION_EXPIRED_EVENT));
      }
    }
  }

  return response;
}

function trimToNull(value: string): string | null {
  const normalized = value.trim();
  return normalized.length > 0 ? normalized : null;
}

function sampleValueForField(definition: FieldDefinition): unknown {
  switch (definition.field_type) {
    case "text":
      return "sample";
    case "integer":
      return 1;
    case "float":
      return 1.5;
    case "boolean":
      return true;
    case "enum":
      return definition.options?.[0] ?? "sample";
    case "date":
      return new Date().toISOString().slice(0, 10);
    case "datetime":
      return new Date().toISOString();
    default:
      return "sample";
  }
}

function readPayloadString(payload: Record<string, unknown>, key: string): string | null {
  const value = payload[key];
  if (typeof value === "string" && value.trim().length > 0) {
    return value;
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  return null;
}

function renderCustomFields(value: Record<string, unknown>): string {
  const entries = Object.entries(value);
  if (entries.length === 0) {
    return "-";
  }

  const preview = entries
    .slice(0, 3)
    .map(([key, fieldValue]) => `${key}:${String(fieldValue)}`)
    .join("; ");

  if (entries.length <= 3) {
    return preview;
  }
  return `${preview} ...`;
}

function statusChipClass(value: string): string {
  const normalized = value.trim().toLowerCase();
  if (normalized.includes("active") || normalized.includes("enabled") || normalized.includes("success") || normalized === "ok") {
    return "status-chip status-chip-success";
  }
  if (
    normalized.includes("fail")
    || normalized.includes("error")
    || normalized.includes("disabled")
    || normalized.includes("reject")
  ) {
    return "status-chip status-chip-danger";
  }
  if (normalized.includes("pending") || normalized.includes("review") || normalized.includes("running")) {
    return "status-chip status-chip-warn";
  }
  return "status-chip";
}

let ownerDraftSequence = 0;

function createOwnerDraft(ownerType: OwnerType, ownerRef: string, keyHint?: string): OwnerDraft {
  ownerDraftSequence += 1;
  return {
    key: keyHint ? `${keyHint}-${ownerDraftSequence}` : `owner-${ownerDraftSequence}`,
    owner_type: ownerType,
    owner_ref: ownerRef
  };
}

function normalizeOwnerType(value: string): OwnerType {
  if (value === "team" || value === "user" || value === "group" || value === "external") {
    return value;
  }
  return "team";
}

function parseBindingList(value: string): string[] {
  const parts = value
    .split(/[\n,]/)
    .map((item) => item.trim())
    .filter((item) => item.length > 0);
  const seen = new Set<string>();
  const normalized: string[] = [];
  for (const item of parts) {
    const key = item.toLowerCase();
    if (!seen.has(key)) {
      seen.add(key);
      normalized.push(item);
    }
  }
  return normalized;
}

function parseImpactDepth(value: string): number | null {
  const parsed = Number.parseInt(value.trim(), 10);
  if (!Number.isFinite(parsed) || parsed < 1 || parsed > 8) {
    return null;
  }
  return parsed;
}

function deriveDefaultAuthSession(): AuthSession | null {
  if (API_AUTH_TOKEN.length > 0) {
    return {
      mode: "bearer",
      principal: "oidc-session",
      token: API_AUTH_TOKEN
    };
  }

  if (API_AUTH_USER.length > 0) {
    return {
      mode: "header",
      principal: API_AUTH_USER,
      token: null
    };
  }

  return null;
}

function loadStoredAuthSession(): AuthSession | null {
  if (typeof window === "undefined") {
    return null;
  }

  const raw = window.localStorage.getItem(AUTH_SESSION_STORAGE_KEY);
  if (!raw) {
    return null;
  }

  try {
    const parsed = JSON.parse(raw) as unknown;
    if (!parsed || typeof parsed !== "object") {
      return null;
    }

    const mode = (parsed as { mode?: unknown }).mode;
    const principal = (parsed as { principal?: unknown }).principal;
    const token = (parsed as { token?: unknown }).token;

    if (mode === "header" && typeof principal === "string" && principal.trim().length > 0) {
      return {
        mode: "header",
        principal: principal.trim(),
        token: null
      };
    }

    if (mode === "bearer" && typeof token === "string" && token.trim().length > 0) {
      return {
        mode: "bearer",
        principal: typeof principal === "string" ? principal : "oidc-session",
        token: token.trim()
      };
    }
  } catch {
    return null;
  }

  return null;
}

function persistAuthSession(session: AuthSession | null): void {
  if (typeof window === "undefined") {
    return;
  }

  if (!session) {
    window.localStorage.removeItem(AUTH_SESSION_STORAGE_KEY);
    return;
  }

  window.localStorage.setItem(AUTH_SESSION_STORAGE_KEY, JSON.stringify(session));
}

function isSessionExpiredError(message: string): boolean {
  const normalized = message.toLowerCase();
  return (
    normalized.includes("invalid or expired")
    || normalized.includes("bearer token cannot be empty")
    || normalized.includes("authorization header is invalid")
  );
}

async function extractApiErrorMessage(response: Response): Promise<string | null> {
  try {
    const payload = (await response.json()) as unknown;
    if (payload && typeof payload === "object" && "error" in payload) {
      const value = (payload as { error?: unknown }).error;
      if (typeof value === "string" && value.trim().length > 0) {
        return value.trim();
      }
    }
  } catch {
    return null;
  }

  return null;
}

const subSectionTitleStyle: CSSProperties = {
  marginTop: 0,
  marginBottom: "0.5rem",
  fontSize: "1rem"
};

const cellStyle: CSSProperties = {
  border: "1px solid #ddd",
  padding: "0.5rem",
  textAlign: "left",
  whiteSpace: "nowrap",
  verticalAlign: "top"
};
