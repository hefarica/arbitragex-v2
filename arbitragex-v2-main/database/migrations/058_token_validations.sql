-- Migration 058 — Token Validation Engine (Phase 1) persistence.
--
-- Stores the composite result of the multi-validator token-safety pipeline
-- so the api-server's hot path (`/opportunities/live`) can render a real
-- safety score without re-running all the validators per request. The
-- engine validates every (chain_id, address) pair that touches the
-- opportunity stream and caches results here keyed by the pair.
--
-- Phase 1 validators (this migration):
--   - OnChainTruthValidator: RPC reads (bytecode size, ERC-20 ABI surface)
--   - LiquidityRealityValidator: DEX Screener API (pair count, liquidity,
--                                volume across windows)
--   - Registry cross-check: surfaces whether the address is in the
--                           Uniswap default token list OR CoinGecko
--                           coinslist (set by the existing tokenRegistry).
--
-- Composite output:
--   final_status: 'VERIFIED' | 'VIABLE' | 'LOW_LIQUIDITY' | 'ILLIQUID' |
--                 'NO_DATA' | 'INVALID' | 'PENDING'
--   score:        0-100. Threshold conventions for the dashboard:
--                   ≥ 70 → green badge, tradeable
--                   50-69 → yellow badge, low-confidence
--                   < 50  → red badge, do NOT trade
--
-- R8 fail-honest: every nullable column means "validator hasn't run or
-- the external source had no data". Never coerce to 0/false.
--
-- Future phases (NOT in this migration):
--   - SecurityRiskValidator (GoPlus + Honeypot.is) — adds honeypot_risk,
--     ownership_risk, etc.
--   - ContractVerificationValidator (Sourcify/Etherscan) — adds
--     source_verified, source_verifier.
--   - HoneypotSimulationValidator (revm sell-sim, depends on simulator-v2).
--   - PriceOracleValidator (Chainlink feeds).
-- The schema below leaves room (JSONB validators column) for those to
-- attach their results without further migrations.

BEGIN;

CREATE TABLE IF NOT EXISTS token_validations (
  chain_id              BIGINT      NOT NULL,
  address               TEXT        NOT NULL,

  -- ── OnChainTruthValidator ──────────────────────────────────────────────
  exists_onchain        BOOLEAN,
  is_contract           BOOLEAN,
  bytecode_size         INTEGER,
  standard              TEXT,       -- 'ERC20' | 'ERC721' | 'unknown' | NULL
  onchain_name          TEXT,
  onchain_symbol        TEXT,
  onchain_decimals      SMALLINT,
  total_supply_str      TEXT,       -- BigInt as string (raw, undivided by decimals)

  -- ── LiquidityRealityValidator (DEX Screener snapshot) ──────────────────
  pair_count            INTEGER,
  liquidity_usd         NUMERIC(28, 8),
  volume_24h_usd        NUMERIC(28, 8),
  volume_1h_usd         NUMERIC(28, 8),
  volume_5m_usd         NUMERIC(28, 8),
  price_usd             NUMERIC(38, 18),
  primary_dex           TEXT,
  primary_pair_address  TEXT,

  -- ── Registry cross-check (tokenRegistry.ts source) ─────────────────────
  registry_verified     BOOLEAN,
  registry_source       TEXT,       -- 'uniswap' | 'coingecko' | NULL
  registry_symbol       TEXT,
  registry_name         TEXT,

  -- ── Composite ──────────────────────────────────────────────────────────
  -- Status taxonomy explained at file head. Always non-null — even when
  -- every validator fails we record 'PENDING' so the row exists.
  final_status          TEXT        NOT NULL,
  score                 SMALLINT    NOT NULL CHECK (score BETWEEN 0 AND 100),
  score_reasons         JSONB,      -- per-validator score deltas + notes

  -- ── Provenance / freshness ─────────────────────────────────────────────
  validators_run        TEXT[]      NOT NULL DEFAULT ARRAY[]::TEXT[],
  errors                JSONB,      -- per-validator errors when applicable
  validated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  stale_after           TIMESTAMPTZ NOT NULL,

  PRIMARY KEY (chain_id, address),

  -- Mirror the `tokens` table's address normalisation (lowercase 0x-prefix).
  CONSTRAINT chk_token_validations_address_lower
    CHECK (address ~ '^0x[a-f0-9]{40}$')
);

COMMENT ON TABLE token_validations IS
  'Token Validation Engine Phase 1: persisted composite results from OnChainTruth + LiquidityReality + Registry validators. Hot path reads from here; async worker writes. See SOP §11 (scam detection) and migration head for status taxonomy.';

-- Look-up index for "what needs revalidation" sweeps by the async worker.
CREATE INDEX IF NOT EXISTS idx_token_validations_stale_after
  ON token_validations (stale_after);

-- Aggregations + dashboard filtering by status.
CREATE INDEX IF NOT EXISTS idx_token_validations_final_status
  ON token_validations (final_status);

COMMIT;
