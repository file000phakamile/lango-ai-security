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
