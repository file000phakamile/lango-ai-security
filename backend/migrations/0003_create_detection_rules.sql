-- Source of truth for the regex side of the detection engine (see
-- src/detection/rules.rs). Rows here are seeded from the same patterns
-- compiled at startup; the table exists so rules are inspectable/auditable
-- and so a future admin UI could add/disable rules without a redeploy.
-- v0.1 does not yet load rules from this table at request time — the engine
-- uses compiled-in patterns for reliability; see docs/ARCHITECTURE.md.
CREATE TABLE detection_rules (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_type TEXT NOT NULL,
    pattern     TEXT NOT NULL,
    rule_type   TEXT NOT NULL CHECK (rule_type IN ('regex', 'ner')),
    active      BOOLEAN NOT NULL DEFAULT true,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
