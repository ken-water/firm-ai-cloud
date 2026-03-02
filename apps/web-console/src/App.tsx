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

type NewFieldForm = {
  field_key: string;
  name: string;
  field_type: string;
  max_length: string;
  options_csv: string;
  required: boolean;
  scanner_enabled: boolean;
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
    void Promise.all([loadAssets(), loadFieldDefinitions()]);
  }, [loadAssets, loadFieldDefinitions]);

  const emptyState = useMemo(() => assets.length === 0, [assets]);

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

const cellStyle: CSSProperties = {
  border: "1px solid #ddd",
  padding: "0.5rem",
  textAlign: "left",
  whiteSpace: "nowrap",
  verticalAlign: "top"
};
