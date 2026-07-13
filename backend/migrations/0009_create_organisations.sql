-- Multi-tenancy, part 1 (schema). See Questions.md for the full design
-- writeup this migration set implements.
--
-- One row per institution. `consent_policy_version` is the version string
-- shown to a user on the data-use consent screen (see migration 0011 and
-- routes/consent.rs) — bumping this forces every user in the organisation
-- to re-consent, since `users.consent_policy_version` (the version a given
-- user actually accepted) is compared against this column at accept time.
CREATE TABLE organisations (
    id                     UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name                   TEXT NOT NULL UNIQUE,
    created_at             TIMESTAMPTZ NOT NULL DEFAULT now(),
    consent_policy_version TEXT NOT NULL DEFAULT 'v1'
);

-- A FIXED, well-known id — not gen_random_uuid() — specifically so this one
-- row can be referenced identically from this migration, migration 0010's
-- backfill, backend/src/bin/seed.rs, and this codebase's own tests, without
-- any of them needing to look it up by name first. This is the demo
-- organisation the AI4I-submission demo account (compliance@lango.demo)
-- belongs to after this change — see the CRITICAL CONSTRAINT note in
-- Questions.md: that account's login, password, and existing audit history
-- must all keep working exactly as before, just now scoped to this one
-- organisation instead of being the only account in the system.
INSERT INTO organisations (id, name, consent_policy_version)
VALUES ('a0000000-0000-0000-0000-000000000001', 'Regional Commercial Bank Demo', 'v1');
