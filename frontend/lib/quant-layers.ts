/**
 * QUANT LAYERS (frontend) — tipos, fetch y presentación del libro cuantitativo.
 *
 * Espejo del contrato de `GET /api/quant/layers` (backend/api-server/src/routes/
 * quant-layers.ts). El endpoint sirve las hojas 05_EDGES → 09_DASHBOARD del libro
 * Excel como capas vivas sobre las detecciones MEDIDAS de la ventana.
 *
 * R8 / RULE 00 — la regla que gobierna TODO este archivo:
 *   El wire transporta `NaN` como `null` (JSON no tiene NaN). Por lo tanto un
 *   campo `null` significa **no computado**, jamás cero. Se renderiza "—" y, si la
 *   fila entera carece de cifras, se declara "NO COMPUTADO" con su razón. Ningún
 *   formateador de este módulo convierte `null` en `0`, `0.00` ni `$0.00`.
 */
import type { BadgeProps } from "@/components/ui/badge";

// ─── Contrato del wire ───────────────────────────────────────────────────────

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

export type QuantVerdict = "EJECUTAR" | "MARGINAL" | "RECHAZAR" | "NO VIABLE";

export interface QuantConfig {
  slippageBps: number;
  capitalUsd: number;
  utilizationCap: number;
  gasBaseUsd: number;
  gasPerHopUsd: number;
  flashBps: number;
  tipBps: number;
  riskHaircutBps: number;
  dustUsd: number;
  targetNetBps: number;
  minBoundUsd: number;
}

export interface QuantLeg {
  legIndex: number;
  poolAddress: string;
  dex: string | null;
  poolType: string | null;
  tokenIn: string;
  tokenOut: string;
  amountIn: number | null;
  amountOut: number | null;
  /** Tasa realizada medida (post-fee). */
  spot: number | null;
  /** Tasa de referencia del par (mediana del cross-section de la ventana). */
  fair: number | null;
  /** F_e = spot/fair — el fee ya está dentro del spot medido. */
  factor: number | null;
  /** w = −LN(F_e). */
  weight: number | null;
  boundUsd: number | null;
  boundReason: string | null;
  feeIncludedInSpot: true;
}

export interface QuantRoute {
  routeKey: string;
  hops: number;
  legs: QuantLeg[];
  sumW: number | null;
  /** Rentabilidad TEÓRICA del ciclo: e^(−Σw) − 1, en %. */
  discoveryReturnPct: number | null;
  signal: boolean;
  bindingBoundUsd: number | null;
  sizingUsd: number | null;
  maxBlockAgeBlocks: number | null;
  quoteBlock: number | null;
  usable: boolean;
  whyNot: string | null;
  status: QuantStatus;
}

export interface QuantPnl {
  routeKey: string;
  hops: number;
  status: QuantStatus;
  sizingUsd: number | null;
  finalUsd: number | null;
  grossUsd: number | null;
  grossBps: number | null;
  gasUsd: number | null;
  flashUsd: number | null;
  tipUsd: number | null;
  haircutUsd: number | null;
  /**
   * QUANT-PNL-01 — desviación de la cadena medida contra la cadena `fair` (≥ 0).
   * DIAGNÓSTICO: el impacto ya está dentro del gross medido, así que NO suma a
   * `totalCostUsd` (hacerlo restaría la misma pérdida dos veces).
   */
  deviationVsFairUsd: number | null;
  /** Valor de la cartera si la cadena se hubiera ejecutado a las tasas `fair`. */
  fairChainUsd: number | null;
  totalCostUsd: number | null;
  netUsd: number | null;
  netBps: number | null;
  verdict: QuantVerdict;
  whyNot: string | null;
}

export interface QuantDashboard {
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
  top: Array<{ routeKey: string; netBps: number | null; netUsd: number | null; hops: number; verdict: string }>;
}

export interface QuantLayerNotComputed {
  route_group_key: string;
  reason: string;
}

export interface QuantLayersResponse {
  ok: true;
  window_minutes: number;
  generated_at: string;
  config: QuantConfig;
  rows_in_window: number;
  layers: {
    routes: QuantRoute[];
    pnl: QuantPnl[];
    dashboard: QuantDashboard;
  };
  not_computed: QuantLayerNotComputed[];
  not_computed_count: number;
  notes: string[];
}

/** El snapshot que viaja del Server Component al Client Component (R1). */
export interface QuantSnapshot {
  ok: boolean;
  /** Presente SÓLO cuando ok=false: la razón exacta, nunca un sustituto. */
  error: string | null;
  data: QuantLayersResponse | null;
  fetchedAt: string | null;
}

export interface QuantLayersQuery {
  windowMinutes?: number;
  limit?: number;
  top?: number;
  signal?: AbortSignal;
}

