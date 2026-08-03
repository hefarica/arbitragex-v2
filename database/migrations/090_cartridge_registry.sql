-- ═══════════════════════════════════════════════════════════════════════════════
-- FASE OMEGA — Dynamic Strategy Cartridge Registry
-- ═══════════════════════════════════════════════════════════════════════════════
--
-- This migration creates the persistence layer for the cartridge runtime.
-- Cartridges are stored in PostgreSQL as source of truth, mirrored to Redis
-- for fast access by searcher-rs nodes.
--
-- ## Architecture
--
-- PostgreSQL (source of truth) → Redis mirror → searcher-rs hot-load
--                                    ↑
--                              API Server writes
--                                    ↑
--                              Strategy Forge UI
--
-- ## Chain Support
--
-- The `target_chains` column is a JSONB array. An empty array `[]` means
-- the cartridge is compatible with ALL chains. This allows operators to
-- deploy chain-agnostic strategies that automatically activate on any
-- new chain added to the system.
-- ═══════════════════════════════════════════════════════════════════════════════

-- Cartridge lifecycle states (idempotent: skip if type already exists)
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'cartridge_state') THEN
        CREATE TYPE cartridge_state AS ENUM (
            'active',
            'paused',
            'failed',
            'archived'
        );
    END IF;
END $$;

-- Main cartridge registry table
CREATE TABLE IF NOT EXISTS cartridge_registry (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- Human-readable slug (unique, used as cartridge_id in runtime)
    slug            TEXT NOT NULL UNIQUE,
    -- Metadata from init_strategy()
    name            TEXT NOT NULL,
    version         TEXT NOT NULL DEFAULT '1.0.0',
    author          TEXT NOT NULL,
    description     TEXT NOT NULL DEFAULT '',
    category        TEXT NOT NULL DEFAULT 'custom',
    -- Source code
    source_code     TEXT NOT NULL,
    content_hash    TEXT NOT NULL,  -- SHA-256 of source_code
    -- Chain targeting (empty array = all chains)
    target_chains   JSONB NOT NULL DEFAULT '[]'::jsonb,
    -- Lifecycle
    state           cartridge_state NOT NULL DEFAULT 'active',
    -- Runtime config
    min_eval_interval_ms INTEGER NOT NULL DEFAULT 100,
    -- Audit
    created_by      TEXT NOT NULL DEFAULT 'system',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    -- Compilation stats
    last_compiled_at    TIMESTAMPTZ,
    compilation_errors  TEXT,
    -- Runtime stats (updated periodically from Redis)
    total_evaluations   BIGINT NOT NULL DEFAULT 0,
    total_opportunities BIGINT NOT NULL DEFAULT 0,
    total_errors        BIGINT NOT NULL DEFAULT 0,
    last_evaluation_at  TIMESTAMPTZ
);

-- Index for fast lookup by state (active cartridges)
CREATE INDEX IF NOT EXISTS idx_cartridge_registry_state ON cartridge_registry(state);

-- Index for chain-specific queries
CREATE INDEX IF NOT EXISTS idx_cartridge_registry_chains ON cartridge_registry USING GIN(target_chains);

-- Index for content hash dedup
CREATE UNIQUE INDEX IF NOT EXISTS idx_cartridge_registry_hash ON cartridge_registry(content_hash)
    WHERE state != 'archived';

-- Cartridge audit log (every injection/update/state change)
CREATE TABLE IF NOT EXISTS cartridge_audit_log (
    id              BIGSERIAL PRIMARY KEY,
    cartridge_id    UUID NOT NULL REFERENCES cartridge_registry(id),
    event_type      TEXT NOT NULL,  -- 'inject', 'update', 'pause', 'resume', 'archive', 'error'
    actor           TEXT NOT NULL,
    details         JSONB DEFAULT '{}'::jsonb,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_cartridge_audit_cartridge ON cartridge_audit_log(cartridge_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_cartridge_audit_time ON cartridge_audit_log(created_at DESC);

-- Cartridge evaluation metrics (aggregated per hour)
CREATE TABLE IF NOT EXISTS cartridge_metrics_hourly (
    id              BIGSERIAL PRIMARY KEY,
    cartridge_id    UUID NOT NULL REFERENCES cartridge_registry(id),
    chain_id        BIGINT NOT NULL,
    hour            TIMESTAMPTZ NOT NULL,
    evaluations     BIGINT NOT NULL DEFAULT 0,
    opportunities   BIGINT NOT NULL DEFAULT 0,
    errors          BIGINT NOT NULL DEFAULT 0,
    avg_eval_ms     DOUBLE PRECISION,
    max_eval_ms     DOUBLE PRECISION,
    total_profit_usd DOUBLE PRECISION DEFAULT 0.0,
    UNIQUE(cartridge_id, chain_id, hour)
);

CREATE INDEX IF NOT EXISTS idx_cartridge_metrics_time ON cartridge_metrics_hourly(hour DESC);

-- ═══════════════════════════════════════════════════════════════════════════════
-- Seed: Register the built-in cartridges
-- ═══════════════════════════════════════════════════════════════════════════════

-- Note: Source code is NOT stored in migrations. The API server reads from
-- the cartridges/ directory at first boot and populates this table.
-- This seed only creates placeholder entries for the built-in strategies. Each
-- content_hash is a DISTINCT pre-boot placeholder ('pending_boot_load:<slug>')
-- because the partial UNIQUE index idx_cartridge_registry_hash (WHERE state !=
-- 'archived') forbids duplicate hashes among active rows — three identical
-- 'pending_boot_load' values would violate it. The API server overwrites each with
-- the real SHA-256 of the cartridge source at first boot.

INSERT INTO cartridge_registry (slug, name, version, author, description, category, source_code, content_hash, target_chains, state)
VALUES
    ('dex_arb', 'DEX Arbitrage Universal', '2.0.0', 'ArbitrageX Core Team',
     'Multi-chain DEX arbitrage detector. Identifies price discrepancies between V2/V3 pools.',
     'dex_arb', '-- loaded from filesystem at boot --', 'pending_boot_load:dex_arb', '[]'::jsonb, 'active'),
    ('triangular_arb', 'Triangular Arbitrage Universal', '2.0.0', 'ArbitrageX Core Team',
     'Multi-chain triangular arbitrage detector. Finds profitable A→B→C→A cycles.',
     'triangular_arb', '-- loaded from filesystem at boot --', 'pending_boot_load:triangular_arb', '[]'::jsonb, 'active'),
    ('liquidation', 'Liquidation Monitor Universal', '2.0.0', 'ArbitrageX Core Team',
     'Multi-chain liquidation opportunity detector for Aave V3 and Compound V2/V3.',
     'liquidation', '-- loaded from filesystem at boot --', 'pending_boot_load:liquidation', '[]'::jsonb, 'active')
ON CONFLICT (slug) DO NOTHING;

-- ═══════════════════════════════════════════════════════════════════════════════
-- Comments
-- ═══════════════════════════════════════════════════════════════════════════════

COMMENT ON TABLE cartridge_registry IS 'FASE OMEGA: Dynamic strategy cartridge registry. Source of truth for all Rhai cartridges deployed in the system.';
COMMENT ON COLUMN cartridge_registry.target_chains IS 'JSONB array of chain IDs. Empty array [] means the cartridge works on ALL chains.';
COMMENT ON COLUMN cartridge_registry.content_hash IS 'SHA-256 of source_code. Used for dedup during hot-reload to avoid recompilation of identical scripts.';
COMMENT ON COLUMN cartridge_registry.slug IS 'Unique human-readable identifier used as cartridge_id in the Rust runtime.';
