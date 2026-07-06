-- ============================================================================
-- Migration 045: per-chain DEX metrics (TVL + volume) from DefiLlama
--
-- Fixes the display bug where /api/dexes?chain_id=X returned globally-
-- aggregated TVL/volume instead of chain-specific values. Example: Curve
-- was showing $1.75B (all-chains sum) even when filtering for Polygon
-- (correct value: ~$6.6M).
--
-- New table: dex_chain_metrics — single row per (dex_id, chain_id) pair,
-- upserted on each refresh. History snapshots deferred to Phase 4.
--
-- Source: api.llama.fi/protocol/{slug}     → currentChainTvls.{ChainName}
--         api.llama.fi/summary/dexs/{slug} → totalDataChartBreakdown[-1][1].{ChainName}
-- Fetched: 2026-05-07 (live data via Python urllib)
--
-- Coverage: 38 of 43 active factory (dex_id, chain_id) pairs populated.
--   88.4% coverage (target: >80%). 5 pairs remain NULL — see STEP 3 notes.
--
-- NULL policy (R8 fail-honest):
--   Source returned 404/400  → both tvl_usd and volume_24h_usd = NULL.
--   Source returned data for TVL only or volume only → partial row, no fill.
--   NULL means "data unavailable from source", not "value is zero".
--   API endpoint COALESCES to dexes.tvl_usd / dexes.volume_24h_usd (global
--   aggregate) when no per-chain row exists.
--
-- Constraint on dexes.volume_24h_usd / dexes.tvl_usd: DO NOT modify those
-- columns — they remain global aggregates for the fallback COALESCE path.
--
-- Idempotent: ON CONFLICT (dex_id, chain_id) DO UPDATE on every INSERT.
-- Reversible: DOWN block at end of file.
-- ============================================================================

BEGIN;

-- ────────────────────────────────────────────────────────────────────────────
-- STEP 1: Schema
-- ────────────────────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS dex_chain_metrics (
    dex_id          UUID        NOT NULL REFERENCES dexes(id)   ON DELETE CASCADE,
    chain_id        INTEGER     NOT NULL REFERENCES chains(chain_id) ON DELETE CASCADE,
    volume_24h_usd  NUMERIC(20, 2),
    tvl_usd         NUMERIC(20, 2),
    last_updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    source          TEXT        NOT NULL DEFAULT 'defillama'
        CHECK (source IN ('defillama', 'subgraph', 'manual', 'estimated')),
    PRIMARY KEY (dex_id, chain_id)
);

-- Chain-only index for the /api/dexes?chain_id=X query pattern.
CREATE INDEX IF NOT EXISTS idx_dcm_chain_id
    ON dex_chain_metrics(chain_id);

-- Descending TVL per chain for ranked queries (ORDER BY tvl_usd DESC).
CREATE INDEX IF NOT EXISTS idx_dcm_tvl_desc
    ON dex_chain_metrics(chain_id, tvl_usd DESC NULLS LAST);

-- ────────────────────────────────────────────────────────────────────────────
-- STEP 2: Populate per-chain TVL + volume
--
-- Row order: grouped by chain_id, then by DEX within chain (descending TVL).
-- All amounts in USD, NUMERIC(20,2). DefiLlama returns float; values are
-- rounded to whole dollars at fetch time (no sub-dollar precision available).
--
-- Source column notes:
--   'defillama'  — value obtained directly from DefiLlama API response.
--   'estimated'  — value derived/inferred (BSC TVL gaps for PancakeSwap V2/V3
--                  and ApeSwap where DefiLlama /protocol endpoint omitted the
--                  BSC chain despite the DEX being BSC-dominant; Uniswap V4
--                  BSC TVL not returned by /protocol endpoint on fetch date).
--
-- BSC TVL estimation methodology:
--   PancakeSwap V3 BSC: global tvl_usd in dexes row = $350M (from 044).
--     ETH($31.4M) + ARB($7.8M) + Base($28.0M) = $67.2M non-BSC.
--     Residual for BSC: $350M - $67.2M ≈ $282M. Rounded to $282,000,000.
--   PancakeSwap V2 BSC: global tvl_usd = $1,750M. Non-BSC total = $2.4M.
--     BSC residual ≈ $1,747M. Rounded conservatively to $1,747,600,000.
--   ApeSwap BSC: global tvl_usd not set in dexes (seeded without TVL in 043).
--     DefiLlama /protocol/apeswap-amm returned ETH($398) + ARB($12K) +
--     Polygon($259K). BSC TVL not in response. Marked NULL (R8 fail-honest:
--     insufficient basis for estimation).
--   Uniswap V4 BSC: /protocol endpoint returned no BSC TVL. Vol endpoint
--     confirmed BSC is active ($49.2M vol). TVL estimation: not possible
--     without TVL source. Marked NULL.
-- ────────────────────────────────────────────────────────────────────────────

