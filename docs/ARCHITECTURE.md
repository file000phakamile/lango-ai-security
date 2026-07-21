# Architecture — Lango / AI Data Guard

This document is split into **deployed v0.1 (this repo, live today)** and **target
production** columns for every row. The v0.1 system is real and deployed: a Rust +
Axum backend on Render, a real PostgreSQL schema, a real (if intentionally simplified)
detection engine, and a Vercel-hosted Next.js frontend calling it directly — confirmed
end-to-end (real login, all five original dashboard views pulling live data, verified
via the browser Network tab — three more views, Policy Builder/Compliance Export/
Active Learning, were added afterward and are exercised by real integration tests and
Playwright, but not yet re-checked against the live Vercel deployment specifically,
since nothing has been redeployed since), not a simulation and not merely "runs on a
laptop." It is still
**not production-hardened**: no live AI provider connection, no live security-event
detection, no scheduled jobs, no dashboard login screen, no load testing. Real
multi-tenant isolation, a real role model, a data-use consent gate, and self-service
organisation signup are now genuinely built and tested (see the Users, Database, and
Security rows below, and [SECURITY_PRIVACY.md](SECURITY_PRIVACY.md)) — this used to be
listed here as future work; it isn't anymore. The columns below are not the same
system — conflating "deployed and working" with "ready for a pilot institution" would
misrepresent what actually exists; the *target production* column describes what's
still genuinely future work (a real dashboard login/signup screen, a live AI provider
connection, rate limiting — see docs/DEPLOYMENT_PLAN.md's roadmap), not something
already built.

Live URLs: frontend at `lango-app-dusky.vercel.app`, backend at
`lango-backend-qwkx.onrender.com` (Render Blueprint, `render.yaml` at the repo root —
see docs/DEPLOYMENT_PLAN.md). The frontend still has an automatic fallback to the
client-side mock generator (`NEXT_PUBLIC_USE_MOCK_DATA`) if the backend is ever
unreachable, but that is not its default deployed behaviour. **Free-tier honesty
note**: Render's free web-service tier spins down after 15 minutes of inactivity, so
the first request after an idle period takes roughly 30-60 seconds while it wakes back
up — expected platform behaviour, not a bug; the frontend's mock-data fallback means a
judge hitting a cold backend still sees the dashboard immediately rather than a blank
screen. See [Questions.md](../Questions.md) for the deployment history, including a
real incident (production login 401s caused by the seed step never having run against
Render) that's since been fixed. See also
[architecture-diagram.svg](architecture-diagram.svg) for the target request pipeline.

## Backend Architecture

| Layer | Deployed v0.1 (this repo, live today) | Target production system |
|---|---|---|
| **Users** | **Real, multi-tenant.** Every user belongs to exactly one `organisations` row and one of three roles — `staff` (scan only, no dashboard), `department_reviewer` (dashboard scoped to their own department), `compliance_admin` (dashboard scoped to their own organisation) — enforced by real, tested query-level filtering on every endpoint (see [SECURITY_PRIVACY.md](SECURITY_PRIVACY.md) and `backend/tests/multi_tenant_isolation.rs`). A new institution can self-register via `POST /api/organisations/signup` (backend-only in v0.1 — no dashboard signup page yet, see [Questions.md](../Questions.md)). **A real login page now exists** (`/login`, native chat feature — no new auth surface, calls the same `POST /api/auth/login` endpoint): a `staff` login lands on `/chat` directly (no dashboard access, per the role model above), `compliance_admin`/`department_reviewer` land on the dashboard. The dashboard's OTHER views are unaffected and still fall back to authenticating transparently as the fixed seeded account, `compliance@lango.demo`, when no real login has happened — preserved deliberately, unchanged, for continuity with the already-submitted AI4I materials, which reference it directly. | A self-service signup UI to match the backend endpoint that already exists, and extending the new real login beyond `/chat` to gate the rest of the dashboard too (today it's opt-in — visiting `/` directly still uses the demo account unless `/login` was used first). |
| **Access channel** | Deployed and live: Vercel frontend calling the Render-hosted backend over HTTPS (subject to the free-tier spin-down note above). `npm run dev` (frontend) + `cargo run` (backend) on localhost remain available for local development against the same codebase. | Same web access channel, but behind institutional authentication; potentially embedded/proxied so staff use it transparently alongside their existing AI tool. |
| **Frontend** | Next.js 16 (App Router) + React 19 + TypeScript, Tailwind CSS v4, shadcn-based UI primitives, Recharts for charts. Single client component (`LangoDashboard`) with six sub-views switched by local state — no routing between views (the sixth, Health Data Guard, was added by the health module — see [HEALTH_MODULE.md](HEALTH_MODULE.md) — without changing the original five). `lib/lango/api-client.ts` fetches real data from the backend and falls back to the old mock generator if it's unreachable. | Same frontend stack, plus a real login screen and a signup page (replacing the fixed demo-account shortcut and the backend-only signup endpoint). |
| **Backend** | **Real.** Rust + Axum HTTP API (`backend/`) implementing `/api/auth/login`, `/api/organisations/signup`, `/api/consent/accept`, `/api/scan`, `/api/audit-log`, `/api/fairness`, `/api/drift`, `/api/security-events`, `/api/command-center/summary`, `/api/health-data-guard/summary` — JWT-authenticated, role-gated, real error handling with a consistent JSON error shape. The AI Gateway pipeline stage is present as a labeled no-op (see AI layer row below), not a live call. | Same API surface, hardened: rate limiting, structured audit logging of admin actions, the AI Gateway stage actually forwarding to a live provider. |
| **Database** | **Real, multi-tenant.** PostgreSQL via `sqlx`, migrations in `backend/migrations/`: `organisations`, `users`, `sessions`, `audit_log`, `detection_rules`, `security_events`, `drift_snapshots` — every tenant-scoped table carries an `organisation_id` foreign key, enforced by a real query-level filter on every endpoint, not just a schema column (see [SECURITY_PRIVACY.md](SECURITY_PRIVACY.md)). Plus migration `0008` (health module) adding `audit_log.sensitivity_class` and `audit_log.facility_type` — see [HEALTH_MODULE.md](HEALTH_MODULE.md). Plus migration `0017` (native chat feature): `organization_api_keys` (one encrypted OpenAI key per organisation), `chat_conversations`, `chat_messages` (redacted content only — see the Native chat section below). `docker-compose.yml` spins up Postgres locally; `backend/src/bin/seed.rs` populates realistic sample data (all under one fixed demo organisation) by running synthetic prompts through the real detection engine. Raw prompt text is never stored — only a SHA-256 hash (`original_prompt_hash`) plus the redacted version; chat messages follow the identical principle. | Same schema, plus row-level security as defense-in-depth on top of the existing query-level filtering, retention policy enforcement, and backup/DR. |
| **AI layer** | Rule-based pattern matching (real regexes, Luhn-checked credit cards, plus health-specific dictionary/context detectors — see [HEALTH_MODULE.md](HEALTH_MODULE.md)) + a **capitalized-word-sequence heuristic standing in for NER** (`backend/src/detection/name_heuristic.rs` — explicitly documented in its own doc comment as not real NER; a full transformer-based NER crate needs a native libtorch/onnxruntime dependency, too heavy for this v0.1 — see [Questions.md](../Questions.md)). `ai_model_used` on an `/api/scan`-originated row (the extension's path) is still a literal "not connected" string — that path never calls a provider, by design. **This is no longer true for every row, though**: the native chat feature's `POST /api/chat` (see the dedicated section below) genuinely calls OpenAI server-side and records the real model used — **this is what finally makes the AI Gateway pipeline stage real, for chat's own request path, rather than the documented no-op it was for years.** | Rule-based pattern matching + a real NER model (once a workable lightweight option exists, or the heavier dependency is judged worth it) for sensitive-entity detection — deliberately not a generative model itself, for explainability (see [DATA_AI_USAGE.md](DATA_AI_USAGE.md)). The sanitised prompt is then forwarded to whichever external generative AI provider the institution already uses; Lango does not replace that provider, it gates access to it. A second provider beyond OpenAI, and model choice within the Policy Builder, remain future work — the `ChatProvider` trait (`backend/src/providers/`) was built general enough for this, but only `OpenAiProvider` is implemented. |
| **Integrations** | **Two, now**: [`extension/`](../extension/), a Manifest V3 browser extension that integrates with five AI chat sites' web UIs client-side (intercepting the composer's submit action AND, as of the "response scanning + observability + hardening" task, the AI's reply once it renders — a client-side interception, not a server-side connector), and — as of the native chat feature — a real **server-side connector to OpenAI** (`backend/src/providers/openai.rs`), used by `POST /api/chat`. The extension covers chatgpt.com, claude.ai, gemini.google.com, chat.deepseek.com, copilot.microsoft.com (Microsoft's consumer web chat, not GitHub Copilot); response scanning specifically covers chatgpt.com, claude.ai, and gemini.google.com. Of the five, **chatgpt.com's prompt-side interception is verified against a live session** (earlier pass), and **gemini.google.com is now verified end-to-end for both prompt AND response scanning** against a real, live, anonymous session — see [Questions.md](../Questions.md) item 26, including a methodology correction to an earlier session's incorrect conclusion that this environment couldn't load browser extensions at all. claude.ai, chat.deepseek.com, and copilot.microsoft.com remain implemented but not verified against live pages; chatgpt.com's own response-side selectors are likewise unverified (chatgpt.com itself remains unreachable for a full session — see `extension/README.md` and `extension/USER_GUIDE.md` for exactly what was and wasn't tested, per site and per direction). The extension's content scripts explicitly exclude this app's own domain (`manifest.json`'s `exclude_matches`, tested — see [Questions.md](../Questions.md) item 50) — the two integrations serve different sites, deliberately, never the same page. The OpenAI connector itself remains unverified against the real live API — see the Native chat section below. | A second AI provider beyond OpenAI (the `ChatProvider` trait already supports this structurally); an alert/notification channel for drift and fairness alerts (see `.env.example`, `ALERT_WEBHOOK_URL`); potentially SSO/identity-provider integration for institutional login. |
| **Security** | JWT session tokens (real, `jsonwebtoken` crate) + Argon2 password hashing (real, `argon2` crate) + role-gated, organisation-scoped endpoints (`staff` / `department_reviewer` / `compliance_admin` — see [SECURITY_PRIVACY.md](SECURITY_PRIVACY.md)). Real tenant isolation: every query filters by the caller's `organisation_id`, verified by dedicated cross-tenant isolation tests (`backend/tests/multi_tenant_isolation.rs`), not just a schema column that happens to exist. A data-use consent gate blocks `/api/scan` for any user who hasn't accepted their organisation's current consent policy (`backend/src/routes/consent.rs`). **A global, per-IP rate limit now exists** ("response scanning + observability + hardening" task, Part 3): `tower_governor` (`backend/src/rate_limit.rs`), 10 requests/second sustained with a burst of 30, applied as the single outermost layer on the whole router — so it covers every route, including every one added by earlier tasks, by construction rather than per-route opt-in. Uses `SmartIpKeyExtractor` (reads `X-Forwarded-For`/`X-Real-IP`/`Forwarded`, correct only because this deployment sits behind Render's trusted reverse proxy — see [SECURITY_PRIVACY.md](SECURITY_PRIVACY.md)). Real tests fire actual HTTP requests through a real `GovernorLayer`-wrapped router and assert both that ordinary traffic passes and that a burst trips a 429 (`cargo test --lib rate_limit`). A `cargo audit`/`npm audit` dependency pass was also run in Part 3: all 22 npm `high`-severity findings were fixed (all traced to the `vercel` CLI devDependency's own sub-dependencies, resolved via a version bump plus `package.json` `overrides`, verified with a real production build); of six `cargo audit` findings, one (`lopdf`, HIGH, pulled in transitively by `printpdf`) could not be safely fixed in this pass and is deferred with reasoning in [Questions.md](../Questions.md); the rest were confirmed either unreachable in the compiled binary or below the high/critical bar. No dedicated prompt-injection detector exists yet — the Drift & Security view's Security Events are seeded illustrative rows (`backend/src/bin/seed.rs`), not output from a live detector. | Same JWT/Argon2/tenant-isolation/rate-limit foundation, plus real prompt-injection detection and finer-grained (per-route, per-organisation) rate limiting and DoS mitigation at the gateway. |
| **Monitoring** | Real PSI / KL-divergence math (`backend/src/detection/drift.rs`, with unit tests) computed once at seed time over synthetic weekly entity-count distributions — no scheduled batch job exists yet, so this doesn't run continuously against live traffic. Real Disparate Impact Ratio / Statistical Parity Difference (`backend/src/routes/fairness.rs`) computed live, on every request, from actual `audit_log` rows grouped by department and language. **Real observability now exists** ("response scanning + observability + hardening" task, Part 2): structured `tracing` logging with real fields across every significant application event (not just errors), `LOG_FORMAT=json` for machine-parseable output, an internal `backend_errors` table + "System Health" dashboard view populated by a single middleware layer (the documented fallback for a free-tier error-tracking service — see [Questions.md](../Questions.md) item 27 for why one wasn't wired in directly), and a GitHub Actions scheduled workflow (`.github/workflows/uptime-check.yml`) pinging `/health` every 30 minutes with GitHub's built-in failure-email notification. | Same PSI/KL and DIR/SPD math, run by a real scheduled job against live traffic instead of seed-time synthetic data; security event logging from a live detector instead of seeded examples; a dedicated uptime-monitoring service independent of GitHub repository activity; a real third-party error-tracking integration once an account/DSN can be provisioned. |
| **Outputs** | A real, queryable `audit_log` table — one row per `/api/scan` call, with entities detected, risk score, decision, reason string, model used, response-scan result, and a hash (never raw text) of the original prompt. `GET /api/audit-log` serves it paginated and filterable to the dashboard. **A structured, date-ranged compliance export now exists**: `GET /api/compliance-export` (`compliance_admin` only) produces a CSV (complete dataset) or PDF (readable summary, capped at 500 most recent audit rows) covering the audit log, fairness metrics, and drift history together for a selected date range — see the Policy Builder / Compliance Export dashboard view and [Questions.md](../Questions.md) item 24. | Same audit log and export mechanism, at production scale/retention. |

