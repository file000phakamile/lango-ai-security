//! Plain-language entity descriptions for end-user-facing messages.
//!
//! `scan.rs`'s `reason_string` (audit log, compliance review) and this
//! module's `user_message` serve two different readers and must NOT be
//! conflated: `reason_string` is deliberately as technical as it needs to be
//! (entity type names, confidence scores, which specific rule fired) for a
//! compliance officer auditing a decision later. `user_message` is what the
//! browser extension shows the person who just typed the prompt, in the
//! moment — it must never expose entity_type strings, confidence numbers, or
//! detector/rule names, only what kind of information was involved and why
//! caution was applied, in ordinary language.
//!
//! This mapping is deliberately short-phrase, not a full sentence — callers
//! compose it into a sentence (see `scan::blocked_outcome`).

/// Maps an internal `entity_type` string to a short, human phrase describing
/// what it protects. Unknown/future entity types fall back to a generic
/// phrase rather than leaking the raw internal name — same "default to a
/// safe generic rather than panic or expose internals" convention already
/// used by `health_rules::sensitivity_class`'s default arm.
pub fn phrase_for(entity_type: &str) -> &'static str {
    match entity_type {
        "national_id" => "a national ID number",
        "bank_account" => "a bank account number",
        "phone_number" => "a phone number",
        "credit_card" => "a credit card number",
        "api_key" => "an API key or access token",
        "medical_record_no" => "a medical record number",
        "full_name" => "a person's name",
        "diagnosis_code" => "a medical diagnosis",
        "medication_name" => "a medication name",
        "medical_aid_number" => "a medical aid membership number",
        "lab_result_value" => "a lab test result",
        "next_of_kin" => "a contact name",
        // See fallback.rs — a generic, keyword-gated identifier match whose
        // specific format wasn't recognized by any of the types above.
        "probable_identifier" => "an ID number or reference code",
        _ => "sensitive information",
    }
}

/// Joins a list of entity types into a single plain-language phrase,
/// deduplicating by PHRASE (not by entity_type — `next_of_kin` and
/// `full_name` would otherwise both surface as "a person's name" / "a
/// contact name" separately even though a user doesn't need to see that
/// distinction twice) and preserving first-seen order, e.g.
/// `["bank_account", "next_of_kin"]` -> `"a bank account number and a
/// contact name"`.
///
/// Returns "sensitive information" for an empty input rather than an empty
/// string — this is only ever called with at least one match in practice
/// (see `scan::blocked_outcome`), but a caller composing a sentence around
/// this shouldn't have to special-case an empty result.
pub fn describe(entity_types: &[&str]) -> String {
    join_with_and(&unique_phrases(entity_types))
}

/// How many DISTINCT phrases `describe` would join — a caller building a
/// grammatically-correct sentence around it ("was" vs. "were") needs this
/// deduplicated count, not `entity_types.len()`: two raw matches that
/// dedupe to the same phrase (e.g. two `bank_account` matches) must read as
/// singular, not plural.
pub fn unique_phrase_count(entity_types: &[&str]) -> usize {
    unique_phrases(entity_types).len()
}

fn unique_phrases(entity_types: &[&str]) -> Vec<&'static str> {
    let mut seen = std::collections::HashSet::new();
    let mut phrases: Vec<&'static str> = Vec::new();
    for &entity_type in entity_types {
        let phrase = phrase_for(entity_type);
        if seen.insert(phrase) {
            phrases.push(phrase);
        }
    }
    phrases
}

fn join_with_and(items: &[&str]) -> String {
    match items {
        [] => "sensitive information".to_string(),
        [only] => only.to_string(),
        [first, second] => format!("{first} and {second}"),
        _ => {
            let (last, rest) = items.split_last().expect("non-empty per the match arms above");
            format!("{}, and {last}", rest.join(", "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_entity_types_get_specific_phrases() {
        assert_eq!(phrase_for("bank_account"), "a bank account number");
        assert_eq!(phrase_for("next_of_kin"), "a contact name");
        assert_eq!(phrase_for("diagnosis_code"), "a medical diagnosis");
    }

    #[test]
    fn unknown_entity_type_falls_back_to_generic_phrase_not_the_raw_name() {
        assert_eq!(phrase_for("some_future_entity_type"), "sensitive information");
    }

    #[test]
    fn describe_single_entity() {
        assert_eq!(describe(&["bank_account"]), "a bank account number");
    }

    #[test]
    fn describe_two_entities_joins_with_and() {
        assert_eq!(
            describe(&["bank_account", "next_of_kin"]),
            "a bank account number and a contact name"
        );
    }

    #[test]
    fn describe_three_entities_uses_oxford_comma() {
        assert_eq!(
            describe(&["bank_account", "next_of_kin", "diagnosis_code"]),
            "a bank account number, a contact name, and a medical diagnosis"
        );
    }

    #[test]
    fn describe_deduplicates_by_phrase_not_by_entity_type() {
        // Two different entity_type strings that map to the SAME plain
        // phrase must only surface once — a user doesn't need "a person's
        // name and a contact name" when both mean the same thing to them.
        assert_eq!(describe(&["bank_account", "bank_account"]), "a bank account number");
    }

    #[test]
    fn describe_never_exposes_raw_entity_type_strings() {
        let result = describe(&["bank_account", "next_of_kin", "national_id"]);
        assert!(!result.contains("bank_account"));
        assert!(!result.contains("next_of_kin"));
        assert!(!result.contains("national_id"));
    }
}
