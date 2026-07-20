use sha2::{Digest, Sha256};

use super::{fallback, health_rules, name_heuristic, plain_language, rules};
use health_rules::SensitivityClass;

/// No live AI provider is connected in v0.1 (see backend/.env.example /
/// docs/ARCHITECTURE.md). Shared by the live /api/scan handler and the seed
/// script so both say the same honest thing rather than drifting apart.
pub const NO_PROVIDER_MODEL_LABEL: &str =
    "none — AI Gateway not connected to a live provider in v0.1";

/// SHA-256 hex digest of a prompt — the only trace of the original prompt
/// text this system ever stores (see audit_log.original_prompt_hash).
pub fn hash_prompt(prompt: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prompt.as_bytes());
    hex::encode(hasher.finalize())
}

/// The `response_scan_result` string for a given decision — shared so the
/// live handler and the seed script can't say different things about
/// whether a response was actually scanned (neither ever is, in v0.1).
pub fn response_scan_result_for(decision: &str) -> &'static str {
    if decision == "blocked_low_confidence" {
        "not sent - request blocked pre-gateway"
    } else {
        // Covers both "redacted_and_forwarded" and
        // "redacted_low_confidence_review" — both actually forward a
        // (redacted) prompt, so neither has anything to scan a response
        // from any more than the other does in v0.1.
        "not applicable - no live AI provider connected in v0.1, nothing was sent to scan"
    }
}

/// A single detected entity occurrence within a prompt. `sensitivity` is
/// stored per-match (not recomputed from `entity_type` on demand) because
/// the generic fallback detector (`fallback.rs`) infers sensitivity from
/// per-match keyword context, not from a fixed entity-type mapping —
/// `health_rules::sensitivity_class` remains the source of truth for every
/// OTHER detector's fixed mapping, but a single lookup function can't also
/// represent "it depends which keyword was actually nearby this specific
/// occurrence". `rule_detail` is the audit-trail string naming which
/// specific rule fired (primary pattern, checksum-verified, dictionary
/// match, or the generic fallback with its matched keyword) — see
/// `reason_string`'s doc comment on `ScanOutcome` for how this surfaces.
struct Match {
    /// Owned, not `&'static str`: custom patterns (policy builder) supply an
    /// organisation-defined label at request time, which cannot have a
    /// `'static` lifetime without leaking memory on every matching scan —
    /// see `resolve_overlaps`'s `FALLBACK_ENTITY_TYPE` comparison and every
    /// other use-site below, all of which work identically against an owned
    /// `String` (comparison, `Display`, `.to_string()`) as they did against
    /// `&'static str`.
    entity_type: String,
    start: usize,
    end: usize,
    confidence: f32,
    sensitivity: SensitivityClass,
    rule_detail: String,
}

/// Below this confidence, we don't trust a match enough to redact-and-forward
/// it without qualification. What happens next depends on entity type — see
/// `NAME_LOW_CONFIDENCE_FLOOR` below and the three-tier design note above
/// `scan_prompt`. This is the same "if detection confidence is low, block
/// rather than forward" principle stated in the proposal, now implemented as
/// real, non-random logic rather than the mock data's `rand() < 0.3` coin
/// flip — refined below into three tiers rather than a single cutoff, based
/// on real testing showing the single-cutoff version was overly blunt for
/// names specifically (see Questions.md).
pub const CONFIDENCE_THRESHOLD: f32 = 0.6;

/// Below this, even a `full_name` match is untrusted enough to block, the
/// same as any structured entity below `CONFIDENCE_THRESHOLD`. Between this
/// floor and `CONFIDENCE_THRESHOLD` (0.30-0.60) is a deliberate middle band
/// that exists ONLY for `full_name` — real testing showed
/// `name_heuristic.rs`'s false-positive rate on ordinary capitalized phrases
/// (see its own doc comment) meant blocking the entire request on every
/// borderline name match was costing real workflow friction for very little
/// actual safety benefit, since a wrongly-redacted ordinary word is a much
/// smaller harm than a wrongly-redacted national ID or account number. So in
/// that middle band, a name match is redacted and forwarded automatically —
/// not blocked — but the decision is tagged `redacted_low_confidence_review`
/// (distinct from ordinary `redacted_and_forwarded`) precisely so a human can
/// audit these later. This exception is intentionally narrow: it does NOT
/// extend to any structured entity type (`national_id`, `bank_account`,
/// `phone_number`, `credit_card`, `medical_record_no`, `api_key`) — an
/// uncertain match on a structured pattern is more likely to be a real
/// entity in an unexpected format than a false positive the way an uncertain
/// name match usually is, so those keep blocking below
/// `CONFIDENCE_THRESHOLD` with no middle band, unchanged from before this
/// change. See docs/SECURITY_PRIVACY.md for the compliance framing of this
/// tradeoff and Questions.md for the full reasoning behind the 0.30 floor
/// specifically (chosen as roughly half of `CONFIDENCE_THRESHOLD`; the name
/// heuristic in this codebase today only ever emits a single fixed
/// confidence of 0.55, comfortably inside this band, so this floor is
/// currently more a statement of intent for if/when that heuristic gains
/// real confidence gradation than something reachable today).
const NAME_LOW_CONFIDENCE_FLOOR: f32 = 0.30;

// ---------------------------------------------------------------------------
// Policy builder (product-depth task, Part 1) — per-organisation confidence
// threshold + custom structured-identifier patterns.
// ---------------------------------------------------------------------------

/// Safe lower bound for an organisation's configurable `confidence_threshold`
/// (see `ScanConfig`). Deliberately well above `NAME_LOW_CONFIDENCE_FLOOR`
/// (0.30) — that floor is NOT configurable and this bound exists precisely
/// so no organisation setting can ever approach it. Also chosen so the
/// deliberately-low-confidence structured patterns already in this codebase
/// (`bank_account` 0.50, `medical_aid_number` 0.55) can never be configured
/// to always pass just because an org set their threshold to something
/// absurdly low like 0.05 — 0.50 is the lowest any real primary-pattern
/// confidence in this codebase goes, so the floor matches that intentionally.
pub const MIN_ORG_CONFIDENCE_THRESHOLD: f32 = 0.50;

/// Safe upper bound — high enough to let a compliance_admin meaningfully
/// tighten detection, but never 1.0 (which would make every match "low
/// confidence" and block everything, a self-inflicted denial of service on
/// their own staff, not a real security posture).
pub const MAX_ORG_CONFIDENCE_THRESHOLD: f32 = 0.95;

/// Maximum accepted length of a custom pattern's regex source text — not a
/// ReDoS defense (Rust's `regex` crate is structurally immune to
/// catastrophic backtracking, guaranteed linear-time; see
/// `detection/mod.rs`), just a sane bound against pathological input driving
/// up compiled-program size or wasting review time.
pub const MAX_CUSTOM_PATTERN_LENGTH: usize = 200;

/// An organisation-specific structured-identifier pattern (policy builder).
/// `regex` is pre-compiled once when the config is built for a request, not
/// per-match — see `routes/policy.rs` for where `pattern` text is validated
/// (must compile, must stay under a bounded compiled-program size) before
/// ever being stored.
#[derive(Clone)]
pub struct CustomPattern {
    pub entity_label: String,
    pub regex: regex::Regex,
    pub confidence: f32,
}

/// Per-organisation scan configuration. `Default` reproduces the fixed,
/// pre-policy-builder behavior exactly (the module constant
/// `CONFIDENCE_THRESHOLD`, no custom patterns) — so `scan_prompt` below,
/// used by every existing test and `seed.rs`, is completely unaffected by
/// this struct existing. Only `routes/scan.rs` (the live request path)
/// builds a non-default `ScanConfig`, from the calling organisation's
/// database row.
#[derive(Clone)]
pub struct ScanConfig {
    pub confidence_threshold: f32,
    pub custom_patterns: Vec<CustomPattern>,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            confidence_threshold: CONFIDENCE_THRESHOLD,
            custom_patterns: Vec::new(),
        }
    }
}

