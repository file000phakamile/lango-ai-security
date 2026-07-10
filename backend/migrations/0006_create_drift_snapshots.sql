-- One row per week. Populated by a real PSI/KL-divergence calculation (see
-- src/detection/drift.rs) run over audit_log's entity-type distribution for
-- that week against a baseline week — computed by the seed script in v0.1,
-- since there's no scheduled job runner yet. `alert` is not stored: it's
-- derived at read time from psi_score >= 0.20 (see routes/drift.rs) so the
-- threshold lives in one place instead of two.
CREATE TABLE drift_snapshots (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    week_start          DATE NOT NULL UNIQUE,
    psi_score           REAL NOT NULL,
    kl_divergence_score REAL NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_drift_snapshots_week_start ON drift_snapshots(week_start);
