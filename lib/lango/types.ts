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
  | "next_of_kin";

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
