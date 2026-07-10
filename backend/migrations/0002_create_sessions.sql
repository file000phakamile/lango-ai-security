-- A "session" here is a login session (one row per successful /api/auth/login),
-- not an HTTP session cookie — auth itself is stateless JWT. This table exists so
-- audit_log rows and security_events can reference which login session a scan
-- happened under, and so sessions can be listed/revoked later if needed.
CREATE TABLE sessions (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_sessions_user_id ON sessions(user_id);
