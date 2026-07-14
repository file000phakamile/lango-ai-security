# UX Design — Lango / AI Data Guard

## User persona

**Name:** Nomsa Ndlovu
**Role:** Head of Compliance, Regional Commercial Bank (candidate pilot institution)
**Context:** Nomsa is responsible for proving to regulators and internal audit that
the bank's data-protection controls are actually working, not just documented on
paper. She doesn't write code and doesn't want to — her job is to be able to answer,
on short notice, "did any customer data leave this bank through an AI tool this
quarter, and can you show me the record?"

**Pain point:** Staff across Credit Risk, Claims Processing, and other departments
have started using AI tools to speed up drafting and summarisation work. Nomsa has no
visibility into this at all — no log of what was pasted into an AI chat window, no way
to stop a National ID number leaving the bank mid-prompt, and nothing to hand a
regulator if asked. Existing DLP (data-loss-prevention) tooling wasn't built with AI
chat interfaces in mind and misses this entirely.

**What she needs from Lango:** A dashboard she can open herself, without needing an
engineer to query a database for her, that shows what was flagged, what was redacted,
whether the system is behaving fairly across language and department, and whether
anything has silently degraded — in language she can act on and defend in an audit.

## User journey

1. **Problem** — Nomsa becomes aware (via an internal audit finding or a near-miss)
   that staff are pasting real customer data into AI tools with no oversight.
2. **Discovers the AI data leak risk** — She realises this is an unmanaged
   data-protection exposure: no logging, no control, no evidence trail if a regulator
   asks.
3. **Deploys Lango** — Lango is set up as a gateway in front of the AI tools her
   department already uses, starting with one department (Credit Risk) as a pilot
   scope (see [DEPLOYMENT_PLAN.md](DEPLOYMENT_PLAN.md)).
4. **Sees the redacted prompt** — When a staff member's prompt contains a sensitive
   entity, Lango redacts it before it reaches the AI provider; Nomsa can see this
   happening in near-real-time on the Command Center view.
5. **Gets the audit trail** — At any point, Nomsa opens the Audit Log view and can
   show exactly what was flagged, why, and what happened to it — the evidence she
   didn't have before.

## Screens

The demo has eight views, switched via the sidebar (no page navigation — a single
client-side dashboard component):

1. **Command Center** — Live-feeling overview: KPI tiles (sessions scanned, blocked/
   redacted count, average risk score, active alerts), an animated request-trace
   walkthrough of the six-stage pipeline, and a recent-events feed mirroring the
   Audit Log.
2. **Audit Log** — The full, filterable, expandable table of every logged request:
   session id, timestamp, department, entities detected, risk score, and decision,
   with a row-expand for the reason string, AI model used, and response-scan result.
3. **Fairness Audit** — Bar charts comparing flag rates by session language and by
   department, alongside the computed Disparate Impact Ratio and Statistical Parity
   Difference, with an inline alert when DIR falls below the 0.80 threshold.
4. **Drift & Security** — A line chart of PSI and KL-divergence over 12 weeks against
   an alert threshold, plus a chronological feed of security events (prompt injection
   blocked, rate limiting, DoS mitigation).
5. **Pilot & Sandbox** — Pilot scope and rollout checklist (users onboarded, data
   isolation, consent sign-off) alongside midpoint success metrics against target.
6. **Health Data Guard** — Cimas Healthathon 3.0 addition (see
   [HEALTH_MODULE.md](HEALTH_MODULE.md)): standard vs. special-category-health split,
   redaction rate for special-category rows, and facility-type parity (DIR/SPD) —
   deliberately no per-condition or per-medication breakdown, to avoid an aggregate
   view that could out a specific health condition by department.
