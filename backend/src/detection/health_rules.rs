//! Health-specific entity detection — built for the Cimas Healthathon 3.0
//! submission (a separate competition from this repo's AI4I materials; see
//! docs/HEALTH_MODULE.md). Additive only: nothing here changes the existing
//! seven entity types in `rules.rs` or `name_heuristic.rs`.
//!
//! This file introduces two things:
//!
//! 1. Five new detectors (diagnosis codes, medications, medical aid numbers,
//!    lab result values, next-of-kin names) — see each detector's own doc
//!    comment for its specific honesty caveat. Every one of these is a
//!    best-effort, illustrative-subset pattern, same honesty standard as the
//!    existing `national_id`/`bank_account`/etc. patterns in `rules.rs`.
//! 2. `SensitivityClass`, a NEW axis independent of detection confidence —
//!    see its own doc comment below. Do not conflate this with the
//!    confidence tiers in `scan.rs`.

use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;

/// A single detected health-entity occurrence — same shape as `scan.rs`'s
/// private `Match` struct (byte offsets into the original prompt, not the
/// dictionary-decoded value), kept as a separate public type here so this
/// module doesn't need visibility into `scan.rs`'s private struct.
pub struct HealthMatch {
    pub entity_type: &'static str,
    pub start: usize,
    pub end: usize,
    pub confidence: f32,
}

// ---------------------------------------------------------------------------
// Sensitivity classification — a NEW, INDEPENDENT axis from confidence.
//
// Confidence (scan.rs) answers "how sure are we this match is real?".
// Sensitivity class answers a completely different question: "if this match
// IS real, how sensitive is the underlying category of data, independent of
// how sure we are?" A national ID and a diagnosis code can both be detected
// at 0.90 confidence, but only the diagnosis code carries special-category
// health data — exposure of HIV status, for instance, carries a distinct,
// additional harm (stigma, discrimination) beyond ordinary PII exposure,
// which is exactly why Part 2's hard rule below exists.
//
// This function is the single source of truth for the mapping — both
// `rules.rs`'s existing seven entity types and this file's five new ones are
// listed explicitly, so the classification of every entity type in the
// system is visible in one place. Per the task that introduced this axis:
// every existing entity type (national_id, bank_account, phone_number,
// credit_card, medical_record_no, api_key, full_name) keeps `Standard`
// unchanged — including `medical_record_no`, even though a hospital record
// number is intuitively "health-adjacent". That's a deliberate scope
// decision, not an oversight: the task that introduced this axis explicitly
// listed which five entity types get `SpecialCategoryHealth`, and
// `medical_record_no` (an existing type, predating this module) was not one
// of them. Reclassifying it was out of scope for an additive change — an
// institution adopting this module could revisit that judgment call later.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SensitivityClass {
    Standard,
    SpecialCategoryHealth,
}

impl SensitivityClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            SensitivityClass::Standard => "standard",
            SensitivityClass::SpecialCategoryHealth => "special_category_health",
        }
    }
}

pub fn sensitivity_class(entity_type: &str) -> SensitivityClass {
    match entity_type {
        "diagnosis_code" | "medication_name" | "medical_aid_number" | "lab_result_value"
        | "next_of_kin" => SensitivityClass::SpecialCategoryHealth,
        // "patient_context" is NOT a real, independently-detectable entity
        // type — it never appears as a match's `entity_type` in
        // `entities_detected`. It exists purely as a keyword-source tag
        // consumed by `fallback.rs` (see `entity_meta.rs`'s doc comment on
        // the group of the same name) so the generic structured-identifier
        // fallback can implement the task's own worked example ("proximity
        // to 'Patient' implies special_category_health") without touching
        // `medical_record_no`'s separately-documented Standard
        // classification below.
        "patient_context" => SensitivityClass::SpecialCategoryHealth,
        // Every other entity type — the five new ones handled above are the
        // ONLY special-category types; everything else (including future
        // types someone forgets to list here) defaults to Standard rather
        // than silently panicking on an unknown string.
        _ => SensitivityClass::Standard,
    }
}

