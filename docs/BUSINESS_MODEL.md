# Business Model — Lango / AI Data Guard

## Problem

Staff at regulated institutions (banks, hospitals, insurers, ministries) are pasting
real customer, patient, and citizen data into consumer AI tools with no oversight.
There is no visibility into what left the organisation, no way to stop it in real
time, and no audit trail to show a regulator afterwards. This is a live, unmanaged
data-protection and compliance exposure, not a hypothetical one — it happens every
time a staff member uses an AI chat tool to draft a letter, summarise a case file, or
speed up a report using real records as the source text.

## Primary user

Frontline staff at a regulated institution who use AI tools day-to-day for their job
(e.g. a credit risk analyst drafting a report, a claims processor summarising a case,
a records clerk searching patient notes) — the people whose prompts pass through
Lango's gateway.

## Beneficiary

The institution itself: its compliance, risk, and legal functions, who are accountable
for data protection and regulatory exposure, and who currently have no visibility into
AI-tool usage at all.

## Customer / payer

The institution (bank, hospital, insurer, or government ministry), purchased and
budgeted at the compliance/IT-security level, not by individual staff. Procurement
would typically sit with a CISO, Head of Compliance, or Head of IT.

## Value proposition

- **Prevents** sensitive data (national IDs, bank account numbers, medical record
  numbers, phone numbers) from leaving the institution via AI prompts, by redacting it
  before the prompt reaches any AI provider.
- **Proves** compliance after the fact with a permanent, queryable audit log of every
  request, decision, and reason — evidence an institution can show a regulator.
- **Explains** every decision, because detection is rule-based pattern matching + NER
  rather than a generative model — a compliance officer can trace exactly which rule
  fired and why, which a black-box model cannot offer.
- **Monitors itself**: fairness checks (Disparate Impact Ratio / Statistical Parity
  Difference across language and department) and drift detection (PSI / KL-divergence)
  are built into the product, not a bolt-on audit exercise.

## Revenue / funding model

Pilot phase: no direct revenue — the goal is a validated pilot deployment at one
institution, one department, to prove the concept and generate the case-study evidence
needed for paid rollout. Post-pilot, the intended model is institutional licensing:
a per-seat or per-institution annual subscription, priced against the cost of a single
compliance incident or regulatory fine avoided, which is the realistic budget line this
competes against rather than a per-request AI usage fee.

## Cost drivers

- AI provider API costs (pass-through/markup on whichever model the sanitised prompt
  is forwarded to).
- Hosting and database costs for the audit log and gateway (scales with request
  volume).
- Ongoing pattern-rule and NER model maintenance (new document/ID formats per market,
  new entity types, false-positive tuning) — this is a recurring, not one-off, cost
  because entity formats and institutional documents change over time.
- Compliance and security overhead (penetration testing, audits, certifications) to be
  credible to regulated-industry buyers.

## Partnerships

- **Pilot institution partnership** (target: a regional commercial bank's Credit Risk
  department, per the current pilot scope in the demo) — needed to validate the
  product against real institutional workflows and generate reference evidence.
- **AI provider relationship** — Lango sits in front of, not instead of, an AI
  provider, so its value depends on a stable connector to whichever provider(s) the
  institution already uses.
- **Regulatory/compliance advisory** — a local data-protection or financial-regulation
  advisor to keep pattern rules and audit-log fields aligned with actual regulatory
  requirements in the target market.

## Pilot market

Zimbabwe-based regulated institutions initially, matching the team's base (NUST
Bulawayo) and the pilot scope already defined in this demo: a regional commercial
bank, starting with the Credit Risk department (~30 target users, one department),
before expanding to other departments (Claims Processing, Patient Records, Bursar's
Office, Legal Affairs) and other institution types.

## Adoption risks

- **Staff friction**: if redaction is too aggressive or slows workflows, staff route
  around the tool entirely (defeats the purpose). The demo's pilot metrics already
  track "staff-reported friction" as a first-class success metric for exactly this
  reason.
- **False negatives**: a missed entity format (e.g. a new national ID card layout)
  means real data leaks despite the tool being in place — this already happened once
  in the simulated pilot timeline (week-9 drift spike) and is the reason drift
  monitoring exists.
- **Trust gap**: asking a regulated institution to route AI traffic through a
  third-party gateway is a significant trust ask; without a credible pilot and audit
  evidence, procurement stalls.
- **Backend not yet pilot-hardened**: the Rust/Axum backend is now real, deployed
  (Render), and verified end-to-end — this is no longer a frontend-only demo — but
  there is real execution risk in hardening it (multi-tenant isolation, a live AI
  provider connection, rate limiting, production security review) before any paid
  pilot can run on real institutional traffic.

## Success metrics at 30 / 60 / 90 days

*(Pilot metrics, matching the cadence used in docs/DEPLOYMENT_PLAN.md and
docs/TEAM_CAPABILITY.md.)*

- **30 days**: Pilot scope and institution partner confirmed; data-use consent signed
  off; tenant-isolated environment provisioned; first cohort of pilot users onboarded.
- **60 days**: Midpoint review complete; redaction accuracy and false-positive rate
  measured against target (>95% accuracy, <4% false positives per the demo's Pilot &
  Sandbox view); staff friction survey run at least once.
- **90 days**: Full pilot user cohort onboarded (30 target); fairness audit (DIR/SPD)
  run at least one full quarterly cycle; go/no-go decision made on expanding beyond
  the single pilot department, backed by real usage data rather than projections.
