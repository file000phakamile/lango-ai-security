import type {
  AuditLogEntry,
  Decision,
  Department,
  DriftWeek,
  EntityType,
  ParityEntry,
  PipelineStage,
  RiskBand,
  SecurityEvent,
} from "./types";

/* ---------------------------------------------------------
   Deterministic PRNG so the mock data is stable across renders
--------------------------------------------------------- */
function mulberry32(seed: number) {
  return function () {
    seed |= 0;
    seed = (seed + 0x6d2b79f5) | 0;
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
function pick<T>(rand: () => number, arr: T[]): T {
  return arr[Math.floor(rand() * arr.length)];
}

export const DEPARTMENTS: Department[] = [
  "Credit Risk",
  "Claims Processing",
  "Patient Records",
  "Bursar's Office",
  "Legal Affairs",
];
const ENTITY_TYPES: EntityType[] = [
  "national_id",
  "bank_account",
  "phone_number",
  "full_name",
  "medical_record_no",
  "api_key",
];
const MODEL_CONNECTORS = ["Connector: Provider A, model v1", "Connector: Provider B, model v1"];

function pad(n: number) {
  return n.toString().padStart(2, "0");
}

export function generateAuditLog(count = 46): AuditLogEntry[] {
  const rand = mulberry32(2026);
  const rows: AuditLogEntry[] = [];
  const now = new Date("2026-07-09T09:00:00Z").getTime();
  for (let i = 0; i < count; i++) {
    const dept = pick(rand, DEPARTMENTS);
    const entityCount = rand() < 0.72 ? 1 + Math.floor(rand() * 2) : 0;
    const entities: EntityType[] = Array.from({ length: entityCount }, () => pick(rand, ENTITY_TYPES));
    const riskBase = entityCount === 0 ? rand() * 0.25 : 0.4 + rand() * 0.55;
    const risk = Math.round(riskBase * 100) / 100;
    let decision: Decision, reason: string, scan: string;
    if (entityCount === 0) {
      decision = "cleared_no_entities";
      reason = "No sensitive entities detected. Prompt forwarded unmodified.";
      scan = "clear - no leakage detected";
    } else if (risk >= 0.85 && rand() < 0.3) {
      decision = "blocked_low_confidence";
      reason = "Scanner confidence below threshold on detected entity. Fail-closed triggered.";
      scan = "not sent - request blocked pre-gateway";
    } else {
      decision = "redacted_and_forwarded";
      reason = `Blocked raw prompt: ${entities.join(", ")} detected, replaced with placeholder tokens.`;
      scan = "clear - no leakage detected";
    }
    const ts = new Date(now - i * (1000 * 60 * (14 + Math.floor(rand() * 40))));
    rows.push({
      id: `${(8000 + i).toString(16)}-${pad(i)}c3-${pad(Math.floor(rand() * 99))}-de${pad(Math.floor(rand() * 99))}`,
      user: `u_${2200 + Math.floor(rand() * 300)}`,
      dept,
      timestamp: `${ts.toISOString().slice(0, 10)}T${pad(ts.getUTCHours())}:${pad(ts.getUTCMinutes())}:${pad(ts.getUTCSeconds())}Z`,
      entities,
      risk,
      decision,
      reason,
      model: pick(rand, MODEL_CONNECTORS),
      scan,
    });
  }
  return rows;
}

export function riskBand(risk: number): RiskBand {
  if (risk >= 0.7) return { label: "high", color: "#A83A3A", bg: "rgba(168,58,58,0.10)" };
  if (risk >= 0.4) return { label: "medium", color: "#8A6323", bg: "rgba(138,99,35,0.10)" };
  return { label: "low", color: "#2F7A53", bg: "rgba(47,122,83,0.10)" };
}

/* ---------------------------------------------------------
   Fairness data - language parity snapshot
--------------------------------------------------------- */
export const LANGUAGE_PARITY: ParityEntry[] = [
  { group: "English", flagRate: 9.0 },
  { group: "Ndebele", flagRate: 7.4 },
  { group: "Shona", flagRate: 6.0 },
];
export const DIR = Math.round((6.0 / 9.0) * 100) / 100;
export const SPD = Math.round((9.0 - 6.0) * 10) / 10;

export const DEPT_PARITY: ParityEntry[] = DEPARTMENTS.map((d, i) => {
  const rate = [8.1, 7.6, 9.4, 6.8, 8.8][i];
  return { group: d, flagRate: rate };
});
const deptMax = Math.max(...DEPT_PARITY.map((d) => d.flagRate));
const deptMin = Math.min(...DEPT_PARITY.map((d) => d.flagRate));
export const DEPT_DIR = Math.round((deptMin / deptMax) * 100) / 100;

/* ---------------------------------------------------------
   Drift data - 12 weeks, PSI + KL-divergence, one pre-tested
   synthetic drift spike that crosses threshold at week 9
--------------------------------------------------------- */
export const DRIFT_WEEKS: DriftWeek[] = (() => {
  const rand = mulberry32(2026);
  return Array.from({ length: 12 }, (_, i) => {
    const week = `W${i + 1}`;
    const spike = i === 8;
    const psi = spike ? 0.27 : Math.round((0.04 + rand() * 0.09) * 100) / 100;
    const kl = spike ? 0.21 : Math.round((0.03 + rand() * 0.07) * 100) / 100;
    return { week, psi, kl, alert: spike };
  });
})();

export const SECURITY_EVENTS: SecurityEvent[] = [
  { time: "2026-07-09 08:41Z", type: "prompt_injection_blocked", detail: "System-instruction override attempt detected in user input, sanitised before AI Gateway." },
  { time: "2026-07-09 07:12Z", type: "rate_limit_triggered", detail: "Per-user token quota exceeded, request queued for next window." },
  { time: "2026-07-08 22:03Z", type: "prompt_injection_blocked", detail: "Delimiter-escape pattern detected and stripped from prompt." },
  { time: "2026-07-08 15:55Z", type: "dos_mitigation_triggered", detail: "Burst of 40 requests from single session in 10s, throttled." },
  { time: "2026-07-08 11:20Z", type: "rate_limit_triggered", detail: "Institution-level API quota at 92%, alert sent to ops." },
  { time: "2026-07-07 19:47Z", type: "prompt_injection_blocked", detail: "Encoded payload in code block flagged and rejected." },
];

export const PIPELINE_STAGES: PipelineStage[] = [
  { key: "auth", label: "Authentication", sub: "JWT + Argon2" },
  { key: "scan", label: "Prompt Scanner", sub: "risk score assigned" },
  { key: "redact", label: "Redaction Engine", sub: "entities replaced" },
  { key: "gateway", label: "AI Gateway", sub: "sanitised prompt sent" },
  { key: "response", label: "Response Scanner", sub: "output checked" },
  { key: "audit", label: "Audit Service", sub: "full record written" },
];
