-- Multi-tenancy, part 4 (consent). Builds the data-use consent step
-- described conceptually (but never built) in docs/SECURITY_PRIVACY.md and
-- docs/DEPLOYMENT_PLAN.md — see routes/consent.rs for the endpoint this
-- schema supports.
--
-- Tracks consent directly on `users` (one row per user's current consent
-- state) rather than a separate consent-history table — v0.1 only needs
-- "has this user ever accepted the version currently in force," not a full
-- audit trail of every prior acceptance. If organisations need to bump
-- `consent_policy_version` and require re-consent from users who accepted
-- an older version, a real re-consent history table would be the natural
-- next step — logged as a deliberate v1 scope call in Questions.md, not an
-- oversight.
--
-- Backfilled to `now()` / the current org's `consent_policy_version` for
-- every EXISTING user, not left null — this is exactly what the CRITICAL
-- CONSTRAINT in Questions.md requires: the AI4I-submission demo account,
-- and every other already-seeded account, must not suddenly be presented
-- with a brand-new consent screen that didn't exist in the judged
-- experience. Only users created AFTER this migration (via the new
-- self-service signup, or any future real per-employee onboarding) start
-- with `consent_accepted_at IS NULL` and go through the real consent gate.
ALTER TABLE users ADD COLUMN consent_accepted_at TIMESTAMPTZ;
ALTER TABLE users ADD COLUMN consent_policy_version TEXT;

UPDATE users u
SET consent_accepted_at = now(),
    consent_policy_version = o.consent_policy_version
FROM organisations o
WHERE u.organisation_id = o.id
  AND u.consent_accepted_at IS NULL;
