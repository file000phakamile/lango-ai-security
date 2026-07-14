import {
  DEPARTMENTS,
  DEPT_DIR,
  DEPT_PARITY,
  DIR,
  DRIFT_WEEKS,
  LANGUAGE_PARITY,
  MOCK_HEALTH_SUMMARY,
  SECURITY_EVENTS,
  SPD,
  generateAuditLog,
} from "./mock-data";
import type {
  AuditLogEntry,
  DriftWeek,
  EntityType,
  HealthSummary,
  ParityEntry,
  PolicySettings,
  SecurityEvent,
  SensitivityClass,
} from "./types";

/**
 * Phase 4 wiring: this module is the only place in the frontend that knows
 * whether it's talking to the real Axum backend or falling back to the
 * client-side mock generator. Every component downstream (`CommandCenter`,
 * `AuditLog`, `FairnessAudit`, `DriftMonitor`) consumes the same
 * `DashboardData` shape either way, so they don't need to know or care
 * which source produced it.
 *
 * `NEXT_PUBLIC_USE_MOCK_DATA=true` forces mock data unconditionally — this
 * is what the deployed Vercel demo uses, since no backend is hosted
 * alongside it (see Questions.md). Otherwise, this tries the real backend
 * first and falls back to mock data (with a console warning) if it's
 * unreachable, so a local `npm run dev` without the backend running still
 * shows something instead of an error screen.
 */

const API_BASE = process.env.NEXT_PUBLIC_API_BASE_URL ?? "http://localhost:8080";

// Demo-only credentials for a seeded "compliance" account (see
// backend/src/bin/seed.rs and README.md). This dashboard has no login
// screen of its own in v0.1 — it authenticates transparently as this fixed
// demo account so every dashboard view can call its real, auth-gated
// endpoint. This is a reasonable shortcut for a local/demo v0.1, NOT a
// pattern to carry into a real multi-tenant deployment (that needs an
// actual login flow backed by per-user credentials).
const DEMO_EMAIL = process.env.NEXT_PUBLIC_DEMO_EMAIL ?? "compliance@lango.demo";
const DEMO_PASSWORD = process.env.NEXT_PUBLIC_DEMO_PASSWORD ?? "LangoDemo123!";

export interface DashboardSummary {
  sessionsToday: number;
  blockedToday: number;
  avgRisk: number;
  activeAlerts: number;
}

export interface DashboardData {
  source: "live" | "mock";
  log: AuditLogEntry[];
  summary: DashboardSummary;
  languageParity: ParityEntry[];
  departmentParity: ParityEntry[];
  dirLanguage: number | null;
  spdLanguage: number | null;
  dirDepartment: number | null;
  spdDepartment: number | null;
  driftWeeks: DriftWeek[];
  securityEvents: SecurityEvent[];
  healthSummary: HealthSummary;
}

class ApiError extends Error {}

async function login(): Promise<string> {
  const res = await fetch(`${API_BASE}/api/auth/login`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ email: DEMO_EMAIL, password: DEMO_PASSWORD }),
  });
  if (!res.ok) {
    throw new ApiError(`login failed: HTTP ${res.status}`);
  }
  const data = await res.json();
  return data.token as string;
}

async function authedGet<T>(path: string, token: string): Promise<T> {
  const res = await fetch(`${API_BASE}${path}`, {
    headers: { Authorization: `Bearer ${token}` },
  });
  if (!res.ok) {
    throw new ApiError(`${path} failed: HTTP ${res.status}`);
  }
  return res.json() as Promise<T>;
}

function formatEventTime(iso: string): string {
  // Matches the "YYYY-MM-DD HH:MMZ" shape the mock generator used, so
  // DriftMonitor's rendering doesn't need to know which source it got data
  // from.
  const d = new Date(iso);
  const pad = (n: number) => n.toString().padStart(2, "0");
  return `${d.getUTCFullYear()}-${pad(d.getUTCMonth() + 1)}-${pad(d.getUTCDate())} ${pad(d.getUTCHours())}:${pad(d.getUTCMinutes())}Z`;
}

interface AuditLogPageResponse {
  rows: Array<{
    id: string;
    user: string;
    dept: string;
    timestamp: string;
    entities: string[];
    risk: number;
    decision: string;
    reason: string;
    model: string;
    scan: string;
    sensitivityClass: SensitivityClass;
    review?: {
      decision: "confirmed" | "overturned";
      reasoning: string | null;
      reviewerEmail: string;
      createdAt: string;
    } | null;
  }>;
  total: number;
}

interface HealthSummaryResponse {
  special_category_total: number;
  standard_count: number;
  special_category_count: number;
  redaction_rate: number;
  facility_parity: Array<{ group: string; flagRate: number }>;
  dir_facility: number | null;
  spd_facility: number | null;
}

interface FairnessResponse {
  language_parity: Array<{ group: string; flagRate: number }>;
  department_parity: Array<{ group: string; flagRate: number }>;
  dir_language: number | null;
  spd_language: number | null;
  dir_department: number | null;
  spd_department: number | null;
}

interface DriftResponse {
  weeks: DriftWeek[];
}

