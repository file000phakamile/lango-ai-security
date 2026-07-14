-- Active learning loop (product-depth task, Part 3): when a compliance_admin
-- or department_reviewer confirms or overturns a flagged low-confidence
-- audit_log row, that human judgment is recorded here as a labelled
-- example — NOT just a status change on the audit_log row itself. This
-- table is intentionally a self-contained snapshot (original detection
-- detail copied in at decision time, not just a foreign key) so a labelled
-- example remains genuinely useful as future training/rule-tuning data
-- even if audit_log's own retention policy later purges the source row.
--
-- This table only ever captures signal. Nothing in this codebase reads from
-- it to retrain or fine-tune anything automatically — see
-- backend/src/routes/labelled_dataset.rs and Questions.md for why that's
-- explicitly out of scope for this task.
CREATE TABLE review_decisions (
    id                          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- UNIQUE, not just indexed: one human decision per audit_log row. A
    -- second attempt to record a decision on the same row is rejected
    -- (see routes/review_decisions.rs) rather than silently overwriting an
    -- earlier reviewer's judgment, which would corrupt the labelled
    -- dataset's provenance.
    audit_log_id                UUID NOT NULL UNIQUE REFERENCES audit_log(id) ON DELETE CASCADE,
    organisation_id             UUID NOT NULL REFERENCES organisations(id) ON DELETE CASCADE,
    reviewer_user_id            UUID NOT NULL REFERENCES users(id),
    reviewer_role                TEXT NOT NULL,
    decision                    TEXT NOT NULL CHECK (decision IN ('confirmed', 'overturned')),
    reasoning                    TEXT,
    -- Snapshot of the original detection, copied from the audit_log row at
    -- the moment of review.
    original_decision            TEXT NOT NULL,
    original_entities_detected  JSONB NOT NULL,
    original_risk_score          REAL NOT NULL,
    original_reason_string       TEXT NOT NULL,
    original_sensitivity_class  TEXT NOT NULL,
    original_department          TEXT NOT NULL,
    created_at                   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_review_decisions_org_id ON review_decisions(organisation_id);
