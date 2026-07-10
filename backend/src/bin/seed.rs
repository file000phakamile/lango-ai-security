//! Seed script — populates a freshly-migrated database with realistic sample
//! data so the dashboard has something to show immediately after setup,
//! instead of an empty database.
//!
//! This is the Phase-5 repurposing of `lib/lango/mock-data.ts`'s generator:
//! same spirit (deterministic seed, a spread of departments/languages/
//! decisions), but the audit-log rows here are produced by actually running
//! synthetic prompt text through the real detection engine
//! (`detection::scan::scan_prompt`) rather than fabricating a risk score and
//! decision directly. Only `security_events` (Phase 3 does not implement
//! live prompt-injection/rate-limit/DoS detection — see
//! docs/ARCHITECTURE.md) and `drift_snapshots`' weekly entity-count buckets
//! (no scheduled batch job exists yet) are illustrative constants; even
//! those still run through the real PSI/KL-divergence math in
//! `detection::drift`.
//!
//! Safe to re-run: truncates the six application tables (in FK-safe order)
//! before reseeding, so `cargo run --bin seed` is idempotent.

use chrono::{Datelike, Duration, NaiveDate, Utc};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

use lango_backend::{
    auth::hash_password,
    config::Config,
    detection::drift::{kl_divergence, normalize_counts, population_stability_index},
    detection::scan::{hash_prompt, response_scan_result_for, scan_prompt, NO_PROVIDER_MODEL_LABEL},
};

struct SeedUser {
    email: &'static str,
    password: &'static str,
    department: &'static str,
    role: &'static str,
}

/// The first two accounts are the demo credentials the frontend logs in
/// with when NEXT_PUBLIC_USE_MOCK_DATA is not set to "true" (see
/// lib/lango/api-client.ts). Documented in README.md and backend/.env.example.
const SEED_USERS: &[SeedUser] = &[
    SeedUser { email: "compliance@lango.demo", password: "LangoDemo123!", department: "Credit Risk", role: "compliance" },
    SeedUser { email: "admin@lango.demo", password: "LangoDemo123!", department: "Legal Affairs", role: "admin" },
    SeedUser { email: "staff1@lango.demo", password: "LangoDemo123!", department: "Credit Risk", role: "staff" },
    SeedUser { email: "staff2@lango.demo", password: "LangoDemo123!", department: "Credit Risk", role: "staff" },
    SeedUser { email: "staff3@lango.demo", password: "LangoDemo123!", department: "Claims Processing", role: "staff" },
    SeedUser { email: "staff4@lango.demo", password: "LangoDemo123!", department: "Claims Processing", role: "staff" },
    SeedUser { email: "staff5@lango.demo", password: "LangoDemo123!", department: "Patient Records", role: "staff" },
    SeedUser { email: "staff6@lango.demo", password: "LangoDemo123!", department: "Patient Records", role: "staff" },
    SeedUser { email: "staff7@lango.demo", password: "LangoDemo123!", department: "Bursar's Office", role: "staff" },
    SeedUser { email: "staff8@lango.demo", password: "LangoDemo123!", department: "Bursar's Office", role: "staff" },
    SeedUser { email: "staff9@lango.demo", password: "LangoDemo123!", department: "Legal Affairs", role: "staff" },
    SeedUser { email: "staff10@lango.demo", password: "LangoDemo123!", department: "Legal Affairs", role: "staff" },
];

const DEPARTMENTS: &[&str] = &[
    "Credit Risk",
    "Claims Processing",
    "Patient Records",
    "Bursar's Office",
    "Legal Affairs",
];
const LANGUAGES: &[&str] = &["English", "Ndebele", "Shona"];

/// Per-language probability that a generated prompt for that language carries
/// a PII-shaped entity rather than being clean. Deliberately unequal (English
/// highest, Shona lowest) so the seeded data exercises the Fairness Audit
/// view's Disparate Impact Ratio alert the same way the original mock data's
/// hardcoded 9.0/7.4/6.0 language flag-rate split did — except here it's a
/// real aggregate over real per-row detection results, not a hardcoded number.
fn language_pii_probability(language: &str) -> f64 {
    match language {
        "English" => 0.62,
        "Ndebele" => 0.50,
        "Shona" => 0.34,
        _ => 0.5,
    }
}