## Response scanning (v0.1)

**This closes out a known limitation documented since early in this project**: the
pipeline only ever scanned what a user *sent*, never what the AI sent back. As of
the "response scanning + observability + hardening" task, that's the second half of
the pipeline, and it's real — not a fabricated demo stage.

**Architecture, consistent with the rest of this project's honesty about what "the
backend" does and doesn't do**: the Rust backend still never talks to an AI
provider server-side (the AI Gateway pipeline stage remains a labeled no-op, as
stated everywhere else in this document). Response scanning happens the same way
prompt scanning does — client-side, in the browser extension — because the
extension is the only part of this system that ever actually sees an AI provider's
real, rendered reply. `content/response-scanner.js` watches the page for the
response to finish rendering and submits its text to `POST /api/scan/response`
(`backend/src/detection/scan.rs`'s `scan_response`), which reuses the exact same
detector pipeline (`detect_all`) the prompt side uses — a leaked national ID or API
key is exactly as real a finding in a reply as in a prompt, and a separate,
parallel detector list would risk drifting from the real one over time. Covers
chatgpt.com, claude.ai, and gemini.google.com; chat.deepseek.com and
copilot.microsoft.com remain prompt-scanning only.

**Why streaming responses are handled with a debounce, not a single event**: a
prompt submission is one well-defined user action (Enter/Send) on text that's
already final the instant it happens. A response has no equivalent moment — it
arrives incrementally over several seconds, not as one block. `response-scanner.js`
approximates "the response is done" by watching for a pause in DOM mutations long
enough to be confident streaming has actually stopped, not merely paused mid-token.
This is a heuristic, not a guarantee, and the specific debounce value used
(`DEBOUNCE_MS = 4000`) is based on real, measured data — a live test against
gemini.google.com recorded actual streaming mutation gaps up to ~2.9 seconds on one
real response — not a guess; see [Questions.md](../Questions.md) item 26 for the
full measurement and the honest caveat that this hasn't been separately measured
for chatgpt.com or claude.ai.

**Why a flagged response is never modified, redacted, or hidden — the central
design decision of this feature, stated here explicitly as required.** There is no
"redacted_response" concept anywhere in this codebase, and there will not be one
without a deliberate, separately-justified decision to add it. When a response is
flagged, the user sees a plain-language warning banner (the same honesty/no-jargon
style already used on the prompt side — see `plain_language.rs`); the AI's actual
reply, exactly as it rendered, is left completely alone.

The reasoning: redacting an outgoing prompt prevents a leak that hasn't happened
yet — the sensitive content never leaves the user's browser. A response is
different in kind, not just in timing: by the time any scan could possibly run,
the user has *already read* the real content. Covertly rewriting or hiding it at
that point would mean the tool silently deciding, after the fact and without
asking, what a person is and isn't allowed to have seen — a materially more
concerning intervention than declining to send something before it's ever sent.
This project's fail-closed principle throughout is about preventing sensitive data
from leaving the organisation's control; it has never been about gatekeeping what
an employee is permitted to read, and quietly repurposing the same mechanism for
that different problem would be exactly the kind of unstated scope creep this
project's documentation has consistently tried to avoid. A warning banner respects
that the user already has the information and preserves their own judgment about
what to do with it, while still surfacing the compliance-relevant signal (this
reply may contain something sensitive) for them to act on and for the audit trail
to record.

**Data handled the same way as the prompt side**: the response's raw text is never
stored — only a SHA-256 hash (`audit_log.response_text_hash`, migration `0015`),
mirroring `original_prompt_hash`. `response_entities_detected`, `response_risk_
score`, `response_flagged`, and `response_scanned_at` are all `NULL` until a
response scan is actually recorded for that row — never a fabricated "clean"
default.

**Honest confidence assessment**: gemini.google.com's response scanning has been
verified end-to-end against a real, live, anonymous production session — the
composer and response-turn selectors, the correlation between a prompt scan and
its response, and the full round trip through a mocked backend (no live Postgres
was reachable in this sandbox to run the real one — see Questions.md) were all
directly observed working, including the correct silent behavior for a clean
response and the correct warning banner for a flagged one. **Independently
re-verified in a later session** (docs-accuracy pass) using the same method on a
fresh browser profile — reproduced cleanly, including the flagged-banner case
specifically, plus one new honest finding: the full round trip (real reply
latency + debounce + fetch + render) took closer to 13-15 seconds end to end in
practice, not just the 4000ms debounce constant alone — see `extension/README.md`'s
gemini.google.com section and Questions.md for the actual observed output. chatgpt.com's and
claude.ai's response-turn selectors remain unverified best-effort guesses (both
sites are unreachable from this project's development environment for a full,
authenticated session) — see `content/chatgpt-adapter.js` and
`content/claude-adapter.js`'s own header comments, and
[extension/README.md](../extension/README.md), for exactly what that does and
doesn't mean.

