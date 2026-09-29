/**
 * QUANT LAYERS — el libro Excel cuantitativo, como álgebra viva en la dapp.
 *
 * Mapeo hoja ↔ capa (mismas fórmulas, mismos nombres, mismos gates):
 *
 *   05_EDGES      → buildEdges()      F_e = spot/fair ; w = −LN(F_e)
 *   06_ROUTES     → buildRoutes()     Σw , bound vinculante , sizing , filtros
 *   07_LEGS       → buildLegs()       cadena medida: AmountOut(i) = AmountIn(i+1)
 *   08_ROUTE_PNL  → buildPnl()        gross , escalera de costes , net , net_bps , veredicto
 *   09_DASHBOARD  → buildDashboard()  embudo por hops + top por net_bps
 *
 * DIFERENCIA DELIBERADA CON EL EXCEL (y es la correcta para datos medidos):
 *   En el libro, `spot` es el precio del pool ANTES de fee y por eso F_e lleva el
 *   factor (1 − fee). Aquí `spot` sale de los amount_in/amount_out MEDIDOS, que ya
 *   vienen post-fee: el AMM ya cobró. Por eso
 *        F_e = spot / fair            (sin factor de fee: ya está dentro del spot)
 *   y se declara explícitamente `fee_included_in_spot: true`. Meter (1 − fee) otra
 *   vez descontaría dos veces el mismo coste.
 *
 * R8: NADA se inventa. Si una pata no trae cadena medida, la capa devuelve
 * `computed: false` con `reason` — nunca un cero, nunca un placeholder.
 */

export interface QuantConfig {
  /** Presupuesto de impacto por pata (= max_slippage_pct 0.0050). */
  slippageBps: number;
  /** Capital autorizado por operación (cap del sizing). */
  capitalUsd: number;
  /** Fracción máxima de la profundidad de la pata más débil. */
  utilizationCap: number;
  gasBaseUsd: number;
  gasPerHopUsd: number;
  flashBps: number;
  tipBps: number;
  riskHaircutBps: number;
  /** Piso de polvo (DUST-NOTIONAL-01/02/03). */
  dustUsd: number;
  /** GATE de rentabilidad: net ≥ 2 % del notional. */
  targetNetBps: number;
  minBoundUsd: number;
}

/** Los valores vivos de la config del operador (trading_config), no constantes. */
export const DEFAULT_QUANT_CONFIG: QuantConfig = {
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
};

export interface LegInput {
  poolAddress: string;
  dex: string | null;
  /** "V2" | "V3" cuando se conoce; null ⇒ no se asume nada. */
  poolType: string | null;
  tokenIn: string;
  tokenOut: string;
  /** Unidades humanas (ya normalizadas por decimals). */
  amountIn: number;
  amountOut: number;
  /** Profundidad medida de la pata en USD, si el wire la trae. */
  depthUsd: number | null;
  /** Liquidez in-tick y √P para el bound V3, si el wire las trae. */
  liquidity: number | null;
  sqrtPriceX96: number | null;
  priceInUsd: number | null;
}

export interface LegView {
  legIndex: number;
  poolAddress: string;
  dex: string | null;
  poolType: string | null;
  tokenIn: string;
  tokenOut: string;
  amountIn: number;
  amountOut: number;
  /** Tasa realizada de la pata (post-fee). */
  spot: number;
  /** Tasa de referencia del par (mediana del cross-section de la ventana). */
  fair: number;
  /** F_e = spot/fair — con el fee YA dentro del spot medido. */
  factor: number;
  /** w = −LN(F_e). */
  weight: number;
  /** Bound operable de la pata en USD (s·profundidad). null ⇒ no computado. */
  boundUsd: number | null;
  boundReason: string | null;
  feeIncludedInSpot: true;
}

