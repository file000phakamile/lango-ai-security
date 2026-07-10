-- v0.1 does not implement live prompt-injection detection, rate limiting, or
-- DoS mitigation (out of scope for this pass — see docs/ARCHITECTURE.md).
-- This table and its read endpoint are real; the seed script populates
-- illustrative example rows so the Drift & Security view has something to
-- show. Treat rows here as demonstrating the schema/UI, not live-detected
-- events, until that detection logic actually exists.
CREATE TABLE security_events (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_type TEXT NOT NULL
               CHECK (event_type IN ('prompt_injection_blocked', 'rate_limit_triggered', 'dos_mitigation_triggered')),
    detail     TEXT NOT NULL,
    session_id UUID REFERENCES sessions(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_security_events_created_at ON security_events(created_at DESC);
