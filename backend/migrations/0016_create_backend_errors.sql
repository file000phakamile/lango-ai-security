-- Real observability ("response scanning + observability + hardening" task,
-- Part 2) — an internal error log, built as the documented fallback for a
-- free-tier error tracking service: actually provisioning one (e.g. Sentry)
-- requires an account and a DSN only the person running this deployment
-- can create, which isn't something this pass could do — see Questions.md
-- for the full reasoning. This table needs no external account, works the
-- moment the backend is deployed, and gives a `compliance_admin` something
-- real to look at today.
--
-- Populated by a single tower/axum middleware layer wrapping every route
-- (see src/observability.rs), not scattered across individual handlers —
-- one choke point that can't be forgotten when a new endpoint is added,
-- the same reasoning `error.rs`'s existing `tracing::error!` call already
-- follows for structured log output.
--
-- Deliberately NOT organisation-scoped: an error can happen before an
-- organisation is even known (a malformed login request, an auth failure),
-- and this table's actual audience is whoever operates this deployment, not
-- any one tenant's compliance team. See routes/backend_errors.rs's own
-- comment for the real, stated limitation this creates today (any
-- compliance_admin, in any organisation, can currently see every
-- organisation's backend errors) and what a real multi-tenant production
-- deployment would need instead (genuine operator-only access, distinct
-- from any tenant's own admin role).
CREATE TABLE backend_errors (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    method        TEXT NOT NULL,
    path          TEXT NOT NULL,
    status_code   SMALLINT NOT NULL,
    -- The already-sanitized, user-facing error message (see error.rs —
    -- AppError::into_response never lets a 5xx leak raw internals into this
    -- message; it's always the same "An internal error occurred." string
    -- for a database/hash/internal error, or the specific message for a
    -- deliberate 5xx this codebase doesn't currently have any of). Never
    -- the raw underlying Rust error text, which could contain connection
    -- strings, file paths, or other detail not meant to leave the process.
    message       TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_backend_errors_created_at ON backend_errors(created_at DESC);
