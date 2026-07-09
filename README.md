# Lango — AI Data Guard

Submission for the **AI4I 2026 Challenge — Track 4 (Deployment)**.

## Problem

Employees at banks, hospitals, insurers, and government ministries are increasingly
pasting real customer, patient, and citizen data into public AI chat tools (ChatGPT,
Copilot, Gemini, etc.) to speed up their work. National IDs, bank account numbers,
medical record numbers, and phone numbers routinely leave the organisation's control
this way — with no logging, no review, and no way for the institution to prove it
happened or stop it happening again. For regulated institutions this is both a
compliance failure and an unmanaged data-protection risk.

## Solution

Lango is a **security and governance gateway** that sits between staff and any AI
provider. Every prompt passes through a fixed pipeline before it reaches an AI model —
**Authentication → Prompt Scanner → Redaction Engine → AI Gateway → Response Scanner →
Audit Service** — so sensitive entities (national IDs, bank account numbers, phone
numbers, full names, medical record numbers, API keys) are detected and redacted
*before* they leave the institution, and every request is written to a permanent,
reviewable audit log. Detection is deliberately **rule-based pattern matching + NER**,
not a generative model, so every redaction decision is explainable and auditable rather
than a black box.

## Demo

This repository is the **judge-facing dashboard demo** for the submission — it shows
what an institution's compliance and security team would see once Lango is deployed.

**Live demo:** https://lango-app-dusky.vercel.app

The five views (sidebar navigation): Command Center, Audit Log, Fairness Audit,
Drift & Security, and Pilot & Sandbox. See [docs/UX_DESIGN.md](docs/UX_DESIGN.md) for
what each one shows and why.

A narrated walkthrough script for a screen recording of this demo is at
[docs/VIDEO_SCRIPT.md](docs/VIDEO_SCRIPT.md); slide-by-slide pitch content is at
[docs/PITCH_DECK_CONTENT.md](docs/PITCH_DECK_CONTENT.md).

## Architecture

**This repo is a frontend-only demo.** It is a Next.js dashboard rendering
**synthetic, deterministically-generated mock data in the browser** — there is no
backend, no database, and no live connection to any AI provider anywhere in this
codebase. Every number, log row, and chart on screen is produced client-side by
[`lib/lango/mock-data.ts`](lib/lango/mock-data.ts) from a seeded PRNG, purely to make
the concept and the UI legible to a reviewer.

The **target production architecture** (not present in this repo) is:
Rust + Axum backend, PostgreSQL database, JWT + Argon2 authentication, and a
rule-based pattern-matching + NER detection layer (not generative AI, by design, for
explainability).

Full breakdown of demo-vs-target for every layer (frontend, backend, database, AI
layer, integrations, security, monitoring, outputs) is in
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md), including a diagram at
[docs/architecture-diagram.svg](docs/architecture-diagram.svg).

## Data

All data shown in this demo is **synthetic**, generated at runtime by a seeded PRNG —
no real user, employee, or institutional data is used or stored anywhere in this repo.
See [docs/DATA_AI_USAGE.md](docs/DATA_AI_USAGE.md) for full detail on data structure,
rights, and validation approach.

## AI Method

Lango's detection layer is **rule-based pattern matching + Named Entity Recognition
(NER)**, not a generative model — this is a deliberate design choice so every
redaction decision is explainable and traceable to a specific rule, which matters for
audit and regulatory review. See [docs/DATA_AI_USAGE.md](docs/DATA_AI_USAGE.md) for
the full rationale and validation approach (Disparate Impact Ratio / Statistical
Parity Difference fairness checks, shown live in the Fairness Audit view).

## Setup

Requires Node.js (18+) and npm.

```bash
git clone <this-repo-url>
cd lango-app
npm install
npm run dev
```

Open http://localhost:3000. That's it — no environment variables, database, or API
keys are required to run this demo (see [Environment Variables](#environment-variables)
below).

Other scripts, from `package.json`:

```bash
npm run build   # production build
npm run start   # serve the production build
npm run lint    # eslint
```

## Environment Variables

**None are required to run this demo.** [`.env.example`](.env.example) documents what
a *production* deployment of the real Lango backend would need (AI provider API key,
database connection string, JWT signing secret, etc.) — those are placeholders for the
target architecture, not something this repo reads or uses today.

## Tests

**There is no automated test suite in this repo yet.** This is an honest gap, not an
oversight we're hiding — see [Known Limitations](#known-limitations) and
[docs/TESTING_LOG.md](docs/TESTING_LOG.md), which tracks manual click-through testing
of the dashboard views instead.

## Deployment

Deployed to Vercel via the Vercel CLI (`vercel --prod`), not a GitHub-integration
auto-deploy — pushing to this repo does **not** automatically redeploy the live demo;
that requires re-running the CLI (or wiring up Vercel's Git integration separately).
See [docs/DEPLOYMENT_PLAN.md](docs/DEPLOYMENT_PLAN.md) for the demo hosting details and
the target pilot deployment plan.

## Known Limitations

Stated plainly, not softened:

- **No backend exists.** Everything in this repo is a static frontend over synthetic,
  client-generated mock data. There is no API, no database, no authentication, and no
  connection to any AI provider.
- **No automated tests.** No unit, integration, or end-to-end test suite exists yet.
  Verification so far is manual click-through, logged in
  [docs/TESTING_LOG.md](docs/TESTING_LOG.md).
- **Mobile responsiveness is confirmed broken at small widths, not just unverified.**
  Tested at 375px width (see [docs/TESTING_LOG.md](docs/TESTING_LOG.md)): the sidebar
  is a fixed 224px column with no responsive breakpoint, leaving only ~150px for
  content — KPI values and chart labels get cut off and the Audit Log table is
  reduced to a single unreadable column. Root cause identified; fix (a collapsible
  sidebar + responsive grid breakpoints) not yet attempted, since it's a real UI
  change that deserves its own scoped pass rather than a rushed patch.
- **No real user feedback yet.** No pilot users have interacted with this demo; see
  [docs/UX_DESIGN.md](docs/UX_DESIGN.md).

## Team

- **Phakamile Mlala** — Team Leader. Electronic Engineering student, National
  University of Science and Technology (NUST), Bulawayo.
- **Vanessa Moyo** — Team Member.

Claude and Claude Code (Anthropic) were used throughout this submission for planning,
content drafting, and implementation (including porting the dashboard to Next.js and
producing this documentation set). All generated output was reviewed and is understood
by the team before submission.
