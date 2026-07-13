//! Generic structured-identifier fallback detector.
//!
//! THE actual fix for the reported gap: a prompt containing "Mark Dlomo
//! Patient ID: 12345678ACD" correctly redacted the name but silently missed
//! the ID, because "12345678ACD" matches no existing specific-format regex
//! (not `national_id`'s dash-shaped pattern, not `medical_aid_number`'s
//! letters-then-digits shape). Every detector before this one is a FIXED
//! pattern for ONE known format — any real-world format variance falls
//! through with no fallback at all. This module is that fallback, run
//! alongside (never instead of) the specific per-type detectors.
//!
//! Design, matching the task spec:
//!   - Any token, once punctuation-stripped, that is 6-14 characters long,
//!     alphanumeric, and mixes at least one digit with at least one letter,
//!     is a *candidate* ("roughly ID-shaped").
//!   - A candidate is only ever reported if it falls within
//!     `KEYWORD_WINDOW_TOKENS` tokens of a recognized identifying keyword
//!     drawn from ANY entity type's keyword list in `entity_meta.rs` (not
//!     just one type — "Patient ID" gates the fallback even though it's
//!     `medical_record_no`'s keyword, not e.g. `national_id`'s).
//!   - No keyword nearby => no match, unconditionally. See
//!     `no_match_without_nearby_keyword` below — this negative case is as
//!     load-bearing as the positive one, since the whole point of gating on
//!     a keyword is to avoid flagging order numbers, invoice references,
//!     confirmation codes, etc. that merely happen to look ID-shaped.
//!
//! WHY 3 TOKENS EITHER SIDE (judgment call, documented per the task's
//! request): narrow enough that a keyword three sentences away in a long
//! prompt can't reach across unrelated text and falsely gate an unrelated
//! number, but wide enough to cover the realistic ways people actually
//! separate a label from its value — "Patient ID: 12345678ACD" (1 token
//! gap), "Patient ID number 12345678ACD" (2-3 token gap with a filler word),
//! "ID 12345678ACD (confirmed)" (adjacent). Untested wider windows (5+)
//! start reaching across an entire clause into unrelated numbers in
//! practice; narrower (1) misses the "ID number: X" filler-word case. 3 was
//! chosen as the middle of that range, not derived from a corpus.
//!
//! WHY 0.70 CONFIDENCE (judgment call, documented per the task's request):
//! deliberately between the generic *ungated* patterns in this codebase
//! (`bank_account`/`medical_aid_number` at 0.50-0.55, which have no keyword
//! requirement at all and are correspondingly untrusted) and a real
//! specific-format match (0.85+, a validated shape). Keyword-gating is a
//! real signal — it is exactly what turns a bare ambiguous number into
//! something worth trusting — so this sits above `CONFIDENCE_THRESHOLD`
//! (0.60 in `scan.rs`) and will redact-and-forward on its own for a
//! `Standard`-sensitivity match. It stays below every validated-format
//! primary pattern because the *format itself* is still unverified — unlike
//! Luhn on a credit card or a dictionary-gated ICD-10 code, nothing here
//! confirms the token is actually well-formed for its inferred type, only
//! that it's plausibly ID-shaped and near an ID-shaped keyword.

use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;

use super::entity_meta::{self, MAX_KEYWORD_WORDS};
use super::health_rules::SensitivityClass;
use super::tokenize::{self, Token};

pub const FALLBACK_ENTITY_TYPE: &str = "probable_identifier";

/// See the module doc comment for the reasoning behind this specific value.
pub const KEYWORD_WINDOW_TOKENS: usize = 3;

/// See the module doc comment for the reasoning behind this specific value.
pub const FALLBACK_CONFIDENCE: f32 = 0.70;

/// Punctuation-stripped token shape: 6-14 alphanumeric characters. The
/// digit+letter mix requirement is checked separately in plain Rust (not in
/// the regex) because Rust's `regex` crate has no lookahead — trying to
/// express "contains a digit AND contains a letter" as a single pattern
/// without lookahead means either alternation blowup or manual code; manual
/// code is both clearer and avoids any backtracking risk entirely (this
/// pattern has a single bounded quantifier over one character class, so
/// there's nothing to backtrack regardless, but the manual digit/letter
/// check keeps the regex itself trivial to audit).
static STRUCTURED_SHAPE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[A-Za-z0-9]{6,14}$").unwrap());

