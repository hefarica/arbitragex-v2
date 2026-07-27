// =============================================================================
// DEV-ONLY DESIGN-PREVIEW FIXTURES — REMOVE BEFORE DEPLOY
// =============================================================================
//
// SAMPLE opportunities consumed ONLY by useOmniOpportunities' dev auto-seed
// (gated by process.env.NODE_ENV !== "production"). Purpose: let the operator
// evaluate the card-grid layout locally when NO edge backend is reachable
// (the common case on the Windows dev box — the edge runs on the VPS only).
//
// These are NOT real data. Every fixture carries trace_id="preview" so the UI
// (OpportunitiesClient) badges them with a visible "DESIGN PREVIEW" banner —
// the operator never confuses them with live detections. The auto-seed fires
// ONLY when the SSR snapshot is empty AND the store is still empty ~2.5s after
// mount (i.e. the poll path genuinely has no data source).
//
// RULE 00 (Zero-Mocks): this file is design-QA dev tooling, NOT pipeline data.
// It is loaded via a DEV-GATED DYNAMIC IMPORT, so Next.js excludes it from the
// production bundle (the import site is unreachable dead code in prod builds).
// Delete this file + the auto-seed effect before opening the deploy PR.
//
// Anti-regression note: a vitest guard in omni-store.test.ts pins the snapshot
// merge contract; these fixtures ride on the SAME mergeSnapshot path as live
// data, so they exercise the real store code (no parallel mock pipeline).

import { mapToOmniOpportunity, type OmniOpportunity } from "./types";

/**
 * Builds fresh preview fixtures with detected_at relative to "now" so the age
 * indicator shows realistic values at seed time. Called by the dev auto-seed.
 */