/// Per-entity-type severity weight, used to build the 0-1 risk score.
/// Reflects roughly how damaging exposure of that entity type is — a
/// national ID or credit card number is more sensitive than a phone number.
/// These weights are a product judgment call, not derived from an external
/// standard; documented here so they're easy to challenge and retune.
fn severity(entity_type: &str) -> f32 {
    match entity_type {
        "national_id" => 0.35,
        "credit_card" => 0.35,
        "diagnosis_code" => 0.35, // special-category health data — same weight as the most sensitive standard types
        "bank_account" => 0.30,
        "api_key" => 0.30,
        "medication_name" => 0.30,
        "medical_aid_number" => 0.30,
        "lab_result_value" => 0.30,
        "medical_record_no" => 0.25,
        // Format-unverified by construction (see fallback.rs) — weighted
        // between medical_record_no and phone_number: real enough to be
        // worth more than a bare phone number, but never format-validated
        // the way e.g. credit_card's Luhn check is.
        "probable_identifier" => 0.25,
        "phone_number" => 0.20,
        "next_of_kin" => 0.20,
        "full_name" => 0.15,
        _ => 0.15,
    }
}

/// Row-level sensitivity classification: a prompt is
/// `SpecialCategoryHealth` if ANY detected entity in it is, regardless of
/// how many other (standard) entities are also present — the same
/// "one sensitive thing is enough" logic already implicit in how
/// `entities_detected` is reported. Used to populate `audit_log.sensitivity_class`
/// (see Part 2 of the task this module was built for) so aggregate reporting
/// can count special-category detections without needing to inspect the
/// `entities_detected` JSON array at query time.
fn row_sensitivity_class(sensitivities: &[SensitivityClass]) -> &'static str {
    if sensitivities
        .iter()
        .any(|s| *s == SensitivityClass::SpecialCategoryHealth)
    {
        SensitivityClass::SpecialCategoryHealth.as_str()
    } else {
        SensitivityClass::Standard.as_str()
    }
}

pub struct ScanOutcome {
    pub entities_detected: Vec<String>,
    pub risk_score: f32,
    pub redacted_prompt: String,
    pub decision: &'static str,
    /// Full technical detail — entity type names, confidence scores, which
    /// specific rule/detector fired (see `Match::rule_detail`). For the
    /// audit log and a compliance officer reviewing a decision later. NEVER
    /// shown directly to the person who submitted the prompt — see
    /// `user_message` below for that.
    pub reason_string: String,
    /// Short, plain-language explanation of what kind of information was
    /// involved and why the scanner was cautious — no entity_type strings,
    /// no confidence numbers, no detector/rule names (see
    /// `plain_language.rs`). This is what the browser extension shows in
    /// its banner to the person who just typed the prompt; `reason_string`
    /// above is the detailed counterpart for whoever audits the decision
    /// later. The split is deliberate: two different readers, two different
    /// levels of detail, from the same underlying match data.
    pub user_message: String,
    /// "standard" or "special_category_health" — see
    /// `health_rules::SensitivityClass`. A NEW, independent axis from
    /// `decision`/confidence: this reflects the sensitivity of the entity
    /// *category* detected, not how confident the scanner was.
    pub sensitivity_class: &'static str,
}

pub fn scan_prompt(prompt: &str) -> ScanOutcome {
    scan_prompt_with_config(prompt, &ScanConfig::default())
}

/// Same detection pipeline as `scan_prompt`, but with the two policy-builder
/// knobs: `config.confidence_threshold` in place of the fixed
/// `CONFIDENCE_THRESHOLD` module constant, and `config.custom_patterns`
/// matched alongside (never instead of) the built-in detectors. Everything
/// else — `NAME_LOW_CONFIDENCE_FLOOR`, the special_category_health leniency
/// hard rule, overlap resolution, redaction — is completely unaffected by
/// `config`, exactly as the task requires ("that rule is not configurable,
/// it stays absolute").
pub fn scan_prompt_with_config(prompt: &str, config: &ScanConfig) -> ScanOutcome {
    let matches: Vec<Match> = detect_all(prompt, config);
    build_prompt_outcome(prompt, matches, config.confidence_threshold)
}

/// Runs every detector (built-in regex rules, health-specific detectors,
/// the name heuristic, the generic structured-identifier fallback, and any
/// organisation custom patterns) and resolves overlaps — the detection step
/// shared by BOTH the prompt-scanning path (`scan_prompt_with_config`,
/// above) and the response-scanning path (`scan_response`, below). Response
/// scanning (product-depth task, "response scanning + observability +
/// hardening") intentionally reuses this exact detection pipeline rather
/// than a separate, parallel one: a leaked national ID or API key is
/// exactly as real a finding in an AI's reply as in a user's prompt, and
/// duplicating the detector list would risk the two silently drifting
/// apart over time. What differs between the two callers is what happens
/// AFTER detection — the prompt path applies the three-tier confidence
/// gating and redaction/blocking decision below; the response path
/// (`scan_response`) does not, since there is no "forward or block"
/// decision to make about a reply the user has already seen render — see
/// that function's own doc comment.
fn detect_all(prompt: &str, config: &ScanConfig) -> Vec<Match> {
    // Every detector below pushes CANDIDATE matches into one flat list,
    // with no per-detector overlap skipping — overlap resolution happens
    // exactly once, after every detector has had a chance to see the whole
    // prompt (`resolve_overlaps`, below the tests module doc comment for
    // its full reasoning). This replaces the previous ad-hoc
    // "each detector skips whatever an earlier detector already claimed"
    // approach, which was really priority-by-insertion-order dressed up as
    // overlap handling, not genuine confidence comparison.
    let mut candidates: Vec<Match> = Vec::new();

    // 1. Regex-based rules (rules.rs) — primary, specific-format patterns.
    // `rule.checksum`, when present, is a real validation (currently only
    // credit_card's Luhn check) — a format match that fails it is very
    // likely a false positive (e.g. an unrelated 16-digit reference number)
    // and is dropped entirely rather than reported at a lower confidence.
    for rule in rules::regex_rules() {
        for m in rule.regex.find_iter(prompt) {
            if let Some(checksum) = rule.checksum {
                if !checksum(m.as_str()) {
                    continue;
                }
            }
            candidates.push(Match {
                entity_type: rule.entity_type.to_string(),
                start: m.start(),
                end: m.end(),
                confidence: rule.confidence,
                sensitivity: health_rules::sensitivity_class(rule.entity_type),
                rule_detail: if rule.checksum.is_some() {
                    "primary pattern match, checksum-verified".to_string()
                } else {
                    "primary pattern match".to_string()
                },
            });
        }
    }

    // 2. Health-specific detectors (health_rules.rs) — each has its own
    // gating mechanism (ICD-10 dictionary, medication-name dictionary, lab-
    // test keyword proximity), named explicitly in `rule_detail` per
    // detector so the audit trail says WHICH gate fired, not just the
    // entity type.
    for hm in health_rules::detect_diagnosis_codes(prompt) {
        candidates.push(Match {
            entity_type: hm.entity_type.to_string(),
            start: hm.start,
            end: hm.end,
            confidence: hm.confidence,
            sensitivity: health_rules::sensitivity_class(hm.entity_type),
            rule_detail: "primary pattern match, ICD-10 shape + dictionary-gated".to_string(),
        });
    }
    for hm in health_rules::detect_medications(prompt) {
        candidates.push(Match {
            entity_type: hm.entity_type.to_string(),
            start: hm.start,
            end: hm.end,
            confidence: hm.confidence,
            sensitivity: health_rules::sensitivity_class(hm.entity_type),
            rule_detail: "primary pattern match, medication-name dictionary".to_string(),
        });
    }
    for hm in health_rules::detect_medical_aid_numbers(prompt) {
        candidates.push(Match {
            entity_type: hm.entity_type.to_string(),
            start: hm.start,
            end: hm.end,
            confidence: hm.confidence,
            sensitivity: health_rules::sensitivity_class(hm.entity_type),
            rule_detail: "primary pattern match, generic format (ungated)".to_string(),
        });
    }
    for hm in health_rules::detect_lab_result_values(prompt) {
        candidates.push(Match {
            entity_type: hm.entity_type.to_string(),
            start: hm.start,
            end: hm.end,
            confidence: hm.confidence,
            sensitivity: health_rules::sensitivity_class(hm.entity_type),
            rule_detail: "keyword-gated numeric match, lab-test-name proximity".to_string(),
        });
    }

    // 3. Name heuristic — see name_heuristic.rs for the honesty note on
    // what this actually is (not real NER). A name near a next-of-kin/
    // emergency-contact/guardian keyword is tagged `next_of_kin` instead of
    // plain `full_name` (see health_rules::is_next_of_kin_context) — a
    // contextual reclassification, not a separate name-finding pass.
    for name in name_heuristic::detect_names(prompt) {
        let is_next_of_kin = health_rules::is_next_of_kin_context(prompt, name.start, name.end);
        let entity_type = if is_next_of_kin { "next_of_kin" } else { "full_name" };
        candidates.push(Match {
            entity_type: entity_type.to_string(),
            start: name.start,
            end: name.end,
            confidence: 0.55, // heuristic, not a real NER model — deliberately capped low
            sensitivity: health_rules::sensitivity_class(entity_type),
            rule_detail: if is_next_of_kin {
                "capitalized-run heuristic match, next-of-kin context".to_string()
            } else {
                "capitalized-run heuristic match".to_string()
            },
        });
    }

    // 4. Generic structured-identifier fallback (fallback.rs) — THE fix for
    // the "12345678ACD" gap: any ID-shaped token near a recognized
    // identifying keyword, from ANY entity type's keyword list, that no
    // detector above already caught. `sensitivity` here is per-match
    // (inferred from the specific nearby keyword), not the fixed
    // entity-type mapping every other detector above uses — see the `Match`
    // struct's own doc comment for why that's necessary.
    for fm in fallback::detect_structured_identifiers(prompt) {
        candidates.push(Match {
            entity_type: fallback::FALLBACK_ENTITY_TYPE.to_string(),
            start: fm.start,
            end: fm.end,
            confidence: fm.confidence,
            sensitivity: fm.sensitivity,
            rule_detail: fm.rule_detail,
        });
    }

    // 5. Organisation-specific custom patterns (policy builder, product-depth
    // task Part 1) — matched exactly like the built-in regex rules in step 1
    // (same `find_iter` shape, same candidate-list path), so they compete on
    // confidence through the SAME `resolve_overlaps` a specific detector
    // does, rather than being bolted on as a separate lower- or higher-
    // priority pass. `sensitivity_class` for a custom label always falls
    // through to `health_rules::sensitivity_class`'s default arm (Standard)
    // since an org cannot name a custom pattern one of the five fixed
    // special_category_health entity types (rejected at creation time in
    // routes/policy.rs) — so a custom pattern can never accidentally reach
    // special-category status or its leniency exclusion; it just never
    // qualifies for `is_leniency_eligible` either way, being a structured
    // (non-`full_name`) entity type.
    for cp in &config.custom_patterns {
        for m in cp.regex.find_iter(prompt) {
            candidates.push(Match {
                entity_type: cp.entity_label.clone(),
                start: m.start(),
                end: m.end(),
                confidence: cp.confidence,
                sensitivity: health_rules::sensitivity_class(&cp.entity_label),
                rule_detail: "organisation custom pattern match".to_string(),
            });
        }
    }

    let mut matches: Vec<Match> = resolve_overlaps(candidates);
    matches.sort_by_key(|m| m.start);
    matches
}

