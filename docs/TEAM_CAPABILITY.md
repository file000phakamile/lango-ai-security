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

- **Role**: Researcher, Product Design. Confirmed by the team.
- **Research focus**: _[Vanessa's specific research focus/domain — still to be
  confirmed by team]_. Her role is now confirmed as Researcher, Product Design, but no
  further detail (specific domain, institution, or prior work) is documented anywhere
  in this repo, so that narrower detail is still intentionally left as a placeholder
  rather than guessed at.
- **Skills / background (reasonable inference from her confirmed role, not
  additional confirmed fact)**: a Researcher/Product Design role on this specific
  submission plausibly combines user/institutional research with UX decision-making —
  this is inferred from the role title and this repo's actual UX artefacts, not a
  separately confirmed skills list. No technical/engineering skills (coding, the
  Next.js frontend build, the Rust backend) are attributed to her here, since none is
  confirmed; see Phakamile Mlala's section above for the confirmed technical work.
- **Contribution area (reasonable inference from her confirmed role, not confirmed
  fact)**: plausibly informed two things already visible in this repo's artefacts:
  - *Target-user framing* — the compliance-officer persona (Nomsa Ndlovu, Head of
    Compliance) and institutional pain points documented in
    [UX_DESIGN.md](UX_DESIGN.md)'s User persona / User journey sections, and the
    primary-user/beneficiary framing in [BUSINESS_MODEL.md](BUSINESS_MODEL.md), are
    the kind of output user/institutional research would produce — plausibly hers,
    not confirmed as hers specifically.
  - *Product design input* — the dashboard's five-view information architecture and
    the per-chart data-visualisation choices (bar vs. line vs. table vs. KPI tile,
    matched to what each comparison actually is), both documented in
    [UX_DESIGN.md](UX_DESIGN.md)'s Information architecture and Data visualisation
    logic sections, are the kind of output a Product Design contribution would
    produce — again plausibly hers, not confirmed as hers specifically.
  **Team: please confirm which of the above (if any) was actually Vanessa's work**,
  and replace this inferred framing with the confirmed specifics — see
  [Questions.md](../Questions.md).

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
  - Sign off the data-use consent flow and provision a tenant-isolated environment
    (the current v0.1 backend — Rust + Axum API and PostgreSQL schema, deployed and
    verified end-to-end on Render — is real but not yet tenant-isolated; see
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
  - Vanessa's role (Researcher, Product Design) is now confirmed above; her specific
    contribution area is still inferred rather than confirmed — replace with concrete
    deliverables once the team confirms it.