## Native chat (v0.1)

**This is what makes the AI Gateway pipeline stage real, stated plainly, for the
first time since it was described in this document.** Every other row above still
describes `ai_model_used` as a literal "not connected" string for the extension's
`/api/scan` path — that remains true and deliberate for that path. `POST /api/chat`
is different: it is a real backend-to-OpenAI connection, not a documented no-op.

**Architecture — reuses the existing pipeline, does not reimplement it.** `POST
/api/chat` runs `scan_prompt_with_config` (the exact function `routes::scan` already
uses) on the incoming message first. A `blocked_low_confidence` decision returns
immediately, same `reason_string`/`user_message` shape as `/api/scan`, and OpenAI is
never called — no exception. Any decision that forwards (`cleared_no_entities`,
`redacted_and_forwarded`, or `redacted_low_confidence_review`) sends the *redacted*
content to `OpenAiProvider` (`backend/src/providers/openai.rs`), which streams the
reply back to the browser as it arrives (Server-Sent-Events parsing over a real
`reqwest` HTTP call — the first outbound third-party HTTP client this backend has ever
had). Once the stream ends, `scan_response` (the same function `routes::response_scan`
uses for the extension) runs on the full assembled reply in a background task, and
`audit_log`/`chat_messages` are updated with the result — deliberately AFTER the
client's own HTTP stream has already closed, so response-scan latency never delays
what the user perceives as "the reply finished."

