use once_cell::sync::Lazy;
use regex::Regex;

/// One compiled detection rule. `confidence` reflects how much we trust a
/// match from this specific pattern — used by the fail-closed logic in
/// `scan.rs`: a match from a well-formed, specific pattern is trusted; a
/// match from a loose generic pattern is not, and blocks rather than
/// forwards. See docs/ARCHITECTURE.md for the full honesty note on what
/// these patterns can and can't actually guarantee.
///
/// `checksum`, when present, is a real validation function run against the
/// matched text on top of the format match — currently only credit_card's
/// Luhn check (`luhn_check` below). A format match that fails its checksum
/// is dropped by `scan.rs` rather than reported with inflated confidence.
/// This field exists so `scan.rs`'s matching loop can treat every rule
/// uniformly (one loop, one optional checksum call) instead of special-
/// casing `entity_type == "credit_card"` by string comparison.
pub struct Rule {
    pub entity_type: &'static str,
    pub regex: &'static Lazy<Regex>,
    pub confidence: f32,
    pub checksum: Option<fn(&str) -> bool>,
}

// ---------------------------------------------------------------------------
// National ID (Zimbabwe)
//
// Assumption/source: the commonly documented Zimbabwean national ID format
// is DD-NNNNNNL DD (2-digit issuing-district code, dash, 6-7 digit serial,
// 1 check letter, 2-digit birth-district suffix — e.g. "63-123456A23"),
// based on publicly circulated examples of Zimbabwean ID cards. This is NOT
// sourced from an official government specification (none is publicly
// published), so it will miss valid IDs with formatting variations and can
// false-positive on unrelated dash-separated alphanumeric strings of the
// same shape. Treat this as a best-effort pattern, not a validated format.
// ---------------------------------------------------------------------------
static NATIONAL_ID_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b\d{2}-?\d{6,7}[A-Za-z]\d{2}\b").unwrap());

// ---------------------------------------------------------------------------
// Phone number (Zimbabwe mobile)
//
// Assumption/source: public Zimbabwean mobile network prefixes — 071
// (NetOne), 073 (Telecel), 077/078 (Econet) — giving a 10-digit local format
// (0 + 7X + 7 digits) or +263 international format (+263 + 7X + 7 digits).
// Landline numbers (area-code based, no single national format) are
// deliberately NOT covered here — that would need a per-city area-code
// table this pattern doesn't attempt.
// ---------------------------------------------------------------------------
static PHONE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(?:\+263|0)7[1378]\d{7}\b").unwrap());

// ---------------------------------------------------------------------------
// Credit card numbers
//
// Format patterns for the major networks (Visa, Mastercard, Amex), 13-19
// digits, optionally grouped with spaces or dashes. A regex match alone is
// not treated as high-confidence on its own — every match is additionally
// checked against the real Luhn checksum algorithm (see `luhn_check` below)
// before being counted, which is a genuine, well-defined validation, not a
// heuristic like the other patterns in this file.
// ---------------------------------------------------------------------------
static CREDIT_CARD_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"\b(?:4[0-9]{3}|5[1-5][0-9]{2}|3[47][0-9]{2})(?:[ -]?[0-9]{4}){2,3}\b|\b3[47][0-9]{13}\b",
    )
    .unwrap()
});