/// The prompt-specific decisioning step: three-tier confidence gating,
/// redaction, and the fail-closed blocking logic — everything that happens
/// once `detect_all` has already produced the match list. Split out from
/// `scan_prompt_with_config` only so `detect_all` can be shared with
/// `scan_response` below; the logic itself is completely unchanged from
/// before this split.
fn build_prompt_outcome(prompt: &str, matches: Vec<Match>, confidence_threshold: f32) -> ScanOutcome {
    if matches.is_empty() {
        return ScanOutcome {
            entities_detected: vec![],
            risk_score: 0.05,
            redacted_prompt: prompt.to_string(),
            decision: "cleared_no_entities",
            reason_string: "No sensitive entities detected. Prompt forwarded unmodified."
                .to_string(),
            user_message: "No sensitive information was found in this message.".to_string(),
            sensitivity_class: SensitivityClass::Standard.as_str(),
        };
    }

    let risk_score: f32 = matches
        .iter()
        .map(|m| severity(&m.entity_type))
        .sum::<f32>()
        .min(1.0);

    let entities_detected: Vec<String> = matches.iter().map(|m| m.entity_type.to_string()).collect();
    let sensitivities: Vec<SensitivityClass> = matches.iter().map(|m| m.sensitivity).collect();
    let sensitivity_class = row_sensitivity_class(&sensitivities);

    // Three-tier, entity-type-aware confidence handling. Structured entities
    // and `full_name` are judged separately, then combined with structured
    // entities taking priority — a low-confidence structured match blocks
    // the whole request regardless of how confident any name match in the
    // same prompt is.
    //
    // HARD RULE (Part 2 of the task that added the sensitivity-class axis):
    // the tier-2 leniency band below — redact-and-forward-but-flag-for-
    // review — exists ONLY for a `full_name` match that is also `Standard`
    // sensitivity. A `special_category_health` match is NEVER eligible for
    // it, even if some future entity type happened to also be literally
    // named "full_name" — checked explicitly via `sensitivity_class`
    // instead of relying only on the entity_type string comparison, so this
    // guarantee holds structurally rather than by naming convention. See
    // `low_confidence_special_category_health_never_gets_review_flag_blocks_instead`
    // below for the regression test this is designed to satisfy.
    let is_leniency_eligible =
        |entity_type: &str, sensitivity: SensitivityClass| entity_type == "full_name" && sensitivity != SensitivityClass::SpecialCategoryHealth;
    let min_structured_confidence = matches
        .iter()
        .filter(|m| !is_leniency_eligible(&m.entity_type, m.sensitivity))
        .map(|m| m.confidence)
        .fold(f32::INFINITY, f32::min);
    let min_name_confidence = matches
        .iter()
        .filter(|m| is_leniency_eligible(&m.entity_type, m.sensitivity))
        .map(|m| m.confidence)
        .fold(f32::INFINITY, f32::min);

    // Tier 3a: any structured entity below the org's configured
    // confidence_threshold blocks, unconditionally — no middle band for
    // these types (see NAME_LOW_CONFIDENCE_FLOOR's doc comment for why).
    if min_structured_confidence < confidence_threshold {
        let low_confidence_matches: Vec<&Match> = matches
            .iter()
            .filter(|m| !is_leniency_eligible(&m.entity_type, m.sensitivity) && m.confidence < confidence_threshold)
            .collect();
        let low_confidence_types: Vec<String> = low_confidence_matches
            .iter()
            .map(|m| format!("{} [{}]", m.entity_type, m.rule_detail))
            .collect();
        let low_confidence_entity_types: Vec<&str> =
            low_confidence_matches.iter().map(|m| m.entity_type.as_str()).collect();
        return blocked_outcome(
            entities_detected,
            risk_score,
            prompt,
            min_structured_confidence,
            confidence_threshold,
            &low_confidence_types,
            &low_confidence_entity_types,
            sensitivity_class,
        );
    }

    // Tier 3b: a full_name match too unreliable even for the relaxed
    // tier-2 handling below.
    if min_name_confidence < NAME_LOW_CONFIDENCE_FLOOR {
        return blocked_outcome(
            entities_detected,
            risk_score,
            prompt,
            min_name_confidence,
            NAME_LOW_CONFIDENCE_FLOOR,
            &["full_name (capitalized-run heuristic match)".to_string()],
            &["full_name"],
            sensitivity_class,
        );
    }

    // Everything from here on forwards a redacted prompt — the redaction
    // step itself is identical whether the result ends up
    // `redacted_and_forwarded` or `redacted_low_confidence_review`; only the
    // decision label and reason_string/user_message differ.
    let redacted = redact(prompt, &matches);

    // Tier 2: a real but low-confidence name match (0.30-0.60), and nothing
    // structured is below threshold (tier 3a already returned above if it
    // were). Redact and forward anyway rather than blocking, flagged
    // distinctly for async compliance review — see NAME_LOW_CONFIDENCE_FLOOR.
    if min_name_confidence < confidence_threshold {
        return ScanOutcome {
            entities_detected,
            risk_score,
            redacted_prompt: redacted,
            decision: "redacted_low_confidence_review",
            reason_string: format!(
                "Low-confidence name match ({:.2}) redacted automatically - flagged for compliance review",
                min_name_confidence
            ),
            user_message: "This message may have contained a person's name we weren't fully \
                confident about. It was redacted and sent, and flagged for a compliance review."
                .to_string(),
            sensitivity_class,
        };
    }

    // Tier 1: everything trusted. Per-match rule detail (point 7 of the
    // detection-engine task) so the AUDIT TRAIL (reason_string) names WHICH
    // rule fired for each entity, not just the entity type — e.g.
    // distinguishing a primary-pattern national_id match from a
    // probable_identifier caught only by the generic fallback. user_message
    // stays plain-language, built from the same matches via
    // `plain_language::describe`.
    let detail_join = matches
        .iter()
        .map(|m| format!("{} [{}]", m.entity_type, m.rule_detail))
        .collect::<Vec<_>>()
        .join("; ");
    let matched_entity_types: Vec<&str> = matches.iter().map(|m| m.entity_type.as_str()).collect();
    ScanOutcome {
        entities_detected: entities_detected.clone(),
        risk_score,
        redacted_prompt: redacted,
        decision: "redacted_and_forwarded",
        reason_string: format!(
            "Blocked raw prompt: {}, replaced with placeholder tokens.",
            detail_join
        ),
        user_message: format!(
            "This message contained {}, which {} automatically redacted before sending.",
            plain_language::describe(&matched_entity_types),
            if plain_language::unique_phrase_count(&matched_entity_types) == 1 { "was" } else { "were" },
        ),
        sensitivity_class,
    }
}

