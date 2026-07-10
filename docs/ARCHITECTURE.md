# Architecture — Lango / AI Data Guard

This document is split into **local-build (v0.1, this repo)** and **target production**
columns for every row. As of this pass, the local build has a real Rust + Axum backend,
a real PostgreSQL schema, and a real (if intentionally simplified) detection engine —
it is genuinely functioning code, not a simulation. It is still **not
production-hardened**: no live AI provider connection, no live security-event
detection, no scheduled jobs, no login UI, no multi-tenant isolation. The columns below
are not the same system — conflating "runs locally" with "ready for a pilot
institution" would misrepresent what actually exists. The **deployed Vercel demo**
(no backend hosted alongside it) still runs on the client-side mock generator via
`NEXT_PUBLIC_USE_MOCK_DATA=true` — see [Questions.md](../Questions.md). See also
[architecture-diagram.svg](architecture-diagram.svg) for the target request pipeline.

## Backend Architecture

| Layer | Local build (v0.1, this repo) | Target production system |
|---|---|---|
| **Users** | Whoever runs the stack locally — the dashboard authenticates transparently as one fixed seeded demo account (no login UI yet). | Authenticated staff at a pilot institution, scoped by department/role. |
| **Access channel** | `npm run dev` (frontend) + `cargo run` (backend) on localhost; the deployed Vercel demo link still serves mock data only, since no backend is hosted publicly. | Same web access channel, but behind institutional authentication; potentially embedded/proxied so staff use it transparently alongside their existing AI tool. |
| **Frontend** | Next.js 16 (App Router) + React 19 + TypeScript, Tailwind CSS v4, shadcn-based UI primitives, Recharts for charts. Single client component (`LangoDashboard`) with five sub-views switched by local state — no routing between views. `lib/lango/api-client.ts` fetches real data from the backend and falls back to the old mock generator if it's unreachable. | Same frontend stack, plus a real login flow (replacing the fixed demo-account shortcut) and multi-tenant awareness. |
| **Backend** | **Real.** Rust + Axum HTTP API (`backend/`) implementing `/api/auth/login`, `/api/scan`, `/api/audit-log`, `/api/fairness`, `/api/drift`, `/api/security-events`, `/api/command-center/summary` — JWT-authenticated, role-gated, real error handling with a consistent JSON error shape. The AI Gateway pipeline stage is present as a labeled no-op (see AI layer row below), not a live call. | Same API surface, hardened: rate limiting, structured audit logging of admin actions, multi-tenant isolation, the AI Gateway stage actually forwarding to a live provider. |
| **Database** | **Real.** PostgreSQL via `sqlx`, migrations in `backend/migrations/`: `users`, `sessions`, `audit_log`, `detection_rules`, `security_events`, `drift_snapshots`. `docker-compose.yml` spins up Postgres locally; `backend/src/bin/seed.rs` populates realistic sample data by running synthetic prompts through the real detection engine. Raw prompt text is never stored — only a SHA-256 hash (`original_prompt_hash`) plus the redacted version. | Same schema, plus tenant-scoping columns/row-level security, retention policy enforcement, and backup/DR. |
| **AI layer** | Rule-based pattern matching (real regexes, Luhn-checked credit cards) + a **capitalized-word-sequence heuristic standing in for NER** (`backend/src/detection/name_heuristic.rs` — explicitly documented in its own doc comment as not real NER; a full transformer-based NER crate needs a native libtorch/onnxruntime dependency, too heavy for this v0.1 — see [Questions.md](../Questions.md)). No live generative-AI provider is connected — `ai_model_used` on every audit-log row is a literal string stating that plainly, not a fabricated model name. | Rule-based pattern matching + a real NER model (once a workable lightweight option exists, or the heavier dependency is judged worth it) for sensitive-entity detection — deliberately not a generative model itself, for explainability (see [DATA_AI_USAGE.md](DATA_AI_USAGE.md)). The sanitised prompt is then forwarded to whichever external generative AI provider the institution already uses; Lango does not replace that provider, it gates access to it. |
| **Integrations** | None. | Connector(s) to the institution's chosen AI provider(s); an alert/notification channel for drift and fairness alerts (see `.env.example`, `ALERT_WEBHOOK_URL`); potentially SSO/identity-provider integration for institutional login. |
| **Security** | JWT session tokens (real, `jsonwebtoken` crate) + Argon2 password hashing (real, `argon2` crate) + role-gated endpoints (`staff` vs `compliance`/`admin`). No prompt-injection detection, rate limiting, or DoS mitigation is implemented — the Drift & Security view's Security Events are seeded illustrative rows (`backend/src/bin/seed.rs`), not output from a live detector. No tenant isolation (single shared schema). | Same JWT/Argon2 foundation, plus tenant-isolated data per institution and real prompt-injection detection, rate limiting, and DoS mitigation at the gateway. |
| **Monitoring** | Real PSI / KL-divergence math (`backend/src/detection/drift.rs`, with unit tests) computed once at seed time over synthetic weekly entity-count distributions — no scheduled batch job exists yet, so this doesn't run continuously against live traffic. Real Disparate Impact Ratio / Statistical Parity Difference (`backend/src/routes/fairness.rs`) computed live, on every request, from actual `audit_log` rows grouped by department and language. | Same PSI/KL and DIR/SPD math, run by a real scheduled job against live traffic instead of seed-time synthetic data; security event logging from a live detector instead of seeded examples. |
| **Outputs** | A real, queryable `audit_log` table — one row per `/api/scan` call, with entities detected, risk score, decision, reason string, model used, response-scan result, and a hash (never raw text) of the original prompt. `GET /api/audit-log` serves it paginated and filterable to the dashboard. | Same audit log, at production scale/retention, with structured export (CSV/JSON) for regulator or internal-audit review. |

