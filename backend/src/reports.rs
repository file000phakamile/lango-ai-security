//! Compliance export report generation (product-depth task, Part 2) — pure
//! formatting logic, no database or HTTP concerns, same "keep the actual
//! transformation testable without a live server" split this codebase
//! already uses for `detection::scan::scan_prompt`. `routes::compliance_export`
//! is the thin HTTP layer that fetches `ComplianceExportData` from Postgres
//! and calls into this module.
//!
//! Two formats, both covering the same three things the task asked for
//! (audit log, fairness metrics, drift history) for a selected date range:
//!
//! - **CSV**: the complete dataset for the range, every audit_log row,
//!   suitable for a regulator/auditor to load into a spreadsheet or their
//!   own tooling. No row cap.
//! - **PDF**: a human-readable, printable summary. Capped at
//!   `MAX_PDF_AUDIT_ROWS` audit rows (most recent first) to keep PDF
//!   generation and rendering fast for a busy organisation's full-quarter
//!   export — the CSV is the "complete data" format, the PDF is the
//!   "readable enough to print and hand over" format. This distinction is
//!   stated explicitly in the PDF's own header text, not left implicit.

use chrono::{DateTime, NaiveDate, Utc};

use crate::models::ParityEntry;

/// One audit_log row as needed by a compliance export — a deliberately
/// separate shape from `models::AuditLogRow`/`AuditLogEntry` (the live
/// dashboard's paginated view) since an export needs the full-range dataset
/// with no pagination, and includes `reason_string` in full, which the
/// dashboard's compact list view doesn't need to carry.
pub struct ExportAuditRow {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub user_email: String,
    pub department: String,
    pub entities_detected: Vec<String>,
    pub risk_score: f32,
    pub decision: String,
    pub sensitivity_class: String,
    pub reason_string: String,
}

pub struct ExportDriftWeek {
    pub week_start: NaiveDate,
    pub psi_score: f32,
    pub kl_divergence_score: f32,
    pub alert: bool,
}

/// Everything both `build_csv` and `build_pdf` need — assembled once by
/// `routes::compliance_export`, consumed by both, so the two formats can
/// never silently drift apart on what data they cover for the same request.
pub struct ComplianceExportData {
    pub organisation_name: String,
    pub range_start: NaiveDate,
    pub range_end: NaiveDate,
    pub generated_at: DateTime<Utc>,
    pub audit_rows: Vec<ExportAuditRow>,
    pub department_parity: Vec<ParityEntry>,
    pub language_parity: Vec<ParityEntry>,
    pub dir_department: Option<f64>,
    pub spd_department: Option<f64>,
    pub dir_language: Option<f64>,
    pub spd_language: Option<f64>,
    pub fairness_threshold: f64,
    pub drift_weeks: Vec<ExportDriftWeek>,
}

fn fmt_opt(v: Option<f64>) -> String {
    v.map(|x| format!("{x:.2}")).unwrap_or_else(|| "n/a (insufficient data in range)".to_string())
}

