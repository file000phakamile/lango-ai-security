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

**This repo now has a real, working backend — v0.1, not production-hardened.** A
Rust + Axum API ([`backend/`](backend/)) backed by a real PostgreSQL schema
implements JWT + Argon2 authentication, a genuine rule-based regex + name-heuristic
detection engine (see [Detection engine](#detection-engine-v01) below for exactly
what "NER" means here), real Disparate Impact Ratio / Statistical Parity Difference
fairness math, and real PSI / KL-divergence drift math — all computed from actual
rows in a `audit_log` table, not fabricated. The Next.js dashboard calls these
endpoints over HTTP; see [Setup](#setup) to run both halves locally.

What's still a deliberate v0.1 simplification, stated plainly: the backend's own "AI
Gateway" pipeline stage remains a labeled no-op — the Rust backend itself still never
calls an AI provider's API server-side (see [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)).
**Separately, a real interception mechanism now exists in front of AI providers' own
web UIs**: [`extension/`](extension/) is a Manifest V3 browser extension that scans
and redacts/blocks prompts *in the browser*, before they ever reach the site's own
send action. It implements five sites — **chatgpt.com is verified working** in a real
browser session; **claude.ai, gemini.google.com, chat.deepseek.com, and
copilot.microsoft.com (Microsoft's consumer web chat, not GitHub Copilot) are
implemented but not yet verified** against a live page (see
[extension/README.md](extension/README.md) for exactly what was and wasn't tested, and
[extension/USER_GUIDE.md](extension/USER_GUIDE.md) for a plain-language walkthrough).
This answers the "nothing intercepts real AI traffic yet" gap for these sites, through
a different architecture (client-side interception, not a server-side forwarding
proxy) than what the backend's AI Gateway stage implies. No live
prompt-injection/rate-limit/DoS detection runs (the Security Events table is seeded
with illustrative rows, not produced by a live detector); there's no scheduled drift
job (drift snapshots are computed once at seed time); and the dashboard has no login
screen of its own — it authenticates transparently as a seeded demo account (see
[Environment Variables](#environment-variables)).

**Both halves are now deployed and wired together**: the backend runs on Render
(`https://lango-backend-qwkx.onrender.com`, real Rust/Axum + PostgreSQL) and the
deployed Vercel demo calls it directly — confirmed pulling live data (not mock) across
all five dashboard views via the browser Network tab. `NEXT_PUBLIC_USE_MOCK_DATA` is
not set on the Vercel deployment, so the mock generator is only ever the *fallback*
path there now (e.g. during Render's free-tier cold start — see
[Deployment](#deployment)), not the default. See [Questions.md](Questions.md) for the
history of how this got wired up.

Full breakdown of demo-vs-target for every layer (frontend, backend, database, AI
layer, integrations, security, monitoring, outputs) is in
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md), including a diagram at
[docs/architecture-diagram.svg](docs/architecture-diagram.svg).

### Detection engine (v0.1)

Real, working, non-simulated code — with real limits, documented honestly:

- **Regex rules** (`backend/src/detection/rules.rs`): national ID, phone number,
  credit card (regex + a real Luhn checksum), API keys/tokens (known-prefix patterns
  plus a deliberately low-confidence generic fallback), medical record number, bank
  account number. Zimbabwean formats used where a public format exists (national ID,
  mobile prefixes); everything else is a documented best-effort/generic pattern — see
  the source comments for exactly what each pattern is and isn't validated against.
- **Names — NOT real NER.** `backend/src/detection/name_heuristic.rs` is a
  capitalized-word-sequence heuristic with a stopword exclusion list, clearly labeled
  in its own doc comment as a simplified stand-in. It was chosen over a transformer-
  based NER crate (`rust-bert`/ONNX options) because those need a native
  libtorch/onnxruntime dependency — too heavy for a "runs locally, not
  production-hardened" v0.1. See [Questions.md](Questions.md) for the full reasoning.
- **Three-tier, entity-type-aware confidence handling** (`backend/src/detection/
  scan.rs`): each match carries a confidence score. High-confidence matches redact and
  forward as always (`redacted_and_forwarded`); near-zero-confidence matches and any
  low-confidence match on a *structured* entity (national ID, bank account, phone
  number, credit card, medical record number, API key) still fail closed and block
  (`blocked_low_confidence`) — unchanged. The one deliberate exception: a low-but-real-
  confidence `full_name` match (the heuristic below has a real false-positive rate on
  ordinary capitalized phrases) is redacted and forwarded automatically rather than
  blocked, tagged `redacted_low_confidence_review` so it's queryable separately for
  async compliance review. Real logic, not a random coin flip like the old mock data
  used — see docs/SECURITY_PRIVACY.md's Human oversight row for the compliance framing
  of this tradeoff.

## Data

All data shown in this demo is **synthetic** — no real user, employee, or
institutional data is used or stored anywhere in this repo, including in the live
deployed system. That data genuinely lives in the production PostgreSQL instance on
Render, produced by [`backend/src/bin/seed.rs`](backend/src/bin/seed.rs), which runs
synthetic prompts through the real detection engine rather than fabricating risk
scores directly — real database rows, real detection output, fabricated input content.
When running against the mock fallback instead (e.g. if the backend is unreachable),
it's generated at runtime in the browser by a seeded PRNG. See
[docs/DATA_AI_USAGE.md](docs/DATA_AI_USAGE.md) for full detail on
data structure, rights, and validation approach, and the note above on what raw
prompt text the backend does (and does not) ever persist.

## AI Method

Lango's detection layer is **rule-based pattern matching + Named Entity Recognition
(NER)**, not a generative model — this is a deliberate design choice so every
redaction decision is explainable and traceable to a specific rule, which matters for
audit and regulatory review. See [docs/DATA_AI_USAGE.md](docs/DATA_AI_USAGE.md) for
the full rationale and validation approach (Disparate Impact Ratio / Statistical
Parity Difference fairness checks, shown live in the Fairness Audit view).

## Setup

### Frontend only (mock data, no backend needed)

Requires Node.js (18+) and npm.

```bash
git clone <this-repo-url>
cd lango-app
npm install
npm run dev
```

Open http://localhost:3000. With no `.env.local`, the dashboard tries the real
backend at `http://localhost:8080` and **falls back to client-side mock data
automatically** if it isn't reachable — so this alone is still enough to see the app
render. See [Environment Variables](#environment-variables) to force mock mode
explicitly, or to point at a real backend.

### Full stack (real backend + real data)

Requires the above, plus Rust (stable) and either Docker, or a local PostgreSQL 14+
instance.

```bash
# 1. Start Postgres (Docker Compose, from the repo root)
docker compose up -d

# 2. Configure and run the backend — applies migrations automatically on boot
cd backend
cp .env.example .env      # defaults already match docker-compose.yml
cargo run --bin lango-backend
```

In a second terminal, seed realistic sample data (safe to re-run — truncates and
reseeds):

```bash
cd backend
cargo run --bin seed
```

This prints the two demo login accounts it creates
(`compliance@lango.demo` / `admin@lango.demo`, password `LangoDemo123!`) — the
frontend uses the `compliance` one automatically (see
[Environment Variables](#environment-variables)).

In a third terminal, run the frontend against it:

```bash
cp .env.local.example .env.local   # sets NEXT_PUBLIC_API_BASE_URL etc.
npm run dev
```

Open http://localhost:3000 — the "system operational" badge in the top-right confirms
you're looking at live backend data, not the mock fallback (it reads "mock data
(backend unavailable)" otherwise).

Test the detection engine directly:

```bash
TOKEN=$(curl -s -X POST http://localhost:8080/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"email":"compliance@lango.demo","password":"LangoDemo123!"}' | jq -r .token)

curl -s -X POST http://localhost:8080/api/scan \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"prompt":"Please verify national ID 63-123456A23 for John Moyo, phone 0771234567."}' | jq
```

Other scripts, from `package.json`:

```bash
npm run build   # production build
npm run start   # serve the production build
npm run lint    # eslint
```

Backend tests: `cd backend && cargo test` (unit tests for the detection engine — see
`detection::scan` and `detection::drift`).

## Environment Variables

**None are required to run just the frontend against mock data** — that's the
default the deployed Vercel demo uses. Two separate `.env.example` files cover the
two halves of the real system:

- [`.env.local.example`](.env.local.example) (frontend, copy to `.env.local`):
  `NEXT_PUBLIC_API_BASE_URL`, `NEXT_PUBLIC_USE_MOCK_DATA`, and the demo login the
  dashboard authenticates as automatically (there's no login screen in v0.1).
- [`backend/.env.example`](backend/.env.example) (backend, copy to `backend/.env`):
  `DATABASE_URL`, `JWT_SIGNING_SECRET`, `PORT`, `CORS_ORIGIN`.
- [`.env.example`](.env.example) at the repo root is legacy/aspirational — it
  documents variables a *hosted production* deployment would additionally need (an AI
  provider API key, an alert webhook) that no code in this repo reads yet, since v0.1
  doesn't connect to a live AI provider or alerting service.

## Tests

Backend: `detection::scan` and `detection::drift` have real unit tests
(`cd backend && cargo test`) covering the regex rules, the Luhn check, the
fail-closed threshold, and the PSI/KL-divergence math. There is no integration or
end-to-end test suite yet, and no frontend test suite — an honest gap, not an
oversight we're hiding. See [Known Limitations](#known-limitations) and
[docs/TESTING_LOG.md](docs/TESTING_LOG.md), which tracks manual click-through testing
of the dashboard views.

## Deployment

**Frontend**: deployed to Vercel via the Vercel CLI (`vercel --prod`), not a
GitHub-integration auto-deploy — pushing to this repo does **not** automatically
redeploy the live demo; that requires re-running the CLI (or wiring up Vercel's Git
integration separately).

**Backend**: set up for deployment to [Render](https://render.com) as a Blueprint
(`render.yaml` at the repo root defines the web service + managed Postgres database,
both free tier) — deploy it via the Render Dashboard's *New > Blueprint*, connecting
this repo; `render.yaml` prompts for the secret values (`JWT_SIGNING_SECRET`,
`CORS_ORIGIN`) rather than storing them in the file. **Free-tier honesty note**: the
web service spins down after 15 minutes of inactivity — the first request after an
idle period takes roughly 30-60 seconds while it spins back up. This is expected
Render free-tier behaviour, not a bug. If the frontend is pointed at a cold backend, it
falls back to mock data automatically (see [Environment
Variables](#environment-variables)) rather than showing a blank screen while it wakes.

See [docs/DEPLOYMENT_PLAN.md](docs/DEPLOYMENT_PLAN.md) for the full demo hosting
details and the target pilot deployment plan.

## Known Limitations

Stated plainly, not softened:

- **v0.1, not production-hardened.** The backend, database, and detection engine are
  real and functioning — deployed and verified end-to-end (Rust/Axum on Render, real
  PostgreSQL, real auth, all five dashboard views confirmed pulling live data) — this
  is no longer a frontend-only simulation. But nothing here has had a security/
  production hardening pass, and it hasn't been load-tested or exercised under real
  institutional traffic. See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the full
  v0.1-vs-target breakdown.
- **The backend's own "AI Gateway" stage is still a labeled no-op** (`ai_model_used`
  is always the literal string documenting that no provider is connected) — the Rust
  backend itself never calls an AI provider's API server-side. **What does now exist**:
  [`extension/`](extension/), a browser extension that intercepts prompts client-side
  in front of five AI providers' web UIs and redacts/blocks them before they're sent —
  a real, working, differently-architected answer to "does anything actually gate real
  AI traffic yet." **Only chatgpt.com's DOM-dependent parts have been verified**
  against a live session; **claude.ai, gemini.google.com, chat.deepseek.com, and
  copilot.microsoft.com are implemented but not yet verified** against live pages —
  this distinction matters and is not collapsed anywhere it's mentioned. See
  [extension/README.md](extension/README.md)'s Verification and Known fragility
  sections, and each `content/<site>-adapter.js` file's own header comment, before
  relying on any one of the four unverified sites.
- **No live security-event detection.** Prompt-injection/rate-limit/DoS detection is
  not implemented; the Security Events table is seeded with illustrative example rows
  (see `backend/src/bin/seed.rs`), not produced by a live detector.
- **No login screen.** The dashboard authenticates as a single fixed seeded demo
  account (see [Environment Variables](#environment-variables)) rather than having its
  own auth UI or real multi-user credentials — fine for a judge/reviewer demo, not how
  a real multi-user deployment would work.
- **The deployed Vercel demo now calls the live Render backend by default**, not mock
  data — `NEXT_PUBLIC_USE_MOCK_DATA` is unset on the Vercel deployment. Mock data is
  only ever the automatic fallback if the backend is unreachable (e.g. mid cold-start
  on Render's free tier — see [Deployment](#deployment)).
- **No integration or end-to-end test suite.** Backend detection-engine logic has
  real unit tests (`cargo test`); everything above that (API integration, frontend)
  is verified by manual click-through, logged in
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