**Fails open on the response side, for the identical reason the extension's response
scanner already does**: a response has already streamed to the browser by the time
any scan could run — there is nothing left to block, and (per this project's
standing design decision — see the Response scanning section above) a flagged
response is never modified or hidden, only flagged. `chat_messages.response_flagged`
and `audit_log.response_flagged` start `NULL` and are filled in once the background
scan completes; the frontend picks this up by polling (this backend has no WebSocket
infrastructure — the existing Command Center/Audit Log polling loop is the only
precedent, and this follows it).

**Chat history stores the redacted version of every message only, never the raw
original** — the same zero-raw-prompt-storage principle `audit_log` already enforces.
`chat_messages.redacted_content` for a `role='user'` row is `scan_prompt`'s
`redacted_prompt`; a blocked prompt creates no `chat_messages` row at all (mirroring
the extension's own "a block prevents sending, nothing else happens" behavior — no
half-sent turn is ever recorded). The one place this differs from the extension: a
`role='assistant'` row stores the reply verbatim. This is not an inconsistency — the
extension never needs to persist a reply at all, since it's already rendered on the
third-party site's own page; this native chat surface is the only place responsible
for redisplaying that reply later, and `scan_response` was never designed to redact a
response in the first place (see the Response scanning section's own reasoning on
why silently altering already-shown content is a materially different, more
concerning intervention than redacting an outgoing prompt before it sends).