7. **Policy Builder** — `compliance_admin`-only. Lets an organisation adjust its own
   confidence threshold within safe, hard-coded bounds and add organisation-specific
   structured-identifier patterns (e.g. a bank's own account-number format), applied
   only to that organisation's own scans. Live-only — deliberately has no mock-data
   fallback, since a fabricated setting value would misrepresent a number that
   actually controls live scans (see [Questions.md](../Questions.md) item 23). The
   near-zero name-confidence floor and the special-category-health hard rule are not
   shown here as settings because neither is configurable, by anyone.
8. **Compliance Export** — `compliance_admin`-only. A date range picker plus two
   buttons ("Download CSV" / "Download PDF") that produce a one-click export of the
   audit log, fairness metrics, and drift history together, formatted plainly enough
   to hand to an external auditor or regulator without further editing. Also
   live-only, same reasoning as Policy Builder — see
   [Questions.md](../Questions.md) item 24.

## Information architecture

A fixed left sidebar (`LangoDashboard` component) lists all eight views with an icon
and label each; the active view is highlighted with a left border accent and a
slightly different background. Below the nav, a static footer note reinforces the
product's core promise ("No raw prompts stored"). The main content area has a header
showing the active view's title, the current pilot institution/department context,
and a live "system operational" status badge — so the institutional context is always
visible regardless of which view is open. There is no routing between views (all
client-side state); this was a deliberate simplicity choice for a single-session demo,
not a claim about how a multi-page production app would be structured.

## Data visualisation logic

Each chart type was chosen to match what the underlying comparison actually is:

- **Bar chart, horizontal (Fairness Audit — language parity)**: comparing flag rates
  across three named categories where the category labels (English, Ndebele, Shona)
  are the primary read; horizontal bars keep the labels legible without rotation and
  make the Shona bar (the one below threshold) easy to pick out visually via a
  distinct fill colour.
- **Bar chart, vertical (Fairness Audit — department parity)**: five department
  categories compared the same way; vertical works here since department names are
  short enough to angle without crowding, and it visually distinguishes this
  second comparison from the language chart above it.
- **Line chart (Drift & Security)**: PSI and KL-divergence are both *continuous
  measurements over time* (12 weeks) being watched against a fixed threshold —
  a line chart is the correct choice for trend-over-time data, and a reference line
  at the 0.20 threshold makes the week-9 breach immediately visible against the trend.
- **Table (Audit Log)**: the audit log is a record-level, multi-field dataset where a
  compliance reviewer needs to scan, filter, and drill into individual rows — a table
  is the only structure that supports that, versus a chart which would need to
  aggregate and lose the record-level detail that's the entire point of an audit log.
- **KPI tiles (Command Center, Pilot & Sandbox)**: single current-value metrics
  (sessions scanned, users onboarded, redaction accuracy) where the point is a fast
  at-a-glance read, not a trend or comparison — a numeric tile with a label and unit
  is the simplest correct representation.

## Accessibility

Stated honestly:

- **Font**: IBM Plex Sans (body/UI) and IBM Plex Mono (data/code values), both
  variable Google Fonts loaded via `next/font`, chosen for legibility at small sizes
  given how much tabular/numeric data the dashboard displays.
- **Colour and contrast**: the app is built and tested in **light theme only**
  (`color-scheme: light` is explicitly set in `app/globals.css`); status colours
  (green `#2F7A53`, amber `#8A6323`, red `#A83A3A`) are paired with icons and text
  labels, not colour alone, for the decision badges and risk bands. Contrast has not
  been run through a formal automated audit (e.g. axe or Lighthouse accessibility
  score) — this is a gap, not a claim of compliance.
- **Mobile responsiveness: fixed and re-tested.** The sidebar collapses to a
  slide-out drawer below the `md` (768px) breakpoint instead of squeezing content
  into ~150px; the Audit Log becomes a stacked card list below `md`; KPI/chart grids
  step down responsively. Verified at 375px, 414px, 768px, 1024px, and 1280px with
  zero horizontal page overflow at any width (see [TESTING_LOG.md](TESTING_LOG.md)
  and [../Questions.md](../Questions.md)). Still not done: a real physical device
  pass (only an emulated viewport in a desktop browser has been tested) and a formal
  accessibility audit — see the Colour and contrast note above and
  TESTING_LOG.md's TODO.
- **Keyboard navigation**: standard browser tab order works for the sidebar buttons,
  the mobile drawer's hamburger/close buttons, table row expansion, and the
  audit-log filter dropdown, since these are native `<button>`/`<select>` elements —
  no custom focus-trap or skip-link work has been done
  beyond that.

## User feedback

**None yet.** No pilot users or external reviewers have used this demo — there is no
real feedback to report, and none is fabricated here. The first feedback this product
will get is from AI4I 2026 judges reviewing this submission.