export interface RouteView {
  routeKey: string;
  hops: number;
  legs: LegView[];
  sumW: number;
  /**
   * Rentabilidad TEÓRICA del descubrimiento: e^(−Σw) − 1 (en %). Es la anomalía
   * matemática, NO la ganancia: el tamaño, el fee ya pagado, el gas y la
   * financiación se cobran más abajo. Null cuando el ciclo no está completo.
   */
  discoveryReturnPct: number | null;
  signal: boolean;
  bindingBoundUsd: number | null;
  sizingUsd: number;
  maxBlockAgeBlocks: number | null;
  /** Bloque de la cotización (la clave de snapshot del wire). */
  quoteBlock: number | null;
  usable: boolean;
  whyNot: string | null;
  /** Escalera de estados: nunca se declara EXECUTE desde aquí. */
  status: QuantStatus;
}

/** La escalera profesional: el modelo NO firma ni ejecuta. */
export type QuantStatus =
  | "DISCOVERED"
  | "QUOTED"
  | "SIZED"
  | "PASS_NET"
  | "READY_TO_SIMULATE"
  | "NO_NEGATIVE_CYCLE"
  | "INSUFFICIENT_DEPTH"
  | "SLIPPAGE_OR_SIZE"
  | "NET_BELOW_TARGET"
  | "QUOTE_INCOMPLETE";

export interface PnlView {
  routeKey: string;
  hops: number;
  /** Escalera de estados hasta READY_TO_SIMULATE (nunca EXECUTE). */
  status: QuantStatus;
  sizingUsd: number;
  finalUsd: number;
  grossUsd: number;
  grossBps: number;
  gasUsd: number;
  flashUsd: number;
  tipUsd: number;
  haircutUsd: number;
  /**
   * QUANT-PNL-01 — desviación de la cadena MEDIDA contra la cadena `fair` (≥ 0).
   * Es DIAGNÓSTICO, no un coste: `grossUsd` sale de la medición (post-fee y
   * post-impacto), así que esta desviación ya está dentro de él. Sumarla a la
   * escalera restaría la misma pérdida dos veces.
   */
  deviationVsFairUsd: number;
  /** Valor de la cartera si la cadena se hubiera ejecutado a las tasas `fair`. */
  fairChainUsd: number;
  totalCostUsd: number;
  netUsd: number;
  netBps: number;
  verdict: "EJECUTAR" | "MARGINAL" | "RECHAZAR" | "NO VIABLE";
  whyNot: string | null;
}

export interface DashboardView {
  pools: number;
  edges: number;
  routes: number;
  withSignal: number;
  viable: number;
  executable: number;
  marginal: number;
  bestNetBps: number | null;
  bestRouteKey: string | null;
  byHops: Array<{ hops: number; routes: number; signal: number; viable: number; executable: number }>;
  top: Array<{ routeKey: string; netBps: number; netUsd: number; hops: number; verdict: string }>;
}

const isNum = (v: unknown): v is number => typeof v === "number" && Number.isFinite(v);
const ln = (x: number) => Math.log(x);

/** Mediana (copia; no muta la entrada). */
export function median(xs: number[]): number | null {
  const v = xs.filter(isNum).slice().sort((a, b) => a - b);
  if (v.length === 0) return null;
  const m = Math.floor(v.length / 2);
  if (v.length % 2 === 1) return v[m] ?? null;
  const lo = v[m - 1];
  const hi = v[m];
  return lo != null && hi != null ? (lo + hi) / 2 : null;
}

/**
 * Clave del par dirigido — la MISMA normalización que usa el cross-section para
 * calcular el `fair`, así el factor de una arista nunca se compara contra un par
 * distinto.
 */
export function pairKey(tokenIn: string, tokenOut: string): string {
  return `${tokenIn.toLowerCase()}->${tokenOut.toLowerCase()}`;
}

/**
 * Bound operable de una pata, en USD. Sólo si el wire trae la profundidad o el
 * estado V3: si no, `null` + razón (R8).
 */
