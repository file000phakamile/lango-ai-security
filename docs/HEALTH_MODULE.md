# Health Module — Lango / AI Data Guard

**Built for the Cimas Healthathon 3.0 submission** — a Zimbabwean digital health
innovation challenge, separate from this repository's other competition materials.
This document is self-contained: everything you need to understand what this module
is, why it's designed the way it is, and its honest limitations is here, without
needing to cross-reference this repo's other, unrelated submission documents.

This module is **additive**. It extends the existing Lango detection engine, database
schema, backend API, dashboard, and browser extension with health-specific
capability — it does not modify, remove, or replace any existing entity type, route,
dashboard view, or extension behaviour. Everything described in this repo's other
documentation (`README.md`, `docs/ARCHITECTURE.md`, `docs/DATA_AI_USAGE.md`,
`docs/SECURITY_PRIVACY.md`) remains true; this document layers new material on top.

## Why a health-specific module

Lango's core product already redacts sensitive entities (national IDs, bank account
numbers, phone numbers, names) out of AI-tool prompts before they leave an
institution. Health data deserves a specific, separate treatment for one direct
reason: **exposure of a person's health information — a diagnosis, a medication, an
HIV status — carries a distinct, additional harm beyond ordinary PII exposure.** In
the Zimbabwean context this module was built for, HIV-status stigma specifically is
real, well-documented, and can affect someone's employment, relationships, and social
standing in ways a leaked phone number simply does not. A generic PII-redaction tool
is not automatically equipped to handle that — this module is the deliberate,
purpose-built extension that is.

## Part 1 — Five new detectors

All five are implemented in `backend/src/detection/health_rules.rs` (kept separate
from the existing `backend/src/detection/rules.rs`, which is unchanged), following
the same detector pattern already established there. Every one of these carries the
**same honesty standard** as this codebase's existing patterns (see
`rules.rs`'s own doc comments) — best-effort, illustrative, and explicit about what
it does and doesn't cover.

1. **`diagnosis_code`** — ICD-10 codes. A general shape regex (a letter, excluding
   `U`, followed by two digits, with an optional decimal-point subcategory — e.g.
   `B20`, `E11.9`) gates against a ~40-entry illustrative dictionary covering
   HIV/AIDS, tuberculosis, malaria, diabetes, hypertension, and common maternal-health
   codes — the conditions most relevant to a Zimbabwean primary/district health
   context, sourced from publicly documented WHO ICD-10 chapter listings.
   **This is NOT a complete ICD-10 implementation.** Real ICD-10 has tens of thousands
   of codes across 22 chapters; this dictionary covers roughly 40 of them. A
   shape-valid code that isn't in the dictionary is silently missed (a false
   negative), not guessed at — that tradeoff is deliberate: it keeps the
   false-positive rate low (an unrelated model number that happens to look like
   `B20` never matches) at the direct cost of not recognising most real codes.
   The dictionary's plain-language condition names (e.g. `B20` → "HIV disease") exist
   for internal documentation only and are **never** surfaced in any API response,
   audit-log row, or dashboard view — only the entity_type label `diagnosis_code` is
   ever returned or stored. Decoding a specific condition anywhere in the system's
   output would itself defeat the stigma-aware design in Part 3 below.
2. **`medication_name`** — a ~50-entry illustrative dictionary of medicines relevant
   to Zimbabwe's essential-medicines context: ART/antiretrovirals, TB treatment,
   antimalarials, and common chronic-disease medications. Same caveat as the ICD-10
   dictionary: this is an illustrative subset, not Zimbabwe's full Essential Drugs
   List (EDLIZ), which runs to several hundred entries. A real medication not on this
   list is silently missed.
3. **`medical_aid_number`** — a generic, best-effort pattern (2-6 uppercase letters
   directly followed by 6-9 digits, e.g. `CIMAS123456`) representative of common
   medical-aid membership-number shapes. **This is explicitly NOT validated against
   any specific real medical aid provider's actual format** — Cimas, PSMAS, First
   Mutual Health, or any other scheme each define their own real format, none of
   which this codebase has access to. Confidence is deliberately kept low (below the
   fail-closed threshold), the same honesty tier as the existing `bank_account`
   pattern.
4. **`lab_result_value`** — genuinely the hardest of the five to pattern-match
   generically, stated plainly: a bare number ("142", "7.2 mmol/L") is not
   identifiable as sensitive on its own. This detector is deliberately conservative —
   it only flags a lab-value-shaped number when it appears within a small
   (25-character) window immediately after a recognised lab test name from a short
   illustrative list (CD4 count, viral load, HbA1c, haemoglobin, creatinine, blood
   glucose, platelet count, etc.). **False negatives are expected and accepted here**
   — a lab value phrased unusually, or with the test name and value far apart, will
   be missed. That is the deliberately safer failure mode: false positives on
   ordinary numbers (a phone number, a price, a page reference) would be far more
   disruptive, given how numeric a normal prompt already is. Only the numeric value
   (plus an optional unit) is redacted — the test-name keyword itself is left
   visible, since the test name alone isn't sensitive, only the value attached to it.
