// frontend/components/cex/__tests__/route-series.test.ts
//
// WO-PRICE-EXCHANGE-V1 — in-memory route series store (the local feed behind
// the card's Sparkline until the backend price_history mirror exists).
// Memory discipline: 20-point cap per route, 500-route cap, consecutive
// duplicate values deduped (the 1s age-ticker must not flood the series).
import { describe, expect, it } from "vitest";

import {
  getRouteSeries,
  MAX_SERIES_POINTS,
  pushRoutePoint,
} from "../route-series";

describe("pushRoutePoint / getRouteSeries — local route stream", () => {
  it("appends values per route key (dex_a + pair grouping)", () => {
    pushRoutePoint("uniswap-v2·WETH/USDC", 1.5);
    pushRoutePoint("uniswap-v2·WETH/USDC", 1.8);
    pushRoutePoint("sushiswap·WETH/USDC", 2.0); // different route, own series
    expect(getRouteSeries("uniswap-v2·WETH/USDC")).toEqual([1.5, 1.8]);
    expect(getRouteSeries("sushiswap·WETH/USDC")).toEqual([2.0]);
  });

  it("consecutive IDENTICAL values are deduped — only real feed changes recorded", () => {
    pushRoutePoint("r-dup", 5);
    pushRoutePoint("r-dup", 5);
    pushRoutePoint("r-dup", 5);
    pushRoutePoint("r-dup", 6);
    pushRoutePoint("r-dup", 6);
    expect(getRouteSeries("r-dup")).toEqual([5, 6]);
  });

  it("caps the series at the 20 most recent points (LRU of the oldest)", () => {
    for (let i = 0; i < MAX_SERIES_POINTS + 10; i++) {
      pushRoutePoint("r-cap", i);
    }
    const s = getRouteSeries("r-cap");
    expect(s).toHaveLength(MAX_SERIES_POINTS);
    expect(s[0]).toBe(10); // oldest trimmed, newest kept
    expect(s[s.length - 1]).toBe(MAX_SERIES_POINTS + 9);
  });

  it("returns a defensive copy — callers cannot mutate the live store", () => {
    pushRoutePoint("r-copy", 1);
    pushRoutePoint("r-copy", 2);
    const copy = getRouteSeries("r-copy");
    copy.push(999);
    expect(getRouteSeries("r-copy")).toEqual([1, 2]);
  });

  it("non-finite values are rejected (R8 — nothing fabricated)", () => {
    pushRoutePoint("r-nan", NaN);
    pushRoutePoint("r-nan", Infinity);
    expect(getRouteSeries("r-nan")).toEqual([]);
  });

  it("unknown route → empty series (honest absence)", () => {
    expect(getRouteSeries("never-seen")).toEqual([]);
  });
});
