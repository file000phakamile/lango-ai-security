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
  | "api_key";

export type Decision =
  | "cleared_no_entities"
  | "blocked_low_confidence"
  | "redacted_and_forwarded";

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
