use sha2::{Digest, Sha256};

use super::{name_heuristic, rules};

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
        "not applicable - no live AI provider connected in v0.1, nothing was sent to scan"
    }
}

/// A single detected entity occurrence within a prompt.
struct Match {
    entity_type: &'static str,
    start: usize,
    end: usize,
    confidence: f32,
}

/// Below this confidence, we don't trust ourselves enough to redact and
/// forward — the request fails closed (blocked) instead. This is the same
/// "if detection confidence is low, block rather than forward" principle
/// stated in the proposal, now implemented as real, non-random logic rather
/// than the mock data's `rand() < 0.3` coin flip.
const CONFIDENCE_THRESHOLD: f32 = 0.6;

/// Per-entity-type severity weight, used to build the 0-1 risk score.
/// Reflects roughly how damaging exposure of that entity type is — a
/// national ID or credit card number is more sensitive than a phone number.
/// These weights are a product judgment call, not derived from an external
/// standard; documented here so they're easy to challenge and retune.
fn severity(entity_type: &str) -> f32 {
    match entity_type {
        "national_id" => 0.35,
        "credit_card" => 0.35,
        "bank_account" => 0.30,
        "api_key" => 0.30,
        "medical_record_no" => 0.25,
        "phone_number" => 0.20,
        "full_name" => 0.15,
        _ => 0.15,
    }
}

pub struct ScanOutcome {
    pub entities_detected: Vec<String>,
    pub risk_score: f32,
    pub redacted_prompt: String,
    pub decision: &'static str,
    pub reason_string: String,
}

pub fn scan_prompt(prompt: &str) -> ScanOutcome {
    let mut matches: Vec<Match> = Vec::new();

    // Regex-based rules, in priority order (rules.rs documents why each
    // pattern is ordered where it is). Overlapping lower-priority matches
    // are dropped so e.g. a phone number isn't also double-counted as a
    // generic bank-account digit run.
    for rule in rules::regex_rules() {
        for m in rule.regex.find_iter(prompt) {
            if matches
                .iter()
                .any(|existing| ranges_overlap(existing.start, existing.end, m.start(), m.end()))
            {
                continue;
            }

            // Credit-card matches get a real Luhn check, not just a format
            // match — a format match that fails Luhn is very likely a
            // false positive (e.g. an unrelated 16-digit reference number)
            // and is dropped rather than reported with inflated confidence.
            if rule.entity_type == "credit_card" && !rules::luhn_check(m.as_str()) {
                continue;
            }

            matches.push(Match {
                entity_type: rule.entity_type,
                start: m.start(),
                end: m.end(),
                confidence: rule.confidence,
            });
        }
    }

    // Name heuristic — see name_heuristic.rs for the honesty note on what
    // this actually is (not real NER).
    for name in name_heuristic::detect_names(prompt) {
        if matches
            .iter()
            .any(|existing| ranges_overlap(existing.start, existing.end, name.start, name.end))
        {
            continue;
        }
        matches.push(Match {
            entity_type: "full_name",
            start: name.start,
            end: name.end,
            confidence: 0.55, // heuristic, not a real NER model — deliberately capped low
        });
    }

    matches.sort_by_key(|m| m.start);

    if matches.is_empty() {
        return ScanOutcome {
            entities_detected: vec![],
            risk_score: 0.05,
            redacted_prompt: prompt.to_string(),
            decision: "cleared_no_entities",
            reason_string: "No sensitive entities detected. Prompt forwarded unmodified."
                .to_string(),
        };
    }

    let min_confidence = matches
        .iter()
        .map(|m| m.confidence)
        .fold(f32::INFINITY, f32::min);

    let risk_score: f32 = matches
        .iter()
        .map(|m| severity(m.entity_type))
        .sum::<f32>()
        .min(1.0);

    let entities_detected: Vec<String> = matches.iter().map(|m| m.entity_type.to_string()).collect();

    if min_confidence < CONFIDENCE_THRESHOLD {
        let low_confidence_types: Vec<&str> = matches
            .iter()
            .filter(|m| m.confidence < CONFIDENCE_THRESHOLD)
            .map(|m| m.entity_type)
            .collect();
        return ScanOutcome {
            entities_detected,
            risk_score,
            // The original, unredacted prompt is never forwarded when we
            // fail closed — nothing is sent anywhere.
            redacted_prompt: prompt.to_string(),
            decision: "blocked_low_confidence",
            reason_string: format!(
                "Scanner confidence below threshold ({:.2} < {:.2}) on detected {}. Fail-closed triggered.",
                min_confidence,
                CONFIDENCE_THRESHOLD,
                low_confidence_types.join(", ")
            ),
        };
    }

    // Redact highest-offset matches first so earlier byte offsets in the
    // string stay valid as we splice replacement tokens in.
    let mut redacted = prompt.to_string();
    let mut sorted_desc = matches.iter().collect::<Vec<_>>();
    sorted_desc.sort_by_key(|m| std::cmp::Reverse(m.start));
    for m in sorted_desc {
        let placeholder = format!("[REDACTED:{}]", m.entity_type.to_uppercase());
        redacted.replace_range(m.start..m.end, &placeholder);
    }

    ScanOutcome {
        entities_detected: entities_detected.clone(),
        risk_score,
        redacted_prompt: redacted,
        decision: "redacted_and_forwarded",
        reason_string: format!(
            "Blocked raw prompt: {} detected, replaced with placeholder tokens.",
            entities_detected.join(", ")
        ),
    }
}

fn ranges_overlap(a_start: usize, a_end: usize, b_start: usize, b_end: usize) -> bool {
    a_start < b_end && b_start < a_end
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
}