// ---------------------------------------------------------------------------
// Response scanning ("response scanning + observability + hardening" task,
// Part 1) — the second half of the pipeline. Scans an AI provider's reply
// (captured client-side by the browser extension after it finishes
// rendering — see extension/content/response-scanner.js) for the same
// sensitive entities/leaked-secret patterns the prompt side detects.
//
// DELIBERATELY NOT the same three-tier confidence/redact/block model
// `build_prompt_outcome` implements. That model exists to decide "should
// THIS user's own outgoing text be forwarded, redacted, or blocked before
// it leaves the browser" — a decision that only makes sense before the
// text has gone anywhere. A response has already rendered in the user's
// browser by the time this function ever sees it; there is no "forward or
// block" step left to gate. The only real decision left is binary: does
// this response contain anything worth warning the user about, yes or no.
// See `scan_response`'s own doc comment for the fuller design reasoning,
// and docs/ARCHITECTURE.md for why silently modifying the AI's actual
// response was ruled out entirely, not just for this simpler-outcome
// reason.
pub struct ResponseScanOutcome {
    pub entities_detected: Vec<String>,
    pub risk_score: f32,
    pub sensitivity_class: &'static str,
    pub flagged: bool,
    /// Plain-language banner text — same honesty/no-jargon standard as
    /// `ScanOutcome::user_message` (see `plain_language.rs`), reused
    /// directly for the entity-naming part of this sentence so the two
    /// surfaces can't drift on how they describe the same entity types.
    pub user_message: String,
}

/// Scans response text (an AI provider's rendered reply, captured
/// client-side) for the same entities the prompt-side detectors already
/// catch — national IDs, bank accounts, API keys/secrets, health data, etc.
/// — via the exact same `detect_all` pipeline `scan_prompt_with_config`
/// uses, so response scanning benefits from every existing detector (and
/// every future one) with zero duplicated matching logic.
///
/// **Design decision, stated here and in docs/ARCHITECTURE.md**: this
/// function never modifies or redacts the scanned text, and there is no
/// "redacted_response" concept anywhere in this codebase. A flagged
/// response is surfaced to the user as a warning banner (see
/// `user_message` below and `extension/content/response-scanner.js`) —
/// the AI's actual reply, exactly as it rendered, is left alone.
/// Covertly rewriting or hiding content the user did not write themselves,
/// the moment after they've already been shown it, is a materially
/// different and more concerning kind of intervention than redacting a
/// prompt before it's ever sent — it would mean the tool silently deciding
/// what a person is and isn't allowed to read, rather than protecting what
/// they send. Redacting an outgoing prompt prevents a leak that hasn't
/// happened yet; silently altering a received response accepts that the
/// content already reached the user and then also lies to them about what
/// they were told. This codebase's fail-closed principle is about
/// preventing sensitive data from leaving the organisation, not about
/// filtering what a user is allowed to read — the two are not the same
/// problem and do not warrant the same mechanism.
pub fn scan_response(text: &str, config: &ScanConfig) -> ResponseScanOutcome {
    let matches: Vec<Match> = detect_all(text, config);

    if matches.is_empty() {
        return ResponseScanOutcome {
            entities_detected: vec![],
            risk_score: 0.0,
            sensitivity_class: SensitivityClass::Standard.as_str(),
            flagged: false,
            user_message: "No sensitive information was found in this response.".to_string(),
        };
    }

    let risk_score: f32 = matches.iter().map(|m| severity(&m.entity_type)).sum::<f32>().min(1.0);
    let entities_detected: Vec<String> = matches.iter().map(|m| m.entity_type.to_string()).collect();
    let sensitivities: Vec<SensitivityClass> = matches.iter().map(|m| m.sensitivity).collect();
    let sensitivity_class = row_sensitivity_class(&sensitivities);
    let entity_type_refs: Vec<&str> = matches.iter().map(|m| m.entity_type.as_str()).collect();

    ResponseScanOutcome {
        entities_detected,
        risk_score,
        sensitivity_class,
        flagged: true,
        user_message: format!(
            "This response may contain {}. Review it carefully before using or sharing it.",
            plain_language::describe(&entity_type_refs),
        ),
    }
}

/// Builds a `blocked_low_confidence` outcome. Shared by both blocking tiers
/// (a low-confidence structured entity, or a near-zero-confidence name) so
/// the fail-closed behavior — the original, unredacted prompt is never
/// forwarded — can't drift between the two call sites.
fn blocked_outcome(
    entities_detected: Vec<String>,
    risk_score: f32,
    prompt: &str,
    min_confidence: f32,
    threshold: f32,
    low_confidence_detail: &[String],
    low_confidence_entity_types: &[&str],
    sensitivity_class: &'static str,
) -> ScanOutcome {
    ScanOutcome {
        entities_detected,
        risk_score,
        redacted_prompt: prompt.to_string(),
        decision: "blocked_low_confidence",
        sensitivity_class,
        // Full technical detail — entity types, confidence numbers, which
        // rule matched — for the audit log only. See `user_message` below
        // for what the person who submitted the prompt actually sees.
        reason_string: format!(
            "Scanner confidence below threshold ({:.2} < {:.2}) on detected {}. Fail-closed triggered.",
            min_confidence,
            threshold,
            low_confidence_detail.join(", ")
        ),
        // Plain language, no entity_type strings, no confidence math, no
        // detector names — see plain_language.rs's own doc comment for why
        // this split exists.
        user_message: format!(
            "This message may contain {} we're not confident about. Please review and remove or \
                rephrase it before sending.",
            plain_language::describe(low_confidence_entity_types),
        ),
    }
}

/// Redacts highest-offset matches first so earlier byte offsets in the
/// string stay valid as replacement tokens are spliced in.
fn redact(prompt: &str, matches: &[Match]) -> String {
    let mut redacted = prompt.to_string();
    let mut sorted_desc = matches.iter().collect::<Vec<_>>();
    sorted_desc.sort_by_key(|m| std::cmp::Reverse(m.start));
    for m in sorted_desc {
        let placeholder = format!("[REDACTED:{}]", m.entity_type.to_uppercase());
        redacted.replace_range(m.start..m.end, &placeholder);
    }
    redacted
}

fn ranges_overlap(a_start: usize, a_end: usize, b_start: usize, b_end: usize) -> bool {
    a_start < b_end && b_start < a_end
}

