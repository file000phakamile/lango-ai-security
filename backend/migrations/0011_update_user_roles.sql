-- Multi-tenancy, part 2 (roles). Replaces the old two-tier role model
-- ('staff' vs. 'compliance'/'admin', both of which got identical dashboard
-- access — see every route handler's old `require_role(&["compliance",
-- "admin"])` call) with the three-tier model the task specifies:
--   - 'staff'               - can only call /api/scan, no dashboard access at all.
--   - 'department_reviewer' - dashboard access scoped to their own department.
--   - 'compliance_admin'    - dashboard access across their whole organisation.
--
-- 'compliance' and 'admin' both had EQUIVALENT full dashboard access before
-- this change (every read endpoint's require_role call treated them
-- identically), so both map to 'compliance_admin' — the new role with
-- equivalent (now org-scoped) access — not split apart or demoted. This is
-- exactly why the AI4I-submission demo account
-- (compliance@lango.demo, previously role='compliance') keeps its current
-- full dashboard access unchanged: it becomes 'compliance_admin' here, the
-- same access tier it already had. 'staff' is unchanged, already matches
-- the new enum.
--
-- Can't edit migration 0001's CHECK constraint in place (already applied
-- against existing databases) — same reason migration 0007 replaced
-- audit_log's decision CHECK with a new migration instead of editing
-- 0004's.
ALTER TABLE users DROP CONSTRAINT users_role_check;

UPDATE users SET role = 'compliance_admin' WHERE role IN ('compliance', 'admin');

ALTER TABLE users ADD CONSTRAINT users_role_check
    CHECK (role IN ('staff', 'department_reviewer', 'compliance_admin'));
