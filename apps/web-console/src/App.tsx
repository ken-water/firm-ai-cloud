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
  created_at: string;
  updated_at: string;
};

type AssetListResponse = {
  items: Asset[];
  total: number;
  limit: number;
  offset: number;
};

const API_BASE_URL = import.meta.env.VITE_API_BASE_URL ?? "http://127.0.0.1:8080";

export function App() {
  const { t } = useTranslation();
  const [assets, setAssets] = useState<Asset[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [creatingSample, setCreatingSample] = useState(false);

  const loadAssets = useCallback(async () => {
    setLoading(true);
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
      setLoading(false);
    }
  }, []);

  const createSampleAsset = useCallback(async () => {
    setCreatingSample(true);
    setError(null);
    try {
      const body = {
        asset_class: "server",
        name: `sample-${Date.now()}`,
        hostname: "sample-host.local",
        ip: "10.0.0.10",
        status: "active",
        site: "dc-a",
        department: "platform",
        owner: "ops"
      };
      const response = await fetch(`${API_BASE_URL}/api/v1/cmdb/assets`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json"
        },
        body: JSON.stringify(body)
      });
      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }
      await loadAssets();
    } catch (err) {
      setError(err instanceof Error ? err.message : "unknown error");
    } finally {
      setCreatingSample(false);
    }
  }, [loadAssets]);

  useEffect(() => {
    void loadAssets();
  }, [loadAssets]);

  const emptyState = useMemo(() => assets.length === 0, [assets]);

  return (
    <main style={{ fontFamily: "sans-serif", padding: "2rem", lineHeight: 1.5 }}>
      <header style={{ marginBottom: "1rem" }}>
        <h1 style={{ marginBottom: "0.25rem" }}>{t("app.title")}</h1>
        <p style={{ marginTop: 0 }}>{t("app.subtitle")}</p>
      </header>

      <section style={{ marginBottom: "1rem", display: "flex", gap: "0.5rem" }}>
        <button onClick={() => void loadAssets()} disabled={loading}>
          {loading ? t("cmdb.actions.loading") : t("cmdb.actions.refresh")}
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

      {emptyState ? (
        <p>{t("cmdb.messages.empty")}</p>
      ) : (
        <div style={{ overflowX: "auto" }}>
          <table style={{ borderCollapse: "collapse", minWidth: "900px", width: "100%" }}>
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
                  <td style={cellStyle}>{new Date(asset.updated_at).toLocaleString()}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </main>
  );
}

const cellStyle: CSSProperties = {
  border: "1px solid #ddd",
  padding: "0.5rem",
  textAlign: "left",
  whiteSpace: "nowrap"
};
