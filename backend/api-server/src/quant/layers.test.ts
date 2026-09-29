/**
 * QUANT LAYERS — tests del álgebra del libro, con los MISMOS números que las
 * hojas: F_e, w = −LN(F_e), Σw, bound, sizing, escalera de costes y gate de 2 %.
 */
import { describe, expect, it } from "vitest";
import {
  DEFAULT_QUANT_CONFIG,
  buildDashboard,
  buildLegs,
  buildPnl,
  buildRoutes,
  fairByPair,
  legBoundUsd,
  median,
  pairKey,
  type LegInput,
} from "../quant/layers.js";

const cfg = DEFAULT_QUANT_CONFIG;

const leg = (over: Partial<LegInput>): LegInput => ({
  poolAddress: "0xpool",
  dex: "UniswapV2",
  poolType: "V2",
  tokenIn: "0xAAA",
  tokenOut: "0xBBB",
  amountIn: 1000,
  amountOut: 1001,
  depthUsd: null,
  liquidity: null,
  sqrtPriceX96: null,
  priceInUsd: null,
  ...over,
});

describe("quant layers · 05_EDGES", () => {
  it("F_e = spot/fair y w = −LN(F_e): un spot 1.001× el fair da Σw < 0 (señal)", () => {
    const fair = new Map([[pairKey("0xAAA", "0xBBB"), 1.0]]);
    const [v] = buildLegs([leg({ amountIn: 1000, amountOut: 1001 })], fair, cfg);
    expect(v.spot).toBeCloseTo(1.001, 12);
    expect(v.fair).toBeCloseTo(1.0, 12);
    expect(v.factor).toBeCloseTo(1.001, 12);
    expect(v.weight).toBeCloseTo(-Math.log(1.001), 12);
    expect(v.weight).toBeLessThan(0);
    expect(v.feeIncludedInSpot).toBe(true);
  });

  it("sin fair del par ⇒ factor y peso NO computados (NaN), nunca 0", () => {
    const [v] = buildLegs([leg({})], new Map(), cfg);
    expect(Number.isNaN(v.factor)).toBe(true);
    expect(Number.isNaN(v.weight)).toBe(true);
  });

  it("mediana del cross-section: par con 3 pools toma el valor central", () => {
    const fair = fairByPair([
      { tokenIn: "0xA", tokenOut: "0xB", spot: 1.0 },
      { tokenIn: "0xA", tokenOut: "0xB", spot: 1.002 },
      { tokenIn: "0xA", tokenOut: "0xB", spot: 0.998 },
    ]);
    expect(fair.get(pairKey("0xA", "0xB"))).toBeCloseTo(1.0, 12);
    expect(median([3, 1, 2])).toBe(2);
  });
});

describe("quant layers · bound por pata", () => {
  it("V2 con profundidad medida: bound = slippage × profundidad", () => {
    const r = legBoundUsd(leg({ depthUsd: 100_000 }), cfg);
    expect(r.reason).toBeNull();
    expect(r.boundUsd).toBeCloseTo((50 / 10_000) * 100_000, 9); // = $500
  });

  it("V3 con L y √P: bound = s·L·√P/2 · precio", () => {
    const sqrtP = 1 * 2 ** 96; // √P = 1
    const r = legBoundUsd(
      leg({ poolType: "V3", liquidity: 1_000_000, sqrtPriceX96: sqrtP, priceInUsd: 1 }),
      cfg,
    );
    expect(r.reason).toBeNull();
    // (0.005 · 1e6 · 1)/2 = 2500
    expect(r.boundUsd).toBeCloseTo(2500, 6);
  });

  it("sin profundidad ni estado V3 ⇒ no computado con razón (R8)", () => {
    const r = legBoundUsd(leg({}), cfg);
    expect(r.boundUsd).toBeNull();
    expect(r.reason).toBe("route_depth_not_on_wire");
  });
});

describe("quant layers · 06_ROUTES", () => {
  it("Σw<0 + bound suficiente ⇒ ruta viable y sizing acotado por capital y utilización", () => {
    const fair = new Map([
      [pairKey("0xAAA", "0xBBB"), 1.0],
      [pairKey("0xBBB", "0xAAA"), 1.0],
    ]);
    const legs = buildLegs(
      [
        leg({ poolAddress: "0xP1", amountIn: 1000, amountOut: 1001, depthUsd: 100_000 }),
        leg({ poolAddress: "0xP2", tokenIn: "0xBBB", tokenOut: "0xAAA", amountIn: 1001, amountOut: 1005, depthUsd: 50_000 }),
      ],
      fair,
      cfg,
    );
    const route = buildRoutes("r1", legs, cfg);
    expect(route.signal).toBe(true);
    expect(route.bindingBoundUsd).toBeCloseTo(0.005 * 50_000, 6); // la pata débil manda
    expect(route.sizingUsd).toBeCloseTo(Math.min(cfg.capitalUsd, route.bindingBoundUsd! * cfg.utilizationCap), 9);
    expect(route.usable).toBe(true);
  });

  it("Σw ≥ 0 ⇒ NO viable con la razón exacta", () => {
    const fair = new Map([[pairKey("0xAAA", "0xBBB"), 1.0]]);
    const legs = buildLegs([leg({ amountIn: 1000, amountOut: 999 })], fair, cfg);
    const route = buildRoutes("r2", legs, cfg);
    expect(route.signal).toBe(false);
    expect(route.usable).toBe(false);
    expect(route.whyNot).toBe("sin señal (Σw ≥ 0)");
  });
});

