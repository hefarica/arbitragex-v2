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

/**
 * QUANT-FAIR-01 (2026-09-29, medido en vivo) — de dónde sale el `fair` de cada
 * arista. Es la decisión que hace o rompe la capa entera:
 *
 *   · `oracle_usd` — fair = price_usd(token_in) / price_usd(token_out), con los
 *     precios del stack soberano del operador (Binance WS + Chainlink, servidos
 *     por trading_config.token_prices_usd). Es el equivalente real de la hoja
 *     04_ORACLE y la ÚNICA forma de que F_e ≠ 1.
 *
 *   · `cross_section_median` — mediana de las tasas realizadas del mismo par
 *     dirigido en la ventana. Es un estimador DEGENERADO cuando el par aparece
 *     una sola vez: spot == fair ⇒ F_e = 1 ⇒ w = 0 ⇒ Σw = 0 ⇒ "sin señal"
 *     siempre. Medido en producción el 2026-09-29: 40/40 rutas con Σw = 0 y
 *     `NO_NEGATIVE_CYCLE`, la capa no podía decir nada. Se conserva SÓLO como
 *     relleno declarado cuando el token no tiene precio, y se publica cuál se usó.
 *
 * Si ninguna de las dos existe, `fairBasis = null` y la arista se declara no
 * computada (R8) — nunca se inventa una tasa.
 */
export type FairBasis = "oracle_usd" | "cross_section_median";

export interface FairRef {
  /** Tasa de referencia del par dirigido (token_out por token_in). */
  rate: number;
  basis: FairBasis;
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
  /** Tasa de referencia del par (oráculo USD o mediana declarada). */
  fair: number;
  /** Procedencia del `fair`: sin ella, F_e no es auditable. */
  fairBasis: FairBasis | null;
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
  /** C-16: null cuando la continuidad de valor está rota (Σw no se publica). */
  sumW: number | null;
  /**
   * Rentabilidad TEÓRICA del descubrimiento: e^(−Σw) − 1 (en %). Es la anomalía
   * matemática, NO la ganancia: el tamaño, el fee ya pagado, el gas y la
   * financiación se cobran más abajo. Null cuando el ciclo no está completo.
   */
  discoveryReturnPct: number | null;
  signal: boolean;
  /** C-16: aristas de continuidad comprobadas (pares consecutivos con cifras). */
  legsChecked: number;
  /** C-16: max |value_out(i) − value_in(i+1)| / value_in(i+1). null si no se pudo medir. */
  maxValueJumpRatio: number | null;
  bindingBoundUsd: number | null;
  /**
   * QUANT-SIZING-NULL-01: tamaño que el modelo autorizaría. **null = no
   * computado** (sin bound vinculante no hay tamaño que el modelo pueda
   * defender). Antes se publicaba `0`, que se lee como "tamaño cero" y no como
   * "no sé" — y encima convive con filas cuyo principal medido es $718.
   */
  sizingUsd: number | null;
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
  | "LEG_VALUE_JUMP"
  | "OUT_OF_BAND"
  | "QUOTE_INCOMPLETE";

/**
 * C-16 — de dónde salen los costes de la capa.
 *
 *   · `measured`            — los cuatro componentes de `o.economics` (gas,
 *                             flash_fee, bribe, other_costs). Es la ÚNICA
 *                             contabilidad: la fila manda.
 *   · `measured_incomplete` — la fila trae economics pero le falta algún
 *                             componente ⇒ no se publica net (fail-closed).
 *   · `config_ladder`       — llamada legacy sin economics (tests); usa
 *                             DEFAULT_QUANT_CONFIG y lo declara.
 */
export type CostBasis = "measured" | "measured_incomplete" | "config_ladder";

/** Componentes del ledger de coste tal como los publica el productor (estimados, no recibos). */
export interface LedgerCosts {
  gasUsd: number | null;
  flashUsd: number | null;
  tipUsd: number | null;
  otherCostsUsd: number | null;
}

export interface PnlView {
  routeKey: string;
  hops: number;
  /** Escalera de estados hasta READY_TO_SIMULATE (nunca EXECUTE). */
  status: QuantStatus;
  sizingUsd: number | null;
  /** `measured` = amount_in_usd de la fila; `model` = route.sizingUsd (sin medición). */
  principalSource: "measured" | "model" | null;
  finalUsd: number | null;
  grossUsd: number | null;
  /** null cuando el principal está fuera de banda: un bps sobre 0.01 USD no mide nada. */
  grossBps: number | null;
  gasUsd: number | null;
  flashUsd: number | null;
  tipUsd: number | null;
  /** `other_costs_usd` del ledger; null si la llamada usa la escalera legacy. */
  otherCostsUsd: number | null;
  /** Haircut de riesgo: SÓLO existe en la escalera legacy. La fila medida no lo tiene. */
  haircutUsd: number | null;
  costBasis: CostBasis;
  /**
   * QUANT-PNL-01 — desviación de la cadena MEDIDA contra la cadena `fair` (≥ 0).
   * Es DIAGNÓSTICO, no un coste: `grossUsd` sale de la medición (post-fee y
   * post-impacto), así que esta desviación ya está dentro de él. Sumarla a la
   * escalera restaría la misma pérdida dos veces.
   */
  deviationVsFairUsd: number | null;
  /** Valor de la cartera si la cadena se hubiera ejecutado a las tasas `fair`. */
  fairChainUsd: number | null;
  totalCostUsd: number | null;
  netUsd: number | null;
  /** null si el principal medido cae fuera de la banda declarada. */
  netBps: number | null;
  /** Banda declarada [minBoundUsd, capitalUsd × utilizationCap], aplicada al principal. */
  bandUsd: { minUsd: number; maxUsd: number };
  inBand: boolean | null;
  /** |net calculado − economics.net_profit_usd|; null si el wire no lo trae. */
  netDeltaUsd: number | null;
  verdict: "EJECUTAR" | "MARGINAL" | "RECHAZAR" | "NO VIABLE" | "OUT_OF_BAND";
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
  top: Array<{ routeKey: string; netBps: number; netUsd: number | null; hops: number; verdict: string }>;
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
  fairByPair: Map<string, FairRef>,
  cfg: QuantConfig,
): LegView[] {
  return legs.map((leg, i) => {
    const spot = leg.amountIn > 0 ? leg.amountOut / leg.amountIn : NaN;
    const ref = fairByPair.get(pairKey(leg.tokenIn, leg.tokenOut));
    const fair = ref?.rate ?? NaN;
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
      fairBasis: ref?.basis ?? null,
      factor,
      weight,
      boundUsd,
      boundReason: reason,
      feeIncludedInSpot: true as const,
    };
  });
}

