-- Migration 125: cartridge_control — acople/desacople por cartucho (audit trail)
--
-- CARTRIDGE-CONTROL (2026-09-24): the searcher reads DESIRED coupling states
-- from the Redis hash `arbx:cartridges:control:<chain>` and applies them via
-- CartridgeRunner pause/resume (boot + hot PubSub commands). This table is the
-- OPERATOR AUDIT TRAIL: the api-server persists every control command here
-- BEFORE publishing to Redis, so any couple/decouple is attributable and
-- replayable. The searcher itself never writes this table (read path is
-- Redis-only by design).
--
-- Rows are append-only history; the LATEST row per (chain_id, cartridge_id)
-- is the current desired state. 264 numerados + 7 raíz + future v4 stems are
-- all addressable by their cartridge_id (the .rhai filename stem).

BEGIN;

CREATE TABLE IF NOT EXISTS cartridge_control (
  id            BIGSERIAL PRIMARY KEY,
  chain_id      BIGINT      NOT NULL,
  cartridge_id  TEXT        NOT NULL,
  desired       TEXT        NOT NULL CHECK (desired IN ('enabled', 'disabled')),
  actor         TEXT        NOT NULL,
  reason        TEXT,
  created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_cartridge_control_lookup
  ON cartridge_control (chain_id, cartridge_id, created_at DESC);

COMMENT ON TABLE cartridge_control IS
  'Append-only audit trail of cartridge couple/decouple commands (latest per cartridge = desired state; runtime source of truth is Redis arbx:cartridges:control:<chain>)';

COMMIT;