// ---------------------------------------------------------------------------
// 1. Diagnosis / ICD-10 codes
//
// Two-part detector, exactly as specified: a general ICD-10 *shape* regex
// (a letter — excluding U, reserved for provisional WHO codes — followed by
// two digits, with an optional decimal-point subcategory of 1-4 more
// characters, e.g. "B20", "E11.9") gates against a small illustrative
// dictionary of ~40 codes relevant to the Zimbabwean context, sourced from
// publicly documented ICD-10 chapter listings (WHO ICD-10 chapter index).
//
// *** HONESTY NOTE, same standard as the existing name-heuristic caveat: ***
// Real ICD-10 has tens of thousands of codes across 22 chapters. This
// dictionary covers ~40 of them — HIV/AIDS, TB, malaria, diabetes,
// hypertension, and common maternal-health codes, the conditions most
// relevant to a Zimbabwean primary/district health context — NOT a complete
// ICD-10 implementation. A shape-valid code that isn't in this dictionary
// (the overwhelming majority of real ICD-10) is silently missed: a false
// negative, not a false positive. That tradeoff is deliberate — gating on
// the dictionary keeps the false-positive rate low (a bare "B20"-shaped
// token that ISN'T a real diagnosis code, e.g. a product model number,
// never matches), at the direct cost of not recognising most real codes.
// See docs/DATA_AI_USAGE.md and docs/HEALTH_MODULE.md for the same caveat
// stated for an end-user audience.
//
// The plain-language condition names in this dictionary exist for internal
// documentation purposes ONLY — deliberately never surfaced by any
// detector, API response, or audit-log row. Only the entity_type label
// "diagnosis_code" is ever returned or stored; decoding B20 into "HIV
// disease" anywhere in a response would itself defeat the stigma-aware
// design principle in Part 3 (see docs/SECURITY_PRIVACY.md and
// `routes/health.rs`) by re-attaching a specific condition to a record.
// ---------------------------------------------------------------------------
static ICD10_SHAPE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b[A-TV-Z][0-9]{2}(?:\.[0-9A-Z]{1,4})?\b").unwrap());

static ICD10_DICTIONARY: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    [
        // HIV/AIDS
        ("B20", "HIV disease (HIV/AIDS)"),
        ("B21", "HIV disease resulting in malignant neoplasms"),
        ("B22", "HIV disease resulting in other specified diseases"),
        ("B23", "HIV disease resulting in other conditions"),
        ("B24", "HIV disease, unspecified"),
        ("Z21", "Asymptomatic HIV infection status"),
        ("Z71.7", "HIV pre-test/post-test counselling"),
        // Tuberculosis
        ("A15", "Respiratory tuberculosis, bacteriologically confirmed"),
        ("A16", "Respiratory tuberculosis, not bacteriologically confirmed"),
        ("A17", "Tuberculosis of the nervous system"),
        ("A18", "Tuberculosis of other organs"),
        ("A19", "Miliary tuberculosis"),
        // Malaria
        ("B50", "Plasmodium falciparum malaria"),
        ("B51", "Plasmodium vivax malaria"),
        ("B52", "Plasmodium malariae malaria"),
        ("B53", "Other parasitologically confirmed malaria"),
        ("B54", "Malaria, unspecified"),
        // Diabetes
        ("E10", "Type 1 diabetes mellitus"),
        ("E11", "Type 2 diabetes mellitus"),
        ("E11.9", "Type 2 diabetes mellitus, without complications"),
        ("E13", "Other specified diabetes mellitus"),
        // Hypertension
        ("I10", "Essential (primary) hypertension"),
        ("I11", "Hypertensive heart disease"),
        ("I12", "Hypertensive chronic kidney disease"),
        ("I15", "Secondary hypertension"),
        // Maternal health
        ("O10", "Pre-existing hypertension complicating pregnancy"),
        ("O14", "Pre-eclampsia"),
        ("O15", "Eclampsia"),
        ("O24", "Diabetes mellitus arising in pregnancy"),
        ("O60", "Preterm labour"),
        ("O72", "Postpartum haemorrhage"),
        ("Z34", "Supervision of normal pregnancy"),
        ("O80", "Single spontaneous delivery"),
        // Other common conditions seen in a Zimbabwean primary-care context
        ("J45", "Asthma"),
        ("J18", "Pneumonia, organism unspecified"),
        ("N18", "Chronic kidney disease"),
        ("F32", "Depressive episode"),
        ("F41", "Anxiety disorder"),
        ("K21", "Gastro-oesophageal reflux disease"),
        ("M54.5", "Low back pain"),
        ("G43", "Migraine"),
    ]
    .into_iter()
    .collect()
});