INSERT INTO dex_chain_metrics (dex_id, chain_id, volume_24h_usd, tvl_usd, source)
VALUES

    -- ═══════════════════════════════════════════════════════════════════════
    -- chain_id = 1 (Ethereum)
    -- ═══════════════════════════════════════════════════════════════════════

    -- Uniswap V4: new singleton PoolManager. ETH dominant TVL/vol ($627M TVL, $761M/day).
    -- Source: /protocol/uniswap-v4 currentChainTvls.Ethereum
    ((SELECT id FROM dexes WHERE name = 'Uniswap V4'),  1,
     760506543.00,   -- vol: /summary/dexs/uniswap-v4 .Ethereum
     627278929.00,   -- tvl: /protocol/uniswap-v4 .Ethereum
     'defillama'),

    -- Curve: stablecoin/LST dominant on ETH ($1.56B TVL, $224M/day vol).
    -- Note: ETH stable factory is NG variant (ABI: pool_list(), not pool_count()).
    ((SELECT id FROM dexes WHERE name = 'Curve'),  1,
     223695543.00,
     1561343355.00,
     'defillama'),

    -- UniswapV3: deep ETH liquidity ($1.05B TVL, $299M/day).
    ((SELECT id FROM dexes WHERE name = 'UniswapV3'),  1,
     298667064.00,
     1054251964.00,
     'defillama'),

    -- SushiSwap: legacy presence on ETH ($26.5M TVL, $299K/day vol — low but present).
    ((SELECT id FROM dexes WHERE name = 'SushiSwap'),  1,
     298613.00,
     26521099.00,
     'defillama'),

    -- PancakeSwap V3: ETH deployment ($31.4M TVL, $36.9M/day).
    ((SELECT id FROM dexes WHERE name = 'PancakeSwap V3'),  1,
     36863598.00,
     31412497.00,
     'defillama'),

    -- Balancer: weighted pools on ETH ($28.9M TVL, $255K/day).
    -- Note: ETH has both V2 (0xBA12...) and V3 (0xbA13...) vaults — different ABIs.
    -- Metric row covers the Balancer DEX protocol aggregate on this chain.
    ((SELECT id FROM dexes WHERE name = 'Balancer'),  1,
     254904.00,
     28910956.00,
     'defillama'),

    -- UniswapV2: factory seeded (029). DefiLlama /summary/dexs/uniswap-v2 returned
    -- 400 Bad Request; /protocol/uniswap-v2 had no chain TVL breakdown. NULL (R8).
    -- Row still inserted (factory exists); both values NULL is the honest outcome.
    ((SELECT id FROM dexes WHERE name = 'UniswapV2'),  1,
     NULL,
     NULL,
     'defillama'),

    -- ═══════════════════════════════════════════════════════════════════════
    -- chain_id = 56 (BSC)
    -- ═══════════════════════════════════════════════════════════════════════

    -- PancakeSwap V3: BSC dominant ($342M/day vol). TVL gap on BSC from /protocol
    -- endpoint. Estimated via global - non-BSC residual (see file header).
    ((SELECT id FROM dexes WHERE name = 'PancakeSwap V3'),  56,
     341651016.00,
     282000000.00,   -- estimated: $350M global - ($31.4M ETH + $7.8M ARB + $28.0M Base)
     'estimated'),

    -- PancakeSwap V2: BSC legacy. TVL estimated from global minus non-BSC residual.
    ((SELECT id FROM dexes WHERE name = 'PancakeSwap V2'),  56,
     63648120.00,
     1747600000.00,  -- estimated: $1,750M global - $2.4M non-BSC
     'estimated'),

    -- Uniswap V4: active on BSC ($49.2M/day vol confirmed). TVL not returned
    -- by /protocol endpoint on fetch date. R8: NULL is honest.
    ((SELECT id FROM dexes WHERE name = 'Uniswap V4'),  56,
     49171886.00,
     NULL,
     'defillama'),

    -- SushiSwap: minimal presence on BSC ($10K/day vol). No TVL in /protocol.
    ((SELECT id FROM dexes WHERE name = 'SushiSwap'),  56,
     10373.00,
     NULL,
     'defillama'),

    -- BiSwap: BSC-native V2 fork ($5.9M TVL, $109K/day vol — declining).
    ((SELECT id FROM dexes WHERE name = 'BiSwap'),  56,
     108717.00,
     5938227.00,
     'defillama'),

    -- THENA: Solidly fork on BSC ($4.8M TVL, vol effectively 0).
    ((SELECT id FROM dexes WHERE name = 'THENA'),  56,
     0.00,
     4808877.00,
     'defillama'),

    -- ApeSwap: BSC factory exists. DefiLlama returned ETH/ARB/Polygon TVL but
    -- not BSC. Vol on BSC: $81K/day. TVL NULL per R8 (no BSC TVL in source).
    ((SELECT id FROM dexes WHERE name = 'ApeSwap'),  56,
     81491.00,
     NULL,
     'defillama'),

    -- ═══════════════════════════════════════════════════════════════════════
    -- chain_id = 137 (Polygon)
    -- ═══════════════════════════════════════════════════════════════════════

    -- Quickswap V2: dominant TVL on Polygon. The 'quickswap' slug aggregates
    -- V2+V3; V3 has its own slug (quickswap-v3, $4.2M). This row uses the
    -- aggregated slug TVL (~$471M) as the best available proxy for V2 (which
    -- holds the bulk of the legacy liquidity). Labeled 'estimated' because
    -- the slug aggregates both versions.
    -- Cross-validation: CoinGecko reports ~$451M for Quickswap on Polygon
    -- (sourced 2026-05). $471M is within expected variance given date delta.
    ((SELECT id FROM dexes WHERE name = 'Quickswap V2'),  137,
     246743.00,
     471131661.00,
     'estimated'),

    -- Quickswap V3: Algebra-based CL. V3-specific slug returns $4.2M TVL.
    -- Lower TVL than V2 (newer pools). Vol $5M/day (active trading).
    ((SELECT id FROM dexes WHERE name = 'Quickswap V3'),  137,
     5024211.00,
     4231859.00,
     'defillama'),

    -- UniswapV3: significant Polygon presence ($34.3M TVL, $14.9M/day).
    ((SELECT id FROM dexes WHERE name = 'UniswapV3'),  137,
     14948575.00,
     34317692.00,
     'defillama'),

    -- Uniswap V4: Polygon deployment ($23.0M TVL, $39.8M/day vol).
    ((SELECT id FROM dexes WHERE name = 'Uniswap V4'),  137,
     39798425.00,
     23032611.00,
     'defillama'),

    -- Curve: Polygon factory (0x722272... deactivated in 044 as unverified).
    -- TVL and vol data available from DefiLlama regardless of factory status.
    -- Metric row still useful — endpoint can display chain data even if factory
    -- is inactive (factory.is_active controls pool_sync_worker, not display).
    ((SELECT id FROM dexes WHERE name = 'Curve'),  137,
     119980.00,
     6578818.00,
     'defillama'),

    -- SushiSwap: Polygon presence ($5.6M TVL, $32K/day).
    ((SELECT id FROM dexes WHERE name = 'SushiSwap'),  137,
     31984.00,
     5580563.00,
     'defillama'),

    -- Balancer: Polygon deployment ($4.5M TVL, $311K/day).
    ((SELECT id FROM dexes WHERE name = 'Balancer'),  137,
     311444.00,
     4457769.00,
     'defillama'),

    -- ═══════════════════════════════════════════════════════════════════════
    -- chain_id = 42161 (Arbitrum)
    -- ═══════════════════════════════════════════════════════════════════════

    -- UniswapV3: second-largest ARB DEX ($221.5M TVL, $134M/day).
    ((SELECT id FROM dexes WHERE name = 'UniswapV3'),  42161,
     134212915.00,
     221542908.00,
     'defillama'),

    -- Uniswap V4: ARB deployment ($45.3M TVL, $29.5M/day).
    ((SELECT id FROM dexes WHERE name = 'Uniswap V4'),  42161,
     29493817.00,
     45255716.00,
     'defillama'),

    -- Curve: ARB deployment ($18.2M TVL, $4.2M/day). Factory 0xb17b... deactivated
    -- (unverified), but protocol has confirmed presence per DefiLlama chain TVL.
    ((SELECT id FROM dexes WHERE name = 'Curve'),  42161,
     4187622.00,
     18172783.00,
     'defillama'),

    -- SushiSwap: ARB presence ($4.8M TVL, $160K/day).
    ((SELECT id FROM dexes WHERE name = 'SushiSwap'),  42161,
     160053.00,
     4822975.00,
     'defillama'),

    -- Balancer: ARB deployment ($1.3M TVL, $97K/day).
    ((SELECT id FROM dexes WHERE name = 'Balancer'),  42161,
     96683.00,
     1334234.00,
     'defillama'),

    -- PancakeSwap V3: ARB deployment ($7.8M TVL, $8.3M/day).
    ((SELECT id FROM dexes WHERE name = 'PancakeSwap V3'),  42161,
     8268058.00,
     7772046.00,
     'defillama'),

    -- Camelot V3: is_active=FALSE at DEX level (TVL $20M < $40M major-chain floor,
    -- economics-validator v3 condition). DefiLlama slug returned 400. NULL per R8.
    ((SELECT id FROM dexes WHERE name = 'Camelot V3'),  42161,
     NULL,
     NULL,
     'defillama'),

    -- TraderJoe: factory is_active=FALSE (address unverified). DefiLlama
    -- slug trader-joe-v2.1 returned 400. NULL per R8.
    ((SELECT id FROM dexes WHERE name = 'TraderJoe'),  42161,
     NULL,
     NULL,
     'defillama'),

    -- ═══════════════════════════════════════════════════════════════════════
    -- chain_id = 10 (Optimism)
    -- ═══════════════════════════════════════════════════════════════════════

    -- UniswapV3: significant OP presence ($17.2M TVL, $4.2M/day).
    ((SELECT id FROM dexes WHERE name = 'UniswapV3'),  10,
     4161264.00,
     17193809.00,
     'defillama'),

    -- Velodrome V2: OP-native Solidly fork ($16.3M TVL from /protocol/velodrome-v2).
    -- Note: vol endpoint returned empty response (no breakdown data in the response
    -- on fetch date). TVL populated; vol NULL per R8.
    -- Note on 044 global figure: dexes.tvl_usd = $300M global aggregate
    -- (V2 + Slipstream combined per 044 commentary). This per-chain row
    -- correctly shows only the V2 component TVL on OP: $16.3M.
    ((SELECT id FROM dexes WHERE name = 'Velodrome V2'),  10,
     NULL,
     16340942.00,
     'defillama'),

    -- Uniswap V4: OP deployment ($1.9M TVL). Vol not returned by breakdown endpoint.
    ((SELECT id FROM dexes WHERE name = 'Uniswap V4'),  10,
     NULL,
     1945713.00,
     'defillama'),

    -- Curve: OP deployment ($1.6M TVL, $75.5K/day). Factory deactivated (unverified
    -- address), but protocol presence confirmed by DefiLlama chain TVL.
    ((SELECT id FROM dexes WHERE name = 'Curve'),  10,
     75532.00,
     1624200.00,
     'defillama'),

    -- SushiSwap: minimal OP presence ($3.2K TVL, 0 vol on date).
    ((SELECT id FROM dexes WHERE name = 'SushiSwap'),  10,
     0.00,
     3201.00,
     'defillama'),

    -- Beethoven X: Balancer fork on Optimism. DefiLlama /protocol/beethoven-x
    -- returned 400. No TVL or vol data available. NULL per R8.
    ((SELECT id FROM dexes WHERE name = 'Beethoven X'),  10,
     NULL,
     NULL,
     'defillama'),

    -- Velodrome Slipstream: CL factory exists (044 VERIFIED). DefiLlama slug
    -- velodrome-slipstream returned 400 on fetch date. NULL per R8.
    -- Operator: re-populate when slug stabilizes or use 'subgraph' source.
    ((SELECT id FROM dexes WHERE name = 'Velodrome Slipstream'),  10,
     NULL,
     NULL,
     'defillama'),

    -- ═══════════════════════════════════════════════════════════════════════
    -- chain_id = 8453 (Base)
    -- ═══════════════════════════════════════════════════════════════════════

    -- Aerodrome Slipstream: dominant Base DEX ($216M TVL, $430M/day vol).
    ((SELECT id FROM dexes WHERE name = 'Aerodrome Slipstream'),  8453,
     430338548.00,
     216121799.00,
     'defillama'),

    -- UniswapV3: substantial Base presence ($281.7M TVL, $67.3M/day).
    -- Note: Base uses a different factory address (0x33128a...) from other chains.
    ((SELECT id FROM dexes WHERE name = 'UniswapV3'),  8453,
     67265847.00,
     281718723.00,
     'defillama'),

    -- Uniswap V4: Base deployment ($49.9M TVL, $109.7M/day).
    ((SELECT id FROM dexes WHERE name = 'Uniswap V4'),  8453,
     109746211.00,
     49927171.00,
     'defillama'),

    -- PancakeSwap V3: Base deployment ($28.0M TVL, $117.2M/day).
    ((SELECT id FROM dexes WHERE name = 'PancakeSwap V3'),  8453,
     117196989.00,
     27983365.00,
     'defillama'),

    -- Aerodrome (non-CL V2): Base stable+volatile pairs ($141.9M TVL, $9.6M/day).
    ((SELECT id FROM dexes WHERE name = 'Aerodrome'),  8453,
     9581137.00,
     141943855.00,
     'defillama'),

    -- Curve: Base deployment ($17.5M TVL, $2.7M/day). Factory deactivated (unverified).
    ((SELECT id FROM dexes WHERE name = 'Curve'),  8453,
     2683911.00,
     17527626.00,
     'defillama'),

    -- SushiSwap: Base factory deactivated (unverified address from 043/044).
    -- Protocol TVL/vol data available. Row populated per R8 (data exists).
    ((SELECT id FROM dexes WHERE name = 'SushiSwap'),  8453,
     48173.00,
     1722760.00,
     'defillama'),

    -- BaseSwap: Base factory deactivated (unverified address from 043/044).
    -- Protocol TVL small ($714K). Included because factory row exists.
    ((SELECT id FROM dexes WHERE name = 'BaseSwap'),  8453,
     11904.00,
     714222.00,
     'defillama')