**One shared OpenAI key per organisation**, provisioned/rotated by a
`compliance_admin` (`organization_api_keys`, encrypted at rest with AES-256-GCM —
`backend/src/crypto.rs`, the first reversible-encryption pattern this codebase has
needed; every prior secret-like value, e.g. passwords, uses one-way hashing instead).
Never returned once saved. Basic usage visibility (a request count over a selectable
window) is computed from this organisation's own `audit_log`, not OpenAI's billing
API — that integration was judged not trivial and the simpler internal count was
shipped instead, exactly as this feature's own task scope anticipated as the fallback.

**Provider adapter, general but only one implementation**: `ChatProvider`
(`backend/src/providers/mod.rs`) is a trait — a decrypted API key and redacted
conversation history in, a stream of text deltas out — deliberately general enough
for a second provider later, but only `OpenAiProvider` exists, and only it is tested.

**Extension exclusion**: the browser extension's content scripts explicitly exclude
this app's own deployed domain (`extension/manifest.json`'s `exclude_matches`,
verified by a real automated test — see [Questions.md](../Questions.md) item 50) —
the extension is for other AI chat sites; this app's own `/chat` page is this
product's own surface, not something for the extension to also intercept.

**Extension vs. web chat — both stay in the product, this is not a replacement.** The
web app's `/chat` is the more complete, more robust path once an institution has
actually rolled it out and provisioned an OpenAI key: every turn gets a durable audit
trail entry the same way `/api/scan` already does, with no browser extension install
required. The extension remains the right tool for any site the web app doesn't cover
(chatgpt.com, claude.ai, etc. — the whole point of a client-side interceptor), or for
any institution that hasn't rolled out `/chat` yet. Neither is described as
deprecated or superseded anywhere in this document, deliberately.

