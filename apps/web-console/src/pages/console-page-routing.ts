export type MenuAxis = "function" | "department" | "business" | "screen";
export type FunctionWorkspace = "full" | "cmdb" | "monitoring" | "workflow";
export type ConsolePage =
  | "setup"
  | "overview"
  | "cmdb"
  | "monitoring"
  | "alerts"
  | "topology"
  | "workflow"
  | "tickets"
  | "admin";

export const defaultConsolePage: ConsolePage = "overview";

export const consolePageSections: Record<ConsolePage, string[]> = {
  setup: ["section-setup-wizard"],
  overview: ["section-cockpit", "section-monitoring-metrics", "section-topology", "section-asset-stats"],
  cmdb: [
    "section-scan",
    "section-fields",
    "section-relations",
    "section-readiness",
    "section-topology",
    "section-asset-stats",
    "section-assets"
  ],
  monitoring: ["section-cockpit", "section-monitoring-sources", "section-monitoring-metrics", "section-topology"],
  alerts: ["section-alert-center"],
  topology: ["section-topology-workspace"],
  workflow: [
    "section-workflow-cockpit",
    "section-workflow-reports",
    "section-workflow",
    "section-discovery",
    "section-notifications"
  ],
  tickets: ["section-tickets"],
  admin: ["section-admin"]
};

const legacySectionToPage: Record<string, ConsolePage> = {
  "section-setup-wizard": "setup",
  "section-alert-center": "alerts",
  "section-admin": "admin",
  "section-workflow-cockpit": "workflow",
  "section-workflow-reports": "workflow",
  "section-workflow": "workflow",
  "section-discovery": "workflow",
  "section-notifications": "workflow",
  "section-tickets": "tickets",
  "section-monitoring-sources": "monitoring",
  "section-monitoring-metrics": "monitoring",
  "section-topology-workspace": "topology",
  "section-scan": "cmdb",
  "section-fields": "cmdb",
  "section-relations": "cmdb",
  "section-readiness": "cmdb",
  "section-assets": "cmdb",
  "section-cockpit": "overview",
  "section-topology": "overview",
  "section-asset-stats": "overview"
};

export function buildConsolePageHash(page: ConsolePage): string {
  return `#/${page}`;
}

export function resolveConsolePageFromHash(hash: string, canAccessAdmin: boolean): ConsolePage {
  const normalized = hash.trim().replace(/^#/, "");
  const primary = normalized.split("?")[0];
  const candidate = primary.replace(/^\/+/, "").split("/")[0];
  const directPage = parseConsolePage(candidate);
  if (directPage) {
    if (directPage === "admin" && !canAccessAdmin) {
      return defaultConsolePage;
    }
    return directPage;
  }

  const legacyPage = legacySectionToPage[candidate];
  if (legacyPage === "admin" && !canAccessAdmin) {
    return defaultConsolePage;
  }
  if (legacyPage) {
    return legacyPage;
  }

  return defaultConsolePage;
}

function parseConsolePage(value: string): ConsolePage | null {
  switch (value.trim().toLowerCase()) {
    case "overview":
    case "cmdb":
    case "monitoring":
    case "setup":
    case "alerts":
    case "topology":
    case "workflow":
    case "tickets":
    case "admin":
      return value.trim().toLowerCase() as ConsolePage;
    default:
      return null;
  }
}
