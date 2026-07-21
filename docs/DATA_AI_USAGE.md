# Data and AI Usage — Lango / AI Data Guard

## Is the data real or synthetic?

**The content is synthetic; the system producing it is real, stated plainly.** This
is no longer a purely client-side simulation — a real Rust/Axum backend, deployed on
Render with a real PostgreSQL database, is live and verified end-to-end. What runs
through it is still synthetic content: [`backend/src/bin/seed.rs`](../backend/src/bin/seed.rs)
generates realistic-looking (but fabricated) prompts and feeds them through the
*actual* detection engine — the same regex rules and name heuristic a real `/api/scan`
call would use — so the resulting audit-log rows, risk scores, and decisions are real
detection output, not fabricated numbers, just produced from made-up input text
rather than real institutional data. No real institution, employee, customer, or
patient data has been used, collected, or stored anywhere in this repository or its
deployed instances.

The frontend has a second, separate data path: [`lib/lango/mock-data.ts`](../lib/lango/mock-data.ts)
generates data entirely client-side with a seeded PRNG (`mulberry32`, seed `2026`),
with no backend involved at all. This is now the *fallback* path — used automatically
if the real backend is unreachable (e.g. mid cold-start on Render's free tier) — not
the primary path. See [`lib/lango/api-client.ts`](../lib/lango/api-client.ts) for
exactly how that fallback decision is made.

## Source and rights

Not applicable in the "real dataset" sense — there is no dataset, real or acquired
from elsewhere. Both data-generation paths (the backend's `seed.rs` and the frontend's
`mock-data.ts`) are original code written for this submission. Some values are still
fixed illustrative constants rather than computed (the security-events feed, since no
live prompt-injection/rate-limit/DoS detector exists yet — see Known limitations
below); most others (fairness flag rates, drift PSI/KL, audit log contents) are now
computed live from whatever's actually in the database at request time, so they will
vary between deployments/reseeds rather than being pinned to one illustrative number.
The team holds full rights to this code and its output.

## Data structure

Generated records follow the shapes defined in
[`lib/lango/types.ts`](../lib/lango/types.ts):

- **`AuditLogEntry`** — session id, user id, department, timestamp, entity types
  detected, risk score (0–1), decision (`cleared_no_entities` /
  `redacted_and_forwarded` / `redacted_low_confidence_review` /
  `blocked_low_confidence`), reason string, AI model/connector label, response-scan
  result.
- **`ParityEntry`** — group label (language or department) + flag rate, used for the
  fairness bar charts.
- **`DriftWeek`** — week label, PSI, KL-divergence, alert flag, used for the drift
  line chart.
- **`SecurityEvent`** — timestamp, event type (prompt injection blocked / rate limit
  triggered / DoS mitigation triggered), detail string.

## Is personal or sensitive data collected?

**No — by design, and this is the entire point of the product.** Lango's purpose is
to prevent sensitive personal data (national IDs, bank account numbers, phone
numbers, full names, medical record numbers, API keys, and — since the health module,
see below — diagnosis codes, medication names, medical aid numbers, lab result values,
and next-of-kin names) from leaving an institution via AI prompts in the first place.
This demo does not collect, store, or transmit any real personal data: the "entities
detected" shown in the Audit Log are entity *type* labels (e.g. `national_id`,
`diagnosis_code`) attached to synthetic rows, never actual ID numbers, names, or
decoded conditions — see the health module section below for why a diagnosis code is
never decoded to a plain-language condition name anywhere in a response. This is real,
implemented behaviour today, not just a target-production principle:
`backend/src/detection/scan.rs` strips matched entities from the prompt before
anything is logged, and `audit_log.original_prompt_hash` stores a SHA-256 hash of the
original prompt — never the raw text — with `redacted_prompt` storing only the
sanitised version. **The native chat feature's `chat_messages` table follows the
identical principle** — a user's message is stored only as its redacted form, never
the raw text they typed; a blocked message is never stored at all. The target
production system carries the same principle forward unchanged; what used to be
aspirational there — the AI Gateway actually forwarding the sanitised prompt to a
live provider — is now real for this one path (native chat, OpenAI only; see
[ARCHITECTURE.md](ARCHITECTURE.md)'s Native chat section), though still not for the
extension's own `/api/scan` path, by design.

## Health module — new entity types, and a new sensitivity-class axis

Built for the Cimas Healthathon 3.0 submission (a separate competition — see
[HEALTH_MODULE.md](HEALTH_MODULE.md) for the full, self-contained writeup). Additive
only: everything above this section describes the original seven entity types and
remains true unchanged. Two things were added:

1. **Five new entity types** (`backend/src/detection/health_rules.rs`):
   `diagnosis_code`, `medication_name`, `medical_aid_number`, `lab_result_value`,
   `next_of_kin`. The same honesty standard already applied to the name heuristic
   above applies here, stated with the same plainness:
   - `diagnosis_code` gates an ICD-10 shape regex against a **~40-entry illustrative
     dictionary** (HIV/AIDS, TB, malaria, diabetes, hypertension, common
     maternal-health codes — the conditions most relevant to a Zimbabwean
     primary/district health context, sourced from publicly documented WHO ICD-10
     chapter listings). **This is not a complete ICD-10 implementation** — real
     ICD-10 has tens of thousands of codes across 22 chapters; a shape-valid code not
     in this dictionary is silently missed (false negative), not guessed at.
   - `medication_name` matches against a **~50-entry illustrative dictionary**
     (ART/antiretrovirals, TB treatment, antimalarials, common chronic-disease
     medications relevant to Zimbabwe's essential-medicines context) — not
     Zimbabwe's full Essential Drugs List (EDLIZ), which runs to several hundred
     entries.
   - `medical_aid_number` is a **generic, unvalidated** pattern (letters + digits),
     not checked against any real medical aid provider's actual format — same
     honesty tier as the existing `bank_account` pattern.
   - `lab_result_value` only fires when a lab-value-shaped number appears near a
     recognised lab-test keyword (CD4 count, viral load, HbA1c, etc.) — a bare
     number is never flagged on its own. False negatives (unusually phrased lab
     values) are expected and accepted; false positives on ordinary numbers are not.
   - `next_of_kin` is not a new detector — it's the existing `full_name` heuristic,
     contextually reclassified when a name appears near "next of kin", "emergency
     contact", or "guardian". It inherits every limitation of that heuristic.
2. **`sensitivity_class`** — a NEW axis, independent of detection confidence: every
   entity type is tagged `standard` (the original seven) or
   `special_category_health` (the five new ones above). A `special_category_health`
   match is never eligible for the `redacted_low_confidence_review` leniency band
   described above under "What does a correct output look like?" — it only ever
   redacts-and-forwards (if confident enough) or fails closed (if not), enforced by a
   real unit test in `backend/src/detection/scan.rs`. See
   [HEALTH_MODULE.md](HEALTH_MODULE.md) for the full reasoning, and
   [SECURITY_PRIVACY.md](SECURITY_PRIVACY.md) for why aggregate/trend views built on
   this data show only a total count, never a per-condition breakdown.

## AI approach category

**Rule-based pattern matching + Named Entity Recognition (NER) for classification —
not generative AI.** This is a deliberate design choice, not a limitation:

- **Explainability**: a compliance officer needs to know exactly *why* a request was
  blocked or redacted, down to which pattern or entity rule fired. A generative model
  making that call would be a black box, unsuitable for evidence a regulator might
  request.
- **Determinism and auditability**: the same input should reliably produce the same
  detection outcome. Pattern/NER-based detection is deterministic and versionable
  (rules can be reviewed, dated, and rolled back); a generative model's outputs are
  probabilistic and drift with provider-side model updates outside the institution's
  control.
- **Where generative AI still fits**: Lango does not replace the institution's
  generative AI tool — it sits in front of it. The *sanitised* prompt is still
  forwarded to whichever generative AI provider the institution already uses; Lango's
  own detection layer is intentionally not generative.
- **This is no longer purely hypothetical, for one specific path.** The native chat
  feature's `POST /api/chat` is a real, live example of exactly this pattern: the
  *redacted* prompt (never the raw original) is forwarded to OpenAI, and the reply
  streams back through this product. This does not change anything about the
  detection layer above — `scan_prompt`/`scan_response` are still the same
  deterministic, rule-based, non-generative detectors described in this document,
  run before and after the generative call respectively. See
  [ARCHITECTURE.md](ARCHITECTURE.md)'s Native chat section for the full design, and
  the Known limitations section below for this path's honest verification status
  (no live OpenAI key was available while building it).

## What does a correct output look like?

A correct decision is one where the `decision` field matches the actual sensitivity of
the prompt:

- **`cleared_no_entities`** — no sensitive entity was present, and none was flagged.
- **`redacted_and_forwarded`** — a sensitive entity was present, correctly detected,
  replaced with a placeholder token, and only the sanitised version was sent onward.
- **`redacted_low_confidence_review`** — a `full_name` match landed in the 0.30-0.60
  confidence band: real enough to redact, not confident enough to treat the same as a
  routine detection. Redacted and forwarded automatically, same as
  `redacted_and_forwarded`, but tagged distinctly so it surfaces separately for async
  compliance review. This is a deliberate, narrow exception scoped to names only — see
  Known limitations below and docs/SECURITY_PRIVACY.md's Human oversight row.
- **`blocked_low_confidence`** — the scanner detected something *likely* sensitive but
  couldn't classify it with enough confidence to safely redact and forward, so it
  fails closed (blocks) rather than guessing. This applies to near-zero-confidence
  matches of any type, and to low-confidence matches on any *structured* entity type
  (national ID, bank account, phone number, credit card, medical record number, API
  key) — for those, an uncertain detection should stop the request, not risk letting
  sensitive data through, since a partial/uncertain match on a structured pattern is
  more likely to be a real entity in an unexpected format than a false positive.

A **wrong** output is a false negative (sensitive data missed and forwarded raw — the
costly failure mode) or an excessive false positive rate (legitimate content
routinely blocked, driving staff to route around the tool — see Adoption risks in
[BUSINESS_MODEL.md](BUSINESS_MODEL.md)).

## How are outputs validated?

Two complementary checks, both real and both visible in this demo's dashboard —
neither is a fixed illustrative figure any more:

- **Fairness validation** (Fairness Audit view): **Disparate Impact Ratio (DIR)** and
  **Statistical Parity Difference (SPD)** are computed live, on every request, by
  `backend/src/routes/fairness.rs` — a real SQL aggregation over actual `audit_log`
  rows grouped by session language (English, Ndebele, Shona) and department, checking
  whether the scanner flags one group's prompts at a disproportionately different rate
  than another's, against the 0.80 DIR threshold. Because it's computed from whatever
  is actually in the database, the specific numbers shown will vary between deployments
  and reseeds — they are no longer a single pinned "worked example," they're a live
  computation over real (if synthetic-content) rows.
- **Drift validation** (Drift & Security view): **Population Stability Index (PSI)**
  and **KL-divergence** (`backend/src/detection/drift.rs`, with unit tests) are real
  statistical math run over weekly entity-type distributions, against a 0.20 alert
  threshold, to catch the detection layer's behaviour silently degrading (e.g. a new
  national ID card format the pattern rules don't recognise). What's still simplified:
  these are computed once at seed time over synthetic weekly distributions, not
  continuously by a scheduled job against live traffic — see Known limitations below.

In the target production system, drift would additionally run on a real recurring
schedule against live traffic (rather than being computed once at seed time); fairness
validation's live-computation approach is already the production-shaped design, just
not yet running against real institutional volume.

## Known limitations and failure modes

- **Detection is real, but not scheduled/continuous everywhere.** `/api/scan` runs
  real detection on every call, and fairness (DIR/SPD) is computed live on every
  request. Drift (PSI/KL) is the one exception: it's real math, but only computed once
  at seed time against synthetic weekly distributions, not by a live scheduled job —
  see docs/ARCHITECTURE.md's Monitoring row.
- **Name detection is a heuristic, not real NER.** `backend/src/detection/name_heuristic.rs`
  is a capitalized-word-sequence pattern with a stopword exclusion list — explicitly
  documented in its own code comment as a simplified stand-in, not production-grade
  NER. A real transformer-based NER model was considered and deliberately not used in
  v0.1 (needs a native libtorch/onnxruntime dependency, too heavy for this stage) —
  see [Questions.md](../Questions.md) for the full reasoning. This heuristic will both
  miss real names (single-word names, lowercase names, names matching the stopword
  list) and false-positive on capitalized phrases that aren't names — which is exactly
  why a low-confidence `full_name` match no longer blocks outright (see
  `redacted_low_confidence_review` above): blocking on every borderline name match was
  costing real workflow friction for very little actual safety benefit, since a
  wrongly-redacted ordinary word is a much smaller harm than a wrongly-redacted
  national ID or account number.
- **Rule/NER-based detection has coverage gaps** — new document or ID formats,
  regional variations, or entities outside the seven defined types
  (`national_id`, `bank_account`, `phone_number`, `credit_card`, `full_name`,
  `medical_record_no`, `api_key`) would not be caught until rules are updated — this
  is exactly the failure mode the drift monitor exists to catch, not something the
  design claims to be immune to.
- **Fairness gaps are possible and expected to surface**, which is why the fairness
  audit exists as a first-class, always-on check rather than a one-time validation —
  the demo's own worked example shows a fairness check failing, on purpose, to
  illustrate that the system reports failures rather than hiding them.
- **Language/dialect coverage is limited** to the three languages shown in the demo
  (English, Ndebele, Shona) as a starting scope, not a claim of broader coverage.
- **The native chat feature's live OpenAI connection is unverified against the real
  API.** No live OpenAI key was available while building it. The provider adapter is
  tested against a mocked SSE response and a real local mock HTTP server standing in
  for OpenAI — never the real API in the automated test suite. One real network call
  did reach the real OpenAI API during manual verification, using a deliberately fake
  key to check error handling (a genuine `401` came back and was handled correctly),
  which is not the same claim as a verified successful completion. See
  [ARCHITECTURE.md](ARCHITECTURE.md)'s Native chat section and
  [Questions.md](../Questions.md) items 47-49.

## Human oversight for high-risk decisions

By design, near-zero-confidence detections and any low-confidence match on a
structured entity type **fail closed** — the request is blocked rather than allowed
through on an uncertain judgement call (`blocked_low_confidence` in the Audit Log).
The one deliberate exception is a low-but-real-confidence `full_name` match: rather
than blocking, it's redacted and forwarded automatically and tagged
`redacted_low_confidence_review`, so a human reviews it asynchronously from the audit
log instead of the requester being stopped outright — a reasoned tradeoff given
`name_heuristic.rs`'s real false-positive rate on ordinary capitalized phrases, not a
loosening of fail-closed for the cases where it matters most (system failure,
near-zero confidence, every structured entity type). Fairness threshold failures (DIR
below 0.80) automatically open a mandatory pattern-rule review rather than
self-correcting silently. Drift alerts crossing the PSI/KL threshold likewise require
a human review-and-update cycle on the detection rules, not an automated rule change.
In all three cases, the system is designed to surface uncertainty and disparity to a
human reviewer rather than resolve them autonomously — appropriate given the
institutional and regulatory stakes involved.