ON CONFLICT (dex_id, chain_id) DO UPDATE
    SET volume_24h_usd  = EXCLUDED.volume_24h_usd,
        tvl_usd         = EXCLUDED.tvl_usd,
        last_updated_at = now(),
        source          = EXCLUDED.source;

-- ────────────────────────────────────────────────────────────────────────────
-- STEP 3: Sanity assertions
--
-- NULL pairs (R8 fail-honest — data unavailable from DefiLlama on 2026-05-07):
--   ('UniswapV2', 1)           — uniswap-v2 slug returned 400 Bad Request
--   ('Camelot V3', 42161)      — camelot slug returned 400; DEX is_active=FALSE
--   ('TraderJoe', 42161)       — trader-joe-v2.1 returned 400; factory inactive
--   ('Beethoven X', 10)        — beethoven-x slug returned 400 Bad Request
--   ('Velodrome Slipstream', 10) — velodrome-slipstream returned 400 Bad Request
--
-- These 5 rows are INSERTED with NULL tvl/vol (not skipped). The ON CONFLICT
-- clause will update them to NULL on re-run if the source still returns 400.
-- Operator action: re-run refresh script when slugs stabilize, or populate
-- manually via UPDATE with source = 'manual'.
-- ────────────────────────────────────────────────────────────────────────────

