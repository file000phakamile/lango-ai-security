-- Response scanning ("response scanning + observability + hardening" task,
-- Part 1) — the second half of the pipeline. `response_scan_result` has
-- existed since migration 0004 as a fixed, honest placeholder string ("not
-- applicable - no live AI provider connected") because nothing ever
-- populated it with a real scan result. The browser extension now closes
-- this loop client-side (same architecture as prompt scanning — the Rust
-- backend still never calls an AI provider server-side): it captures the
-- AI's rendered reply after it stabilises and submits it to
-- POST /api/scan/response, which populates the columns below and rewrites
-- `response_scan_result` with a real descriptive result for that row.
--
-- All nullable: NULL means "no response scan has been recorded for this
-- row yet" (true for every row from before this feature existed, and for
-- any row whose prompt was blocked pre-gateway, since nothing was ever sent
-- for the AI to reply to).
ALTER TABLE audit_log ADD COLUMN response_entities_detected JSONB;
ALTER TABLE audit_log ADD COLUMN response_risk_score REAL
    CHECK (response_risk_score IS NULL OR (response_risk_score >= 0 AND response_risk_score <= 1));
ALTER TABLE audit_log ADD COLUMN response_flagged BOOLEAN;
-- Same "never store raw text, only a hash" principle as
-- original_prompt_hash (migration 0004) — applied symmetrically to the
-- response side.
ALTER TABLE audit_log ADD COLUMN response_text_hash TEXT;
ALTER TABLE audit_log ADD COLUMN response_scanned_at TIMESTAMPTZ;

-- Partial index: only rows a compliance reviewer would actually want to
-- filter for ("show me flagged responses") are indexed, not the far more
-- common NULL/false rows.
CREATE INDEX idx_audit_log_response_flagged ON audit_log(response_flagged) WHERE response_flagged = true;
