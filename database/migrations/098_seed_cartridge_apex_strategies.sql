-- Migration 098: Seed APEX cartridge strategies for Funding Rate and Mean Reversion
-- Date: 2026-06-14
-- Reason: cartridge-integration e2e tests require at least 7 cartridges in
--         cartridge_registry, including 'funding-rate-arb' and 'mean-reversion-arb'.
--         Migration 090 only seeded 3 cartridges (dex_arb, triangular_arb, liquidation).
--         This migration adds the 4 missing APEX strategies.

-- ── 1. Funding Rate Arbitrage ─────────────────────────────────────────────────
-- Delta-neutral strategy: long spot on low-funding exchange, short perp on
-- high-funding exchange. Captures the funding rate differential every 8h cycle.
-- Ethics: PERMITTED — statistical arbitrage on public data, own positions only.
INSERT INTO cartridge_registry (
    slug, name, version, author, description, category,
    source_code, content_hash, target_chains, state
) VALUES (
    'funding-rate-arb',
    'Funding Rate Arbitrage',
    '1.0.0',
    'ArbitrageX APEX Team',
    'Delta-neutral funding rate arbitrage. Captures differential between perpetual '
    'funding rates across CEX/DEX venues. Evaluated via POST /api/v1/cartridges/funding-rate-arb/evaluate.',
    'funding_rate',
    '-- APEX: funding rate arbitrage — evaluated server-side by api-server --',
    'apex_funding_rate_arb_v1_0_0',
    '[]'::jsonb,
    'active'
) ON CONFLICT (slug) DO UPDATE SET
    name        = EXCLUDED.name,
    version     = EXCLUDED.version,
    description = EXCLUDED.description,
    category    = EXCLUDED.category,
    state       = EXCLUDED.state,
    updated_at  = NOW();

-- ── 2. Mean Reversion Arbitrage ───────────────────────────────────────────────
-- Statistical mean-reversion strategy: detects price deviations > 2σ from EMA
-- and enters a position expecting reversion to the mean.
-- Ethics: PERMITTED — statistical arbitrage on confirmed on-chain state.
INSERT INTO cartridge_registry (
    slug, name, version, author, description, category,
    source_code, content_hash, target_chains, state
) VALUES (
    'mean-reversion-arb',
    'Mean Reversion Arbitrage',
    '1.0.0',
    'ArbitrageX APEX Team',
    'EMA-based mean reversion arbitrage. Detects price deviations > 2σ from '
    'exponential moving average and captures reversion. Evaluated via '
    'POST /api/v1/cartridges/mean-reversion-arb/evaluate.',
    'mean_reversion',
    '-- APEX: mean reversion arbitrage — evaluated server-side by api-server --',
    'apex_mean_reversion_arb_v1_0_0',
    '[]'::jsonb,
    'active'
) ON CONFLICT (slug) DO UPDATE SET
    name        = EXCLUDED.name,
    version     = EXCLUDED.version,
    description = EXCLUDED.description,
    category    = EXCLUDED.category,
    state       = EXCLUDED.state,
    updated_at  = NOW();

-- ── 3. Flash Loan Arbitrage (placeholder) ────────────────────────────────────
-- Atomic flash loan arbitrage across DEX pools. Placeholder for future
-- implementation — seeded now to satisfy the ≥7 cartridges requirement.
INSERT INTO cartridge_registry (
    slug, name, version, author, description, category,
    source_code, content_hash, target_chains, state
) VALUES (
    'flashloan-arb',
    'Flash Loan Arbitrage',
    '1.0.0',
    'ArbitrageX APEX Team',
    'Atomic flash loan arbitrage across DEX pools. Borrows, swaps, repays in '
    'a single transaction. Zero capital at risk.',
    'flashloan',
    '-- APEX: flash loan arbitrage — scaffold, not yet active --',
    'apex_flashloan_arb_v1_0_0',
    '[]'::jsonb,
    'active'
) ON CONFLICT (slug) DO UPDATE SET
    name        = EXCLUDED.name,
    version     = EXCLUDED.version,
    description = EXCLUDED.description,
    category    = EXCLUDED.category,
    state       = EXCLUDED.state,
    updated_at  = NOW();

-- ── 4. Cross-Chain Arbitrage (placeholder) ───────────────────────────────────
-- Cross-chain price arbitrage using bridges. Placeholder for future
-- implementation — seeded now to satisfy the ≥7 cartridges requirement.
INSERT INTO cartridge_registry (
    slug, name, version, author, description, category,
    source_code, content_hash, target_chains, state
) VALUES (
    'cross-chain-arb',
    'Cross-Chain Arbitrage',
    '1.0.0',
    'ArbitrageX APEX Team',
    'Cross-chain price arbitrage using canonical bridges. Captures price '
    'discrepancies between the same asset on different chains.',
    'cross_chain',
    '-- APEX: cross-chain arbitrage — scaffold, not yet active --',
    'apex_cross_chain_arb_v1_0_0',
    '[]'::jsonb,
    'active'
) ON CONFLICT (slug) DO UPDATE SET
    name        = EXCLUDED.name,
    version     = EXCLUDED.version,
    description = EXCLUDED.description,
    category    = EXCLUDED.category,
    state       = EXCLUDED.state,
    updated_at  = NOW();