interface SecurityEventsResponse {
  events: Array<{ time: string; type: SecurityEvent["type"]; detail: string }>;
}

interface CommandCenterSummaryResponse {
  sessions_scanned_today: number;
  blocked_today: number;
  avg_risk_score: number;
  active_alerts: number;
}

async function loadLiveDashboardData(): Promise<DashboardData> {
  const token = await login();

  const [auditPage, fairness, drift, security, summary, healthSummaryResponse] = await Promise.all([
    authedGet<AuditLogPageResponse>("/api/audit-log?page_size=100", token),
    authedGet<FairnessResponse>("/api/fairness", token),
    authedGet<DriftResponse>("/api/drift", token),
    authedGet<SecurityEventsResponse>("/api/security-events", token),
    authedGet<CommandCenterSummaryResponse>("/api/command-center/summary", token),
    authedGet<HealthSummaryResponse>("/api/health-data-guard/summary", token),
  ]);

  const log: AuditLogEntry[] = auditPage.rows.map((r) => ({
    id: r.id,
    user: r.user,
    dept: r.dept as AuditLogEntry["dept"],
    timestamp: r.timestamp,
    entities: r.entities as EntityType[],
    risk: r.risk,
    decision: r.decision as AuditLogEntry["decision"],
    reason: r.reason,
    model: r.model,
    scan: r.scan,
    sensitivityClass: r.sensitivityClass,
    review: r.review ?? null,
  }));

  return {
    source: "live",
    log,
    summary: {
      sessionsToday: summary.sessions_scanned_today,
      blockedToday: summary.blocked_today,
      avgRisk: summary.avg_risk_score,
      activeAlerts: summary.active_alerts,
    },
    languageParity: fairness.language_parity,
    departmentParity: fairness.department_parity,
    dirLanguage: fairness.dir_language,
    spdLanguage: fairness.spd_language,
    dirDepartment: fairness.dir_department,
    spdDepartment: fairness.spd_department,
    driftWeeks: drift.weeks,
    securityEvents: security.events.map((e) => ({ ...e, time: formatEventTime(e.time) })),
    healthSummary: {
      specialCategoryTotal: healthSummaryResponse.special_category_total,
      standardCount: healthSummaryResponse.standard_count,
      specialCategoryCount: healthSummaryResponse.special_category_count,
      redactionRate: healthSummaryResponse.redaction_rate,
      facilityParity: healthSummaryResponse.facility_parity,
      dirFacility: healthSummaryResponse.dir_facility,
      spdFacility: healthSummaryResponse.spd_facility,
    },
  };
}

function loadMockDashboardData(): DashboardData {
  const log = generateAuditLog(46);
  const blockedToday = log.filter((r) => r.decision !== "cleared_no_entities").length;
  const avgRisk = log.reduce((a, r) => a + r.risk, 0) / log.length;
  const activeAlerts = DRIFT_WEEKS.filter((w) => w.alert).length + (DIR < 0.8 ? 1 : 0);

  return {
    source: "mock",
    log,
    summary: {
      sessionsToday: log.length,
      blockedToday,
      avgRisk,
      activeAlerts,
    },
    languageParity: LANGUAGE_PARITY,
    departmentParity: DEPT_PARITY,
    dirLanguage: DIR,
    spdLanguage: SPD,
    dirDepartment: DEPT_DIR,
    spdDepartment: null,
    driftWeeks: DRIFT_WEEKS,
    securityEvents: SECURITY_EVENTS,
    healthSummary: MOCK_HEALTH_SUMMARY,
  };
}

export async function loadDashboardData(): Promise<DashboardData> {
  const forceMock = process.env.NEXT_PUBLIC_USE_MOCK_DATA === "true";
  if (forceMock) {
    return loadMockDashboardData();
  }
  try {
    return await loadLiveDashboardData();
  } catch (err) {
    console.warn(
      "Lango: real backend unavailable, falling back to client-side mock data.",
      err,
    );
    return loadMockDashboardData();
  }
}

// Re-exported so callers that only need the department list (e.g. static
// UI copy) don't need to import from mock-data.ts directly.
export { DEPARTMENTS };

// ---------------------------------------------------------------------------
// Policy builder (product-depth task, Part 1) — mutating calls, so each one
// re-authenticates via `login()` rather than reusing a token cached from the
// dashboard's initial read-only load. This module has never persisted a
// token across calls (see `login()`'s own doc comment: the whole dashboard
// authenticates transparently as a fixed demo account, a v0.1 shortcut, not
// a production pattern) — one extra round trip per mutating action is a
// small price for not introducing token-caching/refresh logic just for this
// feature. Live-only, deliberately: unlike the read-only views above, there
// is no mock-data fallback for policy settings, since fabricating a
// threshold value or a list of custom patterns that don't actually exist
// anywhere would be actively misleading for a feature whose entire point is
// "this number is what really controls live scans" (see PolicyBuilder.tsx).
// ---------------------------------------------------------------------------

