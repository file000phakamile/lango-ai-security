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
  `redacted_and_forwarded` / `blocked_low_confidence`), reason string, AI
  model/connector label, response-scan result.
- **`ParityEntry`** — group label (language or department) + flag rate, used for the
  fairness bar charts.
- **`DriftWeek`** — week label, PSI, KL-divergence, alert flag, used for the drift
  line chart.
- **`SecurityEvent`** — timestamp, event type (prompt injection blocked / rate limit
  triggered / DoS mitigation triggered), detail string.

## Is personal or sensitive data collected?

**No — by design, and this is the entire point of the product.** Lango's purpose is
to prevent sensitive personal data (national IDs, bank account numbers, phone
numbers, full names, medical record numbers, API keys) from leaving an institution via
AI prompts in the first place. This demo does not collect, store, or transmit any real
personal data: the "entities detected" shown in the Audit Log are entity *type*
labels (e.g. `national_id`) attached to synthetic rows, never actual ID numbers or
names. This is real, implemented behaviour today, not just a target-production
principle: `backend/src/detection/scan.rs` strips matched entities from the prompt
before anything is logged, and `audit_log.original_prompt_hash` stores a SHA-256 hash
of the original prompt — never the raw text — with `redacted_prompt` storing only the
sanitised version. The target production system carries the same principle forward
unchanged; what's still aspirational there is the AI Gateway actually forwarding the
sanitised prompt to a live provider, not the redaction-before-logging behaviour
itself.

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

## What does a correct output look like?

A correct decision is one where the `decision` field matches the actual sensitivity of
the prompt:

- **`cleared_no_entities`** — no sensitive entity was present, and none was flagged.
- **`redacted_and_forwarded`** — a sensitive entity was present, correctly detected,
  replaced with a placeholder token, and only the sanitised version was sent onward.
- **`blocked_low_confidence`** — the scanner detected something *likely* sensitive but
  couldn't classify it with enough confidence to safely redact and forward, so it
  fails closed (blocks) rather than guessing. This is intentional: an uncertain
  detection should stop the request, not risk letting sensitive data through.

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
  list) and false-positive on capitalized phrases that aren't names.
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

## Human oversight for high-risk decisions

By design, low-confidence detections **fail closed** — the request is blocked rather
than allowed through on an uncertain judgement call (`blocked_low_confidence` in the
Audit Log). Fairness threshold failures (DIR below 0.80) automatically open a
mandatory pattern-rule review rather than self-correcting silently. Drift alerts
crossing the PSI/KL threshold likewise require a human review-and-update cycle on the
detection rules, not an automated rule change. In all three cases, the system is
designed to surface uncertainty and disparity to a human reviewer rather than resolve
them autonomously — appropriate given the institutional and regulatory stakes involved.