**Honest verification status**: no live OpenAI API key was available while building
this. The provider adapter is tested against a mocked SSE response (unit tests,
`providers/openai.rs`) and a real local mock HTTP server standing in for OpenAI
(`backend/tests/chat.rs`, five full-pipeline integration tests using `wiremock`) —
never the real API in the automated test suite. **One real network call did reach
the real OpenAI API** during manual live-verification, using a deliberately fake key
specifically to check error-handling on an auth failure — it received a genuine `401
Unauthorized` and was handled correctly (a clean generic error to the client, the
real reason logged server-side, the key never leaked in full), but this is explicitly
not a verified successful completion. **The live OpenAI integration remains
unverified against the real API.** The frontend (streaming render, role-gated
landing, the retroactive flagged-response warning) was verified live end-to-end with
a real Playwright browser session against a real running backend instance and the
local mock provider — see [Questions.md](../Questions.md) items 47-49 for the full
account, including a real race condition this caught and fixed (the client's
post-stream refresh could beat the backend's own background insert of the assistant
message row).

## API and Integration Checklist

Answered honestly against the actual state of this repo. Where v0.1 falls short of
the target, that gap is stated directly rather than glossed over.

1. **API endpoints documented** — `backend/src/main.rs` wires up the full route
   table, including (native chat feature) `POST /api/chat`, `GET
   /api/chat/conversations`, `GET /api/chat/conversations/:id/messages`, `GET`/`PUT
   /api/policy/openai-key`, and `GET /api/policy/openai-key/usage` alongside the
   original `POST /api/auth/login`, `POST /api/scan`, `GET /api/audit-log`, `GET
   /api/fairness`, `GET /api/drift`, `GET /api/security-events`, `GET
   /api/command-center/summary`, and `GET /health`. No OpenAPI/Swagger spec
   exists yet — route signatures and response shapes are documented in
   `backend/src/models.rs` and this file's README setup section (with a working
   `curl` example) instead.