interface PolicySettingsResponse {
  confidence_threshold: number;
  min_confidence_threshold: number;
  max_confidence_threshold: number;
  custom_patterns: Array<{
    id: string;
    entity_label: string;
    pattern: string;
    confidence: number;
    active: boolean;
    created_at: string;
  }>;
}

function toPolicySettings(r: PolicySettingsResponse): PolicySettings {
  return {
    confidenceThreshold: r.confidence_threshold,
    minConfidenceThreshold: r.min_confidence_threshold,
    maxConfidenceThreshold: r.max_confidence_threshold,
    customPatterns: r.custom_patterns.map((p) => ({
      id: p.id,
      entityLabel: p.entity_label,
      pattern: p.pattern,
      confidence: p.confidence,
      active: p.active,
      createdAt: p.created_at,
    })),
  };
}

async function authedRequest<T>(path: string, method: string, body?: unknown): Promise<T> {
  const token = await login();
  const res = await fetch(`${API_BASE}${path}`, {
    method,
    headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (!res.ok) {
    // Surface the backend's own message (e.g. the exact safe-bounds
    // rejection text) rather than a generic "request failed" — the policy
    // builder shows this string directly to the compliance_admin.
    const detail = await res.json().catch(() => null);
    throw new ApiError(detail?.error?.message ?? `${path} failed: HTTP ${res.status}`);
  }
  return res.json() as Promise<T>;
}

export async function fetchPolicySettings(): Promise<PolicySettings> {
  return toPolicySettings(await authedRequest<PolicySettingsResponse>("/api/policy/settings", "GET"));
}

export async function updatePolicyThreshold(confidenceThreshold: number): Promise<PolicySettings> {
  return toPolicySettings(
    await authedRequest<PolicySettingsResponse>("/api/policy/settings", "PUT", {
      confidence_threshold: confidenceThreshold,
    }),
  );
}

export async function createCustomPattern(
  entityLabel: string,
  pattern: string,
  confidence?: number,
): Promise<PolicySettings> {
  return toPolicySettings(
    await authedRequest<PolicySettingsResponse>("/api/policy/custom-patterns", "POST", {
      entity_label: entityLabel,
      pattern,
      confidence,
    }),
  );
}

export async function deleteCustomPattern(id: string): Promise<PolicySettings> {
  return toPolicySettings(
    await authedRequest<PolicySettingsResponse>(`/api/policy/custom-patterns/${id}`, "DELETE"),
  );
}

// ---------------------------------------------------------------------------
// Compliance export (product-depth task, Part 2) — a file download, not a
// JSON call, so this doesn't go through `authedRequest` above: it fetches
// the raw response, reads the filename the backend chose from
// Content-Disposition (see main.rs's CORS `expose_headers`), and triggers a
// browser download via a temporary anchor element, since a plain
// `window.location` navigation can't attach the Authorization header this
// endpoint requires.
// ---------------------------------------------------------------------------

export async function downloadComplianceExport(
  start: string,
  end: string,
  format: "csv" | "pdf",
): Promise<void> {
  const token = await login();
  const res = await fetch(
    `${API_BASE}/api/compliance-export?start=${encodeURIComponent(start)}&end=${encodeURIComponent(end)}&format=${format}`,
    { headers: { Authorization: `Bearer ${token}` } },
  );
  if (!res.ok) {
    const detail = await res.json().catch(() => null);
    throw new ApiError(detail?.error?.message ?? `compliance export failed: HTTP ${res.status}`);
  }
  const blob = await res.blob();
  const disposition = res.headers.get("content-disposition") ?? "";
  const match = disposition.match(/filename="([^"]+)"/);
  const filename = match ? match[1] : `lango-compliance-export.${format}`;

  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  a.remove();
  URL.revokeObjectURL(url);
}

// ---------------------------------------------------------------------------
// Active learning loop (product-depth task, Part 3) — recording a human
// confirm/overturn judgment on a flagged low-confidence audit_log row
// (`AuditLog`'s row-expand calls this), and downloading everything recorded
// so far as a labelled dataset (same download-via-anchor pattern as
// `downloadComplianceExport` above, reused rather than duplicated).
// ---------------------------------------------------------------------------

export async function recordReviewDecision(
  auditLogId: string,
  decision: "confirmed" | "overturned",
  reasoning: string | undefined,
): Promise<void> {
  await authedRequest(`/api/audit-log/${auditLogId}/review-decision`, "POST", { decision, reasoning });
}

export async function downloadLabelledDataset(format: "csv" | "jsonl"): Promise<void> {
  const token = await login();
  const res = await fetch(`${API_BASE}/api/labelled-dataset?format=${format}`, {
    headers: { Authorization: `Bearer ${token}` },
  });
  if (!res.ok) {
    const detail = await res.json().catch(() => null);
    throw new ApiError(detail?.error?.message ?? `labelled dataset export failed: HTTP ${res.status}`);
  }
  const blob = await res.blob();
  const disposition = res.headers.get("content-disposition") ?? "";
  const match = disposition.match(/filename="([^"]+)"/);
  const filename = match ? match[1] : `lango-labelled-dataset.${format}`;

  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  a.remove();
  URL.revokeObjectURL(url);
}
