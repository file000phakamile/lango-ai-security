# Team Capability — Lango / AI Data Guard

## Phakamile Mlala — Team Leader

- **Background**: Electronic Engineering student, National University of Science and
  Technology (NUST), Bulawayo.
- **Role on this submission**: Team Leader — drove the product concept, pilot scope,
  and end-to-end delivery of this submission, including directing the build and
  documentation of this demo.
- **Skills demonstrated on this project**: hands-on across the stack — directed the
  frontend build (Next.js/TypeScript/Tailwind dashboard), the data model design
  (`lib/lango/types.ts`, the audit-log/fairness/drift record shapes), the product's
  security and compliance framing (the six-stage pipeline, fairness and drift
  monitoring approach), and the deployment to Vercel.
- **Domain grounding**: the pilot scope (regional commercial bank, Credit Risk
  department) and the entity types the product targets (national ID, bank account,
  medical record number, etc.) reflect direct familiarity with the kind of
  institutional data-handling risk this product is meant to address.

## Vanessa Moyo — Team Member

- **Role**: Researcher.
- **Research focus**: _[Vanessa's research focus — to be confirmed by team]_. No
  specific domain, institution, or focus area for Vanessa is documented anywhere in
  this repo (proposal, prior commits, or Questions.md), so this is intentionally left
  as a placeholder rather than guessed at.
- **Plausible contribution areas (inference, not confirmed fact)**: a "Researcher"
  role on a product like Lango could plausibly sit in any of a few places — this is
  the team's judgment call to make, not something inferred here as settled:
  - *Regulatory/compliance research* — e.g. aligning the product with Zimbabwe's Data
    Protection Act and relevant procurement rules for the target institutions.
  - *Market/user research* — e.g. understanding target-institution needs and pilot
    readiness (banks, hospitals, ministries).
  - *Fairness/bias research* — e.g. the Disparate Impact Ratio and language-parity
    methodology used in the Fairness Audit view.
  None of these is confirmed — see [Questions.md](../Questions.md) for the note
  asking the team to confirm and replace this with her actual contribution area
  before final submission.
- **Skills / background**: _TODO — to be filled in by the team. Not invented here;
  leave blank rather than guess._
- **Contribution area on this submission**: _TODO — to be filled in by the team,
  replacing the inferred possibilities above with what she actually did._

## Tools and external support used

Claude and Claude Code (Anthropic) were used throughout this submission for planning,
content drafting, and implementation — including porting the original dashboard
artifact to this Next.js codebase and producing this documentation set (README,
architecture docs, business model, security/privacy, UX design, testing log, and this
file). All Claude-generated output was reviewed by the team, and the team understands
what was built and why before submission. This disclosure is intentionally explicit
and consistent across this file and the [README](../README.md#team) — it is not
hidden or minimised.

## 30 / 60 / 90-day plan

Reformatted from the pilot and deployment milestones already defined in this demo
(Pilot & Sandbox view, [DEPLOYMENT_PLAN.md](DEPLOYMENT_PLAN.md), and
[BUSINESS_MODEL.md](BUSINESS_MODEL.md)) into a single team-execution cadence:

- **Day 30**
  - Confirm a pilot institution and department (target scope already defined: a
    regional commercial bank, Credit Risk department, ~30 users).
  - Sign off the data-use consent flow and provision a tenant-isolated environment.
  - Begin backend build: stand up the Rust + Axum API skeleton and PostgreSQL schema
    for the audit log (this repo currently has neither — see
    [ARCHITECTURE.md](ARCHITECTURE.md)).
  - Onboard the first cohort of pilot users.

- **Day 60**
  - Midpoint pilot review (matches the "Midpoint review (week 4)" checklist item
    already in the demo, extended to this cadence).
  - Measure redaction accuracy and false-positive rate against target (>95% / <4%,
    per the demo's Pilot & Sandbox metrics) on real pilot traffic instead of
    synthetic figures.
  - Run at least one fairness audit cycle (DIR/SPD) and wire up drift monitoring
    (PSI/KL) against live data.
  - Run a first staff-friction survey.

- **Day 90**
  - Full pilot cohort onboarded (30 target users, one department).
  - Go/no-go decision on expanding to additional departments (Claims Processing,
    Patient Records, Bursar's Office, Legal Affairs — all already modelled in this
    demo's data structures) or additional institutions, based on real pilot evidence.
  - Vanessa's contribution area and role to be reflected here with concrete
    deliverables once filled in above — placeholder pending team input.
