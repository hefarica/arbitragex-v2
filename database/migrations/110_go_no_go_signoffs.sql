-- ArbitrageX v2 — Migration 110: go_no_go_signoffs (ARBX-RDY-06 / A.9)
--
-- Durable store for the two-operator GO/NO-GO formal sign-off over a
-- canonical ledger document (served by GET /api/v1/go-no-go/ledger, which
-- persists each generation to audit_log with action
-- 'go_no_go.ledger_generated' and target_id = ledger_hash).
--
-- Contract (api-server routes/go-no-go.ts):
--   - A sign-off ALWAYS references the ledger_hash of the CURRENT ledger
--     generation; a stale hash is rejected 400 by the POST handler.
--   - UNIQUE (ledger_hash, actor) enforces two DISTINCT operators: a second
--     sign-off from the same actor for the same ledger is rejected 409
--     (application-level check first; this constraint is the race backstop).
--   - State derivation for a given ledger_hash:
--       0 rows  -> awaiting_first
--       1 row   -> awaiting_second
--       >=2 all GO    -> signed_go
--       >=2 all NO_GO -> signed_no_go
--       mixed         -> conflicted
--   - This table RECORDS human decisions. It never flips anything live:
--     live_exec_policy stays default-deny (CLAUDE.md §34.3); the derived
--     go_live_eligible flag in GET /api/v1/go-no-go/status is a read of
--     recorded state only.
--
-- Ledger generations themselves are NOT stored here — they live in the
-- append-only audit_log (action go_no_go.ledger_generated), keeping this
-- table a single-purpose sign-off registry.
--
-- Idempotent.

BEGIN;

CREATE TABLE IF NOT EXISTS go_no_go_signoffs (
  id          BIGSERIAL PRIMARY KEY,
  ledger_hash TEXT NOT NULL,          -- sha256 hex of the canonical ledger facts document
  actor       TEXT NOT NULL,          -- x-arbx-actor header value (operator identity)
  decision    TEXT NOT NULL CHECK (decision IN ('GO','NO_GO')),
  signed_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE (ledger_hash, actor)
);

COMMENT ON TABLE go_no_go_signoffs IS
    'ARBX-RDY-06 (A.9) two-operator GO/NO-GO sign-off registry: one row per (ledger_hash, actor). Sign-offs always reference the current ledger generation (stale hashes rejected by the API). Records human decisions only — never enables live execution.';

CREATE INDEX IF NOT EXISTS idx_go_no_go_signoffs_hash_time
    ON go_no_go_signoffs (ledger_hash, signed_at ASC);

COMMIT;
