-- Policy builder (product-depth task, Part 1): lets a compliance_admin adjust
-- their own organisation's confidence threshold within a hard-coded safe
-- range, and add organisation-specific structured-identifier patterns.
--
-- Bounds are enforced in TWO places, deliberately: the CHECK constraints
-- below (defense in depth, matches this codebase's existing pattern — see
-- audit_log's decision CHECK) AND the API handler in routes/policy.rs
-- (returns a clean 400 instead of a raw constraint-violation error). Neither
-- table nor any code path here can touch NAME_LOW_CONFIDENCE_FLOOR or the
-- special_category_health leniency hard rule in backend/src/detection/scan.rs
-- — those stay fully hard-coded, not read from either table.
--
-- One row per organisation, upserted on write — an organisation with no row
-- yet is using the system default (scan::CONFIDENCE_THRESHOLD, 0.60).
CREATE TABLE organisation_detection_settings (
    organisation_id      UUID PRIMARY KEY REFERENCES organisations(id) ON DELETE CASCADE,
    confidence_threshold REAL NOT NULL DEFAULT 0.60
        CHECK (confidence_threshold >= 0.50 AND confidence_threshold <= 0.95),
    updated_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_by             UUID REFERENCES users(id)
);

-- Organisation-specific structured-identifier patterns (e.g. a specific
-- bank's own account-number format) — matched alongside, never instead of,
-- the built-in detectors, and only against scans from the owning
-- organisation (see routes/scan.rs).
CREATE TABLE organisation_custom_patterns (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organisation_id  UUID NOT NULL REFERENCES organisations(id) ON DELETE CASCADE,
    entity_label      TEXT NOT NULL,
    pattern           TEXT NOT NULL,
    confidence        REAL NOT NULL DEFAULT 0.80
        CHECK (confidence >= 0.50 AND confidence <= 0.95),
    active            BOOLEAN NOT NULL DEFAULT true,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by         UUID REFERENCES users(id)
);

CREATE INDEX idx_organisation_custom_patterns_org_id
    ON organisation_custom_patterns(organisation_id);
