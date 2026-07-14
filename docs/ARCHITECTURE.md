# Architecture — Lango / AI Data Guard

This document is split into **deployed v0.1 (this repo, live today)** and **target
production** columns for every row. The v0.1 system is real and deployed: a Rust +
Axum backend on Render, a real PostgreSQL schema, a real (if intentionally simplified)
detection engine, and a Vercel-hosted Next.js frontend calling it directly — confirmed
end-to-end (real login, all five dashboard views pulling live data, verified via the
browser Network tab), not a simulation and not merely "runs on a laptop." It is still
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
| **Users** | **Real, multi-tenant.** Every user belongs to exactly one `organisations` row and one of three roles — `staff` (scan only, no dashboard), `department_reviewer` (dashboard scoped to their own department), `compliance_admin` (dashboard scoped to their own organisation) — enforced by real, tested query-level filtering on every endpoint (see [SECURITY_PRIVACY.md](SECURITY_PRIVACY.md) and `backend/tests/multi_tenant_isolation.rs`). A new institution can self-register via `POST /api/organisations/signup` (backend-only in v0.1 — no dashboard signup page yet, see [Questions.md](../Questions.md)). The dashboard *itself* still authenticates transparently as one fixed seeded account, `compliance@lango.demo`, rather than having its own login screen — preserved deliberately, unchanged, as the seeded account of one specific "Regional Commercial Bank Demo" organisation, for continuity with the already-submitted AI4I materials, which reference it directly. | A real dashboard login screen (replacing the fixed demo-account shortcut), and a self-service signup UI to match the backend endpoint that already exists. |
| **Access channel** | Deployed and live: Vercel frontend calling the Render-hosted backend over HTTPS (subject to the free-tier spin-down note above). `npm run dev` (frontend) + `cargo run` (backend) on localhost remain available for local development against the same codebase. | Same web access channel, but behind institutional authentication; potentially embedded/proxied so staff use it transparently alongside their existing AI tool. |
| **Frontend** | Next.js 16 (App Router) + React 19 + TypeScript, Tailwind CSS v4, shadcn-based UI primitives, Recharts for charts. Single client component (`LangoDashboard`) with six sub-views switched by local state — no routing between views (the sixth, Health Data Guard, was added by the health module — see [HEALTH_MODULE.md](HEALTH_MODULE.md) — without changing the original five). `lib/lango/api-client.ts` fetches real data from the backend and falls back to the old mock generator if it's unreachable. | Same frontend stack, plus a real login screen and a signup page (replacing the fixed demo-account shortcut and the backend-only signup endpoint). |
| **Backend** | **Real.** Rust + Axum HTTP API (`backend/`) implementing `/api/auth/login`, `/api/organisations/signup`, `/api/consent/accept`, `/api/scan`, `/api/audit-log`, `/api/fairness`, `/api/drift`, `/api/security-events`, `/api/command-center/summary`, `/api/health-data-guard/summary` — JWT-authenticated, role-gated, real error handling with a consistent JSON error shape. The AI Gateway pipeline stage is present as a labeled no-op (see AI layer row below), not a live call. | Same API surface, hardened: rate limiting, structured audit logging of admin actions, the AI Gateway stage actually forwarding to a live provider. |
| **Database** | **Real, multi-tenant.** PostgreSQL via `sqlx`, migrations in `backend/migrations/`: `organisations`, `users`, `sessions`, `audit_log`, `detection_rules`, `security_events`, `drift_snapshots` — every tenant-scoped table carries an `organisation_id` foreign key, enforced by a real query-level filter on every endpoint, not just a schema column (see [SECURITY_PRIVACY.md](SECURITY_PRIVACY.md)). Plus migration `0008` (health module) adding `audit_log.sensitivity_class` and `audit_log.facility_type` — see [HEALTH_MODULE.md](HEALTH_MODULE.md). `docker-compose.yml` spins up Postgres locally; `backend/src/bin/seed.rs` populates realistic sample data (all under one fixed demo organisation) by running synthetic prompts through the real detection engine. Raw prompt text is never stored — only a SHA-256 hash (`original_prompt_hash`) plus the redacted version. | Same schema, plus row-level security as defense-in-depth on top of the existing query-level filtering, retention policy enforcement, and backup/DR. |
| **AI layer** | Rule-based pattern matching (real regexes, Luhn-checked credit cards, plus health-specific dictionary/context detectors — see [HEALTH_MODULE.md](HEALTH_MODULE.md)) + a **capitalized-word-sequence heuristic standing in for NER** (`backend/src/detection/name_heuristic.rs` — explicitly documented in its own doc comment as not real NER; a full transformer-based NER crate needs a native libtorch/onnxruntime dependency, too heavy for this v0.1 — see [Questions.md](../Questions.md)). No live generative-AI provider is connected — `ai_model_used` on every audit-log row is a literal string stating that plainly, not a fabricated model name. | Rule-based pattern matching + a real NER model (once a workable lightweight option exists, or the heavier dependency is judged worth it) for sensitive-entity detection — deliberately not a generative model itself, for explainability (see [DATA_AI_USAGE.md](DATA_AI_USAGE.md)). The sanitised prompt is then forwarded to whichever external generative AI provider the institution already uses; Lango does not replace that provider, it gates access to it. |
| **Integrations** | One: [`extension/`](../extension/), a Manifest V3 browser extension that integrates with five AI chat sites' web UIs client-side (intercepting the composer's submit action, not a server-side connector) — chatgpt.com, claude.ai, gemini.google.com, chat.deepseek.com, copilot.microsoft.com (Microsoft's consumer web chat, not GitHub Copilot). Of these, **only chatgpt.com's DOM-dependent parts are verified against a live session**; the other four are implemented but not yet verified against live pages (see `extension/README.md` and `extension/USER_GUIDE.md`). No server-side connector to any AI provider's API exists; the backend's own AI Gateway stage remains a no-op. | Server-side connector(s) to the institution's chosen AI provider(s), in addition to (or instead of) the client-side extension approach; an alert/notification channel for drift and fairness alerts (see `.env.example`, `ALERT_WEBHOOK_URL`); potentially SSO/identity-provider integration for institutional login. |
| **Security** | JWT session tokens (real, `jsonwebtoken` crate) + Argon2 password hashing (real, `argon2` crate) + role-gated, organisation-scoped endpoints (`staff` / `department_reviewer` / `compliance_admin` — see [SECURITY_PRIVACY.md](SECURITY_PRIVACY.md)). Real tenant isolation: every query filters by the caller's `organisation_id`, verified by dedicated cross-tenant isolation tests (`backend/tests/multi_tenant_isolation.rs`), not just a schema column that happens to exist. A data-use consent gate blocks `/api/scan` for any user who hasn't accepted their organisation's current consent policy (`backend/src/routes/consent.rs`). No prompt-injection detection, rate limiting, or DoS mitigation is implemented — the Drift & Security view's Security Events are seeded illustrative rows (`backend/src/bin/seed.rs`), not output from a live detector. | Same JWT/Argon2/tenant-isolation foundation, plus real prompt-injection detection, rate limiting, and DoS mitigation at the gateway. |
| **Monitoring** | Real PSI / KL-divergence math (`backend/src/detection/drift.rs`, with unit tests) computed once at seed time over synthetic weekly entity-count distributions — no scheduled batch job exists yet, so this doesn't run continuously against live traffic. Real Disparate Impact Ratio / Statistical Parity Difference (`backend/src/routes/fairness.rs`) computed live, on every request, from actual `audit_log` rows grouped by department and language. | Same PSI/KL and DIR/SPD math, run by a real scheduled job against live traffic instead of seed-time synthetic data; security event logging from a live detector instead of seeded examples. |
| **Outputs** | A real, queryable `audit_log` table — one row per `/api/scan` call, with entities detected, risk score, decision, reason string, model used, response-scan result, and a hash (never raw text) of the original prompt. `GET /api/audit-log` serves it paginated and filterable to the dashboard. **A structured, date-ranged compliance export now exists**: `GET /api/compliance-export` (`compliance_admin` only) produces a CSV (complete dataset) or PDF (readable summary, capped at 500 most recent audit rows) covering the audit log, fairness metrics, and drift history together for a selected date range — see the Policy Builder / Compliance Export dashboard view and [Questions.md](../Questions.md) item 24. | Same audit log and export mechanism, at production scale/retention. |

## API and Integration Checklist

Answered honestly against the actual state of this repo. Where v0.1 falls short of
the target, that gap is stated directly rather than glossed over.

1. **API endpoints documented** — `backend/src/main.rs` wires up seven real,
   real routes: `POST /api/auth/login`, `POST /api/scan`, `GET /api/audit-log`,
   `GET /api/fairness`, `GET /api/drift`, `GET /api/security-events`,
   `GET /api/command-center/summary`, plus `GET /health`. No OpenAPI/Swagger spec
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
   dashboard's own read path and is unaffected by this addition.
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
    text is never stored, only a SHA-256 hash.