export function legBoundUsd(leg: LegInput, cfg: QuantConfig): { boundUsd: number | null; reason: string | null } {
  const s = cfg.slippageBps / 10_000;
  if (leg.poolType === "V3" && isNum(leg.liquidity) && isNum(leg.sqrtPriceX96) && leg.sqrtPriceX96 > 0 && isNum(leg.priceInUsd)) {
    // Δx ≤ s·L·√P/2  (token0 in). √P = sqrtPriceX96 / 2^96.
    const sqrtP = leg.sqrtPriceX96 / 2 ** 96;
    const amountToken0 = (s * leg.liquidity * sqrtP) / 2;
    return { boundUsd: amountToken0 * leg.priceInUsd, reason: null };
  }
  if (isNum(leg.depthUsd) && leg.depthUsd > 0) {
    // Profundidad medida de la pata: el mismo presupuesto de slippage.
    return { boundUsd: s * leg.depthUsd, reason: null };
  }
  return { boundUsd: null, reason: "route_depth_not_on_wire" };
}

/** 05_EDGES · F_e y w por pata, contra el `fair` del par. */
export function buildLegs(
  legs: Array<Omit<LegInput, "depthUsd" | "liquidity" | "sqrtPriceX96" | "priceInUsd"> & Partial<LegInput>>,
  fairByPair: Map<string, number>,
  cfg: QuantConfig,
): LegView[] {
  return legs.map((leg, i) => {
    const spot = leg.amountIn > 0 ? leg.amountOut / leg.amountIn : NaN;
    const fair = fairByPair.get(pairKey(leg.tokenIn, leg.tokenOut)) ?? NaN;
    const factor = isNum(spot) && isNum(fair) && fair > 0 ? spot / fair : NaN;
    const weight = isNum(factor) && factor > 0 ? -ln(factor) : NaN;
    const { boundUsd, reason } = legBoundUsd(leg as LegInput, cfg);
    return {
      legIndex: i + 1,
      poolAddress: leg.poolAddress,
      dex: leg.dex ?? null,
      poolType: leg.poolType ?? null,
      tokenIn: leg.tokenIn,
      tokenOut: leg.tokenOut,
      amountIn: leg.amountIn,
      amountOut: leg.amountOut,
      spot,
      fair,
      factor,
      weight,
      boundUsd,
      boundReason: reason,
      feeIncludedInSpot: true as const,
    };
  });
}

/** 06_ROUTES · Σw, bound vinculante, sizing y filtros de viabilidad. */
export function buildRoutes(
  routeKey: string,
  legs: LegView[],
  cfg: QuantConfig,
  opts: { maxBlockAgeBlocks?: number | null; quoteBlock?: number | null } = {},
): RouteView {
  const weights = legs.map((l) => l.weight).filter(isNum);
  const complete = legs.length > 0 && weights.length === legs.length;
  const sumW = weights.reduce((a, b) => a + b, 0);
  const signal = complete && sumW < 0;
  const bounds = legs.map((l) => l.boundUsd).filter(isNum);
  const bindingBoundUsd = bounds.length === legs.length && bounds.length > 0 ? Math.min(...bounds) : null;
  const sizingUsd =
    bindingBoundUsd == null ? 0 : Math.max(cfg.dustUsd, Math.min(cfg.capitalUsd, bindingBoundUsd * cfg.utilizationCap));
  const discoveryReturnPct = complete ? (Math.exp(-sumW) - 1) * 100 : null;

  let whyNot: string | null = null;
  let status: QuantStatus;
  if (legs.length === 0) {
    whyNot = "sin patas";
    status = "QUOTE_INCOMPLETE";
  } else if (!complete) {
    whyNot = "cadena medida incompleta";
    status = "QUOTE_INCOMPLETE";
  } else if (!signal) {
    whyNot = "sin señal (Σw ≥ 0)";
    status = "NO_NEGATIVE_CYCLE";
  } else if (bindingBoundUsd == null) {
    whyNot = "profundidad no computada";
    status = "QUOTE_INCOMPLETE";
  } else if (bindingBoundUsd < cfg.minBoundUsd) {
    whyNot = "profundidad insuficiente";
    status = "INSUFFICIENT_DEPTH";
  } else if (sizingUsd <= cfg.dustUsd) {
    whyNot = "sin tamaño operable";
    status = "SLIPPAGE_OR_SIZE";
  } else {
    status = "SIZED";
  }

  return {
    routeKey,
    hops: legs.length,
    legs,
    sumW,
    discoveryReturnPct,
    signal,
    bindingBoundUsd,
    sizingUsd,
    maxBlockAgeBlocks: opts.maxBlockAgeBlocks ?? null,
    quoteBlock: opts.quoteBlock ?? null,
    usable: whyNot == null,
    whyNot,
    status,
  };
}