pub fn detect_diagnosis_codes(text: &str) -> Vec<HealthMatch> {
    ICD10_SHAPE_RE
        .find_iter(text)
        .filter(|m| ICD10_DICTIONARY.contains_key(m.as_str()))
        .map(|m| HealthMatch {
            entity_type: "diagnosis_code",
            start: m.start(),
            end: m.end(),
            confidence: 0.90, // exact dictionary match on a well-formed code — high confidence, same tier as api_key's specific-prefix match
        })
        .collect()
}

// ---------------------------------------------------------------------------
// 2. Medication names
//
// Dictionary-based, case-insensitive whole-word match against ~50 medicines
// relevant to Zimbabwe's essential-medicines context — ART/antiretrovirals,
// TB treatment, antimalarials, and common chronic-disease medications.
//
// *** HONESTY NOTE, same standard as the ICD-10 dictionary above: *** this
// is an illustrative subset (~50 names), not Zimbabwe's full Essential Drugs
// List (EDLIZ), which runs to several hundred entries across many more drug
// classes. A real medication not on this list is silently missed (false
// negative) rather than guessed at.
// ---------------------------------------------------------------------------
const MEDICATION_NAMES: &[&str] = &[
    // ART / antiretrovirals
    "Tenofovir",
    "Lamivudine",
    "Dolutegravir",
    "Efavirenz",
    "Nevirapine",
    "Zidovudine",
    "Abacavir",
    "Atazanavir",
    "Ritonavir",
    "Lopinavir",
    "Truvada",
    "Emtricitabine",
    // TB treatment
    "Rifampicin",
    "Isoniazid",
    "Pyrazinamide",
    "Ethambutol",
    "Bedaquiline",
    "Rifafour",
    // Antimalarials
    "Artemether",
    "Lumefantrine",
    "Coartem",
    "Artesunate",
    "Fansidar",
    "Sulfadoxine",
    "Pyrimethamine",
    "Quinine",
    "Chloroquine",
    // Common chronic-disease medications
    "Metformin",
    "Insulin",
    "Amlodipine",
    "Enalapril",
    "Hydrochlorothiazide",
    "Atorvastatin",
    "Losartan",
    "Bisoprolol",
    "Furosemide",
    "Digoxin",
    "Warfarin",
    "Levothyroxine",
    // Other common medications
    "Cotrimoxazole",
    "Omeprazole",
    "Salbutamol",
    "Prednisolone",
    "Amoxicillin",
    "Ciprofloxacin",
    "Metronidazole",
    "Paracetamol",
    "Ibuprofen",
    "Aspirin",
    "Diazepam",
    "Fluoxetine",
    "Amitriptyline",
];

static MEDICATION_RE: Lazy<Regex> = Lazy::new(|| {
    let alternation = MEDICATION_NAMES
        .iter()
        .map(|n| regex::escape(n))
        .collect::<Vec<_>>()
        .join("|");
    Regex::new(&format!(r"(?i)\b(?:{alternation})\b")).unwrap()
});

pub fn detect_medications(text: &str) -> Vec<HealthMatch> {
    MEDICATION_RE
        .find_iter(text)
        .map(|m| HealthMatch {
            entity_type: "medication_name",
            start: m.start(),
            end: m.end(),
            confidence: 0.85, // exact dictionary match, case-insensitive — still a specific enough name that a false positive is unlikely
        })
        .collect()
}

// ---------------------------------------------------------------------------
// 3. Medical aid / membership numbers
//
// Zimbabwean medical aid societies (Cimas, PSMAS, First Mutual Health, etc.)
// each define their own membership-number format, none of which is publicly
// documented in a way this codebase has access to — same situation as
// `rules.rs`'s existing `bank_account`/`medical_record_no` patterns. This is
// a GENERIC best-effort format only: 2-6 uppercase letters (a scheme-code-
// style prefix) directly followed by 6-9 digits, optionally dash-separated
// — NOT validated against any specific real provider's actual format.
// Confidence is deliberately low (below the 0.60 fail-closed threshold in
// scan.rs), same honesty framing as `bank_account`'s 0.5 — a real
// institution onboarding for real would need to supply its own verified
// pattern per scheme, the same way `detection_rules` already anticipates for
// other types.
// ---------------------------------------------------------------------------
static MEDICAL_AID_NUMBER_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b[A-Z]{2,6}-?\d{6,9}\b").unwrap());

