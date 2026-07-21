# Lango — AI Data Guard

**Looking for how to actually use this, not the technical detail below?** See
[HOW_TO_USE.md](HOW_TO_USE.md) — a plain-language guide to both the extension
and the dashboard.

## Pitch Deck

A real, ten-slide pitch deck is available in two formats, both sourced verbatim from
[docs/PITCH_DECK_CONTENT.md](docs/PITCH_DECK_CONTENT.md) and kept in sync with each
other:

- **[`pitch-deck/index.html`](pitch-deck/index.html)** — open directly in any browser
  (no build step, no software required), navigate with the on-screen arrows or your
  keyboard's left/right arrow keys, and use the browser's own Print function
  (Ctrl/Cmd+P) for a clean, correctly paginated PDF, one slide per page. Use this for
  viewing, presenting from a browser, or a quick PDF export.
- **[`pitch-deck/Lango_Pitch_Deck.pptx`](pitch-deck/Lango_Pitch_Deck.pptx)** — a real
  PowerPoint file (seven slides as real, editable text; three as high-resolution
  images captured directly from the HTML deck's own verified visuals). Use this if
  you need an editable .pptx directly, e.g. to hand to someone presenting from
  PowerPoint rather than a browser.

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

**Live demo:** https://lango-app-dusky.vercel.app

The eight dashboard views (sidebar navigation): Command Center, Audit Log, Fairness
Audit, Drift & Security, Pilot & Sandbox, Health Data Guard, Policy Builder, and
Compliance Export. See [docs/UX_DESIGN.md](docs/UX_DESIGN.md) for what each one shows
and why. **`/chat`** is a separate, real route (not part of that sidebar switch) — a
native in-app chat interface backed by the organisation's own OpenAI key; see
[Native chat](#native-chat-v01) below for what it is and when to use it instead of, or
alongside, the browser extension.

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

**The backend's own "AI Gateway" pipeline stage is no longer a no-op.** For years this
document said the Rust backend never calls an AI provider server-side — that was true
until the native chat feature below. `POST /api/chat` genuinely does call OpenAI, with
the organisation's own key, and streams the reply back — this is what finally makes
that pipeline stage real rather than documented-but-absent. It's real only for chat's
own request path, though: `/api/scan` (the extension's endpoint) still never touches a
provider itself, by design — see [Native chat](#native-chat-v01) below and
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

**Separately, the client-side interception mechanism in front of AI providers' own
web UIs is unchanged and still fully supported**: [`extension/`](extension/) is a
Manifest V3 browser extension that scans and redacts/blocks prompts *in the browser*
before they ever reach the site's own send action (**Prompt Scanner** in the pipeline
description above), and — as of the "response scanning + observability + hardening"
task — also scans the AI's reply once it finishes rendering, on chatgpt.com, claude.ai,
and gemini.google.com (**Response Scanner**): a flagged reply gets a plain-language
warning banner, but the AI's actual response is never modified, hidden, or altered —
see `detection::scan::scan_response`'s doc comment and the Response scanning section
below for the full reasoning. It implements five sites for prompt scanning —
**chatgpt.com is verified working** in a real browser session; **gemini.google.com is
now ALSO verified working, end-to-end, including response scanning** (a real, loaded
extension driven against a live, anonymous gemini.google.com session — see
[Questions.md](Questions.md) item 26); **claude.ai, chat.deepseek.com, and
copilot.microsoft.com are implemented but not yet verified** against a live page (see
[extension/README.md](extension/README.md) for exactly what was and wasn't tested, and
[extension/USER_GUIDE.md](extension/USER_GUIDE.md) for a plain-language walkthrough).
This answers the "nothing intercepts real AI traffic yet" gap for these sites, through
a different architecture (client-side interception, not a server-side forwarding
proxy) than what the backend's AI Gateway stage implies. No live
prompt-injection/rate-limit/DoS detection runs (the Security Events table is seeded
with illustrative rows, not produced by a live detector); there's no scheduled drift
job (drift snapshots are computed once at seed time); and the dashboard has no login
screen of its own — it authenticates transparently as a seeded demo account (see
[Environment Variables](#environment-variables)), even though the backend itself now
supports real multi-user, multi-organisation login (see
[Multi-tenancy](#multi-tenancy-v01) below) — the dashboard's own login/signup UI is
the gap, not the backend behind it.

### Multi-tenancy (v0.1)

**Real, not aspirational.** Every user belongs to exactly one organisation and one of
three roles, enforced by query-level filtering on every endpoint — not just a schema
column that happens to exist:

- **`staff`** — can call `/api/scan` only; no dashboard access at all.
- **`department_reviewer`** — dashboard access (the audit log specifically) scoped to
  their own department, within their own organisation.
- **`compliance_admin`** — full dashboard access across their own organisation, never
  another organisation's data.

Cross-tenant isolation is verified by dedicated tests
(`backend/tests/multi_tenant_isolation.rs`) that create a second, fully independent
organisation and confirm every endpoint returns zero rows/values derived from it for
a caller in the first — not just that the right rows come back, which would pass even
if the filter were silently missing. A new organisation can register itself via
`POST /api/organisations/signup` (backend-only in v0.1 — see
[Questions.md](Questions.md) for why there's no dashboard signup page yet), and a
brand-new user in a brand-new organisation must accept a data-use consent screen
(shown in the browser extension's popup, recorded with a timestamp and the specific
policy version accepted) before `/api/scan` will do anything for them — checked fresh
against the database on every call, not the JWT.

**The demo account this repo's judged submission depends on
(`compliance@lango.demo`) is deliberately, explicitly preserved unchanged**: it's now
the seeded `compliance_admin` account of one specific "Regional Commercial Bank
Demo" organisation rather than the only account in the system, but it logs in with
the same password, sees the same audit history, and works identically through both
the dashboard and the extension — verified directly, not assumed, as part of this
change. See [Questions.md](Questions.md) for the full design writeup and the
judgment calls behind it.

**Both halves are now deployed and wired together**: the backend runs on Render
(`https://lango-backend-qwkx.onrender.com`, real Rust/Axum + PostgreSQL) and the
deployed Vercel demo calls it directly — confirmed pulling live data (not mock) across
all five original dashboard views via the browser Network tab (three more views have
since been added locally — see [Multi-tenancy](#multi-tenancy-v01) and the Policy
builder/Compliance export/Active learning loop sections below — and are covered by
integration tests and Playwright, but haven't been re-checked against the live Vercel
deployment specifically, since nothing has been redeployed since). `NEXT_PUBLIC_USE_MOCK_DATA` is
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

### Policy builder (v0.1)

A `compliance_admin` can adjust two things for their own organisation, within safe
hard-coded bounds, from the **Policy Builder** dashboard view (`/api/policy/*`,
`backend/src/routes/policy.rs`):

- **Confidence threshold** — replaces the fixed 0.60 default per organisation,
  clamped server-side to `[0.50, 0.95]` (`MIN_ORG_CONFIDENCE_THRESHOLD` /
  `MAX_ORG_CONFIDENCE_THRESHOLD` in `backend/src/detection/scan.rs`), enforced by a
  database `CHECK` constraint AND the API handler, not just client-side validation.
- **Organisation-specific structured-identifier patterns** — a custom regex + label
  (e.g. a specific bank's own account-number format), matched alongside the built-in
  detectors and applied only to that organisation's own scans.

What is explicitly **not** configurable, by anyone, from anywhere: the near-zero
`NAME_LOW_CONFIDENCE_FLOOR` (0.30) and the special-category-health-data leniency
exclusion (a low-confidence match on diagnosis codes, medications, medical aid
numbers, lab values, or next-of-kin names always fails closed — it never gets the
lenient review path a low-confidence name gets). Neither is threaded through the
per-organisation config at all — see [Questions.md](Questions.md) item 23 for the
full design writeup, including how a custom pattern is structurally prevented from
ever reaching special-category-health status.

### Compliance export (v0.1)

A `compliance_admin` can generate a one-click, date-ranged export from the
**Compliance Export** dashboard view (`GET /api/compliance-export`,
`backend/src/routes/compliance_export.rs` + `backend/src/reports.rs`), covering
the audit log, fairness metrics, and drift history for their own organisation
together in one file:

- **CSV** — the complete, unabridged dataset for the selected range, correctly
  quoted/escaped (the `csv` crate, not hand-rolled string concatenation).
- **PDF** — a readable, printable summary of the same three sections, built
  with `printpdf` using a built-in font (no font file to ship), capped at the
  500 most recent audit log rows in range (the CSV has no such cap — see
  [Questions.md](Questions.md) item 24 for why).

### Active learning loop (v0.1)

When a `compliance_admin` or `department_reviewer` confirms or overturns a flagged
low-confidence audit_log row (`blocked_low_confidence` or `redacted_low_confidence_
review` — the two tiers where the detection engine itself was uncertain) from the
Audit Log view's row-expand, that human judgment is recorded as a labelled example
in a new `review_decisions` table (`POST /api/audit-log/:id/review-decision`,
migration `0014`) — not just a status change on the audit_log row. Each row is a
self-contained snapshot (the original detection detail, the human decision, and any
reasoning given), exportable as CSV or JSONL (`GET /api/labelled-dataset`, from the
Compliance Export view) for future rule-tuning. **This only captures the signal —
nothing in this codebase retrains or fine-tunes anything automatically from it**,
by explicit task scope; that's future work. See [Questions.md](Questions.md) item
25 for the eligibility/scoping judgment calls.

### Response scanning (v0.1)

**The second half of the pipeline** — closing out a known limitation that has been
documented since early in this project. The browser extension now scans the AI's
reply, not just the user's outgoing prompt, on chatgpt.com, claude.ai, and
gemini.google.com: once a reply finishes streaming in (detected by debouncing DOM
mutations — there's no single "response complete" event, since replies arrive
incrementally, not as one block — see `content/response-scanner.js`), its text is
sent to `POST /api/scan/response` (`backend/src/detection/scan.rs`'s `scan_response`,
reusing the exact same detector pipeline the prompt side uses) and checked for leaked
secrets, sensitive entities that shouldn't appear in a reply, or anything else
flagged by the existing detection engine.

**A flagged response is never modified, hidden, or redacted — only flagged with a
warning banner, deliberately.** Silently rewriting or hiding content the user did
not write themselves, after they've already been shown it, is a materially
different and more concerning kind of intervention than redacting an outgoing
prompt before it's ever sent: redaction prevents a leak that hasn't happened yet,
while covertly altering a received response would mean the tool deciding, after the
fact, what a person is and isn't allowed to have read. See `scan_response`'s own doc
comment and [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the full reasoning.

**Honest confidence assessment, stated plainly per this task's own instruction**:
response scanning is a genuinely harder DOM problem than prompt interception (no
single "done" event, a debounce heuristic instead), and this was reflected in real
testing, not just claimed. gemini.google.com's response scanning was verified
end-to-end against a real, live, anonymous session — a real prompt was sent, a real
reply arrived, the extension correctly detected it stabilise, scanned it, and showed
the correct warning banner (or correctly stayed silent for a clean reply) — including
real, measured streaming-timing data (individual pauses up to ~2.9 seconds mid-stream
on one real response) that directly set the debounce window. chatgpt.com's and
claude.ai's response-side selectors remain unverified against a live session (both
sites are still unreachable from this project's environment) — see
[extension/README.md](extension/README.md) and [Questions.md](Questions.md) item 26
for the complete verification trail, including a methodology correction to an earlier
session's incorrect conclusion that extensions couldn't be loaded in this environment
at all.

### Native chat (v0.1)

A native in-app chat interface, built entirely on top of the existing pipeline — no
new detection logic, no new auth surface, no second organisation/role model. One app,
one backend, one login. See [Questions.md](Questions.md) items 46-49 for the full
design writeup and judgment calls behind everything below.

- **Reuses the detection pipeline exactly.** `POST /api/chat` runs the exact same
  `scan_prompt`/`scan_response` functions (`backend/src/detection/scan.rs`) the
  extension's `/api/scan` and `/api/scan/response` already use — zero duplicated
  detection logic. A blocked prompt (`blocked_low_confidence`) returns immediately;
  OpenAI is never called. A redacted-and-forwarded (either tier) or clean prompt is
  sent, redacted, to OpenAI, and the reply streams back to the browser as it arrives.
- **A real provider adapter, OpenAI only.** `backend/src/providers/` defines a
  `ChatProvider` trait general enough for a future second provider, but only
  `OpenAiProvider` is implemented and tested. This is the first and only place this
  backend makes an outbound HTTP call to a third-party AI API — `/api/scan` still
  never does, by design (see the AI Gateway note above).
- **Fails open on the response side, for the same reason the extension already
  does.** A response has already streamed to the user by the time it can be scanned —
  there's nothing left to block. A flagged response is never modified or hidden, only
  retroactively flagged (`chat_messages.response_flagged`,
  `audit_log.response_flagged`), surfaced in the chat UI with the same amber "may
  contain sensitive information" warning treatment used throughout this dashboard.
- **Chat history stores the redacted version of every message only, never the raw
  original** — the same zero-raw-prompt-storage principle already enforced in
  `audit_log`. A user's message is stored only as `scan_prompt`'s redacted output; a
  blocked prompt is never stored as a chat message at all (mirroring the extension's
  own "a block prevents sending" behavior). The one place this differs from the
  extension: an assistant reply IS stored verbatim, since (unlike the extension, which
  never needs to persist a reply already rendered on a third-party page) this chat
  surface is the only place responsible for redisplaying it later — `scan_response`
  never redacts a reply either way, only flags it.
- **One shared OpenAI key per organisation**, provisioned/rotated by a
  `compliance_admin` from the **Policy Builder** view's "Chat: OpenAI API Key"
  section, encrypted at rest (AES-256-GCM, `backend/src/crypto.rs`) and never
  displayed again after saving — only a masked `sk-…last4` confirmation. Basic usage
  visibility (a request count over 7/30/90 days) is pulled from this organisation's
  own `audit_log`, not OpenAI's billing API.
- **Role-gated landing**: a `staff` login lands directly on `/chat` (staff has no
  dashboard access in the existing role model); `compliance_admin`/
  `department_reviewer` land on the dashboard, which has a "Chat" sidebar link to
  reach `/chat` too. This required a real login page (`/login`) — see Questions.md
  item 49 for how that was reconciled with not adding a new auth surface (same
  `POST /api/auth/login` endpoint the existing demo account already uses).
- **The extension is not being replaced or deprecated.** Both stay in the product,
  for different situations: the web app's chat is the more complete, more robust path
  once an institution has actually rolled it out and provisioned an OpenAI key — every
  turn gets a durable, queryable audit trail entry the same way `/api/scan` already
  does. The extension remains the lower-friction path for any site the web app doesn't
  cover, or before an institution has rolled out `/chat` at all — an employee can start
  using it immediately with nothing to provision.
- **Verification, stated honestly**: no live OpenAI API key was available in this
  environment. Everything through the provider adapter is tested against a mocked
  OpenAI response — unit tests for the SSE-parsing logic, and full-pipeline
  integration tests (`backend/tests/chat.rs`) against a real local mock HTTP server
  (`wiremock`), never the real API. **One real network call did reach the real OpenAI
  API during manual live-testing**, using a deliberately fake key to verify
  error-handling — it received a real `401 Unauthorized` and was handled correctly,
  but this is not a verified successful completion. **The real, live OpenAI
  integration remains unverified against the actual API** until someone with a real
  key tests it. The frontend UI itself was verified live end-to-end (Playwright,
  against a real running backend + local mock provider) — see Questions.md items 47-49
  for the full accounting, including a real streaming race condition this caught and
  fixed.

### Real observability (v0.1)

Structured logging (`tracing`) now covers the significant application events, not
just errors — login success/failure, a prompt scan decision, a policy change, an
active-learning review decision, a compliance export — each with real structured
fields (organisation id, decision, etc.), not string-interpolated messages.
`LOG_FORMAT=json` switches the whole log stream to machine-parseable JSON for a
hosted deployment; the default stays human-readable for local `cargo run`.

**Error tracking**: a free-tier third-party service (e.g. Sentry) was seriously
researched, then deliberately not integrated — it needs an account/DSN only the
person operating this deployment can provision, and this pass could not confirm the
current `sentry`/`sentry-tracing` API with enough confidence to ship untestable
integration code (see [Questions.md](Questions.md) item 27). Built instead: an
internal `backend_errors` table, populated by a single middleware layer wrapping
every route (`backend/src/observability.rs`), and a "System Health" dashboard view
(`compliance_admin` only) showing the 100 most recent 5xx responses. Known,
stated limitation: not organisation-scoped in v1 (an error can happen before any
organisation is known) — see `routes/backend_errors.rs`'s own comment.

**Uptime check**: `.github/workflows/uptime-check.yml` pings the deployed backend's
`/health` endpoint every 30 minutes (with one retry, to avoid a false-positive alert
from Render's free-tier cold-start delay), using GitHub's own built-in behavior of
emailing repository watchers when a scheduled workflow run fails — no third-party
uptime service or extra webhook needed. Known limitation: GitHub disables scheduled
workflows after 60 days of repository inactivity.

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
  `DATABASE_URL`, `JWT_SIGNING_SECRET`, `PORT`, `CORS_ORIGIN`,
  `API_KEY_ENCRYPTION_KEY` (native chat feature — a 64-hex-character AES-256 key
  used to encrypt organisation OpenAI keys at rest; generate a real one with
  `openssl rand -hex 32`).
- [`.env.example`](.env.example) at the repo root is legacy/aspirational — it
  documents variables a *hosted production* deployment would additionally need (an AI
  provider API key, an alert webhook) that no code in this repo reads yet, since v0.1
  doesn't connect to a live AI provider or alerting service.

## Tests

**Backend unit tests** (`cd backend && cargo test --lib`, no database needed): 100
tests across `detection::scan`, `detection::rules`, `detection::health_rules`,
`detection::fallback`, `detection::tokenize`, and `detection::plain_language` —
regex rules, the Luhn check, the three-tier fail-closed confidence handling, the
special-category-health hard rule, the policy builder's `ScanConfig`/custom-pattern
path, and the PSI/KL-divergence math.

**Backend integration tests** (`cd backend && cargo test`, requires a real Postgres
reachable via `DATABASE_URL` — see [Setup](#full-stack-real-backend--real-data)):
`backend/tests/multi_tenant_isolation.rs`, `consent_flow.rs`,
`organisation_signup.rs`, `policy_builder.rs`, `compliance_export.rs`,
`review_decisions.rs`, `chat_multi_tenant_isolation.rs`, `chat.rs`, and
`organization_api_keys.rs`, each using `#[sqlx::test]` against a freshly-migrated
throwaway database and calling real route handlers directly (no HTTP server, no
mocks) — cross-tenant isolation, the consent gate, org signup, the policy
builder's safe-bounds enforcement (including a direct test that an out-of-range
threshold is rejected by the API itself, not just the UI), the compliance
export's date-range filtering/RBAC/isolation, the active learning loop's
review-eligibility rules, department scoping, one-decision-per-row enforcement,
and cross-tenant isolation, and — for native chat — the full scan/stream/
response-scan pipeline against a real local mock OpenAI server, organisation
API key provisioning/rotation/RBAC, and chat-specific cross-tenant isolation.

**Extension manifest test** (`npm run test:extension`, no dependencies, no
database): asserts the extension's content scripts never activate on this app's
own deployed domain (`extension/manifest.json`'s `exclude_matches`) — see
[HOW_TO_USE.md](HOW_TO_USE.md) for when to use the extension vs. the web app's
native chat.

No frontend automated test suite yet — an honest gap, not an oversight we're hiding.
See [Known Limitations](#known-limitations) and
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
  PostgreSQL, real auth, all five original dashboard views confirmed pulling live
  data — three more, added afterward, are covered by integration tests and Playwright
  but not yet re-verified against the live deployment) — this
  is no longer a frontend-only simulation. But nothing here has had a security/
  production hardening pass, and it hasn't been load-tested or exercised under real
  institutional traffic. See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the full
  v0.1-vs-target breakdown.
- **The backend's own "AI Gateway" stage is still a labeled no-op** (`ai_model_used`
  is always the literal string documenting that no provider is connected) — the Rust
  backend itself never calls an AI provider's API server-side. **What does now exist**:
  [`extension/`](extension/), a browser extension that intercepts prompts AND (as of
  the "response scanning + observability + hardening" task) responses client-side in
  front of AI providers' web UIs — prompts are redacted/blocked before they're sent,
  responses are flagged with a warning banner (never modified) after they render — a
  real, working, differently-architected answer to "does anything actually gate real
  AI traffic yet." **chatgpt.com's prompt-side interception has been verified**
  against a live session (earlier pass); **gemini.google.com is now ALSO verified,
  end-to-end, for BOTH prompt and response scanning** against a real, live, anonymous
  session (this task — see [Questions.md](Questions.md) item 26); **claude.ai,
  chat.deepseek.com, and copilot.microsoft.com remain implemented but not verified**
  against live pages, and chatgpt.com's own response-side selectors are likewise
  unverified (chatgpt.com itself is still unreachable for a full session) — this
  distinction matters and is not collapsed anywhere it's mentioned. See
  [extension/README.md](extension/README.md)'s Verification and Known fragility
  sections, and each `content/<site>-adapter.js` file's own header comment, before
  relying on any one of the unverified sites or code paths.
- **No live security-event detection.** Prompt-injection/rate-limit/DoS detection is
  not implemented; the Security Events table is seeded with illustrative example rows
  (see `backend/src/bin/seed.rs`), not produced by a live detector.
- **No dashboard login/signup screen — but real multi-user, multi-organisation
  accounts underneath.** The dashboard still authenticates as a single fixed seeded
  demo account (see [Environment Variables](#environment-variables)) rather than
  having its own auth UI, but that's now a frontend gap, not a backend one: the
  backend supports real per-organisation, per-role accounts and a self-service
  signup endpoint (`POST /api/organisations/signup`) — see
  [Multi-tenancy](#multi-tenancy-v01) above. A judge/reviewer using the deployed demo
  never needs to notice this gap; a real deployment would need the dashboard UI built
  to match what the API already does.
- **The deployed Vercel demo now calls the live Render backend by default**, not mock
  data — `NEXT_PUBLIC_USE_MOCK_DATA` is unset on the Vercel deployment. Mock data is
  only ever the automatic fallback if the backend is unreachable (e.g. mid cold-start
  on Render's free tier — see [Deployment](#deployment)).
- **No integration or end-to-end test suite.** Backend detection-engine logic has
  real unit tests (`cargo test`); everything above that (API integration, frontend)
  is verified by manual click-through, logged in
  [docs/TESTING_LOG.md](docs/TESTING_LOG.md).
- **Mobile responsiveness — fixed and re-tested, not just the original 375px
  finding.** The sidebar is now a slide-out drawer below the `md` (768px)
  breakpoint (hamburger toggle, backdrop, same nav), unchanged above it; the Audit
  Log renders as a stacked card list below `md` instead of a squeezed table; KPI and
  chart-comparison grids step down responsively. Verified at 375px, 414px, 768px,
  1024px, and 1280px — zero horizontal page overflow at any width, confirmed by
  screenshot, not just a scrollWidth check (see [docs/TESTING_LOG.md](docs/TESTING_LOG.md)
  and [Questions.md](Questions.md)). Not yet tested: a real physical mobile device or
  a screen reader/accessibility pass — both still open, see docs/TESTING_LOG.md's
  TODO.
- **No real user feedback yet.** No pilot users have interacted with this demo; see
  [docs/UX_DESIGN.md](docs/UX_DESIGN.md).
- **Native chat's live OpenAI integration is unverified against the real API.** No
  live OpenAI key was available while building it — tested against a mocked SSE
  response and a real local mock HTTP server standing in for OpenAI, never the real
  one, except for one incidental real network call (a deliberately fake key, to
  verify error handling — received a real `401`, handled correctly, not a verified
  successful completion). See [Native chat](#native-chat-v01) above and
  [Questions.md](Questions.md) items 47-49.

## Team

- **Phakamile Mlala** — Team Leader. Electronic Engineering student, National
  University of Science and Technology (NUST), Bulawayo.
- **Vanessa Moyo** — Team Member.

Claude and Claude Code (Anthropic) were used throughout this submission for planning,
content drafting, and implementation (including porting the dashboard to Next.js and
producing this documentation set). All generated output was reviewed and is understood
by the team before submission.
