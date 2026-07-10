-- Adds the third confidence tier's decision value. See
-- backend/src/detection/scan.rs's NAME_LOW_CONFIDENCE_FLOOR doc comment for
-- the full reasoning: a low-but-real-confidence full_name match (0.30-0.60)
-- is now redacted and forwarded automatically rather than blocked, tagged
-- distinctly so it's queryable/auditable separately from an ordinary
-- redacted_and_forwarded row or a blocked_low_confidence one.
--
-- 0004_create_audit_log.sql's original migration can't be edited in place —
-- it's already applied against existing databases — so this is a new
-- migration that replaces the CHECK constraint with the four-value version.
ALTER TABLE audit_log DROP CONSTRAINT audit_log_decision_check;

ALTER TABLE audit_log ADD CONSTRAINT audit_log_decision_check
    CHECK (decision IN (
        'cleared_no_entities',
        'blocked_low_confidence',
        'redacted_and_forwarded',
        'redacted_low_confidence_review'
    ));
