# Deployment Plan — Lango / AI Data Guard

This plan covers two distinct things, kept clearly separate: (1) how the **judge-facing
demo in this repo** is deployed today, and (2) how a **real pilot deployment of the
target production system** would be run. Do not conflate the two — the demo backend
(when deployed) is a real, working v0.1, not the tenant-isolated, hardened system the
pilot plan below describes.

## Deployment environment

- **Demo frontend**: Vercel's standard Next.js hosting, serverless, no custom
  infrastructure. Single environment, no staging/production split.
- **Demo backend**: Render, via Blueprint (`render.yaml` at the repo root) — a single
  Docker-based web service plus a managed Postgres instance, both on the free tier.
  Single environment, no staging/production split, same as the frontend.
- **Target pilot**: a tenant-isolated environment (per docs/ARCHITECTURE.md) — the
  pilot institution's traffic and data isolated from any other tenant, likely a
  dedicated database and application instance per institution rather than shared
  multi-tenancy, given the sensitivity of the data involved.

## Hosting provider

- **Demo frontend**: Vercel (`lango-app-dusky.vercel.app`), deployed via the Vercel CLI
  (`vercel --prod`), not connected to GitHub for auto-deploy.
- **Demo backend**: Render — a web service (Rust/Axum, built from `backend/Dockerfile`)
  and a managed Postgres database, both free tier, deployed via Render's Blueprint
  (Infrastructure as Code) model from `render.yaml`. **Free-tier honesty note**: the
  web service spins down after 15 minutes of inactivity; the first request after an
  idle period takes roughly 30-60 seconds while it spins back up. This is expected
  Render free-tier behaviour, not a bug — see the same caveat in README.md and
  docs/ARCHITECTURE.md. The frontend's mock-data fallback (`NEXT_PUBLIC_USE_MOCK_DATA`,
  see README.md) means a judge hitting a cold backend still sees the dashboard
  immediately with mock data while the real backend wakes up, rather than a blank
  loading screen.
- **Target pilot**: not yet selected. Candidates would need to satisfy data-residency
  and compliance requirements of the pilot institution's sector (e.g. financial
  services hosting requirements for a bank pilot) — this decision is deferred until a
  specific pilot institution and jurisdiction are confirmed. Render's free tier is a
  demo/evaluation choice only — a real pilot would need a paid plan at minimum (no
  spin-down, real backups, higher resource limits) and likely a different provider
  entirely once data-residency requirements are known.

## Operator

Team Lango (Phakamile Mlala, Vanessa Moyo) operates the demo. For a real pilot, the
operating model would need to be agreed with the pilot institution: whether Lango's
team operates the backend on the institution's behalf, or the institution's own IT
operates it with Lango providing the software — this is an open commercial/operational
question, not yet decided.

## Pilot site

Candidate: a regional commercial bank, Credit Risk department (matches the scope
already reflected in the demo's Pilot & Sandbox view and Command Center header). No
institution has signed on yet — this is the target scope, not a confirmed partner.

## Users to onboard

22 / 30 target users, one department, per the demo's pilot checklist. Real onboarding
would require: account provisioning, JWT credential issuance, and a short induction on
what gets redacted and why (see Training/support below).

## Training / support

Not yet built. A real pilot would need: a short onboarding session for pilot users
explaining what triggers a block vs. a redaction, a point of contact for
false-positive reports (entities that should not have been redacted) and false
negatives (entities that should have been caught but weren't), and a documented escalation
path for the "review" state a fairness or drift alert triggers.

## Monitoring

- **Demo**: Render's built-in service health checks (`healthCheckPath: /health` in
  `render.yaml`) and dashboard logs/metrics — nothing beyond Render's own free-tier
  tooling; no external uptime or alerting service is wired up.
- **Target pilot**: the demo's Drift & Security view illustrates the intended shape —
  PSI / KL-divergence drift tracking with an alert threshold (0.20), and a security
  event feed (prompt injection attempts, rate limiting, DoS mitigation). In production
  these would need to page a real on-call channel (see `ALERT_WEBHOOK_URL` in
  `.env.example`), not just render in a dashboard.

## Backup / recovery

**Demo**: Render's free-tier Postgres has no automated backups and is deleted after 90
days of the database's creation (a Render free-tier limit, not a Lango decision) — the
demo database is reseeded from `backend/src/bin/seed.rs`, not backed up, since
everything in it is synthetic. For the target system: the audit log is the
system-of-record for compliance evidence, so it would need standard PostgreSQL backup
practice (point-in-time recovery, regular tested restores) — audit log loss would
itself be a compliance failure, not just an operational inconvenience.

## Connectivity plan

Target pilot institutions are assumed to have standard office internet connectivity
sufficient for a web-based gateway; no offline mode is planned, since the product's
core function (checking outbound prompts before they reach an AI provider) requires
connectivity by definition.

## Scale pathway

1. **Single department, single institution** (current pilot scope) — validate
   detection accuracy, fairness, and staff friction on real traffic.
2. **Additional departments, same institution** — the demo's department list (Credit
   Risk, Claims Processing, Patient Records, Bursar's Office, Legal Affairs) already
   anticipates this; expand once pilot metrics clear target.
3. **Additional institutions, same sector** — replicate the tenant-isolated deployment
   for a second bank or similar institution.
4. **Cross-sector expansion** — hospitals, ministries — each requiring sector-specific
   entity types and pattern rules (e.g. medical record formats differ from banking ID
   formats).

## Milestones at 30 / 60 / 90 days

- **30 days**: Pilot institution and department confirmed; consent and data-isolation
  agreements signed; environment provisioned; first pilot users onboarded.
- **60 days**: Midpoint review; redaction accuracy and false-positive rate measured
  against target; at least one fairness audit cycle run; monitoring/alerting wired to
  a real channel.
- **90 days**: Full pilot cohort onboarded; go/no-go decision on scale-out, backed by
  real pilot data rather than the synthetic figures shown in this demo.