/// Resolves overlapping/adjacent candidate matches from every detector:
/// the highest-confidence candidate wins each contested span, and every
/// remaining non-overlapping candidate redacts independently (task point
/// 4). Implemented as a greedy interval-scheduling pass over candidates
/// sorted by confidence descending — accept a candidate unless it overlaps
/// something already accepted.
///
/// ONE deliberate refinement beyond a literal "highest number wins" rule,
/// documented here as the judgment call it is: `fallback::FALLBACK_ENTITY_TYPE`
/// candidates are always sorted AFTER every other candidate, regardless of
/// their own numeric confidence. Why: the fallback's confidence (0.70, see
/// `fallback.rs`'s doc comment) is deliberately set high enough to
/// redact-and-forward on its own when it's the ONLY signal for a span —
/// that's the actual bug this module fixes ("12345678ACD" matches nothing
/// else). But several existing specific-format detectors are deliberately
/// LOW-confidence by design specifically so an unverified generic-shaped
/// match fails closed for human review (`medical_aid_number` at 0.55,
/// `bank_account` at 0.50) — and that detector's own keyword commonly
/// appears right next to its match in ordinary phrasing (e.g. "medical aid
/// number: X"). A literal numeric comparison would let the fallback
/// systematically outrank those detectors' own deliberate fail-closed
/// tuning on the common case, not just fill the gap it was built for.
/// Always treating the fallback as lowest priority on overlap preserves
/// both: unopposed, it applies its own confidence to a genuinely
/// unclaimed span (the bug-report case); opposed, the purpose-built
/// detector's existing tuning wins (see
/// `medical_aid_number_fallback_does_not_override_the_deliberately_low_confidence_primary_pattern`
/// in the tests below).
fn resolve_overlaps(mut candidates: Vec<Match>) -> Vec<Match> {
    candidates.sort_by(|a, b| {
        let a_is_fallback = a.entity_type == fallback::FALLBACK_ENTITY_TYPE;
        let b_is_fallback = b.entity_type == fallback::FALLBACK_ENTITY_TYPE;
        a_is_fallback
            .cmp(&b_is_fallback)
            .then_with(|| b.confidence.total_cmp(&a.confidence))
    });
    let mut accepted: Vec<Match> = Vec::new();
    for candidate in candidates {
        let overlaps_existing = accepted
            .iter()
            .any(|m| ranges_overlap(m.start, m.end, candidate.start, candidate.end));
        if !overlaps_existing {
            accepted.push(candidate);
        }
    }
    accepted
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Response scanning ("response scanning + observability +
    // hardening" task, Part 1) ---------------------------------------------

    #[test]
    fn clean_response_is_not_flagged() {
        let outcome = scan_response("The capital of France is Paris.", &ScanConfig::default());
        assert!(!outcome.flagged);
        assert!(outcome.entities_detected.is_empty());
        assert_eq!(outcome.risk_score, 0.0);
    }

    #[test]
    fn response_containing_a_national_id_is_flagged() {
        // Same detector, same pattern as the prompt-side test
        // (national_id_is_redacted below) — proves response scanning
        // reuses the identical detection pipeline, not a separate one.
        let outcome = scan_response(
            "Sure — the customer's national ID on file is 63-123456A23.",
            &ScanConfig::default(),
        );
        assert!(outcome.flagged);
        assert!(outcome.entities_detected.contains(&"national_id".to_string()));
        assert!(outcome.user_message.to_lowercase().contains("national id"));
        // Plain language only — no raw entity_type string in the banner text.
        assert!(!outcome.user_message.contains("national_id"));
    }

    #[test]
    fn response_scan_never_modifies_the_text_it_scans() {
        // There is no redacted-response concept anywhere in this codebase —
        // scan_response takes text by shared reference and returns no
        // transformed copy of it at all (ResponseScanOutcome has no
        // "redacted_response" field), so this is really a compile-time
        // guarantee. This test exists as the explicit, named regression
        // lock for that design decision (see scan_response's own doc
        // comment and docs/ARCHITECTURE.md).
        let original = "Sure — the customer's national ID on file is 63-123456A23.";
        let _ = scan_response(original, &ScanConfig::default());
        // If this compiled and `original` is still usable here unchanged,
        // scan_response did not consume or mutate it.
        assert_eq!(original, "Sure — the customer's national ID on file is 63-123456A23.");
    }

    #[test]
    fn response_flagging_respects_the_organisations_confidence_threshold() {
        // bank_account's primary pattern is 0.50 confidence — below the
        // system default (0.60). scan_response has no confidence-threshold
        // gating of its own (unlike the prompt path); a match is flagged
        // purely on being detected at all, regardless of confidence — so
        // this is actually a test that the ORG'S threshold setting has NO
        // effect on response flagging, a deliberate simplification stated
        // in scan_response's own doc comment ("no forward-or-block decision
        // to gate").
        let lowered_config = ScanConfig { confidence_threshold: MIN_ORG_CONFIDENCE_THRESHOLD, custom_patterns: vec![] };
        let default_outcome = scan_response("Refund account 9988776655443 please.", &ScanConfig::default());
        let lowered_outcome = scan_response("Refund account 9988776655443 please.", &lowered_config);
        assert!(default_outcome.flagged);
        assert!(lowered_outcome.flagged);
        assert_eq!(default_outcome.entities_detected, lowered_outcome.entities_detected);
    }

    #[test]
    fn response_scan_reuses_custom_patterns_from_the_policy_builder() {
        let config = ScanConfig {
            confidence_threshold: CONFIDENCE_THRESHOLD,
            custom_patterns: vec![CustomPattern {
                entity_label: "acme_account_format".to_string(),
                regex: regex::Regex::new(r"ACME-\d{8}").unwrap(),
                confidence: 0.90,
            }],
        };
        let outcome = scan_response("Your reference is ACME-12345678.", &config);
        assert!(outcome.flagged);
        assert!(outcome.entities_detected.contains(&"acme_account_format".to_string()));
    }

    #[test]
    fn clean_prompt_is_cleared() {
        let outcome = scan_prompt("What is the capital of Zimbabwe?");
        assert_eq!(outcome.decision, "cleared_no_entities");
        assert!(outcome.entities_detected.is_empty());
    }

    #[test]
    fn national_id_is_redacted() {
        let outcome = scan_prompt("Please check account for ID 63-123456A23 today.");
        assert_eq!(outcome.decision, "redacted_and_forwarded");
        assert!(outcome.entities_detected.contains(&"national_id".to_string()));
        assert!(!outcome.redacted_prompt.contains("63-123456A23"));
        assert!(outcome.redacted_prompt.contains("[REDACTED:NATIONAL_ID]"));
    }

    #[test]
    fn phone_number_is_redacted() {
        let outcome = scan_prompt("Call the client on 0771234567 about their claim.");
        assert_eq!(outcome.decision, "redacted_and_forwarded");
        assert!(outcome.entities_detected.contains(&"phone_number".to_string()));
    }

    #[test]
    fn valid_luhn_credit_card_is_detected() {
        // 4111 1111 1111 1111 is a standard Visa test number that passes Luhn.
        let outcome = scan_prompt("Card on file: 4111111111111111");
        assert!(outcome.entities_detected.contains(&"credit_card".to_string()));
    }

    #[test]
    fn invalid_luhn_number_is_not_flagged_as_credit_card() {
        let outcome = scan_prompt("Reference number: 4111111111111112");
        assert!(!outcome.entities_detected.contains(&"credit_card".to_string()));
    }

    #[test]
    fn generic_low_confidence_token_fails_closed() {
        // No specific prefix (sk-, AKIA, gh*_), so this only matches the
        // generic low-confidence api_key fallback and should block, not redact.
        let outcome = scan_prompt("token: aZ9xK2mQ7pL4vN8tR3wY6bC1dF5gH0jS2u");
        assert_eq!(outcome.decision, "blocked_low_confidence");
    }

    // --- Policy builder (product-depth task, Part 1): ScanConfig /
    // scan_prompt_with_config ------------------------------------------

    #[test]
    fn scan_prompt_matches_default_config_exactly() {
        // scan_prompt must remain byte-for-byte equivalent to
        // scan_prompt_with_config(prompt, &ScanConfig::default()) — the
        // whole point of keeping it a thin wrapper is that the other 90+
        // tests in this module (and seed.rs, and every multi-tenancy
        // integration test) never had to change for this feature to exist.
        let prompt = "Please verify national ID 63-123456A23 for admission.";
        let a = scan_prompt(prompt);
        let b = scan_prompt_with_config(prompt, &ScanConfig::default());
        assert_eq!(a.decision, b.decision);
        assert_eq!(a.entities_detected, b.entities_detected);
        assert_eq!(a.redacted_prompt, b.redacted_prompt);
        assert_eq!(a.risk_score, b.risk_score);
    }

    #[test]
    fn org_custom_pattern_is_detected_and_redacted() {
        // A hypothetical bank-specific account format ("ACME-" followed by 8
        // digits) that no built-in detector recognizes at all.
        let config = ScanConfig {
            confidence_threshold: CONFIDENCE_THRESHOLD,
            custom_patterns: vec![CustomPattern {
                entity_label: "acme_account_number".to_string(),
                regex: regex::Regex::new(r"ACME-\d{8}").unwrap(),
                confidence: 0.90,
            }],
        };
        let outcome =
            scan_prompt_with_config("Please close account ACME-12345678 today.", &config);
        assert_eq!(outcome.decision, "redacted_and_forwarded");
        assert!(outcome.entities_detected.contains(&"acme_account_number".to_string()));
        assert!(!outcome.redacted_prompt.contains("ACME-12345678"));
        assert!(outcome.redacted_prompt.contains("[REDACTED:ACME_ACCOUNT_NUMBER]"));
    }

    #[test]
    fn org_custom_pattern_is_not_detected_without_the_config() {
        // The exact same prompt, scanned with the default config (no custom
        // pattern registered) — must NOT be flagged. Proves custom patterns
        // are genuinely opt-in per organisation, not a change to the
        // built-in detector set.
        let outcome = scan_prompt("Please close account ACME-12345678 today.");
        assert!(!outcome.entities_detected.contains(&"acme_account_number".to_string()));
    }

    #[test]
    fn org_custom_pattern_cannot_reach_special_category_health_leniency() {
        // A custom pattern's sensitivity always falls through
        // health_rules::sensitivity_class's default arm (Standard), since an
        // org cannot name a custom label one of the five fixed
        // special_category_health types (enforced in routes/policy.rs, not
        // here) — so it is a structured (non-full_name) entity type and
        // therefore never leniency-eligible either way. This test locks
        // that a low-confidence custom-pattern match blocks, exactly like
        // any other low-confidence structured entity — it does NOT get
        // tier-2 review treatment.
        // Lowercase "custref-" + 5 digits deliberately doesn't collide with
        // any built-in detector's shape: MEDICAL_AID_NUMBER_RE requires
        // uppercase letters (`[A-Z]{2,6}-?\d{6,9}`), BANK_ACCOUNT_RE requires
        // 10-13 contiguous digits with nothing else, NATIONAL_ID_RE and the
        // MRN pattern need their own specific literal shapes — none of which
        // this matches. The prompt also avoids every keyword in
        // entity_meta.rs, so the generic fallback never gates open either.
        // This custom pattern is genuinely the only thing that can match.
        let config = ScanConfig {
            confidence_threshold: CONFIDENCE_THRESHOLD,
            custom_patterns: vec![CustomPattern {
                entity_label: "acme_loose_ref".to_string(),
                regex: regex::Regex::new(r"custref-\d{5}").unwrap(),
                confidence: 0.40, // deliberately below CONFIDENCE_THRESHOLD
            }],
        };
        let outcome = scan_prompt_with_config(
            "Customer quoted custref-12345 for the courier pickup.",
            &config,
        );
        assert_eq!(outcome.entities_detected, vec!["acme_loose_ref".to_string()]);
        assert_eq!(outcome.decision, "blocked_low_confidence");
        assert_eq!(outcome.sensitivity_class, "standard");
    }

    #[test]
    fn org_configured_confidence_threshold_changes_the_block_boundary() {
        // bank_account's primary pattern is fixed at 0.50 confidence
        // (rules.rs) — below the SYSTEM default CONFIDENCE_THRESHOLD (0.60),
        // so it blocks under the default config (see
        // low_confidence_structured_entity_still_blocks above). An org that
        // has lowered ITS OWN threshold to exactly 0.50 (the safe floor,
        // MIN_ORG_CONFIDENCE_THRESHOLD) must see the same match clear
        // instead, proving the org setting genuinely changes the boundary,
        // not just the audit text.
        let prompt = "Please refund via account 9988776655443 once approved.";
        let default_outcome = scan_prompt(prompt);
        assert_eq!(default_outcome.decision, "blocked_low_confidence");

        let lowered_config = ScanConfig {
            confidence_threshold: MIN_ORG_CONFIDENCE_THRESHOLD,
            custom_patterns: vec![],
        };
        let lowered_outcome = scan_prompt_with_config(prompt, &lowered_config);
        assert_eq!(lowered_outcome.decision, "redacted_and_forwarded");
        assert!(!lowered_outcome.redacted_prompt.contains("9988776655443"));
    }

    #[test]
    fn org_configured_confidence_threshold_never_touches_the_name_floor_or_the_health_hard_rule() {
        // Even with the org threshold set to its maximum
        // (MAX_ORG_CONFIDENCE_THRESHOLD), NAME_LOW_CONFIDENCE_FLOOR and the
        // special_category_health leniency exclusion must behave exactly as
        // they do under the system default — neither is threaded through
        // ScanConfig at all, so this is really a compile-time guarantee,
        // but this test locks the observable behavior too. Reuses the exact
        // prompt from
        // low_confidence_special_category_health_never_gets_review_flag_blocks_instead.
        let config = ScanConfig {
            confidence_threshold: MAX_ORG_CONFIDENCE_THRESHOLD,
            custom_patterns: vec![],
        };
        let outcome = scan_prompt_with_config(
            "Next of kin: John Moyo, please contact if condition worsens.",
            &config,
        );
        assert_eq!(outcome.sensitivity_class, "special_category_health");
        assert_eq!(outcome.decision, "blocked_low_confidence");
        assert!(outcome.redacted_prompt.contains("John Moyo"));
    }

    // --- Three-tier confidence handling (see NAME_LOW_CONFIDENCE_FLOOR's
    // doc comment above for the full reasoning) ---------------------------

    #[test]
    fn low_confidence_structured_entity_still_blocks() {
        // Uses bank_account, not national_id: national_id's regex rule is
        // fixed at confidence 0.85 (see rules.rs), so there is no prompt
        // that naturally produces a "low-confidence national_id" match to
        // test against without artificially fabricating one. bank_account
        // is the structured entity type that IS naturally low-confidence
        // (0.5, below CONFIDENCE_THRESHOLD) in this codebase's real rule
        // set, so it's used here to prove the same guarantee: a low-
        // confidence match on a STRUCTURED entity type blocks unconditionally,
        // with no tier-2 leniency the way full_name gets — unchanged by this
        // three-tier change.
        let outcome = scan_prompt("Please refund via account 9988776655443 once approved.");
        assert_eq!(outcome.decision, "blocked_low_confidence");
        assert!(outcome.entities_detected.contains(&"bank_account".to_string()));
        // Fail-closed: the original prompt must not have been redacted or
        // forwarded.
        assert!(outcome.redacted_prompt.contains("9988776655443"));
    }

    #[test]
    fn low_confidence_full_name_redacts_with_review_flag_instead_of_blocking() {
        // "Dear" is in name_heuristic.rs's stopword list and gets trimmed,
        // leaving "John Moyo" as the sole match, at the heuristic's fixed
        // 0.55 confidence — squarely inside the tier-2 band (0.30-0.60).
        let outcome = scan_prompt("Dear John Moyo, please review the attached document.");
        assert_eq!(outcome.decision, "redacted_low_confidence_review");
        assert!(outcome.entities_detected.contains(&"full_name".to_string()));
        assert!(!outcome.redacted_prompt.contains("John Moyo"));
        assert!(outcome.redacted_prompt.contains("[REDACTED:FULL_NAME]"));
        assert!(outcome.reason_string.contains("0.55"));
        assert!(outcome.reason_string.to_lowercase().contains("review"));
    }

    #[test]
    fn high_confidence_structured_entities_with_low_confidence_name_still_reviews_not_blocks() {
        // Regression lock for the exact behavior change this three-tier
        // model exists for: before this change, this prompt's incidental
        // low-confidence name match dragged the ENTIRE request down to
        // blocked_low_confidence, even though the national_id and
        // phone_number matches were both high-confidence. Structured-entity
        // confidence must still gate the request (tier 3a checked first),
        // but a fully-trusted structured match must not itself be blocked
        // just because a name elsewhere in the same prompt is only
        // tier-2-confident.
        let outcome =
            scan_prompt("Please verify national ID 63-123456A23 for John Moyo, phone 0771234567.");
        assert_eq!(outcome.decision, "redacted_low_confidence_review");
        assert!(outcome.entities_detected.contains(&"national_id".to_string()));
        assert!(outcome.entities_detected.contains(&"phone_number".to_string()));
        assert!(outcome.entities_detected.contains(&"full_name".to_string()));
        // Everything detected gets redacted, not just the low-confidence part.
        assert!(!outcome.redacted_prompt.contains("63-123456A23"));
        assert!(!outcome.redacted_prompt.contains("0771234567"));
        assert!(!outcome.redacted_prompt.contains("John Moyo"));
    }

    // --- Health module: new entity detectors + the sensitivity-class hard
    // rule (see health_rules.rs's SensitivityClass doc comment) ------------

    #[test]
    fn diagnosis_code_is_redacted_and_forwarded_at_high_confidence() {
        let outcome = scan_prompt("Patient presents with B20 and requires ART initiation this week.");
        assert_eq!(outcome.decision, "redacted_and_forwarded");
        assert!(outcome.entities_detected.contains(&"diagnosis_code".to_string()));
        assert_eq!(outcome.sensitivity_class, "special_category_health");
        assert!(!outcome.redacted_prompt.contains("B20"));
        assert!(outcome.redacted_prompt.contains("[REDACTED:DIAGNOSIS_CODE]"));
    }

    #[test]
    fn medication_name_is_redacted_and_forwarded() {
        let outcome = scan_prompt("Please refill Tenofovir and Lamivudine before month end.");
        assert_eq!(outcome.decision, "redacted_and_forwarded");
        assert!(outcome.entities_detected.contains(&"medication_name".to_string()));
        assert_eq!(outcome.sensitivity_class, "special_category_health");
        assert!(!outcome.redacted_prompt.contains("Tenofovir"));
    }

    #[test]
    fn lab_result_value_redacts_the_number_but_keeps_the_test_name_visible() {
        let outcome = scan_prompt("CD4 count 250 cells/mm3, schedule a follow-up review.");
        assert_eq!(outcome.decision, "redacted_and_forwarded");
        assert!(outcome.entities_detected.contains(&"lab_result_value".to_string()));
        assert!(!outcome.redacted_prompt.contains("250"));
        // The test name itself is not sensitive on its own — only the value
        // attached to it — so it must survive redaction.
        assert!(outcome.redacted_prompt.contains("CD4 count"));
    }

    #[test]
    fn medical_aid_number_generic_low_confidence_pattern_fails_closed() {
        // medical_aid_number's pattern is deliberately low-confidence (0.55,
        // same honesty tier as bank_account) — a match on its own, with
        // nothing else in the prompt, blocks rather than forwards.
        let outcome = scan_prompt("Confirm medical aid number CIMAS123456 is active before admission.");
        assert_eq!(outcome.decision, "blocked_low_confidence");
        assert!(outcome.entities_detected.contains(&"medical_aid_number".to_string()));
        assert_eq!(outcome.sensitivity_class, "special_category_health");
        // Fail-closed: original value must not have been forwarded.
        assert!(outcome.redacted_prompt.contains("CIMAS123456"));
    }

    #[test]
    fn low_confidence_special_category_health_never_gets_review_flag_blocks_instead() {
        // THE hard-rule regression test (Part 2 of the health module task):
        // a next_of_kin match lands at name_heuristic's fixed 0.55
        // confidence — squarely inside the SAME numeric band (0.30-0.60)
        // that gives a plain `full_name` match the lenient
        // `redacted_low_confidence_review` treatment (see
        // `low_confidence_full_name_redacts_with_review_flag_instead_of_blocking`
        // above, same confidence value, same band). Because this match is
        // `special_category_health`, it must NOT get that treatment — it
        // must fail closed instead, exactly like any other low-confidence
        // structured entity. This is the literal guarantee Part 2 asked for:
        // health data does not get the relaxed treatment names get.
        let outcome = scan_prompt("Next of kin: John Moyo, please contact if condition worsens.");
        assert!(outcome.entities_detected.contains(&"next_of_kin".to_string()));
        assert_eq!(outcome.sensitivity_class, "special_category_health");
        assert_ne!(outcome.decision, "redacted_low_confidence_review");
        assert_eq!(outcome.decision, "blocked_low_confidence");
        // Fail-closed: the name must not have been forwarded, redacted or not.
        assert!(outcome.redacted_prompt.contains("John Moyo"));
    }

    #[test]
    fn next_of_kin_reclassification_does_not_affect_an_unrelated_name_in_the_same_prompt() {
        // A name NOT near a next-of-kin/emergency-contact/guardian keyword
        // stays plain `full_name` (Standard sensitivity) and keeps its
        // existing tier-2 leniency — proves the hard rule is scoped to the
        // contextual reclassification, not a blanket change to how ALL
        // names in a health-context prompt are treated.
        let outcome = scan_prompt("Please schedule Tendai Moyo for a follow-up appointment next week.");
        assert!(outcome.entities_detected.contains(&"full_name".to_string()));
        assert!(!outcome.entities_detected.contains(&"next_of_kin".to_string()));
        assert_eq!(outcome.decision, "redacted_low_confidence_review");
        assert_eq!(outcome.sensitivity_class, "standard");
    }

    #[test]
    fn high_confidence_special_category_health_coexists_with_ordinary_name_leniency() {
        // A confident special_category_health match (diagnosis_code, 0.90)
        // must not itself require blocking or reviewing — it clears tier 3a
        // normally. A SEPARATE, unrelated low-confidence full_name match
        // elsewhere in the same prompt still gets its own existing tier-2
        // leniency (redacted_low_confidence_review) exactly as before this
        // module existed — the two mechanisms operate independently.
        let outcome = scan_prompt(
            "Patient diagnosis B20 confirmed. Please also notify Tendai Moyo about the follow-up.",
        );
        assert!(outcome.entities_detected.contains(&"diagnosis_code".to_string()));
        assert!(outcome.entities_detected.contains(&"full_name".to_string()));
        assert_eq!(outcome.decision, "redacted_low_confidence_review");
        assert_eq!(outcome.sensitivity_class, "special_category_health");
        assert!(!outcome.redacted_prompt.contains("B20"));
        assert!(!outcome.redacted_prompt.contains("Tendai Moyo"));
    }

    // --- The reported bug, and the generic fallback that fixes it ---------

    #[test]
    fn regression_mark_dlomo_patient_id_bug_report_both_entities_detected_and_redacted() {
        // THE exact bug report this task exists to fix: "12345678ACD"
        // matches no existing specific-format regex (not national_id's
        // dash-shaped pattern, not medical_aid_number's letters-then-digits
        // shape), so before fallback.rs existed it silently passed through
        // unredacted while the name was correctly caught. Both must now be
        // caught and redacted in this exact sentence.
        let outcome = scan_prompt("Mark Dlomo Patient ID: 12345678ACD");
        assert!(outcome.entities_detected.contains(&"full_name".to_string()));
        assert!(outcome.entities_detected.contains(&"probable_identifier".to_string()));
        assert!(!outcome.redacted_prompt.contains("Mark Dlomo"));
        assert!(!outcome.redacted_prompt.contains("12345678ACD"));
        assert!(outcome.redacted_prompt.contains("[REDACTED:FULL_NAME]"));
        assert!(outcome.redacted_prompt.contains("[REDACTED:PROBABLE_IDENTIFIER]"));
        // Must actually forward (redacted), not fail closed — the whole
        // point of the fallback's keyword-gated confidence (0.70) is that a
        // genuinely unclaimed but keyword-confirmed identifier is trusted
        // enough to redact rather than block for no actionable reason. The
        // decision lands on `redacted_low_confidence_review`, not the plain
        // `redacted_and_forwarded` tier, because "Mark Dlomo" is ALSO only a
        // 0.55-confidence full_name match — same pre-existing tier-2
        // leniency behavior as
        // `high_confidence_structured_entities_with_low_confidence_name_still_reviews_not_blocks`
        // above, unrelated to and unaffected by this task's fix. Both
        // decision values actually forward the redacted prompt (see
        // routes/scan.rs), so either satisfies "redacted correctly".
        assert_ne!(outcome.decision, "blocked_low_confidence");
    }

    #[test]
    fn tier1_reason_string_names_the_specific_rule_that_fired() {
        // Point 7 of the task: the reason_string must say WHICH rule fired,
        // not just the entity type. No name in this prompt, so this stays
        // in tier 1 (redacted_and_forwarded) and its reason_string is the
        // one that lists per-match rule detail.
        let outcome = scan_prompt("Patient ID: 12345678ACD confirmed.");
        assert_eq!(outcome.decision, "redacted_and_forwarded");
        assert!(outcome.reason_string.contains("probable_identifier"));
        assert!(outcome.reason_string.contains("generic structured-identifier fallback"));
    }

    #[test]
    fn fallback_does_not_fire_on_an_id_shaped_token_with_no_nearby_keyword() {
        // The negative twin of the regression test above: same ID shape,
        // no identifying keyword anywhere nearby — must not be flagged at
        // all (order numbers, tracking codes, etc. are exactly what this
        // guards against).
        let outcome = scan_prompt("The shipment tracking code 12345678ACD arrived this morning.");
        assert!(!outcome.entities_detected.contains(&"probable_identifier".to_string()));
        assert!(outcome.redacted_prompt.contains("12345678ACD"));
    }

    #[test]
    fn medical_aid_number_fallback_does_not_override_the_deliberately_low_confidence_primary_pattern() {
        // Two detectors legitimately fire on the exact same span here:
        // medical_aid_number's own primary pattern (0.55, deliberately
        // low/ungated) AND the generic fallback (0.70, keyword-gated on
        // "medical aid number" right next to it). This is the concrete
        // case `resolve_overlaps`'s doc comment reasons through — the
        // specific detector must still win, preserving medical_aid_number's
        // existing fail-closed behavior, rather than the fallback's higher
        // raw confidence silently overriding it just because a keyword
        // happened to be nearby (which is common, ordinary phrasing for
        // this entity type, not a rare edge case).
        let outcome = scan_prompt("Confirm medical aid number CIMAS123456 is active before admission.");
        assert!(outcome.entities_detected.contains(&"medical_aid_number".to_string()));
        assert!(!outcome.entities_detected.contains(&"probable_identifier".to_string()));
        assert_eq!(outcome.decision, "blocked_low_confidence");
    }

    #[test]
    fn medical_record_no_end_to_end_positive() {
        // MEDICAL_RECORD_RE had no end-to-end (scan_prompt-level) coverage
        // before this task's test-corpus expansion — only exercised
        // indirectly via other tests that happened to also contain an MRN.
        let outcome = scan_prompt("Patient record MRN-204981 needs an update.");
        assert!(outcome.entities_detected.contains(&"medical_record_no".to_string()));
        assert_eq!(outcome.decision, "redacted_and_forwarded");
        assert!(!outcome.redacted_prompt.contains("MRN-204981"));
    }

    // --- Overlap resolution (task point 4): explicit test with two
    // detectors firing on the same span, confirming the higher-confidence
    // one wins and the lower one is dropped, not double-reported. ---------

    #[test]
    fn resolve_overlaps_keeps_only_the_highest_confidence_match_for_a_contested_span() {
        let candidates = vec![
            Match {
                entity_type: "bank_account".to_string(),
                start: 10,
                end: 20,
                confidence: 0.5,
                sensitivity: SensitivityClass::Standard,
                rule_detail: "primary pattern match".to_string(),
            },
            Match {
                entity_type: "national_id".to_string(),
                start: 12,
                end: 22, // overlaps the bank_account candidate above
                confidence: 0.85,
                sensitivity: SensitivityClass::Standard,
                rule_detail: "primary pattern match".to_string(),
            },
            Match {
                entity_type: "phone_number".to_string(),
                start: 40,
                end: 50, // does not overlap anything — must survive independently
                confidence: 0.9,
                sensitivity: SensitivityClass::Standard,
                rule_detail: "primary pattern match".to_string(),
            },
        ];
        let resolved = resolve_overlaps(candidates);
        assert_eq!(resolved.len(), 2);
        assert!(resolved.iter().any(|m| m.entity_type == "national_id"));
        assert!(resolved.iter().any(|m| m.entity_type == "phone_number"));
        assert!(!resolved.iter().any(|m| m.entity_type == "bank_account"));
    }

    #[test]
    fn resolve_overlaps_fallback_always_loses_to_a_specific_detector_regardless_of_confidence() {
        // Synthetic version of `medical_aid_number_fallback_does_not_override_the_deliberately_low_confidence_primary_pattern`
        // above, isolating just the resolution rule itself: even with a
        // LOWER raw confidence, a non-fallback candidate beats a fallback
        // candidate on the same span (see `resolve_overlaps`'s doc comment
        // for why this is a deliberate exception to plain highest-number-
        // wins).
        let candidates = vec![
            Match {
                entity_type: fallback::FALLBACK_ENTITY_TYPE.to_string(),
                start: 0,
                end: 10,
                confidence: 0.70,
                sensitivity: SensitivityClass::Standard,
                rule_detail: "generic structured-identifier fallback".to_string(),
            },
            Match {
                entity_type: "medical_aid_number".to_string(),
                start: 0,
                end: 10,
                confidence: 0.55,
                sensitivity: SensitivityClass::SpecialCategoryHealth,
                rule_detail: "primary pattern match".to_string(),
            },
        ];
        let resolved = resolve_overlaps(candidates);
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].entity_type, "medical_aid_number");
    }

    // --- user_message: plain-language banner text, split from reason_string
    // (the technical audit-trail detail) — see plain_language.rs. ----------

    #[test]
    fn blocked_single_entity_user_message_is_plain_language() {
        // bank_account alone, naturally low-confidence (0.5) — a single-
        // entity block, the simple case.
        let outcome = scan_prompt("Please refund via account 9988776655443 once approved.");
        assert_eq!(outcome.decision, "blocked_low_confidence");
        assert_eq!(
            outcome.user_message,
            "This message may contain a bank account number we're not confident about. \
                Please review and remove or rephrase it before sending."
        );
        // The technical detail must still be in reason_string, unchanged —
        // just never in user_message.
        assert!(outcome.reason_string.contains("bank_account"));
        assert!(outcome.reason_string.contains("0.50"));
        assert!(!outcome.user_message.contains("bank_account"));
        assert!(!outcome.user_message.contains("0.50"));
        assert!(!outcome.user_message.to_lowercase().contains("confidence ("));
    }

    #[test]
    fn blocked_multi_entity_user_message_lists_plain_phrases_not_raw_types() {
        // next_of_kin (special_category_health, so it can never reach the
        // tier-2 leniency band — see
        // low_confidence_special_category_health_never_gets_review_flag_blocks_instead
        // above) alongside a low-confidence bank_account match — both
        // structured-tier, both below threshold, blocking together.
        let outcome = scan_prompt(
            "Next of kin: John Moyo. Please refund via account 9988776655443 once approved.",
        );
        assert_eq!(outcome.decision, "blocked_low_confidence");
        assert!(outcome.entities_detected.contains(&"next_of_kin".to_string()));
        assert!(outcome.entities_detected.contains(&"bank_account".to_string()));
        // Order follows text position (next_of_kin's "John Moyo" appears
        // before bank_account's digits in this prompt) — plain_language::describe
        // preserves first-seen order rather than imposing a fixed ordering.
        assert_eq!(
            outcome.user_message,
            "This message may contain a contact name and a bank account number we're not \
                confident about. Please review and remove or rephrase it before sending."
        );
        // Neither raw entity_type string nor rule_detail/confidence-number
        // detail leaks into the plain-language message.
        assert!(!outcome.user_message.contains("next_of_kin"));
        assert!(!outcome.user_message.contains("bank_account"));
        assert!(!outcome.user_message.contains("heuristic"));
        assert!(!outcome.user_message.contains("pattern match"));
        // The audit-trail reason_string keeps the full technical detail,
        // completely unaffected by this change.
        assert!(outcome.reason_string.contains("next_of_kin"));
        assert!(outcome.reason_string.contains("bank_account"));
    }

    #[test]
    fn redacted_and_forwarded_user_message_is_plain_language() {
        let outcome = scan_prompt("Please verify national ID 63-123456A23 for admission.");
        assert_eq!(outcome.decision, "redacted_and_forwarded");
        assert_eq!(
            outcome.user_message,
            "This message contained a national ID number, which was automatically redacted \
                before sending."
        );
        assert!(!outcome.user_message.contains("national_id"));
    }

    #[test]
    fn redacted_low_confidence_review_user_message_is_plain_language() {
        let outcome = scan_prompt("Dear John Moyo, please review the attached document.");
        assert_eq!(outcome.decision, "redacted_low_confidence_review");
        assert!(!outcome.user_message.contains("full_name"));
        assert!(!outcome.user_message.contains("0.55"));
        assert!(outcome.user_message.to_lowercase().contains("name"));
    }
}