## API and Integration Checklist

Answered honestly against the actual state of this repo. Where v0.1 falls short of
the target, that gap is stated directly rather than glossed over.

1. **API endpoints documented** — `backend/src/main.rs` wires up seven real,
   real routes: `POST /api/auth/login`, `POST /api/scan`, `GET /api/audit-log`,
   `GET /api/fairness`, `GET /api/drift`, `GET /api/security-events`,
   `GET /api/command-center/summary`, plus `GET /healthz`. No OpenAPI/Swagger spec
   exists yet — route signatures and response shapes are documented in
   `backend/src/models.rs` and this file's README setup section (with a working
   `curl` example) instead.
2. **Sample request/response provided** — Yes, a real one: see the `curl` example in
   [README.md's Setup section](../README.md#setup), which logs in and calls
   `/api/scan` against the live local backend.
3. **Auth / API key approach explained** — Real: JWT tokens (`jsonwebtoken` crate)
   issued after Argon2-verified login (`argon2` crate); see `backend/.env.example`
   for the signing-secret variable. The dashboard itself has no login UI yet — it
   authenticates as one fixed seeded demo account (see README's Environment
   Variables section) — a v0.1 shortcut, not the target multi-user auth flow.
4. **Input validation explained** — Real: `/api/scan` rejects empty prompts and
   prompts over 20,000 characters with `400 BAD_REQUEST` before touching the
   detection engine. The Prompt Scanner stage *is* the input-validation layer for
   sensitive content — every prompt is scanned for sensitive entities and assigned a
   risk score; low-confidence detections fail closed (`blocked_low_confidence`)
   rather than silently passing through — real logic in `detection::scan`, with unit
   tests.
5. **Database schema provided** — Real: six PostgreSQL migrations in
   `backend/migrations/` (`users`, `sessions`, `audit_log`, `detection_rules`,
   `security_events`, `drift_snapshots`), field names matched deliberately to
   `lib/lango/types.ts`'s `AuditLogEntry` etc. so the API contract lines up cleanly
   with what the frontend already expects.
6. **Data import/export formats explained** — Not implemented. The audit log would
   need a structured export (e.g. CSV/JSON) for regulator or internal-audit review,
   since that's the product's core compliance deliverable — not built in v0.1;
   `GET /api/audit-log` (paginated JSON) is the closest thing today.
7. **External services / costs / dependencies listed** — Local build: PostgreSQL
   (self-hosted via `docker-compose.yml`, no cost), no paid external services — no
   live AI provider is called. Target: whichever AI provider(s) the institution uses
   (cost driver, see [BUSINESS_MODEL.md](BUSINESS_MODEL.md)), plus PostgreSQL hosting
   and any alerting/notification service.
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
11. **Admin / user roles described** — Real, enforced server-side:
    `users.role` is `staff` (can call `/api/scan`) or `compliance`/`admin`
    (additionally gated to the five read-only dashboard endpoints via
    `auth::require_role`) — see `backend/migrations/0001_create_users.sql`.
12. **Audit trail described** — This is the product's core function, and it's real:
    every `/api/scan` call writes one `audit_log` row — user, timestamp, department,
    entities detected, risk score, decision (`cleared_no_entities` /
    `redacted_and_forwarded` / `blocked_low_confidence`), a human-readable reason
    string, the AI model used (a literal "not connected" string in v0.1), and the
    response-scan result. Raw prompt text is never stored, only a SHA-256 hash.