/**
 * C-16 — continuidad de valor por pata. La cadena es válida sólo si el valor
 * que sale de una pata es el que entra en la siguiente dentro de 1 bp:
 *
 *     |value_out(i) − value_in(i+1)| / value_in(i+1) ≤ 0.0001
 *
 * Un salto mayor no es una oportunidad: es un cambio de unidades del
 * plan/encoder (el 985× de la fila 51813500). Mientras no se arregle aguas
 * arriba, Σw y sizing NO se publican para esa ruta.
 */
export const LEG_CONTINUITY_TOLERANCE = 0.0001;

export function legContinuity(legs: LegView[]): {
  checked: number;
  maxRatio: number | null;
  brokenAt: number | null;
  brokenRatio: number | null;
} {
  let checked = 0;
  let maxRatio: number | null = null;
  let brokenAt: number | null = null;
  let brokenRatio: number | null = null;
  for (let i = 0; i + 1 < legs.length; i++) {
    const out = legs[i]?.amountOut;
    const nextIn = legs[i + 1]?.amountIn;
    if (!isNum(out) || !isNum(nextIn) || nextIn <= 0) continue;
    checked += 1;
    const ratio = Math.abs(out - nextIn) / nextIn;
    if (maxRatio == null || ratio > maxRatio) maxRatio = ratio;
    if (ratio > LEG_CONTINUITY_TOLERANCE && brokenAt == null) {
      brokenAt = i + 1;
      brokenRatio = ratio;
    }
  }
  return { checked, maxRatio, brokenAt, brokenRatio };
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
  const continuity = legContinuity(legs);
  const continuous = continuity.brokenAt == null;
  const sumW = complete && continuous ? weights.reduce((a, b) => a + b, 0) : null;
  const signal = sumW != null && sumW < 0;
  const bounds = legs.map((l) => l.boundUsd).filter(isNum);
  const bindingBoundUsd = bounds.length === legs.length && bounds.length > 0 ? Math.min(...bounds) : null;
  const sizingUsd =
    !continuous || bindingBoundUsd == null
      ? null
      : Math.max(cfg.dustUsd, Math.min(cfg.capitalUsd, bindingBoundUsd * cfg.utilizationCap));
  const discoveryReturnPct = sumW != null ? (Math.exp(-sumW) - 1) * 100 : null;

  let whyNot: string | null = null;
  let status: QuantStatus;
  if (legs.length === 0) {
    whyNot = "sin patas";
    status = "QUOTE_INCOMPLETE";
  } else if (!complete) {
    whyNot = "cadena medida incompleta";
    status = "QUOTE_INCOMPLETE";
  } else if (!continuous) {
    whyNot =
      `salto de valor entre patas: ${(continuity.brokenRatio as number).toFixed(6)}× ` +
      `en la pata ${continuity.brokenAt} (> ${LEG_CONTINUITY_TOLERANCE})`;
    status = "LEG_VALUE_JUMP";
  } else if (!signal) {
    whyNot = "sin señal (Σw ≥ 0)";
    status = "NO_NEGATIVE_CYCLE";
  } else if (bindingBoundUsd == null) {
    whyNot = "profundidad no computada";
    status = "QUOTE_INCOMPLETE";
  } else if (bindingBoundUsd < cfg.minBoundUsd) {
    whyNot = "profundidad insuficiente";
    status = "INSUFFICIENT_DEPTH";
  } else if (sizingUsd == null || sizingUsd <= cfg.dustUsd) {
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
    legsChecked: continuity.checked,
    maxValueJumpRatio: continuity.maxRatio,
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
  measured: {
    finalUsd: number | null;
    principalUsd: number | null;
    /** C-16: ledger de costes de `o.economics`. Sin él la capa NO es la única verdad. */
    costs?: LedgerCosts | null;
    /** `economics.net_profit_usd`, sólo para reconciliar — nunca para calcular. */
    measuredNetUsd?: number | null;
  },
  cfg: QuantConfig,
): PnlView {
  // El tamaño REALMENTE medido manda; si no hay medición se usa el del modelo
  // (null = no computado ⇒ null en el wire, jamás un 0 falso).
  const principalMeasured =
    isNum(measured.principalUsd) && measured.principalUsd > 0 ? measured.principalUsd : null;
  const sizingUsd =
    principalMeasured ?? (isNum(route.sizingUsd) ? route.sizingUsd : null);
  const principalSource = principalMeasured != null ? ("measured" as const) : sizingUsd != null ? ("model" as const) : null;
  const finalUsd = isNum(measured.finalUsd) ? measured.finalUsd : null;
  const grossUsd = isNum(finalUsd) && isNum(sizingUsd) ? finalUsd - sizingUsd : null;

  // C-16 — la banda es POLÍTICA DECLARADA, no una identidad derivada: son dos
  // perillas del operador (minBoundUsd y capitalUsd×utilizationCap). Se aplica
  // al principal MEDIDO y se publica como bandera; no es un umbral de coste.
  const bandUsd = { minUsd: cfg.minBoundUsd, maxUsd: cfg.capitalUsd * cfg.utilizationCap };
  const inBand = isNum(sizingUsd) ? sizingUsd >= bandUsd.minUsd && sizingUsd <= bandUsd.maxUsd : null;

  // ── Costes: el ledger de la fila, no una constante de esta capa ────────────
  const c = measured.costs ?? null;
  const measuredComplete =
    c != null && isNum(c.gasUsd) && isNum(c.flashUsd) && isNum(c.tipUsd) && isNum(c.otherCostsUsd);
  let costBasis: CostBasis;
  let gasUsd: number | null = null;
  let flashUsd: number | null = null;
  let tipUsd: number | null = null;
  let otherCostsUsd: number | null = null;
  let haircutUsd: number | null = null;

  if (measuredComplete && c != null) {
    costBasis = "measured";
    gasUsd = c.gasUsd;
    flashUsd = c.flashUsd;
    tipUsd = c.tipUsd;
    otherCostsUsd = c.otherCostsUsd;
  } else if (c != null) {
    // La fila trae economics pero le falta un componente: fail-closed, no se
    // rellena con la escalera (eso sería una segunda contabilidad).
    costBasis = "measured_incomplete";
  } else {
    // Llamada legacy (tests / consumidores sin economics): escalera declarada.
    costBasis = "config_ladder";
    if (isNum(sizingUsd)) {
      gasUsd = cfg.gasBaseUsd + route.hops * cfg.gasPerHopUsd;
      flashUsd = (sizingUsd * cfg.flashBps) / 10_000;
      tipUsd = (sizingUsd * cfg.tipBps) / 10_000;
      haircutUsd = (sizingUsd * cfg.riskHaircutBps) / 10_000;
    }
  }

  const totalCostUsd =
    costBasis === "measured"
      ? (gasUsd as number) + (flashUsd as number) + (tipUsd as number) + (otherCostsUsd as number)
      : costBasis === "config_ladder" && isNum(gasUsd) && isNum(flashUsd) && isNum(tipUsd) && isNum(haircutUsd)
        ? gasUsd + flashUsd + tipUsd + haircutUsd
        : null;
  const netUsd = isNum(grossUsd) && isNum(totalCostUsd) ? grossUsd - totalCostUsd : null;
  const netBpsRaw = isNum(netUsd) && isNum(sizingUsd) && sizingUsd > 0 ? (netUsd / sizingUsd) * 10_000 : null;
  const grossBpsRaw = isNum(grossUsd) && isNum(sizingUsd) && sizingUsd > 0 ? (grossUsd / sizingUsd) * 10_000 : null;

  // C-16 — un bps sobre un principal fuera de banda no mide nada: se publica el
  // principal y el gross en USD, pero NO se publica ningún bps.
  const publishBps = !(principalSource === "measured" && inBand === false);
  const netBps = publishBps ? netBpsRaw : null;
  const grossBps = publishBps ? grossBpsRaw : null;

  // QUANT-PNL-01 (2026-09-29) — la desviación contra `fair` es DIAGNÓSTICO, no
  // coste. El `gross` de esta capa sale de la cadena MEDIDA (post-fee y
  // post-impacto): la ineficiencia frente a `fair` ya está dentro del gross.
  //
  // Aritmética de la fila real WETH→USDC del 2026-09-29 (UniV2+SushiSwap,
  // principal $718.4076, gross −$7.9391, 2 patas):
  //   escalera       = gas (6 + 2×4) + flash (5 bps) + tip (3 bps) + haircut (20 bps)
  //                  = 14 + 0.3592 + 0.2155 + 1.4368 = $16.0115
  //   net correcto   = −7.9391 − 16.0115            = −$23.9506
  //   con el doble conteo (fair ≈ 1: el ciclo cierra en ≈ principal, así que
  //   desviación ≈ la pérdida misma ≈ $7.9391):
  //   net erróneo    = −7.9391 − 16.0115 − 7.9391   = −$31.8897
  const fairChainUsd = isNum(sizingUsd)
    ? route.legs.reduce<number | null>((acc, l, i) => {
        if (!isNum(acc)) return null;
        const prev = i === 0 ? sizingUsd : acc;
        return isNum(l.fair) ? prev * l.fair : null;
      }, sizingUsd)
    : null;
  const deviationVsFairUsd =
    isNum(fairChainUsd) && isNum(finalUsd) ? Math.max(0, fairChainUsd - finalUsd) : null;

  // Reconciliación explícita contra el net de la fila (Δ publicada, no escondida).
  const netDeltaUsd =
    isNum(measured.measuredNetUsd) && isNum(netUsd) ? Math.abs(netUsd - measured.measuredNetUsd) : null;

  let verdict: PnlView["verdict"];
  let whyNot: string | null = null;
  let status: QuantStatus;
  if (principalSource === "measured" && inBand === false) {
    // Bandera declarada: el principal medido está fuera de [min, capital×cap].
    // No es «no viable»: es «fuera de la banda con la que esta capa mide».
    verdict = "OUT_OF_BAND";
    whyNot =
      `principal medido ${(sizingUsd as number).toFixed(4)} USD fuera de la banda declarada ` +
      `[${bandUsd.minUsd}, ${bandUsd.maxUsd}] (política, no coste)`;
    status = "OUT_OF_BAND";
  } else if (!route.usable) {
    // El embudo manda: una ruta sin señal / sin continuidad / sin profundidad /
    // sin tamaño se declara NO VIABLE con SU razón, antes de mirar la medición.
    verdict = "NO VIABLE";
    whyNot = route.whyNot;
    status = route.status;
  } else if (costBasis === "measured_incomplete") {
    verdict = "RECHAZAR";
    whyNot = "costes medidos incompletos — no se publica net";
    status = "QUOTE_INCOMPLETE";
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
    principalSource,
    finalUsd,
    grossUsd,
    grossBps,
    gasUsd,
    flashUsd,
    tipUsd,
    otherCostsUsd,
    haircutUsd,
    costBasis,
    deviationVsFairUsd,
    fairChainUsd,
    totalCostUsd,
    netUsd,
    netBps,
    bandUsd,
    inBand,
    netDeltaUsd,
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
    .filter((p): p is PnlView & { netBps: number } => isNum(p.netBps))
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
 * la MEDIANA de las tasas realizadas por todos los pools del mismo par.
 *
 * QUANT-FAIR-01 — esto es un RELLENO DECLARADO, no el oráculo. Cuando el par
 * aparece una sola vez (lo normal en ventanas cortas) la mediana es la propia
 * tasa ⇒ F_e = 1 ⇒ w = 0, y la capa no ve nada. Se marca con su basis para que
 * la pantalla pueda decir de dónde salió cada F_e.
 */
export function fairByPair(spots: Array<{ tokenIn: string; tokenOut: string; spot: number }>): Map<string, FairRef> {
  const acc = new Map<string, number[]>();
  for (const s of spots) {
    if (!isNum(s.spot) || s.spot <= 0) continue;
    const k = pairKey(s.tokenIn, s.tokenOut);
    const list = acc.get(k) ?? [];
    list.push(s.spot);
    acc.set(k, list);
  }
  const out = new Map<string, FairRef>();
  for (const [k, list] of acc) {
    const m = median(list);
    if (m != null) out.set(k, { rate: m, basis: "cross_section_median" });
  }
  return out;
}

/**
 * QUANT-FAIR-01 — el oráculo: fair = price_usd(token_in) / price_usd(token_out).
 *
 * Los precios vienen del stack soberano del operador (Binance WS + Chainlink)
 * vía `trading_config.token_prices_usd`, el mismo dato que alimenta el scoring y
 * la simulación. Sin precio de alguno de los dos tokens, el par NO entra: la
 * arista quedará sin `fair` (basis null) y la ruta se declarará no computada en
 * vez de fabricar una tasa con su propia medición.
 */
export function fairFromUsdPrices(
  pairs: Array<{ tokenIn: string; tokenOut: string }>,
  priceUsdByToken: Map<string, number>,
): Map<string, FairRef> {
  const out = new Map<string, FairRef>();
  for (const p of pairs) {
    const k = pairKey(p.tokenIn, p.tokenOut);
    if (out.has(k)) continue;
    const pin = priceUsdByToken.get(p.tokenIn.toLowerCase());
    const pout = priceUsdByToken.get(p.tokenOut.toLowerCase());
    if (!isNum(pin) || !isNum(pout) || pin <= 0 || pout <= 0) continue;
    out.set(k, { rate: pin / pout, basis: "oracle_usd" });
  }
  return out;
}

/**
 * El oráculo manda; la mediana del cross-section sólo rellena los pares que el
 * oráculo no cubre, y cada entrada conserva su procedencia. Un `fair` sin
 * procedencia no existe.
 */
export function mergeFair(oracle: Map<string, FairRef>, fallback: Map<string, FairRef>): Map<string, FairRef> {
  const out = new Map<string, FairRef>(oracle);
  for (const [k, v] of fallback) {
    if (!out.has(k)) out.set(k, v);
  }
  return out;
}

/** Cuántas aristas se resolvieron con cada procedencia (visible en la respuesta). */
export function fairBasisCounts(legs: LegView[]): Record<string, number> {
  const out: Record<string, number> = { oracle_usd: 0, cross_section_median: 0, none: 0 };
  for (const l of legs) {
    if (l.fairBasis == null) out["none"] = (out["none"] ?? 0) + 1;
    else out[l.fairBasis] = (out[l.fairBasis] ?? 0) + 1;
  }
  return out;
}