5. **`next_of_kin`** — not a standalone detector. A contextual reclassification of
   the existing `full_name` heuristic: a detected name is tagged `next_of_kin`
   instead of plain `full_name` only when it appears near the keywords "next of
   kin", "emergency contact", or "guardian". It reuses the existing name-detection
   heuristic (and its existing honesty caveats — see `name_heuristic.rs`) unchanged;
   this module adds no new name-finding logic of its own.

## Part 2 — Sensitivity classification: a NEW, independent axis

This is the core new concept this module introduces, and it is genuinely a
**separate axis from detection confidence** — do not conflate the two.

- **Confidence** (the existing three-tier system in `detection::scan`) answers "how
  sure are we this match is real?"
- **Sensitivity class** answers a completely different question: "if this match IS
  real, how sensitive is the underlying category of data, independent of how sure we
  are?"

A national ID and a diagnosis code can both be detected at 0.90 confidence, but only
the diagnosis code carries **special-category health data**. Every entity type in the
system is tagged either `standard` or `special_category_health`
(`health_rules::sensitivity_class`, the single source of truth for this mapping):

| Sensitivity class | Entity types |
|---|---|
| `standard` (unchanged) | `national_id`, `bank_account`, `phone_number`, `credit_card`, `medical_record_no`, `api_key`, `full_name` |
| `special_category_health` (new) | `diagnosis_code`, `medication_name`, `medical_aid_number`, `lab_result_value`, `next_of_kin` |

Note that `medical_record_no` — an existing entity type, predating this module —
**keeps `standard` classification**, even though a hospital record number is
intuitively "health-adjacent". This is a deliberate scope decision, not an
oversight: this module's own scope explicitly listed which five entity types become
`special_category_health`, and `medical_record_no` was not one of them. An
institution adopting this module for real could revisit that judgment call.

### The hard rule

`special_category_health` matches **never** qualify for the existing three-tier
system's "low-confidence redact-and-flag-for-review" leniency (the
`redacted_low_confidence_review` decision that a low-but-real-confidence `full_name`
match gets). A `special_category_health` match either redacts and forwards normally
(if confident enough), or fails closed and blocks (if not) — the same two-outcome
behaviour every other structured entity type already had, unchanged by this module.
**This is intentional and load-bearing: health data does not get the relaxed
treatment names get.** The whole point of the tier-2 leniency band that exists for
names is that `name_heuristic.rs`'s real false-positive rate on ordinary capitalized
phrases makes blocking every borderline match too costly relative to the actual risk.
That tradeoff does not hold for health data — the cost of wrongly letting a genuine
health-data exposure through on a "we'll flag it for later review" basis is
categorically higher, so this module deliberately does not extend that same leniency
to it.

This is enforced structurally in `detection::scan::scan_prompt` (see the
`is_leniency_eligible` closure and its surrounding comment) and verified by a real
unit test,
`low_confidence_special_category_health_never_gets_review_flag_blocks_instead`, which
constructs a `special_category_health` match at the exact same confidence value that
gives an ordinary name match the lenient treatment, and asserts the outcome is
`blocked_low_confidence`, never `redacted_low_confidence_review`.

### Storage

`audit_log.sensitivity_class` (migration `0008_add_health_module_columns.sql`) stores
this per row — a row is `special_category_health` if **any** detected entity in it
is, regardless of how many `standard` entities are also present. This makes the
axis directly queryable and auditable, the same way `decision` and entity type
already were.

## Part 3 — Stigma-aware aggregate reporting

**This is a real design principle, not a cosmetic choice.** Any endpoint or dashboard
component that shows aggregate or trend data (counts over time, department
comparisons, facility comparisons, etc.) shows **only a total count** of
`special_category_health` detections, plus the coarse `standard` vs.
`special_category_health` split — **never** a breakdown by specific condition,
medication, or diagnosis type (e.g. never "N detections were HIV-related this week").

### Why this matters, concretely

A per-department or per-week breakdown by condition type, even with no names
attached, can be enough to identify who a detection came from once a group is small
enough. In the Zimbabwean context this module was built for, HIV-status stigma
specifically is the concrete risk this is guarding against: a handful of
diagnosis-code detections attributed to one small team over one short window can
quietly point back to a specific person, and the harm of that (discrimination, social
exclusion) is categorically worse than an ordinary PII leak. This does not require
anyone to act maliciously — it can happen from entirely ordinary, well-intentioned
dashboard use by a compliance officer who never intended to identify anyone.

