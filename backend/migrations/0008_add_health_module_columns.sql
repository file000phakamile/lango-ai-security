-- Health module (built for the Cimas Healthathon 3.0 submission — see
-- docs/HEALTH_MODULE.md). Additive: no existing column, constraint, or row
-- is changed.
--
-- `sensitivity_class` is the NEW, independent-from-confidence axis described
-- in docs/HEALTH_MODULE.md — "standard" for every existing entity type,
-- "special_category_health" for the five new health entity types (see
-- backend/src/detection/health_rules.rs's SensitivityClass doc comment).
-- NOT NULL with a default so every historical row (all "standard", since the
-- health module didn't exist when they were written) and every future row
-- both always have a value — no nullable tri-state to handle downstream.
ALTER TABLE audit_log
    ADD COLUMN sensitivity_class TEXT NOT NULL DEFAULT 'standard'
    CHECK (sensitivity_class IN ('standard', 'special_category_health'));

-- `facility_type` is nullable and OPTIONAL, mirroring how `language` already
-- works (see migration 0004's comment): a caller-declared tag ("Rural
-- Clinic" / "District Hospital" / "Urban Hospital" in this seed data), not
-- derived from the prompt itself. Only present on rows a caller chose to tag
-- — most callers (and the existing browser extension, unchanged by this
-- module) simply won't send it, and that's fine; it exists so the new
-- Health Data Guard view's facility-type fairness comparison
-- (routes/health.rs, adapting the existing DIR/SPD math from
-- routes/fairness.rs) has a real, live-computable grouping dimension to use.
ALTER TABLE audit_log ADD COLUMN facility_type TEXT;

CREATE INDEX idx_audit_log_sensitivity_class ON audit_log(sensitivity_class);
CREATE INDEX idx_audit_log_facility_type ON audit_log(facility_type);