/// Same idea per department, roughly following the shape of the old mock's
/// DEPT_PARITY constants (Patient Records highest, Bursar's Office lowest).
fn department_pii_probability(department: &str) -> f64 {
    match department {
        "Patient Records" => 0.60,
        "Legal Affairs" => 0.56,
        "Credit Risk" => 0.52,
        "Claims Processing" => 0.48,
        "Bursar's Office" => 0.40,
        _ => 0.5,
    }
}

/// One synthetic prompt per department, containing a realistic-shaped PII
/// entity that the real regex/name-heuristic engine in `detection::rules`
/// and `detection::name_heuristic` will actually catch. Values are entirely
/// fabricated for seeding — not real customer data.
fn pii_prompt_for(department: &str, variant: usize) -> &'static str {
    match (department, variant % 4) {
        ("Credit Risk", 0) => "Please verify the applicant's national ID 63-123456A23 before approving the loan.",
        ("Credit Risk", 1) => "Call the applicant, Tendai Chikwanha, on 0771234567 to confirm income details.",
        ("Credit Risk", 2) => "Credit bureau reference for account 9988776655443 needs a manual review.",
        ("Credit Risk", _) => "Applicant card on file: 4111111111111111, confirm before disbursement.",
        ("Claims Processing", 0) => "Client Farai Mutasa reported an accident, contact number 0781234567.",
        ("Claims Processing", 1) => "Claimant national ID 71-654321B12 attached to claim file for review.",
        ("Claims Processing", 2) => "Refund the claimant via bank account 5566778899001 once approved.",
        ("Claims Processing", _) => "Settlement card charged: 5500005555555559, confirm payout status.",
        ("Patient Records", 0) => "Patient record MRN-204981 needs an update for Tendai Moyo.",
        ("Patient Records", 1) => "Contact patient on 0712345678 to reschedule the follow-up appointment.",
        ("Patient Records", 2) => "Patient national ID 58-234567C45 required for insurance pre-authorisation.",
        ("Patient Records", _) => "Attending physician note for MRN-118823, patient John Ncube.",
        ("Bursar's Office", 0) => "Student account 4111111111111111 needs a tuition refund processed.",
        ("Bursar's Office", 1) => "Parent contact for outstanding fees: 0771122334, please follow up.",
        ("Bursar's Office", 2) => "Scholarship recipient Rutendo Gumbo, national ID 44-112233D67.",
        ("Bursar's Office", _) => "Bursary disbursement to bank account 1122334455667 pending sign-off.",
        ("Legal Affairs", 0) => "Case file references bank account 9988776655443 for asset recovery.",
        ("Legal Affairs", 1) => "Opposing counsel contact: 0733445566, awaiting response on filing.",
        ("Legal Affairs", 2) => "Affidavit signed by Blessing Ndoro, national ID 39-887766E12.",
        ("Legal Affairs", _) => "Internal API token leaked in email thread: sk-liveTestKeyAbcdefghijklmnop123456",
        _ => "General inquiry with no sensitive detail attached.",
    }
}

/// Clean (no-entity) prompts — everyday questions with nothing to redact,
/// so `scan_prompt` legitimately returns `cleared_no_entities`.
const CLEAN_PROMPTS: &[&str] = &[
    "What is the current prime lending rate for personal loans?",
    "Summarise the key changes in the new claims-processing policy.",
    "Draft a polite reminder email about an overdue document submission.",
    "What are the standard turnaround times for a claim review?",
    "Explain the difference between a secured and unsecured loan.",
    "List the documents typically required to open a new account.",
    "Help me phrase a follow-up question about a pending application.",
    "What is our current policy on remote work for this department?",
];

