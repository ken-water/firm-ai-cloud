import { useCallback, useEffect, useMemo, useState } from "react";
import type { CSSProperties } from "react";
import { useTranslation } from "react-i18next";

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

const API_BASE_URL = import.meta.env.VITE_API_BASE_URL ?? "http://127.0.0.1:8080";

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

export function App() {
  const { t } = useTranslation();
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
  const [creatingRelation, setCreatingRelation] = useState(false);
  const [deletingRelationId, setDeletingRelationId] = useState<number | null>(null);
  const [newRelation, setNewRelation] = useState<NewRelationForm>(defaultRelationForm);
  const [discoveryJobs, setDiscoveryJobs] = useState<DiscoveryJob[]>([]);
  const [discoveryCandidates, setDiscoveryCandidates] = useState<DiscoveryCandidate[]>([]);
  const [loadingDiscoveryJobs, setLoadingDiscoveryJobs] = useState(false);
  const [loadingDiscoveryCandidates, setLoadingDiscoveryCandidates] = useState(false);
  const [runningDiscoveryJobId, setRunningDiscoveryJobId] = useState<number | null>(null);
  const [reviewingCandidateId, setReviewingCandidateId] = useState<number | null>(null);
  const [notificationChannels, setNotificationChannels] = useState<NotificationChannel[]>([]);
  const [notificationTemplates, setNotificationTemplates] = useState<NotificationTemplate[]>([]);
  const [notificationSubscriptions, setNotificationSubscriptions] = useState<NotificationSubscription[]>([]);
  const [loadingNotificationChannels, setLoadingNotificationChannels] = useState(false);
  const [loadingNotificationTemplates, setLoadingNotificationTemplates] = useState(false);
  const [loadingNotificationSubscriptions, setLoadingNotificationSubscriptions] = useState(false);
  const [creatingNotificationChannel, setCreatingNotificationChannel] = useState(false);
  const [creatingNotificationTemplate, setCreatingNotificationTemplate] = useState(false);
  const [creatingNotificationSubscription, setCreatingNotificationSubscription] = useState(false);
  const [newNotificationChannel, setNewNotificationChannel] =
    useState<NewNotificationChannelForm>(defaultNotificationChannelForm);
  const [newNotificationTemplate, setNewNotificationTemplate] =
    useState<NewNotificationTemplateForm>(defaultNotificationTemplateForm);
  const [newNotificationSubscription, setNewNotificationSubscription] =
    useState<NewNotificationSubscriptionForm>(defaultNotificationSubscriptionForm);

  const loadAssets = useCallback(async () => {
    setLoadingAssets(true);
    setError(null);
    try {
      const response = await fetch(`${API_BASE_URL}/api/v1/cmdb/assets?limit=50`);
      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
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
      const response = await fetch(`${API_BASE_URL}/api/v1/cmdb/field-definitions`);
      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
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
      const response = await fetch(`${API_BASE_URL}/api/v1/cmdb/relations?asset_id=${assetId}`);
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

  const loadDiscoveryJobs = useCallback(async () => {
    setLoadingDiscoveryJobs(true);
    setError(null);
    try {
      const response = await fetch(`${API_BASE_URL}/api/v1/cmdb/discovery/jobs`);
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
      const response = await fetch(`${API_BASE_URL}/api/v1/cmdb/discovery/candidates?review_status=pending&limit=100`);
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
      const response = await fetch(`${API_BASE_URL}/api/v1/cmdb/discovery/notification-channels`);
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
      const response = await fetch(`${API_BASE_URL}/api/v1/cmdb/discovery/notification-templates`);
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
      const response = await fetch(`${API_BASE_URL}/api/v1/cmdb/discovery/notification-subscriptions`);
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
        status: "active",
        site: "dc-a",
        department: "platform",
        owner: "ops",
        qr_code: `QR-${stamp}`,
        barcode: `BC-${stamp}`,
        custom_fields: customFields
      };

      const response = await fetch(`${API_BASE_URL}/api/v1/cmdb/assets`, {
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
  }, [fieldDefinitions, loadAssets]);

  const createFieldDefinition = useCallback(async () => {
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

      const response = await fetch(`${API_BASE_URL}/api/v1/cmdb/field-definitions`, {
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
  }, [newField, loadFieldDefinitions]);

  const createRelation = useCallback(async () => {
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

    setCreatingRelation(true);
    setError(null);
    try {
      const response = await fetch(`${API_BASE_URL}/api/v1/cmdb/relations`, {
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
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setCreatingRelation(false);
    }
  }, [loadRelations, newRelation.dst_asset_id, newRelation.relation_type, newRelation.source, selectedAssetId, t]);

  const deleteRelation = useCallback(
    async (relationId: number) => {
      const srcAssetId = Number.parseInt(selectedAssetId, 10);
      if (!Number.isFinite(srcAssetId) || srcAssetId <= 0) {
        return;
      }

      setDeletingRelationId(relationId);
      setError(null);
      try {
        const response = await fetch(`${API_BASE_URL}/api/v1/cmdb/relations/${relationId}`, {
          method: "DELETE"
        });
        if (!response.ok) {
          throw new Error(await readErrorMessage(response));
        }
        await loadRelations(srcAssetId);
      } catch (err) {
        setError(err instanceof Error ? err.message : "unknown error");
      } finally {
        setDeletingRelationId(null);
      }
    },
    [loadRelations, selectedAssetId]
  );

  const runDiscoveryJob = useCallback(async (jobId: number) => {
    setRunningDiscoveryJobId(jobId);
    setError(null);
    try {
      const response = await fetch(`${API_BASE_URL}/api/v1/cmdb/discovery/jobs/${jobId}/run`, {
        method: "POST"
      });
      if (!response.ok) {
        throw new Error(await readErrorMessage(response));
      }
      await Promise.all([loadDiscoveryJobs(), loadDiscoveryCandidates(), loadAssets()]);
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setRunningDiscoveryJobId(null);
    }
  }, [loadAssets, loadDiscoveryCandidates, loadDiscoveryJobs]);

  const reviewDiscoveryCandidate = useCallback(
    async (candidateId: number, action: "approve" | "reject") => {
      setReviewingCandidateId(candidateId);
      setError(null);
      try {
        const response = await fetch(`${API_BASE_URL}/api/v1/cmdb/discovery/candidates/${candidateId}/${action}`, {
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
      } catch (err) {
        setError(err instanceof Error ? err.message : "unknown error");
      } finally {
        setReviewingCandidateId(null);
      }
    },
    [loadAssets, loadDiscoveryCandidates]
  );

  const createNotificationChannel = useCallback(async () => {
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
    setError(null);
    try {
      const response = await fetch(`${API_BASE_URL}/api/v1/cmdb/discovery/notification-channels`, {
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
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setCreatingNotificationChannel(false);
    }
  }, [loadNotificationChannels, newNotificationChannel, t]);

  const createNotificationTemplate = useCallback(async () => {
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
    setError(null);
    try {
      const response = await fetch(`${API_BASE_URL}/api/v1/cmdb/discovery/notification-templates`, {
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
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setCreatingNotificationTemplate(false);
    }
  }, [loadNotificationTemplates, newNotificationTemplate, t]);

  const createNotificationSubscription = useCallback(async () => {
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
    setError(null);
    try {
      const response = await fetch(`${API_BASE_URL}/api/v1/cmdb/discovery/notification-subscriptions`, {
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
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setCreatingNotificationSubscription(false);
    }
  }, [loadNotificationSubscriptions, newNotificationSubscription, t]);

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
      const response = await fetch(
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
    loadNotificationSubscriptions
  ]);

  useEffect(() => {
    if (assets.length === 0 || selectedAssetId) {
      return;
    }
    const firstAssetId = String(assets[0].id);
    setSelectedAssetId(firstAssetId);
  }, [assets, selectedAssetId]);

  useEffect(() => {
    const assetId = Number.parseInt(selectedAssetId, 10);
    if (!Number.isFinite(assetId) || assetId <= 0) {
      setRelations([]);
      return;
    }
    void loadRelations(assetId);
  }, [loadRelations, selectedAssetId]);

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
  const relationSummary = useMemo(() => {
    if (!Number.isFinite(selectedAssetNumericId) || selectedAssetNumericId <= 0) {
      return { upstream: 0, downstream: 0 };
    }
    const upstream = relations.filter((item) => item.dst_asset_id === selectedAssetNumericId).length;
    const downstream = relations.filter((item) => item.src_asset_id === selectedAssetNumericId).length;
    return { upstream, downstream };
  }, [relations, selectedAssetNumericId]);

  return (
    <main style={{ fontFamily: "sans-serif", padding: "2rem", lineHeight: 1.5 }}>
      <header style={{ marginBottom: "1rem" }}>
        <h1 style={{ marginBottom: "0.25rem" }}>{t("app.title")}</h1>
        <p style={{ marginTop: 0 }}>{t("app.subtitle")}</p>
      </header>

      <section style={{ marginBottom: "1rem", display: "flex", gap: "0.5rem", flexWrap: "wrap" }}>
        <button onClick={() => void loadAssets()} disabled={loadingAssets}>
          {loadingAssets ? t("cmdb.actions.loading") : t("cmdb.actions.refreshAssets")}
        </button>
        <button onClick={() => void loadFieldDefinitions()} disabled={loadingFields}>
          {loadingFields ? t("cmdb.actions.loading") : t("cmdb.actions.refreshFields")}
        </button>
        <button onClick={() => void createSampleAsset()} disabled={creatingSample}>
          {creatingSample ? t("cmdb.actions.creating") : t("cmdb.actions.createSample")}
        </button>
      </section>

      {error && (
        <p style={{ color: "#b00020", marginTop: 0 }}>
          {t("cmdb.messages.error")}: {error}
        </p>
      )}

      <section style={{ marginBottom: "1.5rem" }}>
        <h2 style={sectionTitleStyle}>{t("cmdb.scan.title")}</h2>
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
      </section>

      <section style={{ marginBottom: "1.5rem" }}>
        <h2 style={sectionTitleStyle}>{t("cmdb.discovery.title")}</h2>
        <div style={{ display: "flex", gap: "0.5rem", flexWrap: "wrap", marginBottom: "0.75rem" }}>
          <button onClick={() => void loadDiscoveryJobs()} disabled={loadingDiscoveryJobs}>
            {loadingDiscoveryJobs ? t("cmdb.actions.loading") : t("cmdb.discovery.actions.refreshJobs")}
          </button>
          <button onClick={() => void loadDiscoveryCandidates()} disabled={loadingDiscoveryCandidates}>
            {loadingDiscoveryCandidates ? t("cmdb.actions.loading") : t("cmdb.discovery.actions.refreshCandidates")}
          </button>
        </div>

        <h3 style={subSectionTitleStyle}>{t("cmdb.discovery.jobsTitle")}</h3>
        {discoveryJobs.length === 0 ? (
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
                    <td style={cellStyle}>{job.status}</td>
                    <td style={cellStyle}>{job.last_run_status ?? "-"}</td>
                    <td style={cellStyle}>{job.last_run_at ? new Date(job.last_run_at).toLocaleString() : "-"}</td>
                    <td style={cellStyle}>
                      <button onClick={() => void runDiscoveryJob(job.id)} disabled={runningDiscoveryJobId === job.id}>
                        {runningDiscoveryJobId === job.id ? t("cmdb.actions.loading") : t("cmdb.discovery.actions.run")}
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}

        <h3 style={subSectionTitleStyle}>{t("cmdb.discovery.candidatesTitle")}</h3>
        {discoveryCandidates.length === 0 ? (
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
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>

      <section style={{ marginBottom: "1.5rem" }}>
        <h2 style={sectionTitleStyle}>{t("cmdb.notifications.title")}</h2>
        <div style={{ display: "flex", gap: "0.5rem", flexWrap: "wrap", marginBottom: "0.75rem" }}>
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

        <h3 style={subSectionTitleStyle}>{t("cmdb.notifications.channelsTitle")}</h3>
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
        {notificationChannels.length === 0 ? (
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
        {notificationTemplates.length === 0 ? (
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
        {notificationSubscriptions.length === 0 ? (
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
      </section>

      <section style={{ marginBottom: "1.5rem" }}>
        <h2 style={sectionTitleStyle}>{t("cmdb.fields.title")}</h2>
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
      </section>

      <section style={{ marginBottom: "1.5rem" }}>
        <h2 style={sectionTitleStyle}>{t("cmdb.relations.title")}</h2>
        {emptyState ? (
          <p>{t("cmdb.relations.messages.noAssets")}</p>
        ) : (
          <>
            <div style={{ display: "flex", gap: "0.5rem", flexWrap: "wrap", alignItems: "center", marginBottom: "0.5rem" }}>
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

            <p style={{ marginTop: 0, marginBottom: "0.75rem" }}>
              {t("cmdb.relations.summary", {
                upstream: relationSummary.upstream,
                downstream: relationSummary.downstream
              })}
            </p>

            <div style={{ display: "flex", gap: "0.5rem", flexWrap: "wrap", marginBottom: "0.75rem", alignItems: "center" }}>
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

            {relations.length === 0 ? (
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
                          <button
                            onClick={() => void deleteRelation(relation.id)}
                            disabled={deletingRelationId === relation.id}
                          >
                            {deletingRelationId === relation.id
                              ? t("cmdb.actions.loading")
                              : t("cmdb.relations.actions.delete")}
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
      </section>

      <section>
        <h2 style={sectionTitleStyle}>{t("cmdb.assets.title")}</h2>
        {emptyState ? (
          <p>{t("cmdb.messages.empty")}</p>
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
                {assets.map((asset) => (
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
      </section>
    </main>
  );
}

function readErrorMessage(response: Response): Promise<string> {
  return response
    .json()
    .then((payload: unknown) => {
      if (payload && typeof payload === "object" && "error" in payload) {
        const value = (payload as { error?: unknown }).error;
        if (typeof value === "string" && value.length > 0) {
          return value;
        }
      }
      return `HTTP ${response.status}`;
    })
    .catch(() => `HTTP ${response.status}`);
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

const sectionTitleStyle: CSSProperties = {
  marginTop: 0,
  marginBottom: "0.5rem"
};

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