describe("quant layers · 08_ROUTE_PNL", () => {
  it("escalera completa: gross = final − principal ; net = gross − (gas+flash+tip+haircut+slippage)", () => {
    const fair = new Map([
      [pairKey("0xAAA", "0xBBB"), 1.0],
      [pairKey("0xBBB", "0xAAA"), 1.0],
    ]);
    const legs = buildLegs(
      [
        leg({ amountIn: 1000, amountOut: 1010, depthUsd: 1_000_000 }),
        leg({ tokenIn: "0xBBB", tokenOut: "0xAAA", amountIn: 1010, amountOut: 1020, depthUsd: 1_000_000 }),
      ],
      fair,
      cfg,
    );
    const route = buildRoutes("r3", legs, cfg);
    const pnl = buildPnl(route, { finalUsd: 1020, principalUsd: 1000 }, cfg);
    expect(pnl.grossUsd).toBeCloseTo(20, 9);
    expect(pnl.grossBps).toBeCloseTo(200, 6);
    expect(pnl.gasUsd).toBeCloseTo(cfg.gasBaseUsd + 2 * cfg.gasPerHopUsd, 9);
    expect(pnl.flashUsd).toBeCloseTo((1000 * cfg.flashBps) / 10_000, 9);
    expect(pnl.totalCostUsd).toBeCloseTo(pnl.gasUsd + pnl.flashUsd + pnl.tipUsd + pnl.haircutUsd + pnl.slippageUsd, 9);
    expect(pnl.netUsd).toBeCloseTo(pnl.grossUsd - pnl.totalCostUsd, 9);
    expect(pnl.netBps).toBeCloseTo((pnl.netUsd / 1000) * 10_000, 6);
    // 20 USD de gross contra ~14 de gas de 2 patas + variables ⇒ margen real
    expect(pnl.verdict).toBe("MARGINAL");
  });

  it("sin cadena medida NO se publica veredicto (R8), y se dice por qué", () => {
    const route = buildRoutes("r4", [], cfg);
    const pnl = buildPnl(route, { finalUsd: null, principalUsd: null }, cfg);
    expect(pnl.verdict).toBe("NO VIABLE");
    expect(pnl.whyNot).toBe("sin patas");
  });

  it("gate de 2 %: un gross enorme pero por debajo del target da MARGINAL, no EJECUTAR", () => {
    const fair = new Map([[pairKey("0xAAA", "0xBBB"), 1.0]]);
    const legs = buildLegs([leg({ amountIn: 1000, amountOut: 1010, depthUsd: 1e9 })], fair, cfg);
    const route = buildRoutes("r5", legs, cfg);
    const pnl = buildPnl(route, { finalUsd: 1016, principalUsd: 1000 }, cfg); // 160 bps
    expect(pnl.netBps).toBeLessThan(cfg.targetNetBps);
    expect(pnl.verdict).toBe("MARGINAL");
  });
});

describe("quant layers · 09_DASHBOARD", () => {
  it("embudo por hops + mejor ruta por net_bps", () => {
    const fair = new Map([
      [pairKey("0xAAA", "0xBBB"), 1.0],
      [pairKey("0xB", "0xA"), 1.0],
    ]);
    const mk = (key: string, out: number, hops: number) => {
      const chain =
        hops === 2
          ? [leg({ amountIn: 1000, amountOut: out, depthUsd: 1e6 }),
             leg({ tokenIn: "0xB", tokenOut: "0xA", amountIn: out, amountOut: out * 1.001, depthUsd: 1e6 })]
          : [leg({ amountIn: 1000, amountOut: out, depthUsd: 1e6 })];
      const r = buildRoutes(key, buildLegs(chain, fair, cfg), cfg);
      return { r, p: buildPnl(r, { finalUsd: out * 1.001, principalUsd: 1000 }, cfg) };
    };
    const a = mk("ra", 1010, 2);
    const b = mk("rb", 1005, 2);
    const d = buildDashboard([a.r, b.r], [a.p, b.p]);
    expect(d.routes).toBe(2);
    expect(d.edges).toBe(4); // dos rutas de 2 patas
    expect(d.pools).toBe(1); // el helper de estas fixtures usa el mismo pool
    expect(d.bestRouteKey).not.toBeNull();
    expect(d.byHops.map((x) => x.hops)).toEqual([2]);
    expect(d.top.length).toBe(2);
    expect(d.top[0].netBps).toBeGreaterThanOrEqual(d.top[1].netBps);
  });
});
