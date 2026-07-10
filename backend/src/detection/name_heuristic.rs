//! Full-name detection: a SIMPLIFIED STAND-IN FOR REAL NER, NOT
//! PRODUCTION-GRADE. Documented plainly here and in docs/ARCHITECTURE.md —
//! do not oversell what this does.
//!
//! No lightweight, reliable, pure-Rust NER crate with a workable footprint
//! for a local v0.1 build was available (the realistic options —
//! `rust-bert` / ONNX-based transformer NER — pull in a native libtorch or
//! onnxruntime dependency, which is a heavy, non-trivial install for a
//! "runs locally, not production-hardened yet" milestone). See
//! Questions.md for the full reasoning behind that call.
//!
//! Instead: a capitalized-word-sequence heuristic. It flags runs of 2-3
//! consecutive Capitalized Words, skipping a stopword list of common
//! capitalized non-name words (sentence starters, days/months, and the
//! institutional vocabulary already used elsewhere in this product, like
//! department names). This WILL both miss real names (single-word names,
//! lowercase names, names that collide with the stopword list) and
//! false-positive on capitalized phrases that aren't names (e.g. unlisted
//! proper nouns, title-case headings). It is a genuine, working piece of
//! code — not a fabricated result — but it is not NER in any real sense.

use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashSet;

static CAPITALIZED_RUN_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(?:[A-Z][a-z']+\s){1,2}[A-Z][a-z']+\b").unwrap());

static STOPWORDS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    [
        // Sentence starters / common capitalized non-names
        "The", "This", "That", "These", "Those", "Dear", "Sincerely", "Regards", "Please",
        "Thank", "Thanks", "Hello", "Hi", "Hey", "Note", "Attention", "Subject", "From", "To",
        // Days / months
        "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday", "January",
        "February", "March", "April", "May", "June", "July", "August", "September", "October",
        "November", "December",
        // This product's own institutional vocabulary (see lib/lango/types.ts Department
        // list) — without this exclusion, department names constantly false-positive as
        // "names" since they're also capitalized multi-word phrases.
        "Credit", "Risk", "Claims", "Processing", "Patient", "Records", "Bursar's", "Office",
        "Legal", "Affairs", "Bank", "Commercial", "Regional", "Institution", "Pilot",
    ]
    .into_iter()
    .collect()
});

pub struct NameMatch {
    pub start: usize,
    pub end: usize,
}

pub fn detect_names(text: &str) -> Vec<NameMatch> {
    CAPITALIZED_RUN_RE
        .find_iter(text)
        .filter_map(|m| trim_stopwords(m.as_str(), m.start()))
        .collect()
}

/// Trims leading/trailing stopwords from a matched run (so "Dear John Moyo"
/// flags "John Moyo", not the greeting) and returns `None` if nothing
/// non-stopword remains (e.g. a run made entirely of department-name words).
fn trim_stopwords(matched: &str, match_start: usize) -> Option<NameMatch> {
    // (word, byte_offset_within_matched, byte_len)
    let words: Vec<(&str, usize)> = matched
        .split_whitespace()
        .map(|w| {
            let offset = w.as_ptr() as usize - matched.as_ptr() as usize;
            (w, offset)
        })
        .collect();

    let first_non_stop = words.iter().position(|(w, _)| !STOPWORDS.contains(w))?;
    let last_non_stop = words.iter().rposition(|(w, _)| !STOPWORDS.contains(w))?;

    let (first_word, first_offset) = words[first_non_stop];
    let (last_word, last_offset) = words[last_non_stop];

    Some(NameMatch {
        start: match_start + first_offset,
        end: match_start + last_offset + last_word.len(),
    })
    .filter(|_| first_word.len() > 0)
}
