-- =====================================================================
-- OMEGA Migration 067 — Config-Hash Registry + Drift + Runtime ACK
-- =====================================================================
-- Implements:
--   • config_hash_registry: global + per-resource + per-chain hashes
--   • drift_observations:   PG ↔ Redis ↔ searcher-rs ↔ frontend deltas
--   • runtime_ack:          per-resource acknowledgement from searcher-rs
--                           workers after Arc-swap reload completion
--
-- This is layer 4 (persistence) and layer 6 (runtime ack) of the
-- 7-Layer Coherence Rule.
-- =====================================================================

BEGIN;

-- ---------------------------------------------------------------------
-- config_hash_registry: authoritative snapshot of every dynamic surface
-- ---------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS config_hash_registry (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    resource        TEXT NOT NULL CHECK (resource IN (
                      'global','chains','rpcs','dexes','pools','tokens','wallets',
                      'strategies','contracts','risk_gates','capital_gates',
                      'relays','agents')),
    chain_id        BIGINT,
    hash_value      CHAR(64) NOT NULL,
    row_count       INTEGER NOT NULL,
    computed_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    snapshot_id     UUID NOT NULL DEFAULT gen_random_uuid(),
    UNIQUE (resource, chain_id, computed_at)
);
CREATE INDEX IF NOT EXISTS idx_config_hash_registry_latest
    ON config_hash_registry (resource, chain_id, computed_at DESC);

-- ---------------------------------------------------------------------
-- runtime_ack: per-resource acknowledgement of hot-reload propagation
-- ---------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS runtime_ack (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id           UUID NOT NULL,
    resource           TEXT NOT NULL,
    chain_id           BIGINT,
    idempotency_key    TEXT NOT NULL,
    config_hash_before CHAR(64),
    config_hash_after  CHAR(64) NOT NULL,
    worker_id          TEXT NOT NULL,
    layer              TEXT NOT NULL CHECK (layer IN (
                         'api','persistence','redis_pubsub','searcher_rs','arc_swap',
                         'frontend_refresh','readiness','audit')),
    status             TEXT NOT NULL CHECK (status IN (
                         'pending','received','applied','rejected','timeout','failed')),
    latency_ms         INTEGER,
    error              TEXT,
    received_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_runtime_ack_event ON runtime_ack (event_id, layer);
CREATE INDEX IF NOT EXISTS idx_runtime_ack_resource ON runtime_ack (resource, chain_id, received_at DESC);

-- ---------------------------------------------------------------------
-- drift_observations: diff between authoritative layers
-- ---------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS drift_observations (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    resource           TEXT NOT NULL,
    chain_id           BIGINT,
    layer_a            TEXT NOT NULL,
    layer_b            TEXT NOT NULL,
    hash_a             CHAR(64),
    hash_b             CHAR(64),
    diff_count         INTEGER NOT NULL DEFAULT 0,
    diff_summary       JSONB NOT NULL DEFAULT '{}'::jsonb,
    severity           TEXT NOT NULL DEFAULT 'info'
                       CHECK (severity IN ('info','warn','error','critical')),
    observed_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    resolved_at        TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_drift_unresolved
    ON drift_observations (resource, chain_id, severity)
    WHERE resolved_at IS NULL;

-- ---------------------------------------------------------------------
-- feature_manifest: backend-declared features each must mirror frontend
-- (drives /api/system/feature_manifest used by mirror_fidelity test)
-- ---------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS feature_manifest (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    feature_key     TEXT NOT NULL UNIQUE,
    description     TEXT NOT NULL,
    layer           TEXT NOT NULL CHECK (layer IN ('backend','contract','runtime','frontend','edge')),
    state_hash      CHAR(64) NOT NULL,
    panel_path      TEXT NOT NULL,
    required        BOOLEAN NOT NULL DEFAULT TRUE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_feature_manifest_layer ON feature_manifest (layer, required);

-- Seed authoritative feature manifest (Mirror Law contract)
INSERT INTO feature_manifest (feature_key, description, layer, state_hash, panel_path, required) VALUES
  ('omega.s5.factory',     'CREATE2 DeterministicFactory deployments',     'contract', repeat('0',64), '/omega-s5/factory',  TRUE),
  ('omega.s5.wallets',     'WalletTopology / GasSponsor / ColdTreasury',   'contract', repeat('0',64), '/omega-s5/wallets',  TRUE),
  ('omega.s5.core',        'ResolutionCore + HolonomicDecoder (Yul)',      'contract', repeat('0',64), '/omega-s5/core',     TRUE),
  ('omega.s5.adapters',    'UniV2 / UniV3 / Balancer DEX adapters',        'contract', repeat('0',64), '/omega-s5/adapters', TRUE),
  ('omega.s5.crucible',    'Holesky + Arb Sepolia + Polygon Amoy stress',  'runtime',  repeat('0',64), '/omega-s5/crucible', TRUE),
  ('omega.s5.operator',    'Operator parametrization (capital_cap_usd)',   'backend',  repeat('0',64), '/omega-s5/operator', TRUE),
  ('omega.s5.drift',       'Drift detection PG↔Redis↔searcher↔frontend',   'backend',  repeat('0',64), '/omega-s5/drift',    TRUE),
  ('omega.registry.rpcs',      'RPC endpoint registry',         'backend', repeat('0',64), '/registry/rpcs',         TRUE),
  ('omega.registry.contracts', 'Contract address registry',     'backend', repeat('0',64), '/registry/contracts',    TRUE),
  ('omega.registry.relays',    'Private relay registry',        'backend', repeat('0',64), '/registry/relays',       TRUE),
  ('omega.registry.capital',   'Capital gate registry',         'backend', repeat('0',64), '/registry/capital-gates',TRUE),
  ('omega.registry.risk',      'Risk gate registry',            'backend', repeat('0',64), '/registry/risk-gates',   TRUE),
  ('omega.registry.agents',    'Sindicato agent registry',      'backend', repeat('0',64), '/registry/agents',       TRUE)
ON CONFLICT (feature_key) DO NOTHING;

COMMIT;
