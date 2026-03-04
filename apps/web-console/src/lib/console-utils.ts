export function trimToNull(value: string): string | null {
  const normalized = value.trim();
  return normalized.length > 0 ? normalized : null;
}

export function sampleValueForField(definition: { field_type: string; options: string[] | null }): unknown {
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

export function readPayloadString(payload: Record<string, unknown>, key: string): string | null {
  const value = payload[key];
  if (typeof value === "string" && value.trim().length > 0) {
    return value;
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  return null;
}

export function renderCustomFields(value: Record<string, unknown>): string {
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

export function statusChipClass(value: string): string {
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

export function createOwnerDraft(
  ownerType: "team" | "user" | "group" | "external",
  ownerRef: string,
  keyHint?: string
): { key: string; owner_type: "team" | "user" | "group" | "external"; owner_ref: string } {
  ownerDraftSequence += 1;
  return {
    key: keyHint ? `${keyHint}-${ownerDraftSequence}` : `owner-${ownerDraftSequence}`,
    owner_type: ownerType,
    owner_ref: ownerRef
  };
}

export function normalizeOwnerType(value: string): "team" | "user" | "group" | "external" {
  if (value === "team" || value === "user" || value === "group" || value === "external") {
    return value;
  }
  return "team";
}

export function parseBindingList(value: string): string[] {
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

export function parseImpactDepth(value: string): number | null {
  const parsed = Number.parseInt(value.trim(), 10);
  if (!Number.isFinite(parsed) || parsed < 1 || parsed > 8) {
    return null;
  }
  return parsed;
}

export function parseImpactRelationTypesInput(value: string, defaultRelationTypes: string[]): string[] {
  const normalized = value
    .split(",")
    .map((item) => item.trim().toLowerCase())
    .filter((item) => item.length > 0)
    .filter((item) => /^[a-z0-9_-]+$/.test(item));

  if (normalized.length === 0) {
    return [...defaultRelationTypes];
  }

  const unique: string[] = [];
  const seen = new Set<string>();
  for (const item of normalized) {
    if (!seen.has(item)) {
      seen.add(item);
      unique.push(item);
    }
  }

  return unique;
}

type TopologyEdge = {
  id: number;
  src_asset_id: number;
  dst_asset_id: number;
  relation_type: string;
  direction: string;
};

type TopologyNode = {
  id: number;
  depth: number;
  status: string;
};

export function topologyEdgeKey(edge: TopologyEdge): string {
  return `${edge.id}-${edge.direction}`;
}

export function buildParallelEdgeMeta(edges: TopologyEdge[]): Map<string, { index: number; total: number }> {
  const groups = new Map<string, TopologyEdge[]>();
  for (const edge of edges) {
    const left = Math.min(edge.src_asset_id, edge.dst_asset_id);
    const right = Math.max(edge.src_asset_id, edge.dst_asset_id);
    const groupKey = `${left}-${right}`;
    const group = groups.get(groupKey) ?? [];
    group.push(edge);
    groups.set(groupKey, group);
  }

  const meta = new Map<string, { index: number; total: number }>();
  for (const group of groups.values()) {
    group.sort((left, right) => {
      if (left.relation_type !== right.relation_type) {
        return left.relation_type.localeCompare(right.relation_type);
      }
      if (left.direction !== right.direction) {
        return left.direction.localeCompare(right.direction);
      }
      return left.id - right.id;
    });

    for (let index = 0; index < group.length; index += 1) {
      meta.set(topologyEdgeKey(group[index]), {
        index,
        total: group.length
      });
    }
  }

  return meta;
}

export function buildTopologyNodePositions(
  nodes: TopologyNode[],
  rootId: number,
  width: number,
  height: number,
  padding: number
): Map<number, { x: number; y: number }> {
  const positions = new Map<number, { x: number; y: number }>();
  if (nodes.length === 0) {
    return positions;
  }

  const centerX = width / 2;
  const centerY = height / 2;
  const radiusLimit = Math.max(60, Math.min(width, height) / 2 - padding);
  const rings = new Map<number, TopologyNode[]>();
  for (const node of nodes) {
    const depth = Math.max(0, node.depth);
    const group = rings.get(depth) ?? [];
    group.push(node);
    rings.set(depth, group);
  }

  const depthLevels = Array.from(rings.keys()).sort((left, right) => left - right);
  const outerLevels = depthLevels.filter((depth) => depth > 0);
  const ringStep = outerLevels.length > 0 ? radiusLimit / outerLevels.length : 0;

  positions.set(rootId, { x: centerX, y: centerY });
  for (const node of nodes) {
    if (node.id === rootId) {
      positions.set(node.id, { x: centerX, y: centerY });
    }
  }

  for (const depth of depthLevels) {
    if (depth === 0) {
      continue;
    }
    const ring = rings.get(depth) ?? [];
    if (ring.length === 0) {
      continue;
    }

    const radius = ringStep * depth;
    for (let index = 0; index < ring.length; index += 1) {
      const angle = ((Math.PI * 2) / ring.length) * index - Math.PI / 2;
      positions.set(ring[index].id, {
        x: centerX + Math.cos(angle) * radius,
        y: centerY + Math.sin(angle) * radius
      });
    }
  }

  return positions;
}

export function buildTopologyEdgePath(
  src: { x: number; y: number },
  dst: { x: number; y: number },
  index: number,
  total: number
): string {
  const dx = dst.x - src.x;
  const dy = dst.y - src.y;
  const distance = Math.sqrt(dx * dx + dy * dy);
  if (distance <= 1) {
    const radius = 22 + index * 7;
    return `M ${src.x} ${src.y} C ${src.x + radius} ${src.y - radius}, ${src.x + radius * 1.4} ${src.y + radius * 0.8}, ${src.x} ${src.y + 0.1}`;
  }

  const midX = (src.x + dst.x) / 2;
  const midY = (src.y + dst.y) / 2;
  const normalX = -dy / distance;
  const normalY = dx / distance;
  const centerIndex = (total - 1) / 2;
  const offset = (index - centerIndex) * 18;
  const controlX = midX + normalX * offset;
  const controlY = midY + normalY * offset;
  return `M ${src.x.toFixed(2)} ${src.y.toFixed(2)} Q ${controlX.toFixed(2)} ${controlY.toFixed(2)} ${dst.x.toFixed(2)} ${dst.y.toFixed(2)}`;
}

export function relationTypeColor(relationType: string): string {
  switch (relationType) {
    case "contains":
      return "#0f766e";
    case "depends_on":
      return "#0369a1";
    case "runs_service":
      return "#be123c";
    case "owned_by":
      return "#b45309";
    default:
      return "#475569";
  }
}

export function topologyNodeFill(status: string, isRoot: boolean): string {
  if (isRoot) {
    return "#1d4ed8";
  }

  const normalized = status.trim().toLowerCase();
  if (normalized === "operational" || normalized === "active") {
    return "#059669";
  }
  if (normalized === "maintenance") {
    return "#d97706";
  }
  if (normalized === "retired") {
    return "#6b7280";
  }
  return "#0f172a";
}

export function truncateTopologyLabel(value: string, maxLength: number): string {
  if (value.length <= maxLength) {
    return value;
  }
  return `${value.slice(0, Math.max(0, maxLength - 1))}...`;
}

export function parseMonitoringWindowMinutes(value: string): number | null {
  const parsed = Number.parseInt(value.trim(), 10);
  if (!Number.isFinite(parsed) || parsed < 5 || parsed > 1440) {
    return null;
  }
  return parsed;
}

export function formatMetricValue(value: number, unit: string): string {
  if (!Number.isFinite(value)) {
    return "-";
  }
  const normalizedUnit = unit.trim();
  const text = Math.abs(value) >= 100 ? value.toFixed(0) : value.toFixed(2);
  return normalizedUnit ? `${text} ${normalizedUnit}` : text;
}

type MonitoringMetricPoint = {
  value: number;
};

export function buildMetricPolylinePoints(
  points: MonitoringMetricPoint[],
  width: number,
  height: number,
  padding: number
): string {
  if (points.length === 0) {
    return "";
  }
  if (points.length === 1) {
    const y = Math.max(padding, height - padding - (height - padding * 2) / 2);
    return `${padding},${y} ${width - padding},${y}`;
  }

  const values = points.map((point) => point.value);
  const minValue = Math.min(...values);
  const maxValue = Math.max(...values);
  const valueRange = maxValue - minValue;
  const chartWidth = Math.max(1, width - padding * 2);
  const chartHeight = Math.max(1, height - padding * 2);

  return points
    .map((point, index) => {
      const x = padding + (index / (points.length - 1)) * chartWidth;
      const ratio = valueRange === 0 ? 0.5 : (point.value - minValue) / valueRange;
      const y = height - padding - ratio * chartHeight;
      return `${x.toFixed(2)},${y.toFixed(2)}`;
    })
    .join(" ");
}

type AssetStatsBucket = {
  asset_total: number;
};

export function maxBucketAssetTotal(buckets: AssetStatsBucket[]): number {
  return buckets.reduce((maxValue, bucket) => Math.max(maxValue, bucket.asset_total), 0);
}

export function bucketBarWidth(value: number, maxValue: number): string {
  if (value <= 0 || maxValue <= 0) {
    return "0%";
  }
  const percent = Math.round((value / maxValue) * 100);
  return `${Math.max(percent, 6)}%`;
}

export function parseWorkflowReportRangeDays(value: string): number {
  const parsed = Number.parseInt(value.trim(), 10);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    return 30;
  }
  if (parsed === 7 || parsed === 30 || parsed === 90) {
    return parsed;
  }
  return 30;
}

export function parseDateMs(value: string): number | null {
  const parsed = new Date(value).getTime();
  if (Number.isNaN(parsed)) {
    return null;
  }
  return parsed;
}

type WorkflowRequest = {
  template_id: number;
  template_name: string;
  status: string;
  created_at: string;
  requester: string;
};

type WorkflowDailyTrendPoint = {
  day_key: string;
  day_label: string;
  total: number;
  completed: number;
  failed: number;
  active: number;
};

type WorkflowTrendRankRow = {
  key: string;
  label: string;
  week_current: number;
  week_previous: number;
  week_delta: number;
  month_current: number;
  month_previous: number;
  month_delta: number;
};

function normalizeWorkflowStatus(value: string): string {
  return value.trim().toLowerCase();
}

function isWorkflowSuccessStatus(status: string): boolean {
  return status === "completed" || status === "succeeded" || status === "success";
}

function isWorkflowFailureStatus(status: string): boolean {
  return (
    status === "failed"
    || status === "error"
    || status === "rejected"
    || status === "cancelled"
    || status === "timeout"
  );
}

function formatLocalDateKey(date: Date): string {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

export function workflowTemplateDisplayName(request: WorkflowRequest): string {
  return request.template_name.trim().length > 0 ? request.template_name.trim() : `#${request.template_id}`;
}

export function buildWorkflowDailyTrend(requests: WorkflowRequest[], rangeDays: number): WorkflowDailyTrendPoint[] {
  const safeRangeDays = Math.max(1, Math.min(rangeDays, 120));
  const points: WorkflowDailyTrendPoint[] = [];
  const byDay = new Map<string, WorkflowDailyTrendPoint>();
  const now = new Date();
  for (let offset = safeRangeDays - 1; offset >= 0; offset -= 1) {
    const day = new Date(now);
    day.setDate(now.getDate() - offset);
    const key = formatLocalDateKey(day);
    const point: WorkflowDailyTrendPoint = {
      day_key: key,
      day_label: `${day.getMonth() + 1}/${day.getDate()}`,
      total: 0,
      completed: 0,
      failed: 0,
      active: 0
    };
    points.push(point);
    byDay.set(key, point);
  }

  for (const request of requests) {
    const createdAt = new Date(request.created_at);
    if (Number.isNaN(createdAt.getTime())) {
      continue;
    }
    const point = byDay.get(formatLocalDateKey(createdAt));
    if (!point) {
      continue;
    }

    point.total += 1;
    const normalized = normalizeWorkflowStatus(request.status);
    if (isWorkflowSuccessStatus(normalized)) {
      point.completed += 1;
    } else if (isWorkflowFailureStatus(normalized)) {
      point.failed += 1;
    } else {
      point.active += 1;
    }
  }

  return points;
}

export function buildWorkflowTrendRankRows(
  requests: WorkflowRequest[],
  keySelector: (request: WorkflowRequest) => string
): WorkflowTrendRankRow[] {
  const dayMs = 24 * 60 * 60 * 1000;
  const now = Date.now();
  const thisWeekStart = now - 7 * dayMs;
  const previousWeekStart = now - 14 * dayMs;
  const thisMonthStart = now - 30 * dayMs;
  const previousMonthStart = now - 60 * dayMs;

  const counters = new Map<string, WorkflowTrendRankRow>();
  for (const request of requests) {
    const labelRaw = keySelector(request).trim();
    const label = labelRaw.length > 0 ? labelRaw : "unknown";
    const key = label.toLowerCase();
    const createdAt = parseDateMs(request.created_at);
    if (createdAt === null) {
      continue;
    }

    const row = counters.get(key) ?? {
      key,
      label,
      week_current: 0,
      week_previous: 0,
      week_delta: 0,
      month_current: 0,
      month_previous: 0,
      month_delta: 0
    };

    if (createdAt >= thisWeekStart) {
      row.week_current += 1;
    } else if (createdAt >= previousWeekStart) {
      row.week_previous += 1;
    }

    if (createdAt >= thisMonthStart) {
      row.month_current += 1;
    } else if (createdAt >= previousMonthStart) {
      row.month_previous += 1;
    }

    counters.set(key, row);
  }

  return Array.from(counters.values())
    .map((row) => ({
      ...row,
      week_delta: row.week_current - row.week_previous,
      month_delta: row.month_current - row.month_previous
    }))
    .filter((row) => row.week_current > 0 || row.week_previous > 0 || row.month_current > 0 || row.month_previous > 0)
    .sort((left, right) => {
      if (left.month_current !== right.month_current) {
        return right.month_current - left.month_current;
      }
      if (left.week_current !== right.week_current) {
        return right.week_current - left.week_current;
      }
      return left.label.localeCompare(right.label);
    });
}

export function formatSignedDelta(value: number): string {
  if (value > 0) {
    return `+${value}`;
  }
  return String(value);
}

export function escapeCsvCell(value: string): string {
  const needsQuote = value.includes(",") || value.includes("\"") || value.includes("\n") || value.includes("\r");
  const escaped = value.replaceAll("\"", "\"\"");
  return needsQuote ? `"${escaped}"` : escaped;
}
