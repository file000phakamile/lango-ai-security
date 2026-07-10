-- The system-of-record: one row per /api/scan request. Never stores the raw
-- prompt — only a SHA-256 hash of it (original_prompt_hash) plus the redacted
-- version, consistent with the "no raw prompts stored" claim already in the UI.
--
-- `language` is not in the field list the task specified, but the existing
-- Fairness Audit view already ships a language-parity chart (English/Ndebele/
-- Shona) that needs *something* to group by. Added as a nullable column so
-- that view has real data instead of being quietly dropped. Logged as a
-- judgment call in Questions.md.
CREATE TABLE audit_log (
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id            UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    user_id               UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    department            TEXT NOT NULL,
    language              TEXT,
    "timestamp"           TIMESTAMPTZ NOT NULL,
    entities_detected     JSONB NOT NULL DEFAULT '[]'::jsonb,
    risk_score            REAL NOT NULL CHECK (risk_score >= 0 AND risk_score <= 1),
    decision              TEXT NOT NULL
                          CHECK (decision IN ('cleared_no_entities', 'blocked_low_confidence', 'redacted_and_forwarded')),
    reason_string         TEXT NOT NULL,
    ai_model_used         TEXT NOT NULL,
    response_scan_result  TEXT NOT NULL,
    original_prompt_hash  TEXT NOT NULL,
    redacted_prompt       TEXT,
    created_at            TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_audit_log_created_at ON audit_log(created_at DESC);
CREATE INDEX idx_audit_log_decision ON audit_log(decision);
CREATE INDEX idx_audit_log_department ON audit_log(department);
CREATE INDEX idx_audit_log_language ON audit_log(language);
