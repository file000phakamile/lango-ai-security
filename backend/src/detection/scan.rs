use sha2::{Digest, Sha256};

use super::{fallback, health_rules, name_heuristic, rules};
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
    entity_type: &'static str,
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
const CONFIDENCE_THRESHOLD: f32 = 0.6;

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
    pub reason_string: String,
    /// "standard" or "special_category_health" — see
    /// `health_rules::SensitivityClass`. A NEW, independent axis from
    /// `decision`/confidence: this reflects the sensitivity of the entity
    /// *category* detected, not how confident the scanner was.
    pub sensitivity_class: &'static str,
}

pub fn scan_prompt(prompt: &str) -> ScanOutcome {
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
                entity_type: rule.entity_type,
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
            entity_type: hm.entity_type,
            start: hm.start,
            end: hm.end,
            confidence: hm.confidence,
            sensitivity: health_rules::sensitivity_class(hm.entity_type),
            rule_detail: "primary pattern match, ICD-10 shape + dictionary-gated".to_string(),
        });
    }
    for hm in health_rules::detect_medications(prompt) {
        candidates.push(Match {
            entity_type: hm.entity_type,
            start: hm.start,
            end: hm.end,
            confidence: hm.confidence,
            sensitivity: health_rules::sensitivity_class(hm.entity_type),
            rule_detail: "primary pattern match, medication-name dictionary".to_string(),
        });
    }
    for hm in health_rules::detect_medical_aid_numbers(prompt) {
        candidates.push(Match {
            entity_type: hm.entity_type,
            start: hm.start,
            end: hm.end,
            confidence: hm.confidence,
            sensitivity: health_rules::sensitivity_class(hm.entity_type),
            rule_detail: "primary pattern match, generic format (ungated)".to_string(),
        });
    }
    for hm in health_rules::detect_lab_result_values(prompt) {
        candidates.push(Match {
            entity_type: hm.entity_type,
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
            entity_type,
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
            entity_type: fallback::FALLBACK_ENTITY_TYPE,
            start: fm.start,
            end: fm.end,
            confidence: fm.confidence,
            sensitivity: fm.sensitivity,
            rule_detail: fm.rule_detail,
        });
    }

    let mut matches: Vec<Match> = resolve_overlaps(candidates);
    matches.sort_by_key(|m| m.start);

    if matches.is_empty() {
        return ScanOutcome {
            entities_detected: vec![],
            risk_score: 0.05,
            redacted_prompt: prompt.to_string(),
            decision: "cleared_no_entities",
            reason_string: "No sensitive entities detected. Prompt forwarded unmodified."
                .to_string(),
            sensitivity_class: SensitivityClass::Standard.as_str(),
        };
    }

    let risk_score: f32 = matches
        .iter()
        .map(|m| severity(m.entity_type))
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
        .filter(|m| !is_leniency_eligible(m.entity_type, m.sensitivity))
        .map(|m| m.confidence)
        .fold(f32::INFINITY, f32::min);
    let min_name_confidence = matches
        .iter()
        .filter(|m| is_leniency_eligible(m.entity_type, m.sensitivity))
        .map(|m| m.confidence)
        .fold(f32::INFINITY, f32::min);

    // Tier 3a: any structured entity below CONFIDENCE_THRESHOLD blocks,
    // unconditionally — no middle band for these types (see
    // NAME_LOW_CONFIDENCE_FLOOR's doc comment for why).
    if min_structured_confidence < CONFIDENCE_THRESHOLD {
        let low_confidence_types: Vec<String> = matches
            .iter()
            .filter(|m| !is_leniency_eligible(m.entity_type, m.sensitivity) && m.confidence < CONFIDENCE_THRESHOLD)
            .map(|m| format!("{} [{}]", m.entity_type, m.rule_detail))
            .collect();
        return blocked_outcome(
            entities_detected,
            risk_score,
            prompt,
            min_structured_confidence,
            CONFIDENCE_THRESHOLD,
            &low_confidence_types,
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
            sensitivity_class,
        );
    }

    // Everything from here on forwards a redacted prompt — the redaction
    // step itself is identical whether the result ends up
    // `redacted_and_forwarded` or `redacted_low_confidence_review`; only the
    // decision label and reason_string differ.
    let redacted = redact(prompt, &matches);

    // Tier 2: a real but low-confidence name match (0.30-0.60), and nothing
    // structured is below threshold (tier 3a already returned above if it
    // were). Redact and forward anyway rather than blocking, flagged
    // distinctly for async compliance review — see NAME_LOW_CONFIDENCE_FLOOR.
    if min_name_confidence < CONFIDENCE_THRESHOLD {
        return ScanOutcome {
            entities_detected,
            risk_score,
            redacted_prompt: redacted,
            decision: "redacted_low_confidence_review",
            reason_string: format!(
                "Low-confidence name match ({:.2}) redacted automatically - flagged for compliance review",
                min_name_confidence
            ),
            sensitivity_class,
        };
    }

    // Tier 1: everything trusted. Per-match rule detail (point 7 of the
    // detection-engine task) so the audit trail names WHICH rule fired for
    // each entity, not just the entity type — e.g. distinguishing a
    // primary-pattern national_id match from a probable_identifier caught
    // only by the generic fallback.
    let detail_join = matches
        .iter()
        .map(|m| format!("{} [{}]", m.entity_type, m.rule_detail))
        .collect::<Vec<_>>()
        .join("; ");
    ScanOutcome {
        entities_detected: entities_detected.clone(),
        risk_score,
        redacted_prompt: redacted,
        decision: "redacted_and_forwarded",
        reason_string: format!(
            "Blocked raw prompt: {}, replaced with placeholder tokens.",
            detail_join
        ),
        sensitivity_class,
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
    low_confidence_types: &[String],
    sensitivity_class: &'static str,
) -> ScanOutcome {
    ScanOutcome {
        entities_detected,
        risk_score,
        redacted_prompt: prompt.to_string(),
        decision: "blocked_low_confidence",
        sensitivity_class,
        reason_string: format!(
            "Scanner confidence below threshold ({:.2} < {:.2}) on detected {}. Fail-closed triggered.",
            min_confidence,
            threshold,
            low_confidence_types.join(", ")
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
                entity_type: "bank_account",
                start: 10,
                end: 20,
                confidence: 0.5,
                sensitivity: SensitivityClass::Standard,
                rule_detail: "primary pattern match".to_string(),
            },
            Match {
                entity_type: "national_id",
                start: 12,
                end: 22, // overlaps the bank_account candidate above
                confidence: 0.85,
                sensitivity: SensitivityClass::Standard,
                rule_detail: "primary pattern match".to_string(),
            },
            Match {
                entity_type: "phone_number",
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
                entity_type: fallback::FALLBACK_ENTITY_TYPE,
                start: 0,
                end: 10,
                confidence: 0.70,
                sensitivity: SensitivityClass::Standard,
                rule_detail: "generic structured-identifier fallback".to_string(),
            },
            Match {
                entity_type: "medical_aid_number",
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
}
