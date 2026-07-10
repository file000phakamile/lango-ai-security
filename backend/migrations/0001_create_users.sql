-- Enables gen_random_uuid(); harmless no-op if the server already ships it in core.
CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE users (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email         TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    department    TEXT NOT NULL,
    -- 'staff' can run /api/scan; 'compliance'/'admin' can read the dashboard endpoints.
    -- See docs/ARCHITECTURE.md for the full role-to-endpoint mapping.
    role          TEXT NOT NULL DEFAULT 'staff'
                  CHECK (role IN ('staff', 'compliance', 'admin')),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
