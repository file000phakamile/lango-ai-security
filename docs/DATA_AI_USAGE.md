# Data and AI Usage — Lango / AI Data Guard

## Is the data real or synthetic?

**Synthetic, stated plainly.** Every data point rendered in this demo — audit log
rows, department flag rates, drift metrics, security events, pilot checklist status —
is generated programmatically by [`lib/lango/mock-data.ts`](../lib/lango/mock-data.ts)
using a seeded pseudo-random number generator (`mulberry32`, seed `2026`). No real
institution, employee, customer, or patient data has been used, collected, or stored
anywhere in this repository.

## Source and rights

Not applicable in the "real dataset" sense — there is no dataset. The mock-data
generator is original code written for this submission, and the fixed constants it
uses (fairness figures, the week-9 drift spike, pilot success metrics) are
illustrative example values chosen to demonstrate the product's monitoring and audit
features, not measurements from any real deployment. The team holds full rights to
this code and its output.

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
names. In the target production system, the same principle holds by design — the
Redaction Engine's job is to strip sensitive values *before* they reach an AI
provider or get logged in plaintext; the audit log is built to record that a
redaction happened, not to store the redacted value itself.

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

Two complementary checks, both visible in this demo's dashboard:

- **Fairness validation** (Fairness Audit view): **Disparate Impact Ratio (DIR)** and
  **Statistical Parity Difference (SPD)** are computed across session language
  (English, Ndebele, Shona) and department, to check the scanner doesn't flag one
  group's prompts at a disproportionately different rate than another's. The demo's
  worked example shows a DIR of 0.67 for Shona vs. English — below the 0.80 threshold
  — which triggers a mandatory pattern-rule review. The same methodology is applied
  across departments.
- **Drift validation** (Drift & Security view): **Population Stability Index (PSI)**
  and **KL-divergence** are tracked weekly against a 0.20 alert threshold, to catch
  the detection layer's behaviour silently degrading (e.g. a new national ID card
  format the pattern rules don't recognise). The demo's worked example shows a
  synthetic week-9 spike to PSI 0.27, tied to a new ID-card format, with rules updated
  the same week.

In the target production system, both checks would run against real audit-log data on
a recurring cycle (quarterly for fairness, weekly for drift, matching the cadence
shown in the demo) rather than the fixed illustrative figures used here.

## Known limitations and failure modes

- **No live detection exists yet** — this demo does not run any actual entity
  detection; the pipeline and its outputs are illustrative, not functional.
- **Rule/NER-based detection has coverage gaps** — new document or ID formats,
  regional variations, or entities outside the six defined types
  (`national_id`, `bank_account`, `phone_number`, `full_name`,
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