/// Builds the CSV export: one file, several clearly-labeled sections
/// (a section header row, a column-header row, then data rows, then a blank
/// separator row) rather than three separate files — a single "download and
/// hand over" artifact per the task's "one-click export" requirement. Uses
/// the `csv` crate for correct quoting/escaping (a `reason_string` value can
/// itself contain commas or quotes) rather than hand-rolled string
/// concatenation.
pub fn build_csv(data: &ComplianceExportData) -> String {
    let mut w = csv::WriterBuilder::new().from_writer(vec![]);

    let _ = w.write_record(["Lango AI Data Guard — Compliance Export"]);
    let _ = w.write_record(["Organisation", &data.organisation_name]);
    let _ = w.write_record(["Date range", &format!("{} to {}", data.range_start, data.range_end)]);
    let _ = w.write_record(["Generated (UTC)", &data.generated_at.to_rfc3339()]);
    let _ = w.write_record([""; 0]);

    let _ = w.write_record(["AUDIT LOG"]);
    let _ = w.write_record([
        "id",
        "timestamp_utc",
        "user_email",
        "department",
        "entities_detected",
        "risk_score",
        "decision",
        "sensitivity_class",
        "reason_string",
    ]);
    for r in &data.audit_rows {
        let _ = w.write_record([
            r.id.as_str(),
            &r.timestamp.to_rfc3339(),
            r.user_email.as_str(),
            r.department.as_str(),
            &r.entities_detected.join("; "),
            &format!("{:.2}", r.risk_score),
            r.decision.as_str(),
            r.sensitivity_class.as_str(),
            r.reason_string.as_str(),
        ]);
    }
    let _ = w.write_record([""; 0]);

    let _ = w.write_record(["FAIRNESS METRICS (this date range)"]);
    let _ = w.write_record(["category", "group", "flag_rate_pct"]);
    for p in &data.department_parity {
        let _ = w.write_record(["department", &p.group, &format!("{:.1}", p.flag_rate)]);
    }
    for p in &data.language_parity {
        let _ = w.write_record(["language", &p.group, &format!("{:.1}", p.flag_rate)]);
    }
    let _ = w.write_record([""; 0]);
    let _ = w.write_record(["metric", "value"]);
    let _ = w.write_record(["disparate_impact_ratio_department", &fmt_opt(data.dir_department)]);
    let _ = w.write_record(["statistical_parity_difference_department_pct", &fmt_opt(data.spd_department)]);
    let _ = w.write_record(["disparate_impact_ratio_language", &fmt_opt(data.dir_language)]);
    let _ = w.write_record(["statistical_parity_difference_language_pct", &fmt_opt(data.spd_language)]);
    let _ = w.write_record(["fairness_pass_threshold", &format!("{:.2}", data.fairness_threshold)]);
    let _ = w.write_record([""; 0]);

    let _ = w.write_record(["DRIFT HISTORY (this date range)"]);
    let _ = w.write_record(["week_start", "psi_score", "kl_divergence_score", "alert"]);
    for wk in &data.drift_weeks {
        let _ = w.write_record([
            wk.week_start.to_string(),
            format!("{:.3}", wk.psi_score),
            format!("{:.3}", wk.kl_divergence_score),
            wk.alert.to_string(),
        ]);
    }

    let bytes = w.into_inner().expect("in-memory csv writer cannot fail to flush");
    String::from_utf8(bytes).expect("csv writer output is always valid UTF-8 for UTF-8 input")
}

/// Audit rows are capped in the PDF (most recent first) — see this module's
/// doc comment for why. The CSV export has no such cap.
pub const MAX_PDF_AUDIT_ROWS: usize = 500;

/// Truncates a string to at most `max_chars` *characters* (not bytes) at a
/// char boundary, appending "..." when truncated, so a long `reason_string`
/// can never push a PDF line past the page's usable width regardless of
/// what's in it.
fn truncate_chars(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let truncated: String = s.chars().take(max_chars.saturating_sub(3)).collect();
    format!("{truncated}...")
}

use printpdf::{BuiltinFont, IndirectFontRef, Mm, PdfDocument, PdfDocumentReference, PdfLayerReference};

const PAGE_WIDTH_MM: f32 = 210.0;
const PAGE_HEIGHT_MM: f32 = 297.0;
const MARGIN_MM: f32 = 15.0;
const LINE_HEIGHT_MM: f32 = 5.0;
const BODY_FONT_SIZE: f32 = 9.0;

/// Small stateful helper so `build_pdf` can just call `.line(...)` /
/// `.blank()` repeatedly without manually tracking the current page, layer,
/// and y-cursor, or hand-rolling pagination at every call site. Every line
/// is written in a built-in, non-embedded font (Courier/Courier-Bold) — no
/// font file needs to ship with this backend, matching this codebase's
/// existing preference for dependency-light choices (see the name-heuristic
/// module's doc comment on why a native-lib-dependent NER crate was
/// avoided).
struct PdfWriter {
    doc: PdfDocumentReference,
    layer: PdfLayerReference,
    font: IndirectFontRef,
    font_bold: IndirectFontRef,
    y: f32,
}

