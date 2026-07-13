//! The detection engine: tokenization (`tokenize.rs`), per-entity-type
//! primary-pattern detectors (`rules.rs`, `health_rules.rs`), the
//! keyword-list registry they share with the generic fallback
//! (`entity_meta.rs`), the generic structured-identifier fallback itself
//! (`fallback.rs`), and the orchestration/overlap-resolution/confidence-
//! tiering layer that ties them together (`scan.rs`).
//!
//! CATASTROPHIC-BACKTRACKING AUDIT (task requirement): every regex pattern
//! in this engine (existing and new — `rules.rs`, `health_rules.rs`,
//! `name_heuristic.rs`, `fallback.rs`, `tokenize.rs`) was reviewed for
//! nested-quantifier / ambiguous-alternation risk. Finding: none carry that
//! risk, and — more fundamentally — none COULD, structurally, regardless of
//! how they were written. This crate (`regex`) is a guaranteed-linear-time
//! engine built on a Thompson NFA / lazy DFA simulation, not a
//! backtracking search; it deliberately doesn't support backtracking-only
//! features like lookaround for exactly this reason (already noted in
//! `rules.rs`'s own doc comment on `API_KEY_SPECIFIC_RE`, predating this
//! task). "Catastrophic backtracking" as a phenomenon requires a
//! backtracking matcher; it cannot occur with this crate no matter what
//! pattern is written. Every pattern was still manually reviewed for sane,
//! bounded quantifiers (documented per-pattern in `rules.rs`/
//! `health_rules.rs`/`fallback.rs`) as basic hygiene and portability
//! insurance (in case a pattern is ever ported to a backtracking engine
//! elsewhere), not because any risk was found here.
pub mod drift;
pub mod entity_meta;
pub mod fallback;
pub mod health_rules;
pub mod name_heuristic;
pub mod plain_language;
pub mod rules;
pub mod scan;
pub mod tokenize;