### What is and isn't restricted

- **Restricted**: `GET /api/health-data-guard/summary` and the Health Data Guard
  dashboard view — aggregate/trend surfaces. See the extensive comment on
  `routes::health::get_health_summary` in the backend source for the full reasoning
  kept next to the code it governs.
- **NOT restricted**: the existing, per-entry Audit Log detail view (the existing
  expandable row, scoped to one specific, already-flagged session). A compliance
  officer reviewing that one row can still see exactly which entity types were
  detected in it — that is the legitimate, authorized, per-case review this product
  exists to support, unaffected by this module. The restriction is specifically on
  *aggregate* reporting, not on that existing per-entry detail.

## Part 4 — Health Data Guard dashboard view

A sixth sidebar entry, added without touching the five existing views (Command
Center, Audit Log, Fairness Audit, Drift & Security, Pilot & Sandbox) — same key
order, same routes, same behaviour, unchanged.

- **KPI strip**: total `special_category_health` detections (aggregate-only, per
  Part 3), redaction rate, and the standard/special-category count split.
- **Facility-type fairness**: reuses the exact same Disparate Impact Ratio /
  Statistical Parity Difference calculation already built for the Fairness Audit
  view's language/department parity (`routes::fairness::compute_dir_spd`, called
  directly, not reimplemented), applied to a new grouping dimension —
  `facility_type` (e.g. "Rural Clinic" vs. "Urban Hospital") — scoped to
  `special_category_health` rows. This checks whether special-category detection
  accuracy is equitable across facility types, the same way the existing view checks
  it across departments and languages.
- **"Why This Matters" panel**: the Part 3 stigma-aware reasoning above, surfaced
  directly in the dashboard UI itself (not only in this document), so a reviewer
  exploring the live product sees the ethical reasoning in context.

`facility_type` is an **optional, caller-declared** field on `/api/scan`
(`ScanRequest.facility_type`), the same pattern the existing `language` field already
uses for the Fairness Audit view's language-parity chart — not derived from the
prompt itself. The existing browser extension does not send it and is entirely
unaffected (see Part 5).

## Part 5 — Browser extension: no changes needed, verified by tracing the code

The extension calls the same `/api/scan` endpoint everything else uses, generically:
`extension/background.js`'s `scanPrompt()` POSTs `{ prompt }` and returns whatever
`{ decision, entities_detected, redacted_prompt, ... }` comes back;
`extension/content/site-adapter.js`'s `handleSubmitAttempt` switches only on
`result.decision` (against the same four decision values that already existed) and
uses `result.entities_detected.length` only as a generic count in banner text — it
never inspects specific entity-type strings. Since this module introduces no new
`decision` value, and `facility_type` is optional (the extension simply omits it),
**zero extension code changes are required** for the five new health entity types to
work automatically through the existing chatgpt.com/claude.ai/gemini.google.com/
chat.deepseek.com/copilot.microsoft.com adapters. This was verified by tracing the
actual code path, not assumed.

## Part 6 — Seed data

`backend/src/bin/seed.rs` gained one additional batch (after the existing department
loop, itself unchanged) that runs synthetic health-context prompts through the real
`scan_prompt()` engine, tagged with a `facility_type` ("Rural Clinic" / "District
Hospital" / "Urban Hospital") so the Health Data Guard view's facility-parity chart
has real, live-computed data immediately after a fresh seed. Same principles as the
rest of this file: TRUNCATE-and-reseed idempotency, and manual-only invocation (not
wired into any deploy step), are both unchanged.

## Known limitations, stated plainly

- The ICD-10 and medication dictionaries are illustrative subsets (~40 and ~50
  entries respectively), not complete clinical coding systems.
- `medical_aid_number`'s pattern is generic and unvalidated against any real
  provider's actual format.
- `lab_result_value` will miss lab values phrased unusually or far from their test
  name — a deliberate false-negative-over-false-positive tradeoff.
- `next_of_kin` inherits every limitation of the existing name heuristic
  (`name_heuristic.rs`) — it is not real NER.
- Because of the sensitivity-class hard rule, a `next_of_kin` match at the name
  heuristic's current fixed confidence (0.55) will always fail closed
  (`blocked_low_confidence`) rather than ever redacting and forwarding — a direct,
  documented consequence of this module's own design, not a bug.
- `facility_type` has no live signal in v0.1 beyond what a caller explicitly
  declares — same honesty caveat as the existing `language` field.
