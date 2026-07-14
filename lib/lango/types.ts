import type { LucideIcon } from "lucide-react";

export type Department =
  | "Credit Risk"
  | "Claims Processing"
  | "Patient Records"
  | "Bursar's Office"
  | "Legal Affairs";

export type EntityType =
  | "national_id"
  | "bank_account"
  | "phone_number"
  | "full_name"
  | "medical_record_no"
  | "api_key"
  | "credit_card"
  // Health module (Cimas Healthathon 3.0 — see docs/HEALTH_MODULE.md).
  // Additive: the seven types above are unchanged.
  | "diagnosis_code"
  | "medication_name"
  | "medical_aid_number"
  | "lab_result_value"
  | "next_of_kin"
  // Generic structured-identifier fallback (see
  // backend/src/detection/fallback.rs) — emitted when a token looks
  // ID-shaped (mixed letters/digits, 6-14 chars) near a recognized
  // identifying keyword, but doesn't match any specific entity type's
  // known format. Its actual sensitivity (standard vs.
  // special_category_health) is inferred per-match from the nearby
  // keyword, not fixed by this type name alone.
  | "probable_identifier";

export type Decision =
  | "cleared_no_entities"
  | "blocked_low_confidence"
  | "redacted_and_forwarded"
  | "redacted_low_confidence_review";

/// A NEW, independent-from-`Decision` axis added by the health module:
/// "standard" for the original seven entity types, "special_category_health"
/// for the five new ones. See docs/HEALTH_MODULE.md and
/// backend/src/detection/health_rules.rs's SensitivityClass doc comment —
/// do not conflate this with `Decision`.
export type SensitivityClass = "standard" | "special_category_health";

/// Active learning loop (product-depth task, Part 3) — a human reviewer's
/// recorded confirm/overturn judgment on a flagged low-confidence row, if
/// one has been recorded. Optional (not present, not `null`, in the mock
/// generator's rows) so `lib/lango/mock-data.ts` doesn't need changes —
/// "no review recorded yet" and "mock data has no review concept at all"
/// are both simply the absence of this field.
export interface ReviewDecisionInfo {
  decision: "confirmed" | "overturned";
  reasoning: string | null;
  reviewerEmail: string;
  createdAt: string;
}

/// `audit_log.decision` values eligible for a human confirm/overturn
/// judgment — must match `REVIEWABLE_DECISIONS` in
/// `backend/src/models.rs` exactly.
export const REVIEWABLE_DECISIONS: Decision[] = ["blocked_low_confidence", "redacted_low_confidence_review"];

export interface AuditLogEntry {
  id: string;
  user: string;
  dept: Department;
  timestamp: string;
  entities: EntityType[];
  risk: number;
  decision: Decision;
  reason: string;
  model: string;
  scan: string;
  sensitivityClass: SensitivityClass;
  review?: ReviewDecisionInfo | null;
}

export interface RiskBand {
  label: "high" | "medium" | "low";
  color: string;
  bg: string;
}

export interface DecisionBadgeInfo {
  label: Decision;
  color: string;
  Icon: LucideIcon;
}

export interface ParityEntry {
  group: string;
  flagRate: number;
}

export interface DriftWeek {
  week: string;
  psi: number;
  kl: number;
  alert: boolean;
}

export type SecurityEventType =
  | "prompt_injection_blocked"
  | "rate_limit_triggered"
  | "dos_mitigation_triggered";

export interface SecurityEvent {
  time: string;
  type: SecurityEventType;
  detail: string;
}

export interface PipelineStage {
  key: string;
  label: string;
  sub: string;
}

export interface ChecklistItem {
  label: string;
  done: boolean;
  note?: string;
}

export interface SuccessMetric {
  label: string;
  target: string;
  current: string;
  ok: boolean;
}

export interface NavItem {
  key: string;
  label: string;
  Icon: LucideIcon;
}

/// Policy builder (product-depth task, Part 1) — see
/// backend/src/routes/policy.rs and components/lango/policy-builder.tsx.
/// Live-only: there is no mock-data equivalent (see PolicyBuilder's own
/// comment for why fabricating settings for a feature that fundamentally
/// mutates server state would be actively misleading, unlike the read-only
/// views elsewhere in this dashboard that have a legitimate mock fallback).
export interface CustomPatternInfo {
  id: string;
  entityLabel: string;
  pattern: string;
  confidence: number;
  active: boolean;
  createdAt: string;
}

export interface PolicySettings {
  confidenceThreshold: number;
  minConfidenceThreshold: number;
  maxConfidenceThreshold: number;
  customPatterns: CustomPatternInfo[];
}

/// Health Data Guard view's summary data — see docs/HEALTH_MODULE.md.
/// Deliberately has no per-entity-type or per-condition breakdown field;
/// see backend/src/routes/health.rs's stigma-aware-aggregate-reporting
/// comment for why that's a hard requirement, not an oversight.
export interface HealthSummary {
  specialCategoryTotal: number;
  standardCount: number;
  specialCategoryCount: number;
  redactionRate: number;
  facilityParity: ParityEntry[];
  dirFacility: number | null;
  spdFacility: number | null;
}

/// Real observability ("response scanning + observability + hardening"
/// task, Part 2) — one recorded 5xx backend response. See
/// backend/src/observability.rs and routes/backend_errors.rs.
export interface BackendErrorEntry {
  id: string;
  method: string;
  path: string;
  statusCode: number;
  message: string | null;
  createdAt: string;
}