/// Every keyword phrase across every entity type, flattened into a single
/// lookup: normalized phrase -> the entity type it came from. Built once at
/// startup (`Lazy`), not per scan.
static KEYWORD_INDEX: Lazy<HashMap<String, &'static str>> = Lazy::new(|| {
    let mut map = HashMap::new();
    for group in entity_meta::ENTITY_KEYWORDS {
        for kw in group.keywords {
            map.insert(kw.to_string(), group.entity_type);
        }
    }
    map
});

struct KeywordOccurrence {
    token_start: usize,
    token_end: usize,
    source_entity_type: &'static str,
}

/// Scans the token stream for every keyword phrase (1 to `MAX_KEYWORD_WORDS`
/// tokens long) from the flattened index above, then drops any occurrence
/// whose token span is a strict subset of a longer occurrence's span.
///
/// WHY the dedup matters, concretely: "Patient ID:" contains BOTH the
/// 2-word phrase "patient id" (`medical_record_no`, => special-category
/// health) AND, as a sub-span, the 1-word phrase "id" (`national_id`,
/// standard). Without preferring the longer, more specific phrase, a
/// candidate token near "Patient ID:" would see two tied-nearest,
/// disagreeing-sensitivity occurrences and fall back to the ambiguous
/// `Standard` default — silently downgrading the exact "Mark Dlomo Patient
/// ID: 12345678ACD" bug-report case this module exists to fix correctly.
/// Keeping only the maximal (non-subsumed) phrase match resolves it to the
/// single, more specific "patient id" occurrence instead.
fn find_keyword_occurrences(tokens: &[Token]) -> Vec<KeywordOccurrence> {
    let mut occurrences = Vec::new();
    for start in 0..tokens.len() {
        for len in 1..=MAX_KEYWORD_WORDS {
            let end = start + len;
            if end > tokens.len() {
                break;
            }
            let phrase = tokens[start..end]
                .iter()
                .map(|t| t.normalize())
                .collect::<Vec<_>>()
                .join(" ");
            if let Some(&source_entity_type) = KEYWORD_INDEX.get(&phrase) {
                occurrences.push(KeywordOccurrence {
                    token_start: start,
                    token_end: end - 1,
                    source_entity_type,
                });
            }
        }
    }
    let keep: Vec<bool> = (0..occurrences.len())
        .map(|i| {
            let occ = &occurrences[i];
            !occurrences.iter().enumerate().any(|(j, other)| {
                j != i
                    && other.token_start <= occ.token_start
                    && other.token_end >= occ.token_end
                    && (other.token_start, other.token_end) != (occ.token_start, occ.token_end)
            })
        })
        .collect();
    occurrences
        .into_iter()
        .zip(keep)
        .filter_map(|(occ, k)| k.then_some(occ))
        .collect()
}

fn token_distance(idx: usize, occ: &KeywordOccurrence) -> usize {
    if idx >= occ.token_start && idx <= occ.token_end {
        0
    } else if idx < occ.token_start {
        occ.token_start - idx
    } else {
        idx - occ.token_end
    }
}

fn is_structured_candidate(normalized: &str) -> bool {
    if !STRUCTURED_SHAPE_RE.is_match(normalized) {
        return false;
    }
    let has_digit = normalized.bytes().any(|b| b.is_ascii_digit());
    let has_alpha = normalized.bytes().any(|b| b.is_ascii_alphabetic());
    has_digit && has_alpha
}