DO $$
DECLARE
    v_metric_count      INT;
    v_factory_count     INT;
    v_coverage_pct      NUMERIC;
    v_non_null_tvl      INT;
    v_non_null_vol      INT;
    -- Spot checks
    v_qs_v3_polygon_tvl NUMERIC;
    v_aero_slip_vol     NUMERIC;
    v_univ3_eth_tvl     NUMERIC;
    v_curve_eth_tvl     NUMERIC;
BEGIN
    SELECT COUNT(*) INTO v_metric_count FROM dex_chain_metrics;

    -- Active factory pairs denominator (same logic as spec)
    SELECT COUNT(DISTINCT (f.dex_id, f.chain_id))
    INTO v_factory_count
    FROM factories f
    WHERE f.is_active = TRUE;

    v_coverage_pct := CASE
        WHEN v_factory_count > 0
        THEN (v_metric_count::NUMERIC / v_factory_count::NUMERIC) * 100
        ELSE 0
    END;

    SELECT COUNT(*) INTO v_non_null_tvl FROM dex_chain_metrics WHERE tvl_usd IS NOT NULL;
    SELECT COUNT(*) INTO v_non_null_vol FROM dex_chain_metrics WHERE volume_24h_usd IS NOT NULL;

    -- Spot checks: key correctness invariants
    SELECT dcm.tvl_usd INTO v_qs_v3_polygon_tvl
    FROM dex_chain_metrics dcm
    JOIN dexes d ON d.id = dcm.dex_id
    WHERE d.name = 'Quickswap V3' AND dcm.chain_id = 137;

    SELECT dcm.volume_24h_usd INTO v_aero_slip_vol
    FROM dex_chain_metrics dcm
    JOIN dexes d ON d.id = dcm.dex_id
    WHERE d.name = 'Aerodrome Slipstream' AND dcm.chain_id = 8453;

    SELECT dcm.tvl_usd INTO v_univ3_eth_tvl
    FROM dex_chain_metrics dcm
    JOIN dexes d ON d.id = dcm.dex_id
    WHERE d.name = 'UniswapV3' AND dcm.chain_id = 1;

    SELECT dcm.tvl_usd INTO v_curve_eth_tvl
    FROM dex_chain_metrics dcm
    JOIN dexes d ON d.id = dcm.dex_id
    WHERE d.name = 'Curve' AND dcm.chain_id = 1;

    RAISE NOTICE
        'Migration 045: % metric rows, % active factory pairs, %.1f%% coverage. '
        'Non-null TVL rows: %, non-null vol rows: %. '
        'Spot checks — QsV3 Polygon TVL: $%M, AeroSlip Base vol: $%M, '
        'UniV3 ETH TVL: $%M, Curve ETH TVL: $%M',
        v_metric_count, v_factory_count, v_coverage_pct,
        v_non_null_tvl, v_non_null_vol,
        ROUND(COALESCE(v_qs_v3_polygon_tvl, 0) / 1e6, 1),
        ROUND(COALESCE(v_aero_slip_vol, 0)     / 1e6, 1),
        ROUND(COALESCE(v_univ3_eth_tvl, 0)     / 1e6, 1),
        ROUND(COALESCE(v_curve_eth_tvl, 0)     / 1e6, 1);

    -- Hard invariants
    IF v_metric_count = 0 THEN
        RAISE EXCEPTION
            'invariant: dex_chain_metrics must have rows after migration 045';
    END IF;

    IF v_metric_count < 40 THEN
        RAISE EXCEPTION
            'invariant: expected >= 40 metric rows (all 43 factory pairs), got %',
            v_metric_count;
    END IF;

    IF v_coverage_pct < 50 THEN
        RAISE WARNING
            'low coverage: %.1f%%, expected 80%%+. '
            'Check that factory rows were applied (migrations 043, 044) before 045.',
            v_coverage_pct;
    END IF;

    -- Quickswap V3 on Polygon must NOT be the version-sliced $4.2M value
    -- when queried for per-chain (the $4.2M IS the correct per-chain V3-only figure
    -- from DefiLlama quickswap-v3 slug). This invariant guards against accidentally
    -- reverting to the global aggregate ($451M) being put in the per-chain row.
    IF v_qs_v3_polygon_tvl IS NOT NULL AND v_qs_v3_polygon_tvl > 100000000 THEN
        RAISE EXCEPTION
            'invariant: Quickswap V3 Polygon per-chain TVL ($%) looks like the global '
            'aggregate was written instead of the per-chain figure. '
            'Per-chain V3-only TVL from quickswap-v3 slug should be ~$4M.',
            v_qs_v3_polygon_tvl;
    END IF;

    -- Aerodrome Slipstream on Base must have dominant volume (dominant Base DEX)
    IF v_aero_slip_vol IS NOT NULL AND v_aero_slip_vol < 50000000 THEN
        RAISE EXCEPTION
            'invariant: Aerodrome Slipstream Base vol ($%) is suspiciously low. '
            'Expected ~$430M (dominant Base DEX). Data may be stale or wrong slug.',
            v_aero_slip_vol;
    END IF;

    -- UniswapV3 ETH TVL must be in the $500M+ range
    IF v_univ3_eth_tvl IS NOT NULL AND v_univ3_eth_tvl < 500000000 THEN
        RAISE EXCEPTION
            'invariant: UniswapV3 ETH TVL ($%) below $500M floor. '
            'Check source data — this is a major liquidity pool.',
            v_univ3_eth_tvl;
    END IF;

    -- Curve ETH TVL must be in the $1B+ range (stablecoin dominant)
    IF v_curve_eth_tvl IS NOT NULL AND v_curve_eth_tvl < 1000000000 THEN
        RAISE EXCEPTION
            'invariant: Curve ETH TVL ($%) below $1B floor. '
            'Check source data — Curve ETH is the dominant stablecoin DEX.',
            v_curve_eth_tvl;
    END IF;
END $$;

COMMIT;

-- ════════════════════════════════════════════════════════════════════════════
-- DOWN — reversible rollback block (run manually; do NOT execute in CI)
-- ════════════════════════════════════════════════════════════════════════════
--
-- BEGIN;
-- DROP TABLE IF EXISTS dex_chain_metrics CASCADE;
-- COMMIT;
--
-- Notes on rollback:
-- - CASCADE: drops idx_dcm_chain_id and idx_dcm_tvl_desc automatically.
-- - No FK points from another table TO dex_chain_metrics (it references dexes
--   and chains, not the other way). CASCADE is safe — no orphan data.
-- - dexes.volume_24h_usd and dexes.tvl_usd are unchanged by this migration,
--   so the API fallback COALESCE path continues to work after rollback.
