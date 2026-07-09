# Deployment Plan — Lango / AI Data Guard

This plan covers two distinct things, kept clearly separate: (1) how the **judge-facing
demo in this repo** is deployed today, and (2) how a **real pilot deployment of the
target production system** would be run. Do not conflate the two — the demo has no
backend; the pilot plan below describes what would need to be built first.

## Deployment environment

- **Demo (as-built)**: Vercel's standard Next.js hosting, serverless, no custom
  infrastructure. Single environment, no staging/production split.
- **Target pilot**: a tenant-isolated environment (per docs/ARCHITECTURE.md) — the
  pilot institution's traffic and data isolated from any other tenant, likely a
  dedicated database and application instance per institution rather than shared
  multi-tenancy, given the sensitivity of the data involved.

## Hosting provider

- **Demo**: Vercel (`lango-app-dusky.vercel.app`), deployed via the Vercel CLI
  (`vercel --prod`), not connected to GitHub for auto-deploy.
- **Target pilot**: not yet selected. Candidates would need to satisfy data-residency
  and compliance requirements of the pilot institution's sector (e.g. financial
  services hosting requirements for a bank pilot) — this decision is deferred until a
  specific pilot institution and jurisdiction are confirmed.

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

- **Demo**: none — it's a static frontend with no live system to monitor.
- **Target pilot**: the demo's Drift & Security view illustrates the intended shape —
  PSI / KL-divergence drift tracking with an alert threshold (0.20), and a security
  event feed (prompt injection attempts, rate limiting, DoS mitigation). In production
  these would need to page a real on-call channel (see `ALERT_WEBHOOK_URL` in
  `.env.example`), not just render in a dashboard.

## Backup / recovery

Not applicable to the demo (no data is persisted anywhere — it's regenerated from a
seeded PRNG on every page load). For the target system: the audit log is the
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
