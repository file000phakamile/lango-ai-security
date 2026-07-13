use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Auth
// ---------------------------------------------------------------------------

#[derive(Debug, sqlx::FromRow)]
pub struct UserRow {
    pub id: Uuid,
    pub email: String,
    pub password_hash: String,
    pub department: String,
    pub role: String,
}

#[derive(Debug, Serialize)]
pub struct UserPublic {
    pub id: Uuid,
    pub email: String,
    pub department: String,
    pub role: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: UserPublic,
}

/// JWT claims. `sub` is the user id; department/role are embedded so route
/// handlers don't need a DB round-trip just to authorize a request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,
    pub session_id: Uuid,
    pub email: String,
    pub department: String,
    pub role: String,
    pub exp: usize,
}

// ---------------------------------------------------------------------------
// Scan
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ScanRequest {
    pub prompt: String,
    /// Optional session-language tag (e.g. "English"/"Ndebele"/"Shona"), used
    /// only to populate the Fairness Audit view's language-parity chart.
    /// Not derived from the prompt itself (no language detection is
    /// implemented in v0.1) — it's whatever the caller declares.
    pub language: Option<String>,
    /// Optional facility-type tag (e.g. "Rural Clinic" / "District Hospital"
    /// / "Urban Hospital"), added for the health module (see
    /// docs/HEALTH_MODULE.md) — same caller-declared, not-derived-from-the-
    /// prompt pattern as `language` above, used only to populate the Health
    /// Data Guard view's facility-type fairness comparison
    /// (`routes/health.rs`). Existing callers (the dashboard, the browser
    /// extension) simply omit this and are entirely unaffected.
    pub facility_type: Option<String>,
}

/// Matches what `lib/lango/mock-data.ts` implied a scan result looked like,
/// plus `redacted_prompt` which the mock never modeled (it had nothing to
/// redact from, since it never had a real prompt to begin with).
#[derive(Debug, Serialize)]
pub struct ScanResponse {
    pub entities_detected: Vec<String>,
    pub risk_score: f32,
    pub redacted_prompt: String,
    pub decision: String,
    /// Full technical detail (entity type names, confidence scores, which
    /// specific rule/detector fired) — this is the audit-trail string,
    /// meant for a compliance officer reviewing the Audit Log, NOT for
    /// direct display to the person who submitted the prompt. Returned here
    /// (as well as stored in `audit_log.reason_string`) because the
    /// dashboard's Audit Log detail view reads it from this same shape via
    /// the seed data / audit-log endpoint — see `user_message` below for
    /// what a live caller (e.g. the browser extension) should actually show
    /// the end user in the moment.
    pub reason_string: String,
    /// Short, plain-language explanation of what kind of information was
    /// involved and why — no entity_type strings, no confidence numbers, no
    /// detector names (see `detection::plain_language`). This is what the
    /// browser extension's banner should display; `reason_string` above is
    /// the detailed counterpart for later audit review.
    pub user_message: String,
    /// "standard" or "special_category_health" — see the health module's
    /// SensitivityClass axis (`detection::health_rules`). Independent from
    /// `decision`.
    pub sensitivity_class: String,
}

// ---------------------------------------------------------------------------
// Audit log — field names match `AuditLogEntry` in lib/lango/types.ts exactly
// (id/user/dept/timestamp/entities/risk/decision/reason/model/scan) so the
// frontend can consume this response with no shape translation.
// ---------------------------------------------------------------------------

#[derive(Debug, sqlx::FromRow)]
pub struct AuditLogRow {
    pub id: Uuid,
    pub user_email: String,
    pub department: String,
    pub timestamp: DateTime<Utc>,
    pub entities_detected: sqlx::types::Json<Vec<String>>,
    pub risk_score: f32,
    pub decision: String,
    pub reason_string: String,
    pub ai_model_used: String,
    pub response_scan_result: String,
    /// "standard" or "special_category_health" — see the health module's
    /// SensitivityClass axis. Shown only in this per-entry detail view (the
    /// existing expandable Audit Log row, scoped to one specific session) —
    /// per Part 3's stigma-aware aggregate-reporting rule, this same
    /// breakdown must NOT be exposed in any aggregate/trend view. See
    /// routes/health.rs's own comment for the full reasoning.
    pub sensitivity_class: String,
}

#[derive(Debug, Serialize)]
pub struct AuditLogEntry {
    pub id: Uuid,
    pub user: String,
    pub dept: String,
    pub timestamp: DateTime<Utc>,
    pub entities: Vec<String>,
    pub risk: f32,
    pub decision: String,
    pub reason: String,
    pub model: String,
    pub scan: String,
    #[serde(rename = "sensitivityClass")]
    pub sensitivity_class: String,
}