// ---------------------------------------------------------------------------
// API keys / tokens
//
// Specific, well-known prefixes (OpenAI-style `sk-…`, AWS access key
// `AKIA…`, GitHub `ghp_…`/`gho_…`/`ghu_…`/`ghs_…`/`ghr_…`) are high
// confidence — these formats are public and unambiguous. There is also a
// generic fallback pattern for any long token of this character class,
// which is NOT high confidence (plenty of non-secret strings look like
// this — hashes, session ids, base64 blobs) — see its lower `confidence`
// value below and the fail-closed behavior it triggers rather than a
// silent redact.
//
// Rust's `regex` crate deliberately does not support look-around
// (lookahead/lookbehind) — it trades that off for a linear-time matching
// guarantee. An earlier version of this pattern used lookahead to require
// "must contain at least one digit AND at least one letter", which
// doesn't compile against this crate at all (caught by `cargo test`, not
// by a hand-review — see Questions.md). The fix here is a length-only
// match instead of trying to work around the missing lookahead — it's
// already the lowest-confidence rule in this file and only ever
// fail-closes rather than silently redacting, so being slightly broader
// (also catching pure-digit or pure-letter 32+ runs) doesn't create a new
// false-redaction risk, just a few more fail-closed blocks to review.
// ---------------------------------------------------------------------------
static API_KEY_SPECIFIC_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b(?:sk-[A-Za-z0-9]{20,}|AKIA[0-9A-Z]{16}|gh[pousr]_[A-Za-z0-9]{36,})\b").unwrap()
});
static API_KEY_GENERIC_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b[A-Za-z0-9_\-]{32,}\b").unwrap());

// ---------------------------------------------------------------------------
// Medical record number
//
// No public, standardized Zimbabwean hospital record-number format exists
// (it's institution-specific) — this is a generic placeholder pattern
// representative of common formats seen across many hospital systems
// (an "MRN" prefix followed by digits), not derived from a specific known
// system. Institutions onboarding for real would need to supply their own
// pattern via `detection_rules`.
// ---------------------------------------------------------------------------
static MEDICAL_RECORD_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\bMRN[-\s]?\d{5,8}\b").unwrap());

// ---------------------------------------------------------------------------
// Bank account number
//
// No public Zimbabwean cross-bank account-number format or checksum exists
// (each bank defines its own). This is a deliberately generic heuristic — a
// bare run of 10-13 digits not already claimed by a more specific pattern
// (national ID, phone, credit card) above. This is the least specific
// pattern in this file and has real false-positive risk (e.g. large
// reference numbers, invoice numbers) — reflected in its lower confidence
// score, which feeds the fail-closed logic in `scan.rs`.
// ---------------------------------------------------------------------------
static BANK_ACCOUNT_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b\d{10,13}\b").unwrap());

pub fn regex_rules() -> Vec<Rule> {
    vec![
        Rule {
            entity_type: "credit_card",
            regex: &CREDIT_CARD_RE,
            confidence: 0.97, // additionally gated by a real Luhn check, via `checksum` below
            checksum: Some(luhn_check),
        },
        Rule {
            entity_type: "national_id",
            regex: &NATIONAL_ID_RE,
            confidence: 0.85,
            checksum: None,
        },
        Rule {
            entity_type: "phone_number",
            regex: &PHONE_RE,
            confidence: 0.9,
            checksum: None,
        },
        Rule {
            entity_type: "api_key",
            regex: &API_KEY_SPECIFIC_RE,
            confidence: 0.95,
            checksum: None,
        },
        Rule {
            entity_type: "medical_record_no",
            regex: &MEDICAL_RECORD_RE,
            confidence: 0.75,
            checksum: None,
        },
        Rule {
            entity_type: "bank_account",
            regex: &BANK_ACCOUNT_RE,
            confidence: 0.5, // generic digit run — genuinely low confidence, by design
            checksum: None,
        },
        Rule {
            entity_type: "api_key",
            regex: &API_KEY_GENERIC_RE,
            confidence: 0.45, // generic fallback — genuinely low confidence, by design
            checksum: None,
        },
    ]
}

