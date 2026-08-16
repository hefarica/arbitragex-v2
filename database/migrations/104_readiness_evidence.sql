-- ArbitrageX v2 — Migration 104: readiness_evidence registry (G-SIM-1 FASE 2)
--
-- Append-only evidence store for gate readiness checklists. Evidence producers
-- (CI jobs of G-SIM-1 FASE 4, operators, reviewers) record each checklist item
-- with provenance (evidence_ref URL + verified_by identity). The FASE 3
-- verifier reads this table directly via its PG pool; GET /admin/readiness-evidence
-- is a read-only convenience for operators.
--
-- Generalizes the scripts/run_a4_fork_validation.sh mechanism (INSERT into
-- gate_c_validation + marker file) with freshness (30-day is_fresh computed by
-- readers) + provenance (verified_by) + a history sister table so nothing is
-- ever deleted or silently overwritten:
--   - readiness_evidence            → latest row per (gate_id, item_key)
--   - readiness_evidence_history    → EVERY write ever made (append-only)
-- The write path (api-server POST /admin/readiness-evidence) inserts into
-- history BEFORE upserting the main row, inside ONE transaction.
--
-- Idempotent.

BEGIN;

CREATE TABLE IF NOT EXISTS readiness_evidence (
  gate_id      TEXT NOT NULL,          -- e.g. G-SIM-1
  item_key     TEXT NOT NULL,          -- unit_tests | modules_merged | fork_suite | variance_benchmark | dep_tree | eth_callbundle_staging | second_signoff
  status       TEXT NOT NULL CHECK (status IN ('evidenced','failed')),
  evidence_ref TEXT NOT NULL,          -- URL CI run / benchmark doc / PR review
  detail       JSONB,
  verified_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
  verified_by  TEXT NOT NULL,          -- ci:rust.yml | operator:<id> | reviewer:<id>
  PRIMARY KEY (gate_id, item_key)
);

-- Append-only history: same columns as readiness_evidence, keyed by
-- (gate_id, item_key, verified_at). No UPDATE/DELETE path exists anywhere in
-- the codebase; the only writer inserts.
CREATE TABLE IF NOT EXISTS readiness_evidence_history (
  gate_id      TEXT NOT NULL,
  item_key     TEXT NOT NULL,
  status       TEXT NOT NULL CHECK (status IN ('evidenced','failed')),
  evidence_ref TEXT NOT NULL,
  detail       JSONB,
  verified_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
  verified_by  TEXT NOT NULL,
  PRIMARY KEY (gate_id, item_key, verified_at)
);

COMMENT ON TABLE readiness_evidence IS
    'G-SIM-1 readiness evidence registry: latest checklist item evidence per (gate_id, item_key). Written only via POST /admin/readiness-evidence (history insert + upsert in one transaction). Freshness contract: readers treat rows older than 30 days as not fresh.';

COMMENT ON TABLE readiness_evidence_history IS
    'Append-only audit trail for readiness_evidence: every accepted write, never updated or deleted. Primary forensic source when the latest row is disputed.';

CREATE INDEX IF NOT EXISTS idx_readiness_evidence_history_gate
    ON readiness_evidence_history (gate_id, item_key, verified_at DESC);

COMMIT;