pub fn detect_medical_aid_numbers(text: &str) -> Vec<HealthMatch> {
    MEDICAL_AID_NUMBER_RE
        .find_iter(text)
        .map(|m| HealthMatch {
            entity_type: "medical_aid_number",
            start: m.start(),
            end: m.end(),
            confidence: 0.55, // generic, unvalidated format — deliberately low, same tier as bank_account
        })
        .collect()
}

// ---------------------------------------------------------------------------
// 4. Lab result values
//
// Genuinely hard to pattern-match generically, stated plainly per the task
// that introduced this: a bare number ("142", "7.2 mmol/L") is not
// identifiable as sensitive on its own — it could be anything, a page
// number, a quantity, a price. The only thing that makes a number a lab
// *result* is context. So this detector is deliberately conservative: it
// only flags a lab-value-shaped number when it appears within a small
// (25-character) token window immediately after a recognised lab test name
// from a short illustrative list (CD4 count, viral load, HbA1c, haemoglobin,
// etc.) — never a bare number on its own, no matter how lab-value-shaped it
// looks.
//
// *** HONESTY NOTE: false negatives are expected and accepted here — a lab
// value phrased unusually ("the CD4 was two-fifty" or with the test name and
// value far apart) will be missed. That is the deliberately safer failure
// mode: false positives on ordinary numbers (a phone number, a price, a
// page reference) would be far more disruptive than an occasional missed
// lab value, given how numeric a normal prompt already is. ***
//
// Only the numeric value (plus an optional unit) is treated as the sensitive
// span and redacted — the test-name keyword itself ("CD4 count") is left
// visible in the redacted prompt, since the test name alone isn't sensitive,
// only the value attached to it is.
// ---------------------------------------------------------------------------
static LAB_TEST_KEYWORD_RE: Lazy<Regex> = Lazy::new(|| {
    // Ordered longest-phrase-first: Rust's regex alternation is
    // leftmost-first, not longest-match — if "CD4" were listed before "CD4
    // count", the engine would stop at the shorter alternative and never
    // try the longer one for the same starting position.
    Regex::new(
        r"(?i)\b(?:CD4 count|white blood cell count|platelet count|blood glucose|viral load|haemoglobin|hemoglobin|creatinine|glucose|HbA1c|CD4|WBC|Hb)\b",
    )
    .unwrap()
});

static LAB_VALUE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\d+(?:\.\d+)?\s*(?:mmol/L|mg/dL|g/dL|cells/mm3|cells/mm³|%)?").unwrap()
});

/// How many bytes after a recognised lab-test keyword a numeric value must
/// appear within to count as "attached to" that test — the "small token
/// window" the task specifies, not an exact word count (byte-based is a
/// simpler, sufficient approximation for this illustrative detector).
const LAB_VALUE_WINDOW_BYTES: usize = 25;

