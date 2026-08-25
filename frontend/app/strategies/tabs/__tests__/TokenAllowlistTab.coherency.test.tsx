// frontend/app/strategies/tabs/__tests__/TokenAllowlistTab.coherency.test.tsx
//
// FE-MASTER · FE-0016 — §11/§3 save coherencia over the EMIT-04 wire.
//
// UniverseSaveCoherency is the pure wrapper the allowlist surface renders
// after a save: configured = the persisted count, effective = the live
// universe KPIs, version = PUT-stamped universe_version vs served one, ack =
// the runtime_ack event_id bijection. SSR renders its steady states (the
// ack-lifecycle transitions live in runtimeSettingState tests of 7b).
//
// Also pins the FE-0016 drift fix: the PUT result schema parses the REAL
// backend answer (dual-channel subscriber counts + universe versioning) and
// REJECTS the old `subscribers_notified` key the backend never sends.
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { UniverseSaveCoherency } from "../TokenAllowlistTab";
import { TradingConfigPutResultSchema } from "@/lib/schemas";
import type { TokenUniverseKpi } from "@/lib/apex/schemas";

const UNIVERSE: TokenUniverseKpi = {
  allowed_tokens: 3,
  possible_pairs: 3,
  directed_token_pairs: 6,
  active_pools: 9,
  active_venues: 4,
  graph_version: 42,
  universe_version: 5,
};

function render(props: {
  putVersion: number;
  universe?: TokenUniverseKpi | null;
  symbolsCount?: number;
}) {
  return renderToStaticMarkup(
    React.createElement(UniverseSaveCoherency, {
      putVersion: props.putVersion,
      ackEventId: null, // steady state — the ack lifecycle is 7b's tested lane
      symbolsCount: props.symbolsCount ?? 3,
      universe: props.universe === undefined ? UNIVERSE : props.universe,
      onAckApplied: undefined,
    }),
  );
}

describe("UniverseSaveCoherency — steady states (FE-0016 · §11)", () => {
  it("versions agree and counts agree → EFFECTIVE with the version pair visible", () => {
    const html = render({ putVersion: 5 });
    expect(html).toContain("token universe");
    expect(html).toContain("EFFECTIVE");
    expect(html).toContain("v5→5");
  });

  it("served universe_version behind the PUT-stamped one → DRIFT (§47)", () => {
    const html = render({ putVersion: 5, universe: { ...UNIVERSE, universe_version: 4 } });
    expect(html).toContain("DRIFT");
    expect(html).toContain("v5→4");
  });

  it("universe not served yet → NOT EXPOSED (R8 dash, never zero)", () => {
    const html = render({ putVersion: 5, universe: null });
    expect(html).toContain("NOT EXPOSED");
    expect(html).toContain("—");
  });

  it("configured/effective carry the counts the operator can cross-check", () => {
    const html = render({ putVersion: 5, symbolsCount: 3 });
    // The line shows configured 3 → effective 3 (both rendered as chips).
    expect(html.match(/>3</g)?.length).toBeGreaterThanOrEqual(2);
  });

  it("the §3 lie is gone — no 'scanner sees' promise anywhere in the line", () => {
    const html = render({ putVersion: 5 });
    expect(html).not.toContain("scanner sees");
    expect(html).not.toContain("≤1s");
  });
});

describe("TradingConfigPutResultSchema — the REAL PUT answer (FE-0016 drift fix)", () => {
  // trading-config.ts:803-812: ok + dual-channel counts + EMIT-04 versioning
  // + the rowToRedisState spread (base fields).
  const REAL_PUT = {
    ok: true,
    chain_id: 1,
    subscribers_trading_config: 2,
    subscribers_hot_reload: 1,
    universe_version: 5,
    runtime_ack_event_id: "b0c1d2e3-4f5a-6789-abcd-ef0123456789",
    channels: ["arbx:trading_config", "arbx:hot_reload"],
    capital_usd: 1000,
    base_token_symbol: "WETH",
    base_token_price_usd: 3000,
    allowed_token_symbols: ["WETH", "USDC", "USDT"],
    token_prices_usd: {},
    min_profit_usd: 0,
    min_roi_pct: 0,
    min_landing_probability: 0,
    min_liquidity_confidence: 0,
    max_token_risk_score: 1,
    gas_price_strategy: "fixed",
    fixed_gas_price_gwei: 20,
    gas_estimate_units: 300000,
    max_slippage_pct: 1,
    failure_risk_buffer_pct: 10,
    flashloan_fee_pct: 0.05,
    enabled_strategies: [],
    strategy_configs: {},
    enabled: true,
    updated_at: "2026-08-24T00:00:00.000Z",
    updated_by: "operator",
  };

  it("parses the backend's real answer (unknown `channels` key strips, per repo Zod)", () => {
    const result = TradingConfigPutResultSchema.safeParse(REAL_PUT);
    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.universe_version).toBe(5);
      expect(result.data.runtime_ack_event_id).toBe("b0c1d2e3-4f5a-6789-abcd-ef0123456789");
    }
  });

  it("a knobs-only edit parses with null versioning (no bump, no ACK row)", () => {
    const result = TradingConfigPutResultSchema.safeParse({
      ...REAL_PUT,
      universe_version: null,
      runtime_ack_event_id: null,
    });
    expect(result.success).toBe(true);
  });

  it("REJECTS the old `subscribers_notified` shape — the backend never sends it", () => {
    const { subscribers_trading_config: _t, subscribers_hot_reload: _h, ...old } = REAL_PUT;
    void _t; void _h;
    const result = TradingConfigPutResultSchema.safeParse({
      ...old,
      subscribers_notified: 2,
    });
    expect(result.success).toBe(false);
  });
});