/** 08_ROUTE_PNL · escalera de costes + gate de rentabilidad. */
export function buildPnl(
  route: RouteView,
  measured: { finalUsd: number | null; principalUsd: number | null },
  cfg: QuantConfig,
): PnlView {
  const sizingUsd = isNum(measured.principalUsd) && measured.principalUsd > 0 ? measured.principalUsd : route.sizingUsd;
  const finalUsd = isNum(measured.finalUsd) ? (measured.finalUsd as number) : NaN;
  const grossUsd = finalUsd - sizingUsd;
  const gasUsd = cfg.gasBaseUsd + route.hops * cfg.gasPerHopUsd;
  const flashUsd = (sizingUsd * cfg.flashBps) / 10_000;
  const tipUsd = (sizingUsd * cfg.tipBps) / 10_000;
  const haircutUsd = (sizingUsd * cfg.riskHaircutBps) / 10_000;
  // QUANT-PNL-01 (2026-09-29) — la desviación contra `fair` es DIAGNÓSTICO, no
  // coste. El `gross` de esta capa sale de la cadena MEDIDA (post-fee y
  // post-impacto): la ineficiencia frente a `fair` ya está dentro del gross. Si
  // se sumara otra vez al coste, la misma pérdida se contaría dos veces —
  // medido en la fila real WETH→USDC del 2026-09-29: gross −$7.94, desviación
  // $7.61 ⇒ el net publicado habría sido −$31.48 en vez de −$23.87, un número
  // que no reconcilia con la medición del searcher (net_profit_usd −$8.62 antes
  // de la escalera de gas propia de esta capa).
  //
  // Se conserva el valor de la cadena `fair` para poder auditar de dónde sale la
  // desviación: fairChain − final = lo que la ruta dejó sobre la mesa frente a
  // la mediana del par. Es información, no una línea de coste.
  const fairChainUsd = route.legs.reduce((acc, l, i) => {
    const prev = i === 0 ? sizingUsd : acc;
    return isNum(l.fair) ? prev * l.fair : NaN;
  }, sizingUsd);
  const deviationVsFairUsd =
    isNum(fairChainUsd) && isNum(finalUsd) ? Math.max(0, fairChainUsd - finalUsd) : NaN;
  const variableCosts = flashUsd + tipUsd + haircutUsd;
  const totalCostUsd = gasUsd + variableCosts;
  const netUsd = isNum(totalCostUsd) ? grossUsd - totalCostUsd : NaN;
  const netBps = isNum(netUsd) && sizingUsd > 0 ? (netUsd / sizingUsd) * 10_000 : NaN;

  let verdict: PnlView["verdict"];
  let whyNot: string | null = null;
  let status: QuantStatus;
  if (!route.usable) {
    // El embudo manda: una ruta sin señal / sin profundidad / sin tamaño se
    // declara NO VIABLE con SU razón, antes de mirar la medición.
    verdict = "NO VIABLE";
    whyNot = route.whyNot;
    status = route.status;
  } else if (!isNum(finalUsd) || !isNum(netBps)) {
    verdict = "RECHAZAR";
    whyNot = "cadena medida incompleta — no se publica veredicto";
    status = "QUOTE_INCOMPLETE";
  } else if (netBps >= cfg.targetNetBps) {
    // El techo de esta capa: listo para SIMULAR. Firmar y emitir es del
    // terminus de ejecución (relays-client), nunca de este modelo.
    verdict = "EJECUTAR";
    status = "READY_TO_SIMULATE";
  } else if (netBps > 0) {
    verdict = "MARGINAL";
    status = "PASS_NET";
  } else {
    verdict = "RECHAZAR";
    whyNot = "coste > gross";
    status = "NET_BELOW_TARGET";
  }

  return {
    routeKey: route.routeKey,
    hops: route.hops,
    status,
    sizingUsd,
    finalUsd,
    grossUsd,
    grossBps: sizingUsd > 0 ? (grossUsd / sizingUsd) * 10_000 : NaN,
    gasUsd,
    flashUsd,
    tipUsd,
    haircutUsd,
    deviationVsFairUsd,
    fairChainUsd,
    totalCostUsd,
    netUsd,
    netBps,
    verdict,
    whyNot,
  };
}