/// A prompt that trips only the generic, deliberately low-confidence
/// fallback pattern (see `detection::rules::API_KEY_GENERIC_RE`) — this is
/// meant to exercise the fail-closed `blocked_low_confidence` path for real,
/// the same way `detection::scan`'s own unit test does.
const LOW_CONFIDENCE_PROMPT: &str = "Here is the token: aZ9xK2mQ7pL4vN8tR3wY6bC1dF5gH0jS2u, please use it.";

#[tokio::main]
async fn main() {
    let config = Config::from_env();

    println!("lango seed: connecting to {}", mask_db_url(&config.database_url));
    let db = PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.database_url)
        .await
        .expect("failed to connect to Postgres — is it running? see docker-compose.yml");

    println!("lango seed: running migrations");
    sqlx::migrate!("./migrations")
        .run(&db)
        .await
        .expect("failed to run database migrations");

    println!("lango seed: clearing existing data (safe to re-run)");
    sqlx::query(
        "TRUNCATE audit_log, security_events, drift_snapshots, sessions, detection_rules, users CASCADE",
    )
    .execute(&db)
    .await
    .expect("failed to truncate tables");

    let mut rng = StdRng::seed_from_u64(2026);

    // -----------------------------------------------------------------
    // Users + sessions
    // -----------------------------------------------------------------
    let mut user_ids: Vec<(Uuid, &'static str, &'static str)> = Vec::new(); // (id, department, role)
    let mut session_by_user: std::collections::HashMap<Uuid, Uuid> = std::collections::HashMap::new();

    for u in SEED_USERS {
        let password_hash = hash_password(u.password).expect("hash seed password");
        let user_id: Uuid = sqlx::query_scalar(
            "INSERT INTO users (email, password_hash, department, role) VALUES ($1, $2, $3, $4) RETURNING id",
        )
        .bind(u.email)
        .bind(password_hash)
        .bind(u.department)
        .bind(u.role)
        .fetch_one(&db)
        .await
        .expect("insert seed user");

        // Backdated session so it predates the historical audit_log rows
        // that reference it below — a real login session's expiry is 12h
        // (see auth::SESSION_TTL_HOURS), but seeded historical data needs a
        // session id that already existed when those rows' timestamps claim
        // requests happened, so this one is deliberately long-lived.
        let session_created = Utc::now() - Duration::days(35);
        let session_expires = Utc::now() + Duration::days(365);
        let session_id: Uuid = sqlx::query_scalar(
            "INSERT INTO sessions (user_id, created_at, expires_at) VALUES ($1, $2, $3) RETURNING id",
        )
        .bind(user_id)
        .bind(session_created)
        .bind(session_expires)
        .fetch_one(&db)
        .await
        .expect("insert seed session");

        user_ids.push((user_id, u.department, u.role));
        session_by_user.insert(user_id, session_id);
    }
    println!("lango seed: inserted {} users + sessions", user_ids.len());

    // -----------------------------------------------------------------
    // Detection rules — inspectable rows mirroring the compiled-in patterns
    // in detection::rules (see migration 0003's comment: v0.1 runs the
    // compiled patterns at request time, not these rows, but they exist so
    // the rule set is auditable and editable in a future admin UI).
    // -----------------------------------------------------------------
    let rule_rows: &[(&str, &str, &str)] = &[
        ("national_id", r"\b\d{2}-?\d{6,7}[A-Za-z]\d{2}\b", "regex"),
        ("phone_number", r"\b(?:\+263|0)7[1378]\d{7}\b", "regex"),
        (
            "credit_card",
            r"\b(?:4[0-9]{3}|5[1-5][0-9]{2}|3[47][0-9]{2})(?:[ -]?[0-9]{4}){2,3}\b|\b3[47][0-9]{13}\b (Luhn-checked)",
            "regex",
        ),
        (
            "api_key",
            r"\b(?:sk-[A-Za-z0-9]{20,}|AKIA[0-9A-Z]{16}|gh[pousr]_[A-Za-z0-9]{36,})\b",
            "regex",
        ),
        ("medical_record_no", r"(?i)\bMRN[-\s]?\d{5,8}\b", "regex"),
        ("bank_account", r"\b\d{10,13}\b", "regex"),
        (
            "api_key",
            r"generic 32+ char mixed alnum token (low confidence fallback)",
            "regex",
        ),
        (
            "full_name",
            "capitalized 2-3 word run, stopword-excluded (see detection::name_heuristic — heuristic stand-in for NER)",
            "ner",
        ),
    ];
    for (entity_type, pattern, rule_type) in rule_rows {
        sqlx::query(
            "INSERT INTO detection_rules (entity_type, pattern, rule_type, active) VALUES ($1, $2, $3, true)",
        )
        .bind(entity_type)
        .bind(pattern)
        .bind(rule_type)
        .execute(&db)
        .await
        .expect("insert detection rule");
    }
    println!("lango seed: inserted {} detection_rules rows", rule_rows.len());

    // -----------------------------------------------------------------
    // Audit log — real scan_prompt() output over synthetic prompts, spread
    // across the last ~34 hours (mirrors mock-data.ts's spacing) so several
    // rows land "today" for the Command Center KPIs, with older rows too.
    // -----------------------------------------------------------------
    let row_count = 60;
    let mut inserted = 0usize;
    let mut cursor = Utc::now();

    for i in 0..row_count {
        let department = DEPARTMENTS[i % DEPARTMENTS.len()];
        let language = LANGUAGES[rng.gen_range(0..LANGUAGES.len())];
        let candidates: Vec<(Uuid, Uuid)> = user_ids
            .iter()
            .filter(|(_, dept, _)| *dept == department)
            .map(|(id, _, _)| (*id, session_by_user[id]))
            .collect();
        let (user_id, session_id) = candidates[rng.gen_range(0..candidates.len())];

        let pii_probability = (language_pii_probability(language) + department_pii_probability(department)) / 2.0;
        let roll: f64 = rng.gen_range(0.0..1.0);
        let prompt = if roll < 0.06 {
            LOW_CONFIDENCE_PROMPT
        } else if roll < pii_probability {
            pii_prompt_for(department, i)
        } else {
            CLEAN_PROMPTS[rng.gen_range(0..CLEAN_PROMPTS.len())]
        };

        let outcome = scan_prompt(prompt);
        let original_prompt_hash = hash_prompt(prompt);
        let response_scan_result = response_scan_result_for(outcome.decision);
        let redacted_prompt_for_storage = if outcome.decision == "redacted_and_forwarded" {
            Some(outcome.redacted_prompt.clone())
        } else {
            None
        };
        let entities_json = serde_json::to_value(&outcome.entities_detected).expect("serialize entities");

        let gap_minutes = rng.gen_range(14..54);
        cursor -= Duration::minutes(gap_minutes);

        sqlx::query(
            r#"
            INSERT INTO audit_log (
                session_id, user_id, department, language, "timestamp",
                entities_detected, risk_score, decision, reason_string,
                ai_model_used, response_scan_result, original_prompt_hash, redacted_prompt
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            "#,
        )
        .bind(session_id)
        .bind(user_id)
        .bind(department)
        .bind(language)
        .bind(cursor)
        .bind(&entities_json)
        .bind(outcome.risk_score)
        .bind(outcome.decision)
        .bind(&outcome.reason_string)
        .bind(NO_PROVIDER_MODEL_LABEL)
        .bind(response_scan_result)
        .bind(&original_prompt_hash)
        .bind(&redacted_prompt_for_storage)
        .execute(&db)
        .await
        .expect("insert seed audit_log row");

        inserted += 1;
    }
    println!("lango seed: inserted {inserted} audit_log rows (real scan_prompt() output, not fabricated)");

    // -----------------------------------------------------------------
    // Security events — illustrative rows only; v0.1 has no live
    // prompt-injection/rate-limit/DoS detection to generate these for real
    // (see docs/ARCHITECTURE.md and migration 0005's comment).
    // -----------------------------------------------------------------
    let first_session = session_by_user.values().next().copied();
    let security_rows: &[(&str, &str, i64)] = &[
        ("prompt_injection_blocked", "System-instruction override attempt detected in user input, sanitised before AI Gateway.", 5),
        ("rate_limit_triggered", "Per-user token quota exceeded, request queued for next window.", 40),
        ("prompt_injection_blocked", "Delimiter-escape pattern detected and stripped from prompt.", 200),
        ("dos_mitigation_triggered", "Burst of 40 requests from single session in 10s, throttled.", 480),
        ("rate_limit_triggered", "Institution-level API quota at 92%, alert sent to ops.", 900),
        ("prompt_injection_blocked", "Encoded payload in code block flagged and rejected.", 1500),
    ];
    for (event_type, detail, minutes_ago) in security_rows {
        sqlx::query(
            "INSERT INTO security_events (event_type, detail, session_id, created_at) VALUES ($1, $2, $3, $4)",
        )
        .bind(event_type)
        .bind(detail)
        .bind(first_session)
        .bind(Utc::now() - Duration::minutes(*minutes_ago))
        .execute(&db)
        .await
        .expect("insert security event");
    }
    println!("lango seed: inserted {} security_events rows (illustrative, see docs/ARCHITECTURE.md)", security_rows.len());

    // -----------------------------------------------------------------
    // Drift snapshots — 12 weeks of entity-type distributions, with a
    // synthetic shift injected at week 9 (index 8), run through the real
    // PSI / KL-divergence math in detection::drift against a week-1
    // baseline. Category order: [national_id, credit_card, bank_account,
    // phone_number, api_key, medical_record_no, full_name].
    // -----------------------------------------------------------------
    let baseline_counts: [u32; 7] = [30, 10, 15, 25, 8, 5, 20];
    let baseline_dist = normalize_counts(&baseline_counts);

    let today = Utc::now().date_naive();
    let this_monday = today - Duration::days(today.weekday().num_days_from_monday() as i64);
    let first_week_start = this_monday - Duration::weeks(11);

    let mut drift_inserted = 0usize;
    for week in 0..12u32 {
        let week_start: NaiveDate = first_week_start + Duration::weeks(week as i64);
        let counts: [u32; 7] = if week == 8 {
            // Week 9: a new national-ID card format at one institution
            // causes a real distributional shift, as narrated in the
            // Drift & Security view.
            [95, 9, 14, 24, 7, 6, 19]
        } else {
            let mut jitter = |base: u32| -> u32 {
                let delta = rng.gen_range(-3i32..=3);
                (base as i32 + delta).max(1) as u32
            };
            [
                jitter(baseline_counts[0]),
                jitter(baseline_counts[1]),
                jitter(baseline_counts[2]),
                jitter(baseline_counts[3]),
                jitter(baseline_counts[4]),
                jitter(baseline_counts[5]),
                jitter(baseline_counts[6]),
            ]
        };
        let current_dist = normalize_counts(&counts);
        let psi = population_stability_index(&baseline_dist, &current_dist);
        let kl = kl_divergence(&current_dist, &baseline_dist);

        sqlx::query(
            "INSERT INTO drift_snapshots (week_start, psi_score, kl_divergence_score) VALUES ($1, $2, $3)",
        )
        .bind(week_start)
        .bind(psi as f32)
        .bind(kl as f32)
        .execute(&db)
        .await
        .expect("insert drift snapshot");
        drift_inserted += 1;
    }
    println!("lango seed: inserted {drift_inserted} drift_snapshots rows (real PSI/KL-divergence math)");

    println!();
    println!("=================================================================");
    println!("Seed complete. Demo login credentials (also in README.md):");
    println!("  compliance@lango.demo / LangoDemo123!  (role: compliance)");
    println!("  admin@lango.demo      / LangoDemo123!  (role: admin)");
    println!("=================================================================");
}

fn mask_db_url(url: &str) -> String {
    match url.split_once('@') {
        Some((_, host_part)) => format!("postgres://***:***@{host_part}"),
        None => "postgres://***".to_string(),
    }
}