/// Real Luhn checksum (mod-10 algorithm), used to gate credit-card regex
/// matches. This is a correct, well-defined algorithm — not a heuristic.
pub fn luhn_check(digits: &str) -> bool {
    let digits_only: Vec<u32> = digits.chars().filter_map(|c| c.to_digit(10)).collect();
    if digits_only.len() < 12 {
        return false;
    }
    let sum: u32 = digits_only
        .iter()
        .rev()
        .enumerate()
        .map(|(i, &d)| {
            if i % 2 == 1 {
                let doubled = d * 2;
                if doubled > 9 {
                    doubled - 9
                } else {
                    doubled
                }
            } else {
                d
            }
        })
        .sum();
    sum % 10 == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule_for(entity_type: &str) -> Rule {
        regex_rules()
            .into_iter()
            .find(|r| r.entity_type == entity_type)
            .unwrap()
    }

    // --- national_id: 3 positive formats, 1 edge case, 1 negative --------

    #[test]
    fn national_id_matches_plausible_formats() {
        let rule = rule_for("national_id");
        assert!(rule.regex.is_match("63-123456A23")); // dashed, 6-digit serial
        assert!(rule.regex.is_match("71-6543210B12")); // dashed, 7-digit serial
        assert!(rule.regex.is_match("58234567C45")); // no dash at all
    }

    #[test]
    fn national_id_edge_case_lowercase_check_letter_still_matches() {
        // The pattern's character class is case-insensitive by construction
        // ([A-Za-z]), not a documented guarantee about real ID cards, but
        // worth locking in since it's an easy regression to introduce.
        let rule = rule_for("national_id");
        assert!(rule.regex.is_match("63-123456a23"));
    }

    #[test]
    fn national_id_negative_bare_digits_do_not_match() {
        // A random 8-digit number with no check letter or district suffix
        // shape must not be mistaken for a national ID.
        let rule = rule_for("national_id");
        assert!(!rule.regex.is_match("12345678"));
    }

    // --- bank_account: 2 formats (10-digit and 13-digit boundary), 1 edge
    // case, 1 negative ------------------------------------------------------

    #[test]
    fn bank_account_matches_both_boundary_lengths() {
        let rule = rule_for("bank_account");
        assert!(rule.regex.is_match("1234567890")); // 10 digits, the floor
        assert!(rule.regex.is_match("9988776655443")); // 13 digits, the ceiling
    }

    #[test]
    fn bank_account_edge_case_nine_digits_does_not_match() {
        let rule = rule_for("bank_account");
        assert!(!rule.regex.is_match("123456789")); // one below the floor
    }

    #[test]
    fn bank_account_negative_fourteen_contiguous_digits_does_not_match() {
        // \b\d{10,13}\b requires a WORD BOUNDARY on both sides of the run —
        // a contiguous 14-digit run has no internal boundary, so nothing
        // inside it can match either. This is a real, deliberate
        // consequence of the pattern's word-boundary anchoring, not a gap
        // this task is trying to close (that run isn't a bank account
        // format this codebase claims to support).
        let rule = rule_for("bank_account");
        assert!(!rule.regex.is_match("12345678901234"));
    }

    // --- phone_number: 2 formats, 1 edge case, 1 negative -----------------

    #[test]
    fn phone_number_matches_local_format() {
        let rule = rule_for("phone_number");
        assert!(rule.regex.is_match("0771234567")); // local, Econet prefix
    }

    #[test]
    fn phone_number_edge_case_all_three_mobile_prefixes_match() {
        let rule = rule_for("phone_number");
        assert!(rule.regex.is_match("0711234567")); // NetOne
        assert!(rule.regex.is_match("0731234567")); // Telecel
    }

    #[test]
    fn phone_number_international_plus_prefix_is_a_known_unreached_branch() {
        // Discovered while broadening this test corpus, not introduced by
        // this task: `\b` immediately before the `+263` alternative can
        // only match where the character preceding `+` is itself a word
        // character (`\b` requires a word/non-word transition on either
        // side) — but `+` is realistically always preceded by whitespace,
        // punctuation, or the start of the prompt, none of which are word
        // characters, so this branch never actually fires in normal usage.
        // Fixing PHONE_RE itself is out of scope here — this is a separate,
        // pre-existing regex bug, not the format-variance gap this task's
        // fallback detector addresses — so it's documented as a known
        // limitation (also noted in Questions.md) rather than silently
        // left untested.
        let rule = rule_for("phone_number");
        assert!(!rule.regex.is_match("Call them on +263771234567 today."));
    }

    #[test]
    fn phone_number_negative_unsupported_prefix_does_not_match() {
        // "076" isn't one of the three documented mobile prefixes (071/073/
        // 077/078) — deliberately not matched (see rules.rs's own honesty
        // note on landlines/other prefixes not being covered).
        let rule = rule_for("phone_number");
        assert!(!rule.regex.is_match("0761234567"));
    }

    // --- credit_card: 3 formats, 1 edge case (Amex 15-digit), 1 negative
    // (format-valid but Luhn-invalid, exercised end-to-end in scan.rs) -----

    #[test]
    fn credit_card_matches_spaced_and_dashed_formats() {
        let rule = rule_for("credit_card");
        assert!(rule.regex.is_match("4111 1111 1111 1111"));
        assert!(rule.regex.is_match("4111-1111-1111-1111"));
        assert!(rule.regex.is_match("4111111111111111"));
    }

    #[test]
    fn credit_card_edge_case_amex_fifteen_digit_format_matches() {
        let rule = rule_for("credit_card");
        assert!(rule.regex.is_match("378282246310005")); // well-known Amex test number
        assert!(luhn_check("378282246310005"));
    }

    #[test]
    fn credit_card_negative_wrong_length_does_not_match() {
        let rule = rule_for("credit_card");
        // Prefix group (4 digits) plus only ONE more 4-digit group — one
        // short of the pattern's minimum of two repeat groups (12 digits
        // total), so it must not match.
        assert!(!rule.regex.is_match("4111 1111"));
    }

    // --- api_key: 3 specific-prefix formats, 1 edge case, 1 negative ------

    #[test]
    fn api_key_matches_known_specific_prefixes() {
        let rule = rule_for("api_key");
        assert!(rule.regex.is_match("sk-liveTestKeyAbcdefghijklmnop123456"));
        assert!(rule.regex.is_match("AKIAABCDEFGHIJKLMNOP"));
        assert!(rule.regex.is_match("ghp_abcdefghijklmnopqrstuvwxyz0123456789"));
    }

    #[test]
    fn api_key_edge_case_other_github_token_prefixes_match() {
        let rule = rule_for("api_key");
        assert!(rule.regex.is_match("gho_abcdefghijklmnopqrstuvwxyz0123456789"));
        assert!(rule.regex.is_match("ghs_abcdefghijklmnopqrstuvwxyz0123456789"));
    }

    #[test]
    fn api_key_negative_ordinary_word_does_not_match_specific_pattern() {
        // Must not match the SPECIFIC-prefix pattern (the generic 32+-char
        // fallback is a separate, deliberately low-confidence rule, tested
        // in scan.rs's own `generic_low_confidence_token_fails_closed`).
        let rule = rule_for("api_key");
        assert!(!rule.regex.is_match("sk8ing"));
    }

    // --- medical_record_no: 3 formats, 1 edge case, 1 negative ------------

    #[test]
    fn medical_record_no_matches_plausible_formats() {
        let rule = rule_for("medical_record_no");
        assert!(rule.regex.is_match("MRN-204981"));
        assert!(rule.regex.is_match("MRN204981")); // no separator
        assert!(rule.regex.is_match("mrn 204981")); // lowercase, space-separated
    }

    #[test]
    fn medical_record_no_edge_case_minimum_digit_count_matches() {
        let rule = rule_for("medical_record_no");
        assert!(rule.regex.is_match("MRN12345")); // 5 digits, the documented floor
    }

    #[test]
    fn medical_record_no_negative_missing_prefix_does_not_match() {
        let rule = rule_for("medical_record_no");
        assert!(!rule.regex.is_match("204981")); // bare digits, no MRN prefix at all
    }

    // --- Luhn checksum, used to gate credit_card matches -------------------

    #[test]
    fn luhn_check_rejects_short_input() {
        assert!(!luhn_check("123"));
    }
}