/** 09_DASHBOARD · embudo + top por net_bps. */
export function buildDashboard(routes: RouteView[], pnls: PnlView[], topN = 10): DashboardView {
  const pnlByRoute = new Map(pnls.map((p) => [p.routeKey, p]));
  const byHopsMap = new Map<number, { hops: number; routes: number; signal: number; viable: number; executable: number }>();
  for (const r of routes) {
    const e = byHopsMap.get(r.hops) ?? { hops: r.hops, routes: 0, signal: 0, viable: 0, executable: 0 };
    e.routes += 1;
    if (r.signal) e.signal += 1;
    if (r.usable) e.viable += 1;
    if (pnlByRoute.get(r.routeKey)?.verdict === "EJECUTAR") e.executable += 1;
    byHopsMap.set(r.hops, e);
  }
  const ranked = pnls
    .filter((p) => isNum(p.netBps))
    .slice()
    .sort((a, b) => b.netBps - a.netBps);
  const best = ranked[0] ?? null;
  return {
    pools: new Set(routes.flatMap((r) => r.legs.map((l) => l.poolAddress))).size,
    edges: routes.reduce((a, r) => a + r.legs.length, 0),
    routes: routes.length,
    withSignal: routes.filter((r) => r.signal).length,
    viable: routes.filter((r) => r.usable).length,
    executable: pnls.filter((p) => p.verdict === "EJECUTAR").length,
    marginal: pnls.filter((p) => p.verdict === "MARGINAL").length,
    bestNetBps: best && isNum(best.netBps) ? best.netBps : null,
    bestRouteKey: best?.routeKey ?? null,
    byHops: Array.from(byHopsMap.values()).sort((a, b) => a.hops - b.hops),
    top: ranked.slice(0, topN).map((p) => ({
      routeKey: p.routeKey,
      netBps: p.netBps,
      netUsd: p.netUsd,
      hops: p.hops,
      verdict: p.verdict,
    })),
  };
}

/**
 * Cross-section de la ventana: la tasa `fair` de cada par dirigido, tomada como
 * la MEDIANA de las tasas realizadas por todos los pools del mismo par. Es el
 * equivalente data-driven de la hoja 04_ORACLE: no necesita oráculo externo y no
 * inventa nada — si un par sólo aparece una vez, su propio spot es su `fair` y
 * su factor queda 1 (sin señal), que es la lectura honesta.
 */
export function fairByPair(spots: Array<{ tokenIn: string; tokenOut: string; spot: number }>): Map<string, number> {
  const acc = new Map<string, number[]>();
  for (const s of spots) {
    if (!isNum(s.spot) || s.spot <= 0) continue;
    const k = pairKey(s.tokenIn, s.tokenOut);
    const list = acc.get(k) ?? [];
    list.push(s.spot);
    acc.set(k, list);
  }
  const out = new Map<string, number>();
  for (const [k, list] of acc) {
    const m = median(list);
    if (m != null) out.set(k, m);
  }
  return out;
}