impl From<AuditLogRow> for AuditLogEntry {
    fn from(r: AuditLogRow) -> Self {
        Self {
            id: r.id,
            user: r.user_email,
            dept: r.department,
            timestamp: r.timestamp,
            entities: r.entities_detected.0,
            risk: r.risk_score,
            decision: r.decision,
            reason: r.reason_string,
            model: r.ai_model_used,
            scan: r.response_scan_result,
            sensitivity_class: r.sensitivity_class,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct AuditLogQuery {
    pub decision: Option<String>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct AuditLogPage {
    pub rows: Vec<AuditLogEntry>,
    pub total: i64,
    pub page: u32,
    pub page_size: u32,
}

// ---------------------------------------------------------------------------
// Fairness — matches `ParityEntry { group, flagRate }` in lib/lango/types.ts
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ParityEntry {
    pub group: String,
    #[sqlx(rename = "flag_rate")]
    #[serde(rename = "flagRate")]
    pub flag_rate: f64,
}

#[derive(Debug, Serialize)]
pub struct FairnessResponse {
    pub language_parity: Vec<ParityEntry>,
    pub department_parity: Vec<ParityEntry>,
    pub dir_language: Option<f64>,
    pub spd_language: Option<f64>,
    pub dir_department: Option<f64>,
    pub spd_department: Option<f64>,
    pub threshold: f64,
}

// ---------------------------------------------------------------------------
// Drift — matches `DriftWeek { week, psi, kl, alert }`
// ---------------------------------------------------------------------------

#[derive(Debug, sqlx::FromRow)]
pub struct DriftSnapshotRow {
    pub week_start: chrono::NaiveDate,
    pub psi_score: f32,
    pub kl_divergence_score: f32,
}

#[derive(Debug, Serialize)]
pub struct DriftWeek {
    pub week: String,
    pub psi: f32,
    pub kl: f32,
    pub alert: bool,
}

pub const PSI_ALERT_THRESHOLD: f32 = 0.20;

impl From<(usize, DriftSnapshotRow)> for DriftWeek {
    fn from((i, r): (usize, DriftSnapshotRow)) -> Self {
        Self {
            week: format!("W{}", i + 1),
            psi: r.psi_score,
            kl: r.kl_divergence_score,
            alert: r.psi_score >= PSI_ALERT_THRESHOLD,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct DriftResponse {
    pub weeks: Vec<DriftWeek>,
}

// ---------------------------------------------------------------------------
// Security events — matches `SecurityEvent { time, type, detail }`
// ---------------------------------------------------------------------------

#[derive(Debug, sqlx::FromRow)]
pub struct SecurityEventRow {
    pub event_type: String,
    pub detail: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct SecurityEvent {
    pub time: DateTime<Utc>,
    #[serde(rename = "type")]
    pub event_type: String,
    pub detail: String,
}

impl From<SecurityEventRow> for SecurityEvent {
    fn from(r: SecurityEventRow) -> Self {
        Self {
            time: r.created_at,
            event_type: r.event_type,
            detail: r.detail,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct SecurityEventsResponse {
    pub events: Vec<SecurityEvent>,
}

// ---------------------------------------------------------------------------
// Command Center summary
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct CommandCenterSummary {
    pub sessions_scanned_today: i64,
    pub blocked_today: i64,
    pub avg_risk_score: f64,
    pub active_alerts: i64,
}

// ---------------------------------------------------------------------------
// Health module — Health Data Guard summary (routes/health.rs)
//
// Deliberately does NOT include any breakdown by entity type (diagnosis_code
// vs medication_name vs etc.), let alone by specific condition/medication —
// see Part 3's stigma-aware aggregate-reporting rule, and the comment on
// `routes::health::get_health_summary` for the full reasoning. Only a total
// count and the standard/special-category split (both explicitly permitted)
// are exposed at the aggregate level.
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct HealthSummaryResponse {
    pub special_category_total: i64,
    pub standard_count: i64,
    pub special_category_count: i64,
    /// % of special_category_health rows with decision =
    /// 'redacted_and_forwarded' (out of all special_category_health rows,
    /// including blocked ones) — never `redacted_low_confidence_review`,
    /// per Part 2's hard rule, so that decision value structurally cannot
    /// appear in this denominator/numerator relationship for health rows.
    pub redaction_rate: f64,
    /// Same DIR/SPD math as routes/fairness.rs's department/language
    /// parity, adapted to a new grouping dimension (facility_type) and
    /// scoped to special_category_health rows only — checks whether
    /// special-category detection is equitable across facility types (e.g.
    /// a rural clinic vs. an urban hospital), not just departments.
    pub facility_parity: Vec<ParityEntry>,
    pub dir_facility: Option<f64>,
    pub spd_facility: Option<f64>,
    pub threshold: f64,
}
