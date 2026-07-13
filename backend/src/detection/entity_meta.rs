//! Central registry of per-entity-type context keywords, shared by:
//!   - `fallback.rs`'s generic structured-identifier detector (keyword
//!     proximity gating — the actual fix for the "12345678ACD" bug, see
//!     that module's doc comment), and
//!   - `scan.rs`'s reason-string generation, so a fallback match's
//!     explanation can name which specific keyword and which entity type's
//!     keyword list it came from.
//!
//! `health_rules::sensitivity_class` stays the single source of truth for
//! entity_type -> SensitivityClass (unchanged by this module, per that
//! file's own doc comment) — this registry only adds keyword lists, it does
//! not duplicate the sensitivity mapping.
//!
//! Keyword lists here are illustrative, same honesty standard as every
//! regex/dictionary in `rules.rs` and `health_rules.rs`: a reasonable
//! best-effort set of phrases a person would plausibly write next to that
//! kind of identifier, not an exhaustive or validated list. Missing a
//! keyword is a false negative on the fallback path only — the primary
//! per-type regex detectors are unaffected either way.

/// One entity type's fallback-relevant metadata: the keywords that, when
/// found near an unclassified structured-looking token, suggest that token
/// is an instance of this entity type.
pub struct EntityKeywords {
    pub entity_type: &'static str,
    pub keywords: &'static [&'static str],
}

/// Every entity type in the system that has a meaningful "context keyword"
/// notion — i.e. everything except the two detectors that don't work off a
/// nearby-keyword idea at all (`credit_card`, gated by Luhn instead, and
/// `full_name`, gated by the capitalization heuristic instead). Listed
/// explicitly (not derived) so it's visible in one place which entity types
/// participate in fallback keyword matching, same rationale as
/// `health_rules::sensitivity_class`'s explicit-listing doc comment.
// NOTE: every keyword below is written WITHOUT trailing punctuation (no
// "id:", no "ref:") even though people naturally type it that way ("ID:
// 12345"). That's intentional, not an oversight: `Token::normalize` (see
// `tokenize.rs`) strips leading/trailing punctuation before comparison, so
// the punctuation-bearing form would never match anything and would be a
// silently dead list entry. The punctuation is still handled correctly at
// match time — normalization is what makes "ID:" match a "id" keyword.
pub const ENTITY_KEYWORDS: &[EntityKeywords] = &[
    EntityKeywords {
        entity_type: "national_id",
        keywords: &["national id", "id number", "id no", "id", "identity number"],
    },
    EntityKeywords {
        entity_type: "bank_account",
        keywords: &["account no", "account number", "acc no", "iban", "sort code"],
    },
    EntityKeywords {
        entity_type: "phone_number",
        keywords: &["phone", "cell", "mobile", "contact number", "tel", "telephone"],
    },
    EntityKeywords {
        entity_type: "api_key",
        keywords: &["api key", "token", "secret", "key", "credentials"],
    },
    EntityKeywords {
        entity_type: "medical_record_no",
        keywords: &["mrn", "medical record", "record no", "record number"],
    },
    // Not a real detectable entity_type — a keyword-source TAG that exists
    // purely so `fallback.rs`'s sensitivity inference can implement the
    // task's own worked example ("proximity to 'Patient' implies
    // special_category_health") without altering `medical_record_no`'s own
    // separately-documented, deliberate `Standard` classification (see
    // `health_rules::sensitivity_class`'s doc comment — that scope decision
    // predates this module and is intentionally left unchanged). Any
    // "Patient ..." phrase implies a health context regardless of which
    // specific structured entity type it turns out to be attached to, which
    // is exactly the judgment call this pseudo-type encodes. See
    // `health_rules::sensitivity_class`'s own match arm for where this
    // resolves to `SpecialCategoryHealth`.
    EntityKeywords {
        entity_type: "patient_context",
        keywords: &["patient", "patient id", "patient number", "patient record"],
    },
    EntityKeywords {
        entity_type: "diagnosis_code",
        keywords: &["diagnosis", "icd", "icd-10", "condition code", "dx"],
    },
    EntityKeywords {
        entity_type: "medication_name",
        keywords: &["medication", "prescribed", "prescription", "dosage", "rx"],
    },
    EntityKeywords {
        entity_type: "medical_aid_number",
        keywords: &[
            "medical aid",
            "medical aid number",
            "membership no",
            "membership number",
            "scheme no",
            "scheme number",
        ],
    },
    EntityKeywords {
        entity_type: "lab_result_value",
        keywords: &["lab result", "test result", "result", "lab ref"],
    },
    EntityKeywords {
        entity_type: "next_of_kin",
        keywords: &["next of kin", "emergency contact", "guardian"],
    },
    // A deliberately entity-agnostic list: phrases that mark "this token is
    // *some* kind of identifier or reference" without implying a specific
    // entity type on their own. Modeled as its own pseudo-entity_type
    // ("identifier_generic") purely so it has a sensitivity_class lookup
    // (Standard, via health_rules::sensitivity_class's default arm) — it is
    // never itself emitted as a match's entity_type. Deliberately narrow
    // (just "ref"/"reference" variants, not bare "no" or "number") — those
    // two are common enough English words that including them here would
    // gate the fallback open on almost any nearby alphanumeric-looking
    // token, defeating the point of requiring a keyword at all.
    EntityKeywords {
        entity_type: "identifier_generic",
        keywords: &["ref", "reference", "reference no", "reference number"],
    },
];

/// The longest keyword phrase, in words, across the whole registry — bounds
/// how many tokens the fallback detector needs to join when checking for a
/// phrase match at each position. Computed at compile time isn't possible
/// with `const fn` over `&str::split`, so this is a documented manual bound
/// instead: the longest phrase above is "reference number" / "next of kin"
/// / "membership number" at 2-3 words. Verified by a test below so this
/// constant can't silently drift out of sync with the list.
pub const MAX_KEYWORD_WORDS: usize = 3;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_keyword_words_bound_is_not_exceeded() {
        for group in ENTITY_KEYWORDS {
            for kw in group.keywords {
                let words = kw.split_whitespace().count();
                assert!(
                    words <= MAX_KEYWORD_WORDS,
                    "keyword '{kw}' for {} has {words} words, exceeds MAX_KEYWORD_WORDS",
                    group.entity_type
                );
            }
        }
    }

    #[test]
    fn all_keywords_are_lowercase() {
        for group in ENTITY_KEYWORDS {
            for kw in group.keywords {
                assert_eq!(*kw, kw.to_lowercase(), "keyword '{kw}' must be lowercase (matched against normalized tokens)");
            }
        }
    }
}
