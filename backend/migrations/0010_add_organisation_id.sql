-- Multi-tenancy, part 1 (schema), continued: adds organisation_id to every
-- table that should be tenant-scoped (per the task's explicit list: users,
-- audit_log, detection_rules, security_events, drift_snapshots — sessions
-- is deliberately NOT included, see Questions.md: a session's tenant is
-- always derivable via sessions.user_id -> users.organisation_id, and no
-- query filters sessions directly).
--
-- Each column is added nullable, backfilled to the fixed demo organisation
-- from migration 0009, then set NOT NULL — never added NOT NULL directly.
-- This migration cannot assume a fresh database: sqlx::migrate! runs
-- automatically on every boot (see src/main.rs), including against an
-- already-seeded Render production database, so a bare `ADD COLUMN ...
-- NOT NULL` with no default would fail outright against any existing row.
-- Backfilling into the demo org is also exactly what the CRITICAL
-- CONSTRAINT in Questions.md requires: every row that existed before this
-- change keeps working, now simply attributed to one specific organisation
-- instead of implicitly "the only organisation."

ALTER TABLE users ADD COLUMN organisation_id UUID REFERENCES organisations(id);
UPDATE users SET organisation_id = 'a0000000-0000-0000-0000-000000000001' WHERE organisation_id IS NULL;
ALTER TABLE users ALTER COLUMN organisation_id SET NOT NULL;
CREATE INDEX idx_users_organisation_id ON users(organisation_id);

ALTER TABLE audit_log ADD COLUMN organisation_id UUID REFERENCES organisations(id);
UPDATE audit_log SET organisation_id = 'a0000000-0000-0000-0000-000000000001' WHERE organisation_id IS NULL;
ALTER TABLE audit_log ALTER COLUMN organisation_id SET NOT NULL;
CREATE INDEX idx_audit_log_organisation_id ON audit_log(organisation_id);

ALTER TABLE detection_rules ADD COLUMN organisation_id UUID REFERENCES organisations(id);
UPDATE detection_rules SET organisation_id = 'a0000000-0000-0000-0000-000000000001' WHERE organisation_id IS NULL;
ALTER TABLE detection_rules ALTER COLUMN organisation_id SET NOT NULL;
CREATE INDEX idx_detection_rules_organisation_id ON detection_rules(organisation_id);

ALTER TABLE security_events ADD COLUMN organisation_id UUID REFERENCES organisations(id);
UPDATE security_events SET organisation_id = 'a0000000-0000-0000-0000-000000000001' WHERE organisation_id IS NULL;
ALTER TABLE security_events ALTER COLUMN organisation_id SET NOT NULL;
CREATE INDEX idx_security_events_organisation_id ON security_events(organisation_id);

ALTER TABLE drift_snapshots ADD COLUMN organisation_id UUID REFERENCES organisations(id);
UPDATE drift_snapshots SET organisation_id = 'a0000000-0000-0000-0000-000000000001' WHERE organisation_id IS NULL;
ALTER TABLE drift_snapshots ALTER COLUMN organisation_id SET NOT NULL;
CREATE INDEX idx_drift_snapshots_organisation_id ON drift_snapshots(organisation_id);

-- drift_snapshots.week_start was globally UNIQUE (one row per calendar week
-- across the whole system) — now that multiple organisations each track
-- their own weekly drift, uniqueness needs to be per-organisation instead,
-- or a second organisation's seed data would fail to insert entirely.
ALTER TABLE drift_snapshots DROP CONSTRAINT drift_snapshots_week_start_key;
ALTER TABLE drift_snapshots ADD CONSTRAINT drift_snapshots_org_week_start_key UNIQUE (organisation_id, week_start);