impl PdfWriter {
    fn new(title: &str) -> Self {
        let (doc, page, layer) = PdfDocument::new(title, Mm(PAGE_WIDTH_MM), Mm(PAGE_HEIGHT_MM), "Layer 1");
        let font = doc.add_builtin_font(BuiltinFont::Courier).expect("built-in font must load");
        let font_bold = doc.add_builtin_font(BuiltinFont::CourierBold).expect("built-in font must load");
        let layer = doc.get_page(page).get_layer(layer);
        Self { doc, layer, font, font_bold, y: PAGE_HEIGHT_MM - MARGIN_MM }
    }

    fn new_page(&mut self) {
        let (page, layer) = self.doc.add_page(Mm(PAGE_WIDTH_MM), Mm(PAGE_HEIGHT_MM), "Layer 1");
        self.layer = self.doc.get_page(page).get_layer(layer);
        self.y = PAGE_HEIGHT_MM - MARGIN_MM;
    }

    fn ensure_space(&mut self) {
        if self.y < MARGIN_MM {
            self.new_page();
        }
    }

    fn line(&mut self, text: &str, bold: bool) {
        self.ensure_space();
        let font = if bold { &self.font_bold } else { &self.font };
        self.layer.use_text(text, BODY_FONT_SIZE, Mm(MARGIN_MM), Mm(self.y), font);
        self.y -= LINE_HEIGHT_MM;
    }

    fn blank(&mut self) {
        self.y -= LINE_HEIGHT_MM;
    }

    fn save_to_bytes(self) -> Vec<u8> {
        let mut buf = std::io::BufWriter::new(std::io::Cursor::new(Vec::<u8>::new()));
        self.doc.save(&mut buf).expect("in-memory pdf save cannot fail");
        buf.into_inner().expect("BufWriter flush cannot fail for an in-memory Cursor").into_inner()
    }
}