2. **Sample request/response provided** — Yes, a real one: see the `curl` example in
   [README.md's Setup section](../README.md#setup), which logs in and calls
   `/api/scan` against the live local backend.
3. **Auth / API key approach explained** — Real: JWT tokens (`jsonwebtoken` crate,
   carrying `organisation_id`, department, and role) issued after Argon2-verified
   login (`argon2` crate); see `backend/.env.example` for the signing-secret
   variable. Real multi-user, multi-organisation accounts and self-service signup
   (`POST /api/organisations/signup`) already exist server-side — see
   [Multi-tenancy](../README.md#multi-tenancy-v01). The dashboard *itself* has no
   login/signup UI yet, though — it authenticates as one fixed seeded demo account
   (see README's Environment Variables section), a frontend gap, not a backend one.
4. **Input validation explained** — Real: `/api/scan` rejects empty prompts and
   prompts over 20,000 characters with `400 BAD_REQUEST` before touching the
   detection engine. The Prompt Scanner stage *is* the input-validation layer for
   sensitive content — every prompt is scanned for sensitive entities and assigned a
   risk score. Near-zero-confidence detections and any low-confidence match on a
   structured entity type still fail closed (`blocked_low_confidence`) rather than
   silently passing through; a low-but-real-confidence `full_name` match instead
   redacts and forwards automatically, flagged as `redacted_low_confidence_review` for
   async compliance review (see docs/SECURITY_PRIVACY.md's Human oversight row) — real
   three-tier logic in `detection::scan`, with unit tests.
5. **Database schema provided** — Real: six PostgreSQL migrations in
   `backend/migrations/` (`users`, `sessions`, `audit_log`, `detection_rules`,
   `security_events`, `drift_snapshots`), field names matched deliberately to
   `lib/lango/types.ts`'s `AuditLogEntry` etc. so the API contract lines up cleanly
   with what the frontend already expects.
6. **Data import/export formats explained** — Real: `GET /api/compliance-export`
   (`?start=YYYY-MM-DD&end=YYYY-MM-DD&format=csv|pdf`, `compliance_admin` only)
   exports the audit log, fairness metrics, and drift history together for a
   date range — CSV (complete dataset, correctly quoted/escaped via the `csv`
   crate) or PDF (readable summary, built with `printpdf`, no external font
   file needed). `GET /api/audit-log` (paginated JSON) remains the live
   dashboard's own read path and is unaffected by this addition. A second,
   separate export exists for the active learning loop (product-depth task,
   Part 3): `GET /api/labelled-dataset` (CSV or JSONL, `compliance_admin`
   only) exports every recorded human confirm/overturn judgment — see item
   below and [Questions.md](../Questions.md) item 25.
7. **External services / costs / dependencies listed** — Deployed v0.1: Vercel
   (frontend hosting) and Render (backend web service + managed Postgres), both on
   free tiers — no cost today, but see docs/DEPLOYMENT_PLAN.md's free-tier honesty
   notes (spin-down, no automated backups). Local dev uses `docker-compose.yml`
   instead of Render's managed Postgres. No paid external services; no live AI
   provider is called. Target: whichever AI provider(s) the institution uses (cost
   driver, see [BUSINESS_MODEL.md](BUSINESS_MODEL.md)), plus paid-tier PostgreSQL
   hosting and any alerting/notification service.
8. **Notification integrations described** — Not implemented. Target: drift and
   fairness alerts, plus security events, would push to an operational channel via
   `ALERT_WEBHOOK_URL` (see root `.env.example`) — no code sends anything today.
9. **No credentials exposed in repo** — Confirmed. `backend/.env.example` and
   `.env.local.example` contain placeholder/dev-only values (the seeded demo
   password is intentionally documented in the open, since it only ever protects
   synthetic local data) — real `.env`/`.env.local` files are git-ignored (see
   `.gitignore` and `backend/.gitignore`).
10. **Rate limits / retry logic considered** — Not implemented. No request is
    rate-limited at the API layer today. The Drift & Security view shows illustrative
    seeded example events (e.g. "Per-user token quota exceeded, request queued for
    next window") — not output from a live rate limiter.
11. **Admin / user roles described** — Real, enforced server-side, and organisation-
    scoped: `users.role` is `staff` (can call `/api/scan` only, no dashboard access),
    `department_reviewer` (dashboard access scoped to their own department), or
    `compliance_admin` (dashboard access across their own organisation, never
    another organisation's) — see `backend/migrations/0011_update_user_roles.sql`
    and `auth::require_role`.
12. **Audit trail described** — This is the product's core function, and it's real:
    every `/api/scan` call writes one `audit_log` row — user, timestamp, department,
    entities detected, risk score, decision (`cleared_no_entities` /
    `redacted_and_forwarded` / `redacted_low_confidence_review` /
    `blocked_low_confidence`), a human-readable reason string, the AI model used (a
    literal "not connected" string in v0.1), and the response-scan result. Raw prompt
    text is never stored, only a SHA-256 hash. **The response-scan result is now real,
    not a placeholder, on rows whose prompt was actually sent**: `POST /api/scan/
    response` (see the dedicated Response scanning section below) updates the same
    row with the AI reply's scan outcome once the browser extension has captured and
    scanned it — a hash of the response text (same "never store raw content"
    principle, applied symmetrically), whether anything was flagged, and which entity
    types. A row whose prompt was blocked pre-gateway, or whose response was never
    submitted for scanning (e.g. an older row from before this feature existed, or a
    site response scanning doesn't cover), simply has these columns `NULL` — not a
    fabricated "clean" result.