pub fn detect_lab_result_values(text: &str) -> Vec<HealthMatch> {
    let mut out = Vec::new();
    for kw in LAB_TEST_KEYWORD_RE.find_iter(text) {
        let rest = &text[kw.end()..];
        if let Some(val) = LAB_VALUE_RE.find(rest) {
            if val.start() <= LAB_VALUE_WINDOW_BYTES {
                out.push(HealthMatch {
                    entity_type: "lab_result_value",
                    start: kw.end() + val.start(),
                    end: kw.end() + val.end(),
                    confidence: 0.80, // gated by proximity to a known test name — reasonably confident when it fires at all, deliberately narrow about when it fires
                });
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// 5. Next-of-kin / emergency contact
//
// NOT a standalone detector — a contextual reclassification of the existing
// `full_name` heuristic (`name_heuristic.rs`). A name is tagged
// `next_of_kin` instead of plain `full_name` only when it appears within a
// small window of one of the keywords "next of kin", "emergency contact", or
// "guardian". `scan.rs` calls `is_next_of_kin_context` once per name match
// already produced by `name_heuristic::detect_names` and swaps the
// entity_type accordingly — this file adds no new name-finding logic of its
// own, reusing the same heuristic (and its same honesty caveats) unchanged.
//
// Confidence is NOT bumped up for this reclassification — it stays at
// whatever `name_heuristic.rs` assigned (currently a fixed 0.55, the same
// value every full_name match gets). One direct, deliberate consequence of
// that, combined with Part 2's hard rule below: since 0.55 is below
// scan.rs's CONFIDENCE_THRESHOLD (0.60) and `next_of_kin` is barred from the
// tier-2 leniency band that plain `full_name` gets, a next-of-kin match at
// today's heuristic confidence will currently always fail closed
// (`blocked_low_confidence`), never `redacted_and_forwarded`. That is not a
// bug — it is Part 2's "health data doesn't get the relaxed treatment names
// get" principle playing out concretely, the same way
// `NAME_LOW_CONFIDENCE_FLOOR`'s doc comment in scan.rs already notes for the
// (currently unreached) full_name near-zero tier.
// ---------------------------------------------------------------------------
static NEXT_OF_KIN_KEYWORD_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\b(?:next of kin|emergency contact|guardian)\b").unwrap());

/// How many bytes of separation between a next-of-kin keyword and a name
/// still counts as "in proximity to" that keyword.
const NEXT_OF_KIN_WINDOW_BYTES: usize = 50;

pub fn is_next_of_kin_context(text: &str, name_start: usize, name_end: usize) -> bool {
    NEXT_OF_KIN_KEYWORD_RE.find_iter(text).any(|kw| {
        let near_before = name_start >= kw.end() && name_start - kw.end() <= NEXT_OF_KIN_WINDOW_BYTES;
        let near_after = kw.start() >= name_end && kw.start() - name_end <= NEXT_OF_KIN_WINDOW_BYTES;
        near_before || near_after
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sensitivity_class_covers_all_entity_types() {
        assert_eq!(sensitivity_class("national_id"), SensitivityClass::Standard);
        assert_eq!(sensitivity_class("bank_account"), SensitivityClass::Standard);
        assert_eq!(sensitivity_class("phone_number"), SensitivityClass::Standard);
        assert_eq!(sensitivity_class("credit_card"), SensitivityClass::Standard);
        assert_eq!(sensitivity_class("medical_record_no"), SensitivityClass::Standard);
        assert_eq!(sensitivity_class("api_key"), SensitivityClass::Standard);
        assert_eq!(sensitivity_class("full_name"), SensitivityClass::Standard);

        assert_eq!(
            sensitivity_class("diagnosis_code"),
            SensitivityClass::SpecialCategoryHealth
        );
        assert_eq!(
            sensitivity_class("medication_name"),
            SensitivityClass::SpecialCategoryHealth
        );
        assert_eq!(
            sensitivity_class("medical_aid_number"),
            SensitivityClass::SpecialCategoryHealth
        );
        assert_eq!(
            sensitivity_class("lab_result_value"),
            SensitivityClass::SpecialCategoryHealth
        );
        assert_eq!(
            sensitivity_class("next_of_kin"),
            SensitivityClass::SpecialCategoryHealth
        );
    }

    #[test]
    fn detects_known_icd10_code_in_sentence() {
        let matches = detect_diagnosis_codes("Patient presents with B20 and requires ART initiation.");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].entity_type, "diagnosis_code");
    }

    #[test]
    fn ignores_shape_valid_code_not_in_dictionary() {
        // "Q99" is a plausible ICD-10 shape but deliberately not in the
        // illustrative dictionary — must be silently missed (false
        // negative), not guessed at.
        let matches = detect_diagnosis_codes("Reference code Q99 needs follow-up.");
        assert!(matches.is_empty());
    }

    #[test]
    fn detects_medication_name_case_insensitively() {
        let matches = detect_medications("Please refill tenofovir for the patient.");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].entity_type, "medication_name");
    }

    #[test]
    fn detects_medical_aid_number_shape() {
        let matches = detect_medical_aid_numbers("Confirm medical aid number CIMAS123456 is active.");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].entity_type, "medical_aid_number");
    }

    #[test]
    fn detects_lab_value_near_test_name_but_redacts_only_the_number() {
        let matches = detect_lab_result_values("CD4 count 250 cells/mm3, schedule a follow-up review.");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].entity_type, "lab_result_value");
        let text = "CD4 count 250 cells/mm3, schedule a follow-up review.";
        let matched_span = &text[matches[0].start..matches[0].end];
        assert!(matched_span.contains("250"));
        assert!(!matched_span.contains("CD4")); // test name itself must NOT be part of the redacted span
    }

    #[test]
    fn does_not_flag_a_bare_number_with_no_lab_test_context() {
        let matches = detect_lab_result_values("Please call the client on 0771234567 about their order of 142 units.");
        assert!(matches.is_empty());
    }

    #[test]
    fn recognises_next_of_kin_context_near_a_name() {
        let text = "Next of kin: Rutendo Gumbo, please contact if condition worsens.";
        // "Rutendo Gumbo" starts at byte 13, ends at byte 26 in this string.
        let start = text.find("Rutendo Gumbo").unwrap();
        let end = start + "Rutendo Gumbo".len();
        assert!(is_next_of_kin_context(text, start, end));
    }

    #[test]
    fn does_not_flag_an_unrelated_name_as_next_of_kin() {
        let text = "Please schedule Rutendo Gumbo for a follow-up appointment next week.";
        let start = text.find("Rutendo Gumbo").unwrap();
        let end = start + "Rutendo Gumbo".len();
        assert!(!is_next_of_kin_context(text, start, end));
    }

    // --- diagnosis_code: 2 more positive formats, 1 edge case, negative
    // already covered above (ignores_shape_valid_code_not_in_dictionary) --

    #[test]
    fn detects_diagnosis_code_subcategory_format() {
        let matches = detect_diagnosis_codes("Assessment: E11.9, continue current management.");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].entity_type, "diagnosis_code");
    }

    #[test]
    fn detects_another_dictionary_diagnosis_code() {
        let matches = detect_diagnosis_codes("History of I10, on current antihypertensives.");
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn diagnosis_code_edge_case_lowercase_is_not_matched() {
        // A real limitation, not a false negative this task is meant to
        // fix: ICD10_SHAPE_RE has no (?i) flag, so a lowercased code is
        // silently missed. Locked in here so a future change to this
        // pattern is a deliberate decision, not an accident.
        let matches = detect_diagnosis_codes("assessment: b20, continue art.");
        assert!(matches.is_empty());
    }

    // --- medication_name: 1 more positive, 1 edge case, 1 negative --------

    #[test]
    fn detects_another_dictionary_medication() {
        let matches = detect_medications("Give Paracetamol for the fever.");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].entity_type, "medication_name");
    }

    #[test]
    fn medication_name_edge_case_all_uppercase_still_matches() {
        let matches = detect_medications("PRESCRIBED: COTRIMOXAZOLE daily.");
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn does_not_flag_an_unlisted_medication_name() {
        // Plausible-looking but not in the illustrative dictionary — must
        // be silently missed, not guessed at (same honesty standard as
        // diagnosis codes).
        let matches = detect_medications("Prescribed Aspirinol twice daily.");
        assert!(matches.is_empty());
    }

    // --- medical_aid_number: 1 edge case, 1 negative (shape-only, no
    // keyword needed for THIS detector, unlike the generic fallback) -------

    #[test]
    fn medical_aid_number_edge_case_minimum_length_matches() {
        let matches = detect_medical_aid_numbers("Scheme ref AB123456 on file.");
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn medical_aid_number_negative_pure_digits_do_not_match() {
        // No letter prefix at all — must not match the letters+digits shape.
        let matches = detect_medical_aid_numbers("Reference 123456789 on file.");
        assert!(matches.is_empty());
    }

    // --- lab_result_value: 2 more positive test-name keywords -------------

    #[test]
    fn detects_lab_value_for_viral_load() {
        let matches = detect_lab_result_values("Viral load 45 copies/mL, undetectable range.");
        assert_eq!(matches.len(), 1);
        assert!("Viral load 45 copies/mL, undetectable range."[matches[0].start..matches[0].end]
            .contains("45"));
    }

    #[test]
    fn detects_lab_value_for_hba1c_with_percent_unit() {
        let matches = detect_lab_result_values("HbA1c 7.2%, review medication.");
        assert_eq!(matches.len(), 1);
    }
}