/// A generic-fallback match — same shape as `health_rules::HealthMatch` /
/// `scan.rs`'s private `Match`, plus the extra provenance fields `scan.rs`
/// needs to build a specific (not just entity-type-level) `reason_string`.
pub struct FallbackMatch {
    pub start: usize,
    pub end: usize,
    pub confidence: f32,
    pub sensitivity: SensitivityClass,
    /// e.g. "generic structured-identifier fallback (keyword \"patient id\"
    /// from medical_record_no, 1 token away)" — the audit detail this
    /// module exists to make possible (task point 7).
    pub rule_detail: String,
}

/// Default when the nearest keyword's sensitivity is ambiguous (see
/// `infer_sensitivity`'s doc comment) — deliberately `Standard`, matching
/// this codebase's existing convention of defaulting unknown/ambiguous
/// entity types to `Standard` rather than guessing "special category"
/// (see `health_rules::sensitivity_class`'s own default-arm comment).
const AMBIGUOUS_SENSITIVITY_DEFAULT: SensitivityClass = SensitivityClass::Standard;

/// Infers sensitivity from whichever keyword occurrence(s) are nearest to
/// the candidate token. WHY this specific tie-break (judgment call,
/// documented per the task's request): if the single nearest keyword is
/// unambiguous, trust it directly (e.g. "Patient ID" near the token =>
/// special_category_health, since it's drawn from `medical_record_no`'s
/// keyword list). If multiple keywords tie for nearest AND they disagree on
/// sensitivity (e.g. a health keyword and a non-health keyword equidistant
/// on either side of the same token), that's genuinely ambiguous — default
/// to `Standard` rather than guessing toward the more sensitive class,
/// consistent with this codebase's existing "unknown defaults to Standard"
/// convention rather than inventing a new "guess toward caution" rule just
/// for this one path.
fn infer_sensitivity(nearest: &[&KeywordOccurrence]) -> SensitivityClass {
    let mut classes = nearest
        .iter()
        .map(|occ| super::health_rules::sensitivity_class(occ.source_entity_type));
    let first = match classes.next() {
        Some(c) => c,
        None => return AMBIGUOUS_SENSITIVITY_DEFAULT,
    };
    if classes.all(|c| c == first) {
        first
    } else {
        AMBIGUOUS_SENSITIVITY_DEFAULT
    }
}

pub fn detect_structured_identifiers(text: &str) -> Vec<FallbackMatch> {
    let tokens = tokenize::tokenize(text);
    let occurrences = find_keyword_occurrences(&tokens);
    if occurrences.is_empty() {
        // No keyword anywhere in the prompt at all => nothing this
        // detector can ever gate on. Short-circuit rather than scanning
        // every token for shape only to reject all of them below.
        return Vec::new();
    }

    let mut out = Vec::new();
    for (idx, token) in tokens.iter().enumerate() {
        let trimmed = token.trimmed();
        if trimmed.len() < 6 || !is_structured_candidate(trimmed) {
            continue;
        }

        let mut best_distance = usize::MAX;
        let mut nearest: Vec<&KeywordOccurrence> = Vec::new();
        for occ in &occurrences {
            let d = token_distance(idx, occ);
            if d > KEYWORD_WINDOW_TOKENS {
                continue;
            }
            match d.cmp(&best_distance) {
                std::cmp::Ordering::Less => {
                    best_distance = d;
                    nearest.clear();
                    nearest.push(occ);
                }
                std::cmp::Ordering::Equal => nearest.push(occ),
                std::cmp::Ordering::Greater => {}
            }
        }

        if nearest.is_empty() {
            // No keyword within the window — the deliberate non-match case
            // this module must never override. See
            // `no_match_without_nearby_keyword` in the tests below.
            continue;
        }

        let sensitivity = infer_sensitivity(&nearest);
        let (start, end) = token.trimmed_span();
        let closest = &nearest[0];
        out.push(FallbackMatch {
            start,
            end,
            confidence: FALLBACK_CONFIDENCE,
            sensitivity,
            rule_detail: format!(
                "generic structured-identifier fallback (keyword \"{}\" from {} list, {} token{} away)",
                text_of_occurrence(&tokens, closest),
                closest.source_entity_type,
                best_distance,
                if best_distance == 1 { "" } else { "s" }
            ),
        });
    }
    out
}