export function quantLayersPath(q: QuantLayersQuery = {}): string {
  const p = new URLSearchParams();
  if (q.windowMinutes != null) p.set("window_minutes", String(q.windowMinutes));
  if (q.limit != null) p.set("limit", String(q.limit));
  if (q.top != null) p.set("top", String(q.top));
  const qs = p.toString();
  return `/api/quant/layers${qs.length > 0 ? `?${qs}` : ""}`;
}

/** Un solo camino de fetch para SSR y para el poll del browser (misma URL). */
export async function fetchQuantLayers(baseUrl: string, q: QuantLayersQuery = {}): Promise<QuantSnapshot> {
  const url = `${baseUrl}${quantLayersPath(q)}`;
  try {
    const res = await fetch(url, {
      cache: "no-store",
      headers: { accept: "application/json" },
      signal: q.signal,
    });
    const body: unknown = await res.json().catch(() => null);
    const rec = (body ?? null) as ({ error?: unknown; ok?: unknown } & Record<string, unknown>) | null;
    if (!res.ok) {
      const why = typeof rec?.error === "string" ? rec.error : "quant_layers_failed";
      return { ok: false, error: `HTTP ${res.status} — ${why}`, data: null, fetchedAt: null };
    }
    if (rec?.ok !== true) {
      return { ok: false, error: "respuesta sin ok=true", data: null, fetchedAt: null };
    }
    return {
      ok: true,
      error: null,
      data: body as QuantLayersResponse,
      fetchedAt: new Date().toISOString(),
    };
  } catch (e) {
    // AbortError es una cancelación deliberada del poll, no un fallo del sistema.
    const msg = e instanceof Error ? e.message : String(e);
    return { ok: false, error: msg, data: null, fetchedAt: null };
  }
}

// ─── Presentación honesta (null ⇒ "—", nunca 0) ──────────────────────────────

export const NOT_COMPUTED = "—";

const finite = (v: number | null | undefined): v is number => typeof v === "number" && Number.isFinite(v);

export function fmtUsd(v: number | null | undefined, digits = 2): string {
  if (!finite(v)) return NOT_COMPUTED;
  const sign = v < 0 ? "-" : "";
  return `${sign}$${Math.abs(v).toLocaleString("en-US", {
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  })}`;
}

export function fmtNum(v: number | null | undefined, digits = 4): string {
  if (!finite(v)) return NOT_COMPUTED;
  return v.toLocaleString("en-US", { minimumFractionDigits: digits, maximumFractionDigits: digits });
}

/** bps con signo; el signo importa (una ruta que pierde no se maquilla). */
export function fmtBps(v: number | null | undefined): string {
  if (!finite(v)) return NOT_COMPUTED;
  const sign = v > 0 ? "+" : "";
  return `${sign}${v.toFixed(1)} bps`;
}

export function fmtPct(v: number | null | undefined, digits = 3): string {
  if (!finite(v)) return NOT_COMPUTED;
  const sign = v > 0 ? "+" : "";
  return `${sign}${v.toFixed(digits)}%`;
}

/** Peso logarítmico: se muestran 6 decimales porque w suele ser pequeño. */
export function fmtWeight(v: number | null | undefined): string {
  if (!finite(v)) return NOT_COMPUTED;
  return `${v > 0 ? "+" : ""}${v.toFixed(6)}`;
}

export function shortKey(key: string, max = 34): string {
  if (key.length <= max) return key;
  return `${key.slice(0, max - 1)}…`;
}

export function shortAddress(addr: string): string {
  if (addr.length <= 12) return addr;
  return `${addr.slice(0, 6)}…${addr.slice(-4)}`;
}

export function verdictVariant(v: QuantVerdict | string): BadgeProps["variant"] {
  switch (v) {
    case "EJECUTAR":
      return "success";
    case "MARGINAL":
      return "warning";
    case "RECHAZAR":
      return "destructive";
    default:
      return "secondary";
  }
}

export function statusVariant(s: QuantStatus | string): BadgeProps["variant"] {
  switch (s) {
    case "READY_TO_SIMULATE":
      return "success";
    case "PASS_NET":
    case "SIZED":
      return "info";
    case "NO_NEGATIVE_CYCLE":
    case "INSUFFICIENT_DEPTH":
    case "SLIPPAGE_OR_SIZE":
    case "NET_BELOW_TARGET":
      return "warning";
    case "QUOTE_INCOMPLETE":
      return "destructive";
    default:
      return "outline";
  }
}

// ─── Álgebra de pantalla (pura, testeable) ───────────────────────────────────

export interface QuantGridRow {
  routeKey: string;
  hops: number;
  status: QuantStatus;
  verdict: QuantVerdict;
  signal: boolean;
  sumW: number | null;
  discoveryReturnPct: number | null;
  bindingBoundUsd: number | null;
  sizingUsd: number | null;
  finalUsd: number | null;
  grossUsd: number | null;
  totalCostUsd: number | null;
  netUsd: number | null;
  netBps: number | null;
  whyNot: string | null;
  legs: QuantLeg[];
}

