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
  fairBasisCounts,
  fairByPair,
  fairFromUsdPrices,
  legBoundUsd,
  median,
  mergeFair,
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
    const fair = new Map([[pairKey("0xAAA", "0xBBB"), { rate: 1.0, basis: "cross_section_median" as const }]]);
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
    expect(fair.get(pairKey("0xA", "0xB"))?.rate).toBeCloseTo(1.0, 12);
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
      [pairKey("0xAAA", "0xBBB"), { rate: 1.0, basis: "cross_section_median" as const }],
      [pairKey("0xBBB", "0xAAA"), { rate: 1.0, basis: "cross_section_median" as const }],
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
    const fair = new Map([[pairKey("0xAAA", "0xBBB"), { rate: 1.0, basis: "cross_section_median" as const }]]);
    const legs = buildLegs([leg({ amountIn: 1000, amountOut: 999 })], fair, cfg);
    const route = buildRoutes("r2", legs, cfg);
    expect(route.signal).toBe(false);
    expect(route.usable).toBe(false);
    expect(route.whyNot).toBe("sin señal (Σw ≥ 0)");
  });
});

describe("quant layers · 08_ROUTE_PNL", () => {
  it("escalera completa: gross = final − principal ; net = gross − (gas+flash+tip+haircut)", () => {
    const fair = new Map([
      [pairKey("0xAAA", "0xBBB"), { rate: 1.0, basis: "cross_section_median" as const }],
      [pairKey("0xBBB", "0xAAA"), { rate: 1.0, basis: "cross_section_median" as const }],
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
    // QUANT-PNL-01: la escalera de costes NO incluye la desviación contra fair
    // (ya vive dentro del gross medido).
    expect(pnl.totalCostUsd).toBeCloseTo(pnl.gasUsd + pnl.flashUsd + pnl.tipUsd + pnl.haircutUsd, 9);
    expect(pnl.netUsd).toBeCloseTo(pnl.grossUsd - pnl.totalCostUsd, 9);
    expect(pnl.netBps).toBeCloseTo((pnl.netUsd / 1000) * 10_000, 6);
    // 20 USD de gross contra ~14 de gas de 2 patas + variables ⇒ margen real
    expect(pnl.verdict).toBe("MARGINAL");
  });

  it("QUANT-PNL-01: la pérdida medida NO se resta dos veces (regresión del doble conteo)", () => {
    // Fila real del 2026-09-29 (WETH→USDC vía UniV2+Sushi): principal $718.4076,
    // gross −$7.9391. Con fair ≈ 1 el ciclo cierra en ≈ principal, así que la
    // desviación ≈ la pérdida misma ($7.9391) y el doble conteo habría publicado
    // −$31.8897 en vez de −$23.9506.
    const fair = new Map([
      [pairKey("0xAAA", "0xBBB"), { rate: 1.0, basis: "cross_section_median" as const }],
      [pairKey("0xBBB", "0xAAA"), { rate: 1.0, basis: "cross_section_median" as const }],
    ]);
    const legs = buildLegs(
      [
        leg({ amountIn: 1000, amountOut: 990, depthUsd: 1_000_000 }),
        leg({ tokenIn: "0xBBB", tokenOut: "0xAAA", amountIn: 990, amountOut: 980, depthUsd: 1_000_000 }),
      ],
      fair,
      cfg,
    );
    const route = buildRoutes("r3b", legs, cfg);
    const pnl = buildPnl(route, { finalUsd: 980, principalUsd: 1000 }, cfg);

    expect(pnl.grossUsd).toBeCloseTo(-20, 9);
    // La desviación contra fair es exactamente la pérdida: por eso NO puede ser
    // una línea de coste (sería contarla dos veces).
    expect(pnl.fairChainUsd).toBeCloseTo(1000, 9);
    expect(pnl.deviationVsFairUsd).toBeCloseTo(20, 9);
    const ladder = pnl.gasUsd + pnl.flashUsd + pnl.tipUsd + pnl.haircutUsd;
    expect(pnl.totalCostUsd).toBeCloseTo(ladder, 9);
    expect(pnl.netUsd).toBeCloseTo(-20 - ladder, 9);
    // El error que este test bloquea: net = gross − escalera − desviación.
    expect(pnl.netUsd).toBeGreaterThan(-20 - ladder - pnl.deviationVsFairUsd + 1e-9);
  });

  it("sin cadena medida NO se publica veredicto (R8), y se dice por qué", () => {
    const route = buildRoutes("r4", [], cfg);
    const pnl = buildPnl(route, { finalUsd: null, principalUsd: null }, cfg);
    expect(pnl.verdict).toBe("NO VIABLE");
    expect(pnl.whyNot).toBe("sin patas");
  });

  it("gate de 2 %: un gross enorme pero por debajo del target da MARGINAL, no EJECUTAR", () => {
    const fair = new Map([[pairKey("0xAAA", "0xBBB"), { rate: 1.0, basis: "cross_section_median" as const }]]);
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
      [pairKey("0xAAA", "0xBBB"), { rate: 1.0, basis: "cross_section_median" as const }],
      [pairKey("0xB", "0xA"), { rate: 1.0, basis: "cross_section_median" as const }],
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

describe("QUANT-FAIR-01 · el fair viene del oráculo, no de la propia medición", () => {
  it("fair = price_usd(in)/price_usd(out) y queda marcado como oráculo", () => {
    const prices = new Map([
      ["0xweth", 2666.83],
      ["0xusdc", 1.0001],
    ]);
    const fare = fairFromUsdPrices(
      [{ tokenIn: "0xWETH", tokenOut: "0xUSDC" }],
      prices,
    );
    expect(fare.get(pairKey("0xWETH", "0xUSDC"))?.basis).toBe("oracle_usd");
    expect(fare.get(pairKey("0xWETH", "0xUSDC"))?.rate).toBeCloseTo(2666.83 / 1.0001, 6);
  });

  it("sin precio de un token el par NO entra (nunca se inventa una tasa)", () => {
    const prices = new Map([["0xweth", 2666.83]]);
    const fare = fairFromUsdPrices([{ tokenIn: "0xWETH", tokenOut: "0xPEPE" }], prices);
    expect(fare.size).toBe(0);
  });

  it("el oráculo manda y la mediana sólo rellena huecos, conservando la procedencia", () => {
    const oracle = fairFromUsdPrices(
      [{ tokenIn: "0xA", tokenOut: "0xB" }],
      new Map([
        ["0xa", 2],
        ["0xb", 1],
      ]),
    );
    const fallback = fairByPair([
      { tokenIn: "0xA", tokenOut: "0xB", spot: 99 },
      { tokenIn: "0xC", tokenOut: "0xD", spot: 5 },
    ]);
    const merged = mergeFair(oracle, fallback);
    expect(merged.get(pairKey("0xA", "0xB"))?.rate).toBeCloseTo(2, 9);
    expect(merged.get(pairKey("0xA", "0xB"))?.basis).toBe("oracle_usd");
    expect(merged.get(pairKey("0xC", "0xD"))?.basis).toBe("cross_section_median");
  });

  it("con oráculo la señal EXISTE: F_e ≠ 1 cuando el pool se desvía del precio de mercado", () => {
    const prices = new Map([
      ["0xweth", 2666.83],
      ["0xusdc", 1.0],
    ]);
    const fare = fairFromUsdPrices([{ tokenIn: "0xWETH", tokenOut: "0xUSDC" }], prices);
    // El pool paga 2700 USDC por WETH: 1.24% por encima del precio de mercado.
    const legs = buildLegs(
      [leg({ tokenIn: "0xWETH", tokenOut: "0xUSDC", amountIn: 1, amountOut: 2700, depthUsd: 1e6 })],
      fare,
      cfg,
    );
    expect(legs[0]?.fairBasis).toBe("oracle_usd");
    expect(legs[0]?.factor).toBeGreaterThan(1.01);
    expect(legs[0]?.weight).toBeLessThan(0);
  });

  it("sin oráculo ni mediana, la arista se declara sin basis (no computada)", () => {
    const legs = buildLegs([leg({ tokenIn: "0xX", tokenOut: "0xY" })], new Map(), cfg);
    expect(legs[0]?.fairBasis).toBeNull();
    expect(Number.isNaN(legs[0]?.factor)).toBe(true);
  });

  it("fairBasisCounts cuenta cada procedencia, incluida la ausencia", () => {
    const legs = buildLegs(
      [
        leg({ tokenIn: "0xA", tokenOut: "0xB" }),
        leg({ tokenIn: "0xC", tokenOut: "0xD" }),
        leg({ tokenIn: "0xE", tokenOut: "0xF" }),
      ],
      new Map([
        [pairKey("0xA", "0xB"), { rate: 1, basis: "oracle_usd" as const }],
        [pairKey("0xC", "0xD"), { rate: 1, basis: "cross_section_median" as const }],
      ]),
      cfg,
    );
    expect(fairBasisCounts(legs)).toEqual({ oracle_usd: 1, cross_section_median: 1, none: 1 });
  });
});

describe("QUANT-SIZING-NULL-01 · sin bound no hay tamaño, y se dice (null), no 0", () => {
  it("sin profundidad en el wire: sizing null y estado no computado", () => {
    const fair = new Map([[pairKey("0xAAA", "0xBBB"), { rate: 1.0, basis: "oracle_usd" as const }]]);
    const legs = buildLegs([leg({ amountIn: 1000, amountOut: 1010 })], fair, cfg); // sin depthUsd
    const route = buildRoutes("r-null", legs, cfg);
    expect(route.bindingBoundUsd).toBeNull();
    expect(route.sizingUsd).toBeNull();
    expect(route.status).toBe("QUOTE_INCOMPLETE");
    expect(route.whyNot).toBe("profundidad no computada");
  });

  it("con profundidad el sizing vuelve a ser un número operable", () => {
    const fair = new Map([[pairKey("0xAAA", "0xBBB"), { rate: 1.0, basis: "oracle_usd" as const }]]);
    const legs = buildLegs([leg({ amountIn: 1000, amountOut: 1010, depthUsd: 1e6 })], fair, cfg);
    const route = buildRoutes("r-num", legs, cfg);
    expect(route.sizingUsd).toBeCloseTo(Math.min(cfg.capitalUsd, 0.005 * 1e6 * cfg.utilizationCap), 9);
  });
});
