use once_cell::sync::Lazy;
use regex::Regex;

/// One compiled detection rule. `confidence` reflects how much we trust a
/// match from this specific pattern — used by the fail-closed logic in
/// `scan.rs`: a match from a well-formed, specific pattern is trusted; a
/// match from a loose generic pattern is not, and blocks rather than
/// forwards. See docs/ARCHITECTURE.md for the full honesty note on what
/// these patterns can and can't actually guarantee.
pub struct Rule {
    pub entity_type: &'static str,
    pub regex: &'static Lazy<Regex>,
    pub confidence: f32,
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
            confidence: 0.97, // additionally gated by a real Luhn check in scan.rs
        },
        Rule {
            entity_type: "national_id",
            regex: &NATIONAL_ID_RE,
            confidence: 0.85,
        },
        Rule {
            entity_type: "phone_number",
            regex: &PHONE_RE,
            confidence: 0.9,
        },
        Rule {
            entity_type: "api_key",
            regex: &API_KEY_SPECIFIC_RE,
            confidence: 0.95,
        },
        Rule {
            entity_type: "medical_record_no",
            regex: &MEDICAL_RECORD_RE,
            confidence: 0.75,
        },
        Rule {
            entity_type: "bank_account",
            regex: &BANK_ACCOUNT_RE,
            confidence: 0.5, // generic digit run — genuinely low confidence, by design
        },
        Rule {
            entity_type: "api_key",
            regex: &API_KEY_GENERIC_RE,
            confidence: 0.45, // generic fallback — genuinely low confidence, by design
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
