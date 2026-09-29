/**
 * Tests de la capa de presentación cuantitativa (frontend).
 *
 * Lo que se protege aquí no es cosmética: es la doctrina R8 en pantalla.
 * Un `null` (no computado) jamás puede convertirse en `$0.00` ni en `NaN`.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  NOT_COMPUTED,
  buildGrid,
  fetchQuantLayers,
  fmtBps,
  fmtNum,
  fmtPct,
  fmtUsd,
  fmtWeight,
  funnelSteps,
  quantLayersPath,
  reasonBreakdown,
  shortAddress,
  shortKey,
  sortRows,
  splitGrid,
  statusVariant,
  verdictVariant,
  windowReadout,
  type QuantDashboard,
  type QuantLayersResponse,
  type QuantPnl,
  type QuantRoute,
} from "../quant-layers";

const route = (over: Partial<QuantRoute> = {}): QuantRoute => ({
  routeKey: "1|dex_arb|WETH|USDC",
  hops: 2,
  legs: [],
  sumW: -0.0012,
  discoveryReturnPct: 0.12,
  signal: true,
  bindingBoundUsd: 1500,
  sizingUsd: 225,
  maxBlockAgeBlocks: null,
  quoteBlock: 20_000_000,
  usable: true,
  whyNot: null,
  status: "SIZED",
  ...over,
});

const pnl = (over: Partial<QuantPnl> = {}): QuantPnl => ({
  routeKey: "1|dex_arb|WETH|USDC",
  hops: 2,
  status: "PASS_NET",
  sizingUsd: 225,
  finalUsd: 230,
  grossUsd: 5,
  grossBps: 222,
  gasUsd: 14,
  flashUsd: 0.11,
  tipUsd: 0.07,
  haircutUsd: 0.45,
  deviationVsFairUsd: 0.5,
  fairChainUsd: 230.5,
  totalCostUsd: 15.13,
  netUsd: -10.13,
  netBps: -450,
  verdict: "RECHAZAR",
  whyNot: "coste > gross",
  ...over,
});

describe("formato honesto: null nunca es cero", () => {
  it("un campo no computado se muestra como raya, no como 0", () => {
    expect(fmtUsd(null)).toBe(NOT_COMPUTED);
    expect(fmtUsd(undefined)).toBe(NOT_COMPUTED);
    expect(fmtBps(null)).toBe(NOT_COMPUTED);
    expect(fmtPct(null)).toBe(NOT_COMPUTED);
    expect(fmtNum(null)).toBe(NOT_COMPUTED);
    expect(fmtWeight(null)).toBe(NOT_COMPUTED);
  });

  it("NaN del motor (wire) se degrada a raya, jamás a 'NaN' en pantalla", () => {
    expect(fmtUsd(Number.NaN)).toBe(NOT_COMPUTED);
    expect(fmtBps(Number.NaN)).toBe(NOT_COMPUTED);
    expect(fmtWeight(Number.NaN)).toBe(NOT_COMPUTED);
  });

  it("un cero REAL sí se muestra: computado y exactamente cero", () => {
    expect(fmtUsd(0)).toBe("$0.00");
    expect(fmtBps(0)).toBe("0.0 bps");
    expect(fmtPct(0)).toBe("0.000%");
  });

  it("conserva el signo de las cifras negativas", () => {
    expect(fmtUsd(-10.125)).toBe("-$10.13");
    expect(fmtBps(-450)).toBe("-450.0 bps");
    expect(fmtUsd(1234.5)).toBe("$1,234.50");
  });

  it("acorta claves y direcciones sin perder identificabilidad", () => {
    expect(shortKey("abc")).toBe("abc");
    expect(shortKey("x".repeat(50)).endsWith("…")).toBe(true);
    expect(shortAddress("0x60594a405d53811d3bc4766596efd80fd545a270")).toBe("0x6059…a270");
  });
});

describe("badges", () => {
  it("el veredicto mapea a color sin ocultar el rechazo", () => {
    expect(verdictVariant("EJECUTAR")).toBe("success");
    expect(verdictVariant("MARGINAL")).toBe("warning");
    expect(verdictVariant("RECHAZAR")).toBe("destructive");
    expect(verdictVariant("NO VIABLE")).toBe("secondary");
  });

  it("la escalera de estados no declara éxito antes de tiempo", () => {
    expect(statusVariant("READY_TO_SIMULATE")).toBe("success");
    expect(statusVariant("NET_BELOW_TARGET")).toBe("warning");
    expect(statusVariant("QUOTE_INCOMPLETE")).toBe("destructive");
  });
});

describe("query del endpoint", () => {
  it("construye la ruta con y sin parámetros", () => {
    expect(quantLayersPath()).toBe("/api/quant/layers");
    expect(quantLayersPath({ windowMinutes: 60, limit: 300, top: 10 })).toBe(
      "/api/quant/layers?window_minutes=60&limit=300&top=10",
    );
  });
});

describe("buildGrid: 06_ROUTES ⨝ 08_ROUTE_PNL", () => {
  it("une por routeKey y ordena por net_bps desc", () => {
    const rows = buildGrid(
      [route({ routeKey: "A" }), route({ routeKey: "B" }), route({ routeKey: "C" })],
      [
        pnl({ routeKey: "A", netBps: -450 }),
        pnl({ routeKey: "B", netBps: 310, verdict: "EJECUTAR", status: "READY_TO_SIMULATE" }),
        pnl({ routeKey: "C", netBps: 12, verdict: "MARGINAL" }),
      ],
    );
    expect(rows.map((r) => r.routeKey)).toEqual(["B", "C", "A"]);
    expect(rows[0]?.verdict).toBe("EJECUTAR");
  });

  it("una ruta sin P&L medido NO se inventa: entra con cifras nulas", () => {
    const rows = buildGrid([route({ routeKey: "SIN_PNL", status: "QUOTE_INCOMPLETE", whyNot: "cadena medida incompleta" })], []);
    expect(rows).toHaveLength(1);
    expect(rows[0]?.netBps).toBeNull();
    expect(rows[0]?.verdict).toBe("NO VIABLE");
    expect(rows[0]?.whyNot).toBe("cadena medida incompleta");
  });

  it("las filas sin cifras van al final, nunca intercaladas", () => {
    const rows = sortRows([
      { ...buildGrid([route({ routeKey: "N" })], [])[0]!, routeKey: "N", netBps: null },
      { ...buildGrid([route({ routeKey: "P" })], [pnl({ routeKey: "P", netBps: -1 })])[0]!, routeKey: "P" },
      { ...buildGrid([route({ routeKey: "M" })], [pnl({ routeKey: "M", netBps: 5 })])[0]!, routeKey: "M" },
    ]);
    expect(rows.map((r) => r.routeKey)).toEqual(["M", "P", "N"]);
  });

  it("splitGrid separa lo computado de lo declarado no computado", () => {
    const rows = buildGrid(
      [route({ routeKey: "A" }), route({ routeKey: "B" })],
      [pnl({ routeKey: "A", netBps: 10 })],
    );
    const { withFigures, noFigures } = splitGrid(rows);
    expect(withFigures.map((r) => r.routeKey)).toEqual(["A"]);
    expect(noFigures.map((r) => r.routeKey)).toEqual(["B"]);
  });

  it("lectura de ventana: cuenta y reporta el mejor net_bps sin maquillar", () => {
    const rows = buildGrid(
      [route({ routeKey: "A" }), route({ routeKey: "B" }), route({ routeKey: "C" }), route({ routeKey: "D" })],
      [
        pnl({ routeKey: "A", netBps: 310, verdict: "EJECUTAR" }),
        pnl({ routeKey: "B", netBps: 20, verdict: "MARGINAL" }),
        pnl({ routeKey: "C", netBps: -900, verdict: "RECHAZAR" }),
      ],
    );
    const readout = windowReadout(rows);
    expect(readout).toEqual({
      withFigures: 3,
      execute: 1,
      marginal: 1,
      reject: 1,
      noViable: 1,
      bestNetBps: 310,
    });
  });
});

describe("not_computed y embudo", () => {
  it("agrupa razones por frecuencia (R8 explícito, no silencioso)", () => {
    const reasons = reasonBreakdown([
      { route_group_key: "a", reason: "leg_amounts_unpriced" },
      { route_group_key: "b", reason: "leg_amounts_unpriced" },
      { route_group_key: "c", reason: "measured_leg_chain_incomplete" },
    ]);
    expect(reasons).toEqual([
      { reason: "leg_amounts_unpriced", count: 2 },
      { reason: "measured_leg_chain_incomplete", count: 1 },
    ]);
  });

  it("el embudo encadena cada paso con su denominador", () => {
    const d: QuantDashboard = {
      pools: 40,
      edges: 120,
      routes: 60,
      withSignal: 9,
      viable: 4,
      executable: 0,
      marginal: 1,
      bestNetBps: 12,
      bestRouteKey: "r",
      byHops: [],
      top: [],
    };
    const steps = funnelSteps(d);
    expect(steps.map((s) => s.key)).toEqual(["pools", "edges", "routes", "signal", "viable", "executable"]);
    expect(steps[0]?.of).toBeNull();
    expect(steps[1]?.of).toBe(40);
    expect(steps[5]?.of).toBe(4);
  });
});

describe("fetchQuantLayers: falla honestamente", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("expone el código y la razón exacta del backend cuando no es 200", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => new Response(JSON.stringify({ ok: false, error: "db_unavailable" }), { status: 503 })),
    );
    const snap = await fetchQuantLayers("http://edge", { windowMinutes: 60 });
    expect(snap.ok).toBe(false);
    expect(snap.error).toBe("HTTP 503 — db_unavailable");
    expect(snap.data).toBeNull();
  });

  it("rechaza un 200 sin ok=true (no se pinta un contrato que no se cumplió)", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => new Response(JSON.stringify({ layers: {} }), { status: 200 })));
    const snap = await fetchQuantLayers("http://edge");
    expect(snap.ok).toBe(false);
    expect(snap.error).toBe("respuesta sin ok=true");
  });

  it("un 200 válido devuelve el payload íntegro y su hora de lectura", async () => {
    const payload: QuantLayersResponse = {
      ok: true,
      window_minutes: 60,
      generated_at: "2026-09-29T10:00:00.000Z",
      config: {
        slippageBps: 50,
        capitalUsd: 1000,
        utilizationCap: 0.15,
        gasBaseUsd: 6,
        gasPerHopUsd: 4,
        flashBps: 5,
        tipBps: 3,
        riskHaircutBps: 20,
        dustUsd: 0.01,
        targetNetBps: 200,
        minBoundUsd: 100,
      },
      rows_in_window: 1,
      layers: {
        routes: [route()],
        pnl: [pnl()],
        dashboard: {
          pools: 1,
          edges: 2,
          routes: 1,
          withSignal: 1,
          viable: 1,
          executable: 0,
          marginal: 0,
          bestNetBps: -450,
          bestRouteKey: "1|dex_arb|WETH|USDC",
          byHops: [{ hops: 2, routes: 1, signal: 1, viable: 1, executable: 0 }],
          top: [],
        },
      },
      not_computed: [],
      not_computed_count: 0,
      notes: ["F_e = spot/fair con el spot MEDIDO (post-fee): el fee ya está dentro del spot."],
    };
    vi.stubGlobal("fetch", vi.fn(async () => new Response(JSON.stringify(payload), { status: 200 })));
    const snap = await fetchQuantLayers("http://edge", { windowMinutes: 60, limit: 300, top: 10 });
    expect(snap.ok).toBe(true);
    expect(snap.error).toBeNull();
    expect(snap.data?.layers.dashboard.bestNetBps).toBe(-450);
    expect(snap.fetchedAt).not.toBeNull();
  });
});