export function buildPreviewOpportunities(): OmniOpportunity[] {
  const iso = (secsAgo: number) => new Date(Date.now() - secsAgo * 1000).toISOString();
  const raw: Record<string, unknown>[] = [
    {
      id: "preview-1",
      trace_id: "preview",
      chain_id: 1,
      strategy_kind: "dex_arb",
      detected_at: iso(3),
      dex_a: "uniswap-v2",
      dex_b: "sushiswap",
      pair_symbol: "WETH/USDC",
      token_in: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
      token_out: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
      token_in_info: { symbol: "WETH", decimals: 18, logo_url: null, resolved_via: "onchain_full", verified: true },
      token_out_info: { symbol: "USDC", decimals: 6, logo_url: null, resolved_via: "onchain_full", verified: true },
      chain_base_token_symbol: "WETH",
      expected_profit_usd: 87.42,
      net_expected_profit_usd: 55.18,
      roi_pct: 2.48,
      risk_score: 0.87,
      status: "detected",
      rejection_reason: null,
      paper_status: "paper_viable",
      chains_used: [1],
      dexes_used: ["uniswap-v2", "sushiswap"],
    },
    {
      id: "preview-2",
      trace_id: "preview",
      chain_id: 1,
      strategy_kind: "dex_arb",
      detected_at: iso(8),
      dex_a: "uniswap-v3",
      dex_b: "balancer-v2",
      pair_symbol: "WBTC/USDT",
      token_in: "0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599",
      token_out: "0xdAC17F958D2ee523a2206206994597C13D831ec7",
      token_in_info: { symbol: "WBTC", decimals: 8, logo_url: null, resolved_via: "onchain_full", verified: true },
      token_out_info: { symbol: "USDT", decimals: 6, logo_url: null, resolved_via: "onchain_full", verified: true },
      chain_base_token_symbol: "WBTC",
      expected_profit_usd: 162.3,
      net_expected_profit_usd: 112.05,
      roi_pct: 1.85,
      risk_score: 0.92,
      status: "detected",
      rejection_reason: null,
      paper_status: "paper_viable",
      chains_used: [1],
      dexes_used: ["uniswap-v3", "balancer-v2"],
    },
    {
      id: "preview-3",
      trace_id: "preview",
      chain_id: 137,
      strategy_kind: "dex_arb",
      detected_at: iso(14),
      dex_a: "quickswap",
      dex_b: "sushiswap",
      pair_symbol: "WMATIC/USDC",
      token_in: "0x0d500B1d8E8eF31E21C99d1Db9A6444d3ADf1270",
      token_out: "0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359",
      token_in_info: { symbol: "WMATIC", decimals: 18, logo_url: null, resolved_via: "onchain_full", verified: true },
      token_out_info: { symbol: "USDC", decimals: 6, logo_url: null, resolved_via: "onchain_full", verified: true },
      chain_base_token_symbol: "MATIC",
      expected_profit_usd: 5.4,
      net_expected_profit_usd: 3.41,
      roi_pct: 0.62,
      risk_score: 0.71,
      status: "detected",
      rejection_reason: null,
      paper_status: "paper_viable",
      chains_used: [137],
      dexes_used: ["quickswap", "sushiswap"],
    },
    {
      id: "preview-4",
      trace_id: "preview",
      chain_id: 42161,
      strategy_kind: "dex_arb",
      detected_at: iso(20),
      dex_a: "camelot-v2",
      dex_b: "uniswap-v3",
      pair_symbol: "WETH/USDC",
      token_in: "0x82aF49447D8a07e3bd95BD0d56f35241523fBab1",
      token_out: "0xaf88d065e77c8cC2239327C5EDb3A432268e5831",
      token_in_info: { symbol: "WETH", decimals: 18, logo_url: null, resolved_via: "onchain_full", verified: true },
      token_out_info: { symbol: "USDC", decimals: 6, logo_url: null, resolved_via: "onchain_full", verified: true },
      chain_base_token_symbol: "WETH",
      expected_profit_usd: 14.5,
      net_expected_profit_usd: 9.2,
      roi_pct: 0.45,
      risk_score: 0.55,
      status: "rejected",
      rejection_reason: "token_meta_unavailable",
      paper_status: "paper_rejected",
      chains_used: [42161],
      dexes_used: ["camelot-v2", "uniswap-v3"],
    },
    {
      id: "preview-5",
      trace_id: "preview",
      chain_id: 1,
      strategy_kind: "dex_arb",
      detected_at: iso(28),
      dex_a: "sushiswap",
      dex_b: "uniswap-v2",
      pair_symbol: "LINK/WETH",
      token_in: "0x514910771AF9Ca656af840dff83E8264EcF986CA",
      token_out: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
      token_in_info: { symbol: "LINK", decimals: 18, logo_url: null, resolved_via: "onchain_full", verified: true },
      token_out_info: { symbol: "WETH", decimals: 18, logo_url: null, resolved_via: "onchain_full", verified: true },
      chain_base_token_symbol: "WETH",
      expected_profit_usd: 31.4,
      net_expected_profit_usd: 18.93,
      roi_pct: 1.12,
      risk_score: 0.96,
      status: "detected",
      rejection_reason: null,
      paper_status: "paper_viable",
      chains_used: [1],
      dexes_used: ["sushiswap", "uniswap-v2"],
    },
    {
      id: "preview-6",
      trace_id: "preview",
      chain_id: 1,
      strategy_kind: "dex_arb",
      detected_at: iso(45),
      dex_a: "uniswap-v2",
      dex_b: "sushiswap",
      pair_symbol: "PEPE/USDC",
      token_in: "0x6982508145454Ce325dDbE47a25d4ec3d2311933",
      token_out: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
      token_in_info: { symbol: "PEPE", decimals: 18, logo_url: null, resolved_via: "onchain_full", verified: true },
      token_out_info: { symbol: "USDC", decimals: 6, logo_url: null, resolved_via: "onchain_full", verified: true },
      chain_base_token_symbol: "WETH",
      expected_profit_usd: null,
      net_expected_profit_usd: null,
      simulated_net_profit_usd: 7.3,
      roi_pct: 0.38,
      risk_score: 0.6,
      status: "detected",
      rejection_reason: null,
      paper_status: null,
      chains_used: [1],
      dexes_used: ["uniswap-v2", "sushiswap"],
    },
  ];
  return raw.map(mapToOmniOpportunity);
}
