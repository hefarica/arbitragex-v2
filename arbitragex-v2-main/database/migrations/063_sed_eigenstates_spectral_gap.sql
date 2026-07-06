-- ============================================================
-- Migration 063: sed_eigenstates — add spectral_gap column
-- Sprint: SED Bridge Integration
-- Description: Adds spectral_gap column consumed by /api/v1/sed/status
-- ============================================================

ALTER TABLE sed_eigenstates
    ADD COLUMN IF NOT EXISTS spectral_gap FLOAT8;

COMMENT ON COLUMN sed_eigenstates.spectral_gap
    IS 'Spectral gap between ground state and first excited state energy. Null = not computed (R8).';

-- Index for dashboard queries filtering by spectral gap
CREATE INDEX IF NOT EXISTS idx_sed_eigenstates_spectral_gap
    ON sed_eigenstates(spectral_gap DESC)
    WHERE spectral_gap IS NOT NULL;
