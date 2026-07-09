# Architecture — Lango / AI Data Guard

This document is deliberately split into **demo-as-built** and **target production**
columns for every row. They are not the same system — conflating them would misrepresent
what a judge is actually looking at when they open the live demo link. See also
[architecture-diagram.svg](architecture-diagram.svg) for the target request pipeline.

## Backend Architecture

| Layer | Demo (as-built, this repo) | Target production system |
|---|---|---|
| **Users** | Anyone with the demo link — no login, single fixed view. | Authenticated staff at a pilot institution, scoped by department/role. |
| **Access channel** | Public web URL (Vercel), any modern browser. | Same web access channel, but behind institutional authentication; potentially embedded/proxied so staff use it transparently alongside their existing AI tool. |
| **Frontend** | Next.js 16 (App Router) + React 19 + TypeScript, Tailwind CSS v4, shadcn-based UI primitives, Recharts for charts. Single client component (`LangoDashboard`) with five sub-views switched by local state — no routing between views. | Same frontend stack, extended to call a real API instead of local mock data, plus a login flow. |
| **Backend** | **None.** No server-side application code, no API routes, no request handling of any kind — this is a static frontend. | Rust + Axum HTTP API implementing the six-stage pipeline: Authentication → Prompt Scanner → Redaction Engine → AI Gateway → Response Scanner → Audit Service. |
| **Database** | **None.** All data (`lib/lango/mock-data.ts`) is generated in-browser from a seeded PRNG (`mulberry32`, seed `2026`) on every page load; nothing is persisted. | PostgreSQL. Stores the audit log (system-of-record for compliance evidence), user/role data, pilot configuration, and fairness/drift metric history. |
| **AI layer** | **None** — no calls to any AI provider are made anywhere in this codebase; all "model" fields in the audit log (e.g. "Connector: Provider A, model v1") are synthetic placeholder strings. | Rule-based pattern matching + Named Entity Recognition (NER) for sensitive-entity detection (national ID, bank account, phone number, full name, medical record number, API key) — deliberately not a generative model, for explainability (see [DATA_AI_USAGE.md](DATA_AI_USAGE.md)). The sanitised prompt is then forwarded to whichever external generative AI provider the institution already uses; Lango does not replace that provider, it gates access to it. |
| **Integrations** | None. | Connector(s) to the institution's chosen AI provider(s); an alert/notification channel for drift and fairness alerts (see `.env.example`, `ALERT_WEBHOOK_URL`); potentially SSO/identity-provider integration for institutional login. |
| **Security** | Standard static-site delivery over HTTPS (Vercel default). No auth, because there is nothing sensitive to protect — all data on screen is synthetic. | JWT session tokens + Argon2 password hashing; tenant-isolated data per institution; prompt-injection detection, rate limiting, and DoS mitigation at the gateway (illustrated in the demo's Drift & Security view as example event types, not live protections). |
| **Monitoring** | None — nothing is running server-side to monitor. | Drift detection (PSI / KL-divergence, weekly, alert threshold 0.20) on entity-detection distributions; fairness monitoring (Disparate Impact Ratio / Statistical Parity Difference) by language and department; security event logging (injection attempts, rate limits, DoS mitigation). |
| **Outputs** | Read-only dashboard views for a judge/reviewer to inspect the concept. | A permanent, queryable audit log per request (user, timestamp, entities detected, risk score, decision, reason, model used, response-scan result) — the compliance evidence artefact the whole product exists to produce. |

## API and Integration Checklist

Answered honestly against the actual state of this repo. Where the demo has nothing,
the target-system answer explains what would exist instead of leaving it blank.

1. **API endpoints documented** — Not applicable in this demo; there is no API. Target:
   the six-stage pipeline (`/auth`, `/scan`, `/redact`, `/gateway`, `/response-scan`,
   `/audit`) would be documented with OpenAPI/Swagger once the Rust/Axum backend exists.
2. **Sample request/response provided** — Not applicable; no live endpoint exists to
   sample. The demo's Audit Log view shows the *shape* of a completed record (session
   id, user, department, timestamp, entities detected, risk score, decision, reason,
   model, response-scan result) which maps directly to the intended audit-log API
   response shape.
3. **Auth / API key approach explained** — Demo requires no auth (nothing sensitive is
   served). Target: JWT tokens issued after Argon2-verified login; see
   `.env.example` for the signing-secret and hashing-parameter placeholders.
4. **Input validation explained** — Not applicable in the demo (no user input is
   accepted or processed — it's a read-only view). Target: the Prompt Scanner stage
   *is* the input-validation layer — every prompt is scanned for sensitive entities and
   assigned a risk score before anything is forwarded onward; low-confidence detections
   fail closed (blocked) rather than silently passing through.
5. **Database schema provided** — Not applicable; no database exists in this repo.
   `lib/lango/types.ts` documents the intended record shapes (`AuditLogEntry`,
   `SecurityEvent`, `DriftWeek`, `ParityEntry`, etc.) which are the closest thing to a
   schema sketch available today and a reasonable starting point for the real
   PostgreSQL schema.
6. **Data import/export formats explained** — Not applicable in the demo (nothing is
   imported or exported). Target: the audit log would need a structured export
   (e.g. CSV/JSON) for regulator or internal-audit review, since that's the product's
   core compliance deliverable.
7. **External services / costs / dependencies listed** — Demo: none — Vercel hosting
   only, no paid external services. Target: whichever AI provider(s) the institution
   uses (cost driver, see [BUSINESS_MODEL.md](BUSINESS_MODEL.md)), plus PostgreSQL
   hosting and any alerting/notification service.
8. **Notification integrations described** — Not implemented in the demo (the Drift &
   Security view renders example events, it does not send anything). Target: drift and
   fairness alerts, plus security events, would push to an operational channel via
   `ALERT_WEBHOOK_URL` (see `.env.example`).
9. **No credentials exposed in repo** — Confirmed. This repo contains no real API
   keys, database URLs, or secrets — `.env.example` contains placeholder values only,
   and `.env*` is git-ignored (see `.gitignore`).
10. **Rate limits / retry logic considered** — Not implemented in the demo (there is no
    API to rate-limit). The Drift & Security view illustrates the intended behaviour as
    an example event ("Per-user token quota exceeded, request queued for next window")
    — target system would enforce this at the AI Gateway stage.
11. **Admin / user roles described** — Not implemented in the demo (single fixed view,
    no login). Target: at minimum a staff role (submits prompts) and a
    compliance/admin role (reviews audit log, fairness/drift alerts, manages pattern
    rules) — the dashboard views in this demo (Audit Log, Fairness Audit, Drift &
    Security, Pilot & Sandbox) are shaped for the admin/compliance role specifically.
12. **Audit trail described** — This is the product's core function. Every request
    would be permanently logged with user, timestamp, department, entities detected,
    risk score, decision (`cleared_no_entities` / `redacted_and_forwarded` /
    `blocked_low_confidence`), a human-readable reason string, the AI model used, and
    the response-scan result — exactly the record shape shown (with synthetic data) in
    this demo's Audit Log view.