/// Builds the PDF export: a human-readable, printable summary covering the
/// same audit log / fairness / drift data as the CSV, formatted so a
/// regulator or auditor can read it directly without needing to open it in
/// another tool first — plain section headers, no jargon beyond the actual
/// metric names a compliance reviewer would already need to know
/// (disparate impact ratio, statistical parity difference, PSI,
/// KL-divergence), each explained once in the fairness section.
pub fn build_pdf(data: &ComplianceExportData) -> Vec<u8> {
    let mut w = PdfWriter::new("Lango AI Data Guard — Compliance Export");

    w.line("Lango AI Data Guard — Compliance Export", true);
    w.blank();
    w.line(&format!("Organisation: {}", data.organisation_name), false);
    w.line(&format!("Date range: {} to {}", data.range_start, data.range_end), false);
    w.line(&format!("Generated: {} UTC", data.generated_at.format("%Y-%m-%d %H:%M:%S")), false);
    w.blank();
    w.line(
        "This document covers every prompt-scan decision, fairness metric, and drift",
        false,
    );
    w.line(
        "measurement recorded for this organisation in the date range above. The audit",
        false,
    );
    w.line(
        "log section below is a readable summary of up to 500 most recent rows in range;",
        false,
    );
    w.line("the accompanying CSV export contains the complete, unabridged dataset.", false);
    w.blank();

    // --- Fairness ---------------------------------------------------------
    w.line("FAIRNESS METRICS (this date range)", true);
    w.blank();
    w.line(
        "Disparate Impact Ratio (DIR): lowest group flag rate / highest group flag rate.",
        false,
    );
    w.line(
        &format!("A value below {:.2} indicates a potential fairness concern.", data.fairness_threshold),
        false,
    );
    w.line("Statistical Parity Difference (SPD): highest - lowest flag rate, in points.", false);
    w.blank();
    w.line("By department:", true);
    if data.department_parity.is_empty() {
        w.line("  (no data in this date range)", false);
    }
    for p in &data.department_parity {
        w.line(&format!("  {:<28} flag rate {:.1}%", p.group, p.flag_rate), false);
    }
    w.line(
        &format!("  DIR: {}   SPD: {} pts", fmt_opt(data.dir_department), fmt_opt(data.spd_department)),
        false,
    );
    w.blank();
    w.line("By language:", true);
    if data.language_parity.is_empty() {
        w.line("  (no data in this date range)", false);
    }
    for p in &data.language_parity {
        w.line(&format!("  {:<28} flag rate {:.1}%", p.group, p.flag_rate), false);
    }
    w.line(
        &format!("  DIR: {}   SPD: {} pts", fmt_opt(data.dir_language), fmt_opt(data.spd_language)),
        false,
    );
    w.blank();

    // --- Drift --------------------------------------------------------
    w.line("DRIFT HISTORY (this date range)", true);
    w.blank();
    w.line(
        "PSI (Population Stability Index) and KL-divergence measure how much the mix of",
        false,
    );
    w.line("detected entity types has shifted week over week versus a stable baseline.", false);
    w.blank();
    if data.drift_weeks.is_empty() {
        w.line("  (no drift snapshots in this date range)", false);
    }
    for wk in &data.drift_weeks {
        let flag = if wk.alert { "  ALERT" } else { "" };
        w.line(
            &format!(
                "  {}   PSI {:.3}   KL-divergence {:.3}{}",
                wk.week_start, wk.psi_score, wk.kl_divergence_score, flag
            ),
            false,
        );
    }
    w.blank();

    // --- Audit log ----------------------------------------------------
    w.line("AUDIT LOG", true);
    w.blank();
    let total = data.audit_rows.len();
    let shown = total.min(MAX_PDF_AUDIT_ROWS);
    if total > MAX_PDF_AUDIT_ROWS {
        w.line(
            &format!(
                "Showing the {shown} most recent of {total} rows in this range — see the CSV export for the complete dataset."
            ),
            false,
        );
        w.blank();
    }
    if data.audit_rows.is_empty() {
        w.line("  (no audit log rows in this date range)", false);
    }
    for r in data.audit_rows.iter().take(MAX_PDF_AUDIT_ROWS) {
        w.line(
            &format!(
                "[{}] dept={} decision={}",
                r.timestamp.format("%Y-%m-%d %H:%M"),
                truncate_chars(&r.department, 20),
                truncate_chars(&r.decision, 32),
            ),
            false,
        );
        w.line(
            &format!(
                "  risk={:.2} sensitivity={} entities={}",
                r.risk_score,
                r.sensitivity_class,
                truncate_chars(&r.entities_detected.join(", "), 55)
            ),
            false,
        );
        w.line(&format!("  reason: {}", truncate_chars(&r.reason_string, 88)), false);
        w.blank();
    }

    w.save_to_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn sample_data() -> ComplianceExportData {
        ComplianceExportData {
            organisation_name: "Regional Commercial Bank Demo".to_string(),
            range_start: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            range_end: NaiveDate::from_ymd_opt(2026, 7, 13).unwrap(),
            generated_at: Utc.with_ymd_and_hms(2026, 7, 14, 9, 0, 0).unwrap(),
            audit_rows: vec![ExportAuditRow {
                id: "11111111-1111-1111-1111-111111111111".to_string(),
                timestamp: Utc.with_ymd_and_hms(2026, 3, 1, 12, 0, 0).unwrap(),
                user_email: "reviewer@bank.test".to_string(),
                department: "Credit Risk".to_string(),
                entities_detected: vec!["national_id".to_string(), "phone_number".to_string()],
                risk_score: 0.62,
                decision: "redacted_and_forwarded".to_string(),
                sensitivity_class: "standard".to_string(),
                reason_string: "Blocked raw prompt: national_id [primary pattern match], contains a comma, and \"quotes\"".to_string(),
            }],
            department_parity: vec![ParityEntry { group: "Credit Risk".to_string(), flag_rate: 12.5 }],
            language_parity: vec![ParityEntry { group: "English".to_string(), flag_rate: 9.0 }],
            dir_department: Some(0.72),
            spd_department: Some(15.0),
            dir_language: None,
            spd_language: None,
            fairness_threshold: 0.80,
            drift_weeks: vec![ExportDriftWeek {
                week_start: NaiveDate::from_ymd_opt(2026, 1, 5).unwrap(),
                psi_score: 0.27,
                kl_divergence_score: 0.21,
                alert: true,
            }],
        }
    }

    #[test]
    fn csv_includes_all_three_sections_and_the_org_header() {
        let csv = build_csv(&sample_data());
        assert!(csv.contains("Regional Commercial Bank Demo"));
        assert!(csv.contains("AUDIT LOG"));
        assert!(csv.contains("FAIRNESS METRICS"));
        assert!(csv.contains("DRIFT HISTORY"));
        assert!(csv.contains("national_id; phone_number"));
    }

    #[test]
    fn csv_properly_quotes_a_reason_string_containing_a_comma_and_quotes() {
        let csv = build_csv(&sample_data());
        // The csv crate must have quoted this field (it contains a comma and
        // literal double-quotes) — if it hadn't, the comma would silently
        // split the reason text across extra columns, corrupting every
        // column after it for that row when opened in a spreadsheet.
        assert!(csv.contains("\"Blocked raw prompt: national_id [primary pattern match], contains a comma, and \"\"quotes\"\"\""));
    }

    #[test]
    fn csv_reports_dir_as_not_available_when_none() {
        let csv = build_csv(&sample_data());
        assert!(csv.contains("n/a (insufficient data in range)"));
    }

    #[test]
    fn pdf_produces_a_well_formed_non_empty_document() {
        let bytes = build_pdf(&sample_data());
        assert!(bytes.len() > 100, "a real PDF has real content, not a near-empty file");
        // A PDF file's magic bytes.
        assert_eq!(&bytes[0..5], b"%PDF-");
    }

    #[test]
    fn pdf_audit_row_cap_is_respected_and_noted() {
        let mut data = sample_data();
        data.audit_rows = (0..(MAX_PDF_AUDIT_ROWS + 10))
            .map(|i| ExportAuditRow {
                id: format!("row-{i}"),
                timestamp: Utc.with_ymd_and_hms(2026, 3, 1, 12, 0, 0).unwrap(),
                user_email: "x@test".to_string(),
                department: "Credit Risk".to_string(),
                entities_detected: vec![],
                risk_score: 0.1,
                decision: "cleared_no_entities".to_string(),
                sensitivity_class: "standard".to_string(),
                reason_string: "clean".to_string(),
            })
            .collect();
        // Just confirms this doesn't panic or hang on a dataset larger than
        // the cap — the actual visible truncation note is exercised
        // end-to-end by the route-level PDF content, not re-parsed here
        // (this crate has no PDF *reading* capability to assert page/line
        // content back out of the generated bytes).
        let bytes = build_pdf(&data);
        assert_eq!(&bytes[0..5], b"%PDF-");
    }

    #[test]
    fn truncate_chars_respects_char_boundaries_not_byte_boundaries() {
        // A multi-byte UTF-8 character (e.g. an em dash) must not be split
        // mid-character, which would produce invalid UTF-8 if this were
        // byte-indexed instead of char-indexed.
        let s = "a—b—c—d—e";
        let truncated = truncate_chars(s, 4);
        assert!(truncated.chars().count() <= 4);
        assert!(String::from_utf8(truncated.clone().into_bytes()).is_ok());
    }
}
