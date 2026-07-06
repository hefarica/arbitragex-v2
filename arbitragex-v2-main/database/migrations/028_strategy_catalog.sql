-- ArbitrageX v2 — Migration 028: strategy_catalog (universal MEV strategy library).
--
-- Doctrine: this is the read-only metadata catalog of every strategy the platform
-- CAN run, independent of which are currently activated for a chain. Activation
-- lives in `trading_config.enabled_strategies TEXT[]` (migration 026).
--
-- Powers the Strategy Panel dropdown "+ Add strategy from catalog". New strategies
-- land via PR (this migration insert + skill .md + Rust patterns code) — never via
-- the UI. `is_implemented = TRUE` gates whether `enabled` actually does anything in
-- the searcher hot path.

CREATE TABLE IF NOT EXISTS strategy_catalog (
  kind                     TEXT PRIMARY KEY,                  -- slug, e.g. 'dex_arb'
  display_name             TEXT NOT NULL,
  description              TEXT NOT NULL,
  category                 TEXT NOT NULL CHECK (category IN (
                             'atomic','cross-chain','cross-venue','liquidation','mev-share',
                             'stat','stables','derivatives','curve','balancer','l2','solana',
                             'oracle','yield','jit')),
  is_implemented           BOOLEAN NOT NULL DEFAULT FALSE,
  is_default               BOOLEAN NOT NULL DEFAULT FALSE,
  risk_level               TEXT NOT NULL CHECK (risk_level IN ('low','medium','high','very-high')),
  capital_required         TEXT,
  expected_complexity      TEXT,
  competitive_advantage    TEXT CHECK (competitive_advantage IN ('baja','media','alta','muy-alta','extrema')),
  ethical_constraint       TEXT CHECK (ethical_constraint IN ('none','defensive_only')),
  skill_refs               TEXT[]    NOT NULL DEFAULT '{}',
  requires_flashloan       BOOLEAN   NOT NULL DEFAULT FALSE,
  chains_supported         BIGINT[]  NOT NULL DEFAULT '{1}'::BIGINT[],
  created_at               TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

GRANT SELECT ON strategy_catalog TO arbx_ro;
GRANT SELECT, INSERT, UPDATE ON strategy_catalog TO arbx_rw;

-- ── Seed: 10 strategies from SOP_ArbitrageX_2026.pdf §3 (Tabla 1) ──────────────
-- 6 marked is_default=TRUE (visible cards on dashboard), 4 in catalog only.

INSERT INTO strategy_catalog (
  kind, display_name, description, category, is_implemented, is_default,
  risk_level, capital_required, expected_complexity, competitive_advantage,
  ethical_constraint, skill_refs, requires_flashloan
) VALUES
  ('dex_arb', 'DEX-DEX Atomic',
   'Buy DEX A, sell DEX B in the same atomic tx. Most common pattern; the only one currently producing live opportunities.',
   'atomic', TRUE, TRUE, 'low', 'alto', 'baja', 'media', 'none',
   ARRAY['.agents/skills/sop_executive_summary/SKILL.md'], FALSE),

  ('triangular', 'Triangular Flash Loan',
   'Three-token cycle within the same DEX (A→B→C→A). Capital sourced from a flash loan so own-capital is zero.',
   'atomic', FALSE, TRUE, 'medium', 'cero', 'media', 'alta', 'none',
   ARRAY['.agents/skills/sop_dex_triangular/SKILL.md'], TRUE),

  ('cex_dex', 'CEX-DEX Arbitrage',
   'Exploit price gaps between centralized exchanges (Binance/OKX) and on-chain DEXs. SOP §5 marks this competitive_advantage extrema.',
   'cross-venue', FALSE, TRUE, 'medium', 'alto', 'alta', 'extrema', 'none',
   ARRAY['.agents/skills/sop_cex_dex/SKILL.md'], FALSE),

  ('liquidation', 'Liquidation Sniping',
   'Liquidate undercollateralized loans on Aave/Compound when health_factor < 1.05 to claim the liquidation bonus.',
   'liquidation', FALSE, TRUE, 'high', 'variable', 'alta', 'alta', 'none',
   ARRAY['.agents/skills/sop_liquidations/SKILL.md'], FALSE),

  ('jit_liquidity_v3', 'JIT Liquidity (Uniswap V3)',
   'Provide concentrated liquidity just-in-time for a large incoming swap, capture the swap fees, then withdraw — all atomic.',
   'jit', FALSE, TRUE, 'medium', 'medio', 'alta', 'alta', 'none',
   ARRAY['.agents/skills/sop_jit_liquidity/SKILL.md'], FALSE),

  ('flashloan_arb', 'Flashloan Multi-DEX',
   'Flash loan-funded multi-hop arbitrage across 3+ DEXs in a single atomic bundle.',
   'atomic', FALSE, TRUE, 'medium', 'cero', 'media', 'media', 'none',
   ARRAY['.agents/skills/sop_flashbots_bundles/SKILL.md'], TRUE),

  ('cross_chain', 'Cross-Chain Bridge Arb',
   'Exploit price gaps for the same token across L1/L2 bridges (Base, Arbitrum, Optimism, zkSync). Non-atomic — bridge latency is the risk.',
   'cross-chain', FALSE, FALSE, 'high', 'medio', 'alta', 'extrema', 'none',
   ARRAY['.agents/skills/sop_strategy_matrix/SKILL.md'], FALSE),

  ('micro_hft', 'Micro-Arbitrage HFT (L2)',
   'Volume-over-home-runs strategy: 100-1000 small ops/day on Arbitrum/Base/Optimism where gas << $0.01 per swap.',
   'atomic', FALSE, FALSE, 'low', 'medio', 'media', 'media', 'none',
   ARRAY['.agents/skills/sop_micro_arb_hft/SKILL.md'], FALSE),

  ('sandwich_def', 'Sandwich (DEFENSIVE)',
   'Detection-only. Powers anti-sandwich protections for our own swaps and orderflow customers. NEVER executed offensively.',
   'mev-share', FALSE, FALSE, 'very-high', 'alto', 'alta', 'baja', 'defensive_only',
   ARRAY['.agents/skills/sop_sandwich_defensive/SKILL.md'], FALSE),

  ('mev_boost_block', 'MEV-Boost Block Building',
   'Top-tier searcher: negotiate full block construction with builders to capture multi-tx MEV. Highest barrier to entry.',
   'mev-share', FALSE, FALSE, 'very-high', 'alto', 'alta', 'extrema', 'none',
   ARRAY['.agents/skills/sop_flashbots_bundles/SKILL.md'], FALSE)
ON CONFLICT (kind) DO NOTHING;
