-- ArbitrageX v2 — Migration 061: B1 chains_runtime
--
-- Operator Admin Console multi-chain (B1): tabla PG-backed que permite al
-- operador CRUD-ear chains sin reiniciar searcher-rs ni editar TOML.
--
-- Precedence rule (CRITICAL):
--   1. PG `chains_runtime` gana si existe registro explícito.
--   2. TOML bootstrap `configs/app.toml [[chains]]` solo para seed inicial.
--   3. Conflicto PG vs TOML: PG gana; api-server emite warning + métrica.
--   4. Delete = soft-disable (`enabled=false`), NUNCA DELETE físico.
--
-- chain_id: BIGINT (no INT) — soporta chain IDs grandes como 8453 (Base).
-- config_hash: SHA-256 del config serializado; usado por searcher-rs para
--   dedup de hot-reload events (no-op si hash sin cambio).
-- enabled flag: gobierna si searcher-rs debe spawnear run_chain task.

BEGIN;

CREATE TABLE IF NOT EXISTS chains_runtime (
  id              BIGSERIAL PRIMARY KEY,
  chain_id        BIGINT NOT NULL UNIQUE,
  name            TEXT NOT NULL,
  rpc_http_url    TEXT NOT NULL,
  rpc_ws_url      TEXT,
  native_currency TEXT NOT NULL DEFAULT 'ETH',
  block_time_ms   INT NOT NULL DEFAULT 12000,
  enabled         BOOLEAN NOT NULL DEFAULT TRUE,
  config_hash     TEXT,
  notes           TEXT,
  created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  created_by      TEXT,
  updated_by      TEXT,
  CONSTRAINT chains_runtime_block_time_positive CHECK (block_time_ms > 0),
  CONSTRAINT chains_runtime_chain_id_positive CHECK (chain_id > 0)
);

CREATE INDEX IF NOT EXISTS idx_chains_runtime_enabled
  ON chains_runtime (enabled, chain_id);

CREATE INDEX IF NOT EXISTS idx_chains_runtime_updated_at
  ON chains_runtime (updated_at DESC);

COMMENT ON TABLE chains_runtime IS
  'B1 (migration 061, 2026-05-13): operator-mutable chain catalog. '
  'PG precedence over TOML configs/app.toml [[chains]] seed. '
  'Soft-delete only (enabled=false); no physical DELETE.';

COMMENT ON COLUMN chains_runtime.config_hash IS
  'SHA-256 of the canonical config payload. Used by searcher-rs hot-reload '
  'subscriber to dedup no-op reloads (same hash = no rebuild).';

COMMENT ON COLUMN chains_runtime.enabled IS
  'Operator gate. When TRUE, searcher-rs spawns run_chain task. '
  'When FALSE, the existing task receives CancellationToken (30s timeout) '
  'and exits cleanly. Per-chain ONLY — never affects other chains.';

GRANT SELECT, INSERT, UPDATE ON chains_runtime TO arbx_rw;
GRANT SELECT ON chains_runtime TO arbx_ro;
GRANT USAGE, SELECT ON SEQUENCE chains_runtime_id_seq TO arbx_rw;

COMMIT;

-- ─── ROLLBACK (run manually if needed) ─────────────────────────────────────
-- BEGIN;
--   DROP INDEX IF EXISTS idx_chains_runtime_updated_at;
--   DROP INDEX IF EXISTS idx_chains_runtime_enabled;
--   DROP TABLE IF EXISTS chains_runtime;
-- COMMIT;
