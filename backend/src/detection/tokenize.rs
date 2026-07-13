//! Shared tokenization layer for the detection engine.
//!
//! Every detector that needs word-level context (keyword proximity, the
//! generic structured-identifier fallback in `fallback.rs`) works off ONE
//! pass over the prompt here, rather than each detector independently
//! re-scanning the raw string. The specific-format regex detectors in
//! `rules.rs` / `health_rules.rs` still run their own `find_iter` over the
//! raw text (a token boundary isn't meaningful for e.g. a credit-card regex
//! that intentionally spans multiple whitespace-separated groups) — this
//! layer exists for the keyword-proximity logic specifically, not as a
//! replacement for every regex scan in the engine.
//!
//! Tokens are maximal runs of non-whitespace characters, with their exact
//! byte span in the original prompt. Byte spans (not char indices) are what
//! `scan.rs`'s redaction step needs, since it splices into the original
//! `String` by byte range.

use once_cell::sync::Lazy;
use regex::Regex;

static TOKEN_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\S+").unwrap());

#[derive(Debug, Clone, Copy)]
pub struct Token<'a> {
    /// The raw token text, including any leading/trailing punctuation
    /// (e.g. "ID:" or "12345678ACD," keep their punctuation here) — use
    /// `normalize` for keyword/shape comparisons, and `trimmed_span` for
    /// the punctuation-free span when a match needs an exact redaction
    /// range.
    pub text: &'a str,
    pub start: usize,
    pub end: usize,
}

impl<'a> Token<'a> {
    /// Leading/trailing ASCII punctuation stripped, CASE PRESERVED — a
    /// literal substring of the original prompt (needed so `trimmed_span`
    /// below can compute a byte offset via pointer arithmetic; a lowercased
    /// copy wouldn't share the original's backing memory). Use this for the
    /// structured-identifier shape check and for computing a match's
    /// redaction span; use `normalize` for keyword-phrase comparison.
    pub fn trimmed(&self) -> &'a str {
        self.text.trim_matches(|c: char| {
            c.is_ascii_punctuation() && c != '-' /* keep internal-looking dashes, e.g. national-id-shaped tokens */
        })
    }

    /// Lowercased version of `trimmed()` — used for keyword-phrase
    /// comparison (so "ID:" matches a keyword list entry of "id"). Returns
    /// an owned `String` rather than `&str`: ASCII-lowercasing can't reuse
    /// the original slice's backing memory, so this necessarily allocates —
    /// callers that also need the match's byte span should use
    /// `trimmed_span`, not try to recover a position from this method.
    pub fn normalize(&self) -> String {
        self.trimmed().to_lowercase()
    }

    /// Byte span of `trimmed()`'s output within the original prompt — the
    /// span a match should actually redact, excluding punctuation that
    /// isn't part of the identifier itself (e.g. a trailing colon or
    /// comma).
    pub fn trimmed_span(&self) -> (usize, usize) {
        let trimmed = self.trimmed();
        if trimmed.is_empty() {
            return (self.start, self.start);
        }
        let offset = trimmed.as_ptr() as usize - self.text.as_ptr() as usize;
        (self.start + offset, self.start + offset + trimmed.len())
    }
}

/// Tokenizes `text` into whitespace-normalized tokens with their original
/// byte spans. Whitespace itself (run length, tabs vs. spaces) carries no
/// information for any detector in this engine, so it's collapsed away here
/// rather than re-parsed per detector.
pub fn tokenize(text: &str) -> Vec<Token<'_>> {
    TOKEN_RE
        .find_iter(text)
        .map(|m| Token {
            text: m.as_str(),
            start: m.start(),
            end: m.end(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_and_preserves_byte_spans() {
        let text = "Mark Dlomo Patient ID: 12345678ACD";
        let tokens = tokenize(text);
        let texts: Vec<&str> = tokens.iter().map(|t| t.text).collect();
        assert_eq!(texts, vec!["Mark", "Dlomo", "Patient", "ID:", "12345678ACD"]);
        for t in &tokens {
            assert_eq!(&text[t.start..t.end], t.text);
        }
    }

    #[test]
    fn normalize_strips_punctuation_and_lowercases() {
        let tokens = tokenize("ID: 12345678ACD,");
        assert_eq!(tokens[0].normalize(), "id");
        assert_eq!(tokens[1].normalize(), "12345678acd");
    }

    #[test]
    fn trimmed_preserves_case_for_span_computation() {
        let tokens = tokenize("ID: 12345678ACD,");
        assert_eq!(tokens[1].trimmed(), "12345678ACD");
    }

    #[test]
    fn trimmed_span_excludes_trailing_punctuation() {
        let text = "Ref: 12345678ACD, thanks.";
        let tokens = tokenize(text);
        let id_token = tokens.iter().find(|t| t.text.starts_with("12345678")).unwrap();
        let (start, end) = id_token.trimmed_span();
        assert_eq!(&text[start..end], "12345678ACD");
    }

    #[test]
    fn collapses_irregular_whitespace() {
        let tokens = tokenize("a   b\t\tc\n\nd");
        let texts: Vec<&str> = tokens.iter().map(|t| t.text).collect();
        assert_eq!(texts, vec!["a", "b", "c", "d"]);
    }
}