/**
 * Une 06_ROUTES con 08_ROUTE_PNL por routeKey. Una ruta sin su P&L medido NO se
 * inventa: entra con sus cifras en `null` (y la UI la marca no computada).
 */
export function buildGrid(routes: QuantRoute[], pnls: QuantPnl[]): QuantGridRow[] {
  const pnlByKey = new Map(pnls.map((p) => [p.routeKey, p]));
  const rows: QuantGridRow[] = routes.map((r) => {
    const p = pnlByKey.get(r.routeKey);
    return {
      routeKey: r.routeKey,
      hops: r.hops,
      status: p?.status ?? r.status,
      verdict: p?.verdict ?? "NO VIABLE",
      signal: r.signal,
      sumW: numOrNull(r.sumW),
      discoveryReturnPct: numOrNull(r.discoveryReturnPct),
      bindingBoundUsd: numOrNull(r.bindingBoundUsd),
      sizingUsd: numOrNull(p?.sizingUsd ?? r.sizingUsd),
      finalUsd: numOrNull(p?.finalUsd),
      grossUsd: numOrNull(p?.grossUsd),
      totalCostUsd: numOrNull(p?.totalCostUsd),
      netUsd: numOrNull(p?.netUsd),
      netBps: numOrNull(p?.netBps),
      whyNot: p?.whyNot ?? r.whyNot,
      legs: r.legs,
    };
  });
  return sortRows(rows);
}

const numOrNull = (v: number | null | undefined): number | null => (finite(v) ? v : null);

/** Mejor red primero; las filas sin cifras van al final, nunca intercaladas. */
export function sortRows(rows: QuantGridRow[]): QuantGridRow[] {
  return rows.slice().sort((a, b) => {
    if (a.netBps == null && b.netBps == null) return a.hops - b.hops || a.routeKey.localeCompare(b.routeKey);
    if (a.netBps == null) return 1;
    if (b.netBps == null) return -1;
    if (b.netBps !== a.netBps) return b.netBps - a.netBps;
    return a.hops - b.hops;
  });
}

/** El corte honesto de la grilla: con cifras vs declaradas no computadas. */
export function splitGrid(rows: QuantGridRow[]): { withFigures: QuantGridRow[]; noFigures: QuantGridRow[] } {
  const withFigures: QuantGridRow[] = [];
  const noFigures: QuantGridRow[] = [];
  for (const r of rows) (r.netBps == null ? noFigures : withFigures).push(r);
  return { withFigures, noFigures };
}

export function reasonBreakdown(
  notComputed: QuantLayerNotComputed[],
): Array<{ reason: string; count: number }> {
  const acc = new Map<string, number>();
  for (const n of notComputed) acc.set(n.reason, (acc.get(n.reason) ?? 0) + 1);
  return Array.from(acc.entries())
    .map(([reason, count]) => ({ reason, count }))
    .sort((a, b) => b.count - a.count || a.reason.localeCompare(b.reason));
}

export interface FunnelStep {
  key: string;
  label: string;
  value: number;
  /** Denominador del paso anterior, para leer la conversión. */
  of: number | null;
}

/** 09_DASHBOARD como embudo: pools → edges → routes → señal → viable → listo. */
export function funnelSteps(d: QuantDashboard): FunnelStep[] {
  const steps: Array<{ key: string; label: string; value: number }> = [
    { key: "pools", label: "Pools distintas", value: d.pools },
    { key: "edges", label: "Aristas medidas", value: d.edges },
    { key: "routes", label: "Rutas", value: d.routes },
    { key: "signal", label: "Con señal (Σw<0)", value: d.withSignal },
    { key: "viable", label: "Viables (bound ok)", value: d.viable },
    { key: "executable", label: "Listas p/ simular", value: d.executable },
  ];
  return steps.map((s, i) => ({ ...s, of: i === 0 ? null : (steps[i - 1]?.value ?? null) }));
}

/** El veredicto dominante de la ventana — sin maquillar el signo. */
export function windowReadout(rows: QuantGridRow[]): {
  withFigures: number;
  execute: number;
  marginal: number;
  reject: number;
  noViable: number;
  bestNetBps: number | null;
} {
  let execute = 0;
  let marginal = 0;
  let reject = 0;
  let noViable = 0;
  let best: number | null = null;
  for (const r of rows) {
    if (r.netBps == null) {
      noViable += 1;
      continue;
    }
    if (best == null || r.netBps > best) best = r.netBps;
    if (r.verdict === "EJECUTAR") execute += 1;
    else if (r.verdict === "MARGINAL") marginal += 1;
    else reject += 1;
  }
  return { withFigures: execute + marginal + reject, execute, marginal, reject, noViable, bestNetBps: best };
}