fn text_of_occurrence(tokens: &[Token], occ: &KeywordOccurrence) -> String {
    tokens[occ.token_start..=occ.token_end]
        .iter()
        .map(|t| t.normalize())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_id_shaped_token_near_patient_id_keyword() {
        // The exact reported bug: "12345678ACD" matches no existing
        // specific-format regex, so only this fallback can catch it.
        let matches = detect_structured_identifiers("Mark Dlomo Patient ID: 12345678ACD");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].sensitivity, SensitivityClass::SpecialCategoryHealth);
        assert!(matches[0].confidence >= 0.60);
    }

    #[test]
    fn no_match_without_nearby_keyword() {
        // Same ID-shaped token, but with NO identifying keyword anywhere
        // near it — must NOT match. This is the negative case the task
        // explicitly calls out as equally important as the positive one.
        let matches = detect_structured_identifiers(
            "The shipment tracking code 12345678ACD arrived at the depot this morning.",
        );
        assert!(matches.is_empty());
    }

    #[test]
    fn keyword_outside_window_does_not_gate_a_match() {
        // "Reference" appears in the prompt, but far more than 3 tokens
        // from the candidate token — must not match.
        let matches = detect_structured_identifiers(
            "Reference this old thread from last quarter about something unrelated to code 12345678ACD entirely.",
        );
        assert!(matches.is_empty());
    }

    #[test]
    fn generic_ref_keyword_infers_standard_sensitivity() {
        let matches = detect_structured_identifiers("Ref: 9A8B7C6D5E");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].sensitivity, SensitivityClass::Standard);
    }

    #[test]
    fn membership_number_infers_special_category_health() {
        let matches = detect_structured_identifiers("Membership Number: CMX88291XQ please confirm.");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].sensitivity, SensitivityClass::SpecialCategoryHealth);
    }

    #[test]
    fn pure_digit_token_is_not_a_structured_candidate() {
        // Must mix letters AND digits — a bare digit run is left to the
        // existing national_id/bank_account/phone patterns, not this
        // fallback, to avoid double-claiming the same span differently.
        let matches = detect_structured_identifiers("Account No: 12345678");
        assert!(matches.is_empty());
    }

    #[test]
    fn pure_letter_token_is_not_a_structured_candidate() {
        let matches = detect_structured_identifiers("Account No: ABCDEFGHIJ");
        assert!(matches.is_empty());
    }

    #[test]
    fn too_short_token_is_not_a_structured_candidate() {
        let matches = detect_structured_identifiers("ID: A1B2C");
        assert!(matches.is_empty());
    }

    #[test]
    fn too_long_token_is_not_a_structured_candidate() {
        let matches = detect_structured_identifiers("ID: A1B2C3D4E5F6G7H8");
        assert!(matches.is_empty());
    }

    #[test]
    fn longer_keyword_phrase_wins_over_a_subsumed_shorter_one() {
        // "Patient ID:" contains both the 2-word "patient id"
        // (medical_record_no, special_category_health) and, as a sub-span,
        // the 1-word "id" (national_id, standard). Without preferring the
        // longer, more specific phrase, this would tie and fall back to
        // the ambiguous Standard default — see `find_keyword_occurrences`'s
        // doc comment for why that matters for the exact bug-report case.
        let matches = detect_structured_identifiers("Patient ID: 12345678ACD");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].sensitivity, SensitivityClass::SpecialCategoryHealth);
        assert!(matches[0].rule_detail.contains("patient id"));
    }

    #[test]
    fn bare_id_keyword_still_gates_a_match_on_its_own() {
        let matches = detect_structured_identifiers("ID: 9A8B7C6D5E, confirm before filing.");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].sensitivity, SensitivityClass::Standard);
    }

    #[test]
    fn trimmed_span_excludes_trailing_punctuation_from_the_match() {
        let text = "Patient ID: 12345678ACD, confirmed today.";
        let matches = detect_structured_identifiers(text);
        assert_eq!(matches.len(), 1);
        assert_eq!(&text[matches[0].start..matches[0].end], "12345678ACD");
    }
}
