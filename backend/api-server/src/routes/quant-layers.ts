/**
 * GET /api/quant/layers — el libro cuantitativo servido por la dapp.
 *
 * Lee las detecciones MEDIDAS de la ventana (economics.computed + cadena de patas)
 * y aplica el álgebra de `src/quant/layers.ts` (idéntica a las hojas 05-09 del
 * Excel). No recalcula la economía en USD: usa la medición del searcher
 * (`amount_in_usd`, `gross_profit_usd`) y sólo añade la capa de álgebra relativa
 * (F_e, w, Σw, bound, sizing) más la escalera de costes y el gate de 2 %.
 *
 * R8: una fila sin cadena medida no entra (se cuenta en `not_computed`).
 */
import type { Express, Request, Response } from "express";
import type { Pool } from "pg";
import type { Redis } from "ioredis";
import {
  DEFAULT_QUANT_CONFIG,
  buildDashboard,
  buildLegs,
  buildPnl,
  buildRoutes,
  fairBasisCounts,
  fairByPair,
  fairFromUsdPrices,
  mergeFair,
  type FairRef,
  type LegInput,
  type LedgerCosts,
  type PnlView,
  type RouteView,
} from "../quant/layers.js";
import {
  getTradingConfigForChain,
  type TradingConfigSnapshot,
} from "../simulation/tradingConfigSnapshot.js";

interface Logger {
  info?: (o: unknown, m?: string) => void;
  warn?: (o: unknown, m?: string) => void;
}

/**
 * QUANT-FAIR-01 — registro de tokens (dirección → símbolo) para poder cruzar el
 * wire (direcciones) con el oráculo de precios (símbolos). Caché en proceso de
 * 60 s: el registro cambia por descubrimiento, no por request.
 */
const TOKEN_SYMBOL_TTL_MS = 60_000;
let tokenSymbolCache: { at: number; map: Map<string, string> } | null = null;

async function loadTokenSymbols(pool: Pool): Promise<Map<string, string>> {
  const now = Date.now();
  if (tokenSymbolCache != null && now - tokenSymbolCache.at < TOKEN_SYMBOL_TTL_MS) {
    return tokenSymbolCache.map;
  }
  const map = new Map<string, string>();
  try {
    const q = await pool.query<{ address: string; symbol: string }>(
      `SELECT lower(address) AS address, symbol FROM tokens WHERE symbol IS NOT NULL`,
    );
    for (const r of q.rows) map.set(r.address, r.symbol);
  } catch {
    // R8: sin registro el oráculo no se puede cruzar — se sigue con la mediana
    // declarada, nunca con una tasa inventada.
  }
  tokenSymbolCache = { at: now, map };
  return map;
}

interface Row {
  route_group_key: string;
  chain_id: number;
  hops: number;
  pools: string[] | null;
  dexes: string[] | null;
  decimals: Record<string, number> | null;
  legs: Array<{ token_in: string; token_out: string; amount_in_wei: string; amount_out_wei: string }> | null;
  principal_usd: number | string | null;
  gross_usd: number | string | null;
  net_usd: number | string | null;
  /** C-16: ledger de costes del productor (la fila manda). */
  gas_usd: number | string | null;
  flash_fee_usd: number | string | null;
  bribe_usd: number | string | null;
  other_costs_usd: number | string | null;
  /** Bloque de cotización: existe en el ledger, la query lo tiraba. */
  quote_block: number | string | null;
  detected_at: string;
}

/**
 * QUANT-DEDUP-01 (medido en vivo el 2026-09-29): el `route_group_key` identifica
 * la ruta por (cadena, estrategia, tokens, dexes) — NO por pools — así que las
 * re-detecciones de la misma ruta entran como filas distintas. La primera
 * respuesta real devolvió 40 "rutas" que eran 1 ruta repetida 40 veces, con el
 * Top copado por duplicados. Se elige UN representante por ruta: el de mejor
 * net medido (misma semántica que ROUTE-REP-01 en /opportunities/live), y esos
 * son los que entran a las capas.
 */
const REPRESENTATIVE_QUERY = `
  WITH recent AS (
    SELECT
      concat_ws('|',
        o.chain_id::text,
        COALESCE(o.chain_id_out::text, ''),
        COALESCE(o.strategy_kind, ''),
        o.token_in,
        o.token_out,
        o.dex_a,
        COALESCE(o.dex_b, '')
      ) AS route_group_key,
      o.chain_id AS chain_id,
      jsonb_array_length(o.economics->'legs') AS hops,
      o.route_metadata->'pool_addresses' AS pools,
      o.route_metadata->'dex_adapters' AS dexes,
      o.route_metadata->'decimals'->'map' AS decimals,
      o.economics->'legs' AS legs,
      (o.economics->>'amount_in_usd')::float  AS principal_usd,
      (o.economics->>'gross_profit_usd')::float AS gross_usd,
      (o.economics->>'net_profit_usd')::float  AS net_usd,
      (o.economics->>'gas_usd')::float         AS gas_usd,
      (o.economics->>'flash_fee_usd')::float   AS flash_fee_usd,
      (o.economics->>'bribe_usd')::float       AS bribe_usd,
      (o.economics->>'other_costs_usd')::float AS other_costs_usd,
      (o.economics->>'quote_block')::bigint    AS quote_block,
      to_char(o.detected_at, 'HH24:MI:SS')     AS detected_at
    FROM opportunities o
    WHERE o.detected_at >= NOW() - ($1::int * INTERVAL '1 minute')
      AND (o.economics->>'computation_status') = 'computed'
      AND jsonb_array_length(COALESCE(o.economics->'legs', '[]'::jsonb)) BETWEEN 2 AND 7
    ORDER BY o.detected_at DESC
    LIMIT $2
  )
  SELECT DISTINCT ON (route_group_key) *
  FROM recent
  ORDER BY route_group_key, net_usd DESC NULLS LAST
`;

const num = (v: unknown): number | null => {
  if (v == null) return null;
  const n = typeof v === "number" ? v : Number(v);
  return Number.isFinite(n) ? n : null;
};

/** wei → unidades humanas con los decimales del token (el wire los trae). */
function human(wei: string, decimals: number | undefined): number | null {
  const n = Number(wei);
  if (!Number.isFinite(n)) return null;
  const d = Number.isInteger(decimals) ? (decimals as number) : 18;
  return n / 10 ** d;
}

/**
 * C-16 — el ledger de costes de la fila, tal cual. Si la fila no trae ninguno de
 * los cuatro (pre-migración), null ⇒ la capa lo declara `config_ladder`; si
 * trae algunos pero no todos, se pasa igual y `buildPnl` falla cerrado
 * (`measured_incomplete`). Nunca se completa con la constante.
 */
function rowCosts(row: Row): LedgerCosts | null {
  const gasUsd = num(row.gas_usd);
  const flashUsd = num(row.flash_fee_usd);
  const tipUsd = num(row.bribe_usd);
  const otherCostsUsd = num(row.other_costs_usd);
  if (gasUsd == null && flashUsd == null && tipUsd == null && otherCostsUsd == null) return null;
  return { gasUsd, flashUsd, tipUsd, otherCostsUsd };
}

/**
 * C-16 — `resolve_gas_price_gwei(0.0, 0.0)` del productor, replicado en TS para
 * PUBLICAR los supuestos del ledger (no se usa para calcular el coste: el coste
 * es el `gas_usd` de la fila). Con `DynamicBasefeePlusTip` y sin señal viva:
 * 0 + max(1, 0) = 1 gwei.
 */
function resolveGasGwei(snap: TradingConfigSnapshot): number {
  switch (snap.gas_price_strategy) {
    case "fixed":
      return snap.fixed_gas_price_gwei ?? 0;
    case "dynamic_basefee_plus_tip":
      return 0 + Math.max(1, 0);
    case "percentile_75":
      return 0;
    default:
      return 0;
  }
}

export function mountQuantLayers(
  app: Express,
  deps: { pool: Pool | null; redis?: Redis | null; logger?: Logger },
): void {
  const { pool } = deps;

  app.get("/api/quant/layers", async (req: Request, res: Response) => {
    if (pool == null) {
      // R8: sin base de datos no hay capa que servir — se dice, no se inventa.
      res.status(503).json({ ok: false, error: "db_unavailable" });
      return;
    }
    const windowMinutes = Math.max(1, Math.min(24 * 60, Number(req.query["window_minutes"] ?? 60)));
    const limit = Math.max(1, Math.min(1000, Number(req.query["limit"] ?? 300)));
    const topN = Math.max(1, Math.min(50, Number(req.query["top"] ?? 10)));

    try {
      const q = await pool.query<Row>(REPRESENTATIVE_QUERY, [windowMinutes, limit]);
      const notComputed: Array<{ route_group_key: string; reason: string }> = [];

      // ── 1. Normalizar patas medidas → spots por par (cross-section) ─────────
      type Parsed = { row: Row; legs: LegInput[] };
      const parsed: Parsed[] = [];
      for (const row of q.rows) {
        const chain = row.legs ?? [];
        const dec = row.decimals ?? {};
        if (chain.length < 2 || chain.length !== (row.hops ?? 0)) {
          notComputed.push({ route_group_key: row.route_group_key, reason: "measured_leg_chain_incomplete" });
          continue;
        }
        const legs: LegInput[] = [];
        let ok = true;
        for (let i = 0; i < chain.length; i++) {
          const l = chain[i];
          if (l == null) {
            ok = false;
            break;
          }
          const aIn = human(l.amount_in_wei, dec[l.token_in?.toLowerCase?.()] ?? dec[l.token_in]);
          const aOut = human(l.amount_out_wei, dec[l.token_out?.toLowerCase?.()] ?? dec[l.token_out]);
          if (aIn == null || aOut == null || aIn <= 0) {
            ok = false;
            break;
          }
          legs.push({
            poolAddress: (row.pools ?? [])[i] ?? "",
            dex: (row.dexes ?? [])[i] ?? null,
            poolType: /v3/i.test((row.dexes ?? [])[i] ?? "") ? "V3" : /v2|sushi|pancake|aerodrome/i.test((row.dexes ?? [])[i] ?? "") ? "V2" : null,
            tokenIn: l.token_in,
            tokenOut: l.token_out,
            amountIn: aIn,
            amountOut: aOut,
            depthUsd: null,        // no viene en el wire: el bound queda "no computado" (R8)
            liquidity: null,
            sqrtPriceX96: null,
            priceInUsd: null,
          });
        }
        if (!ok) {
          notComputed.push({ route_group_key: row.route_group_key, reason: "leg_amounts_unpriced" });
          continue;
        }
        parsed.push({ row, legs });
      }

      // ── 2. fair por par dirigido ───────────────────────────────────────────
      // QUANT-FAIR-01: PRIMERO el oráculo (price_usd del stack soberano, vía
      // trading_config), y sólo donde no llegue, la mediana del cross-section
      // declarada como tal. Con la mediana sola, F_e ≡ 1 y la capa nunca ve nada.
      const fallbackFair = fairByPair(
        parsed.flatMap((p) =>
          p.legs.map((l) => ({ tokenIn: l.tokenIn, tokenOut: l.tokenOut, spot: l.amountOut / l.amountIn })),
        ),
      );

      const priceUsdByToken = new Map<string, number>();
      const snapshots = new Map<number, TradingConfigSnapshot>();
      let oracleChains = 0;
      for (const chainId of new Set(parsed.map((p) => (Number.isFinite(p.row.chain_id) ? p.row.chain_id : 1)))) {
        const snap = await getTradingConfigForChain(deps.redis ?? null, chainId);
        if (snap == null) continue;
        oracleChains += 1;
        snapshots.set(chainId, snap);
        const symbols = await loadTokenSymbols(pool);
        for (const [address, symbol] of symbols) {
          const px = snap.token_prices_usd[symbol];
          if (typeof px === "number" && Number.isFinite(px) && px > 0) priceUsdByToken.set(address, px);
        }
      }
      const oracleFair = fairFromUsdPrices(
        parsed.flatMap((p) => p.legs.map((l) => ({ tokenIn: l.tokenIn, tokenOut: l.tokenOut }))),
        priceUsdByToken,
      );
      const fair: Map<string, FairRef> = mergeFair(oracleFair, fallbackFair);

      // ── 3. Capas 05-09 ─────────────────────────────────────────────────────
      const routes: RouteView[] = [];
      const pnls: PnlView[] = [];
      const basisTotals: Record<string, number> = { oracle_usd: 0, cross_section_median: 0, none: 0 };
      for (const p of parsed) {
        const legs = buildLegs(p.legs as Array<LegInput>, fair, DEFAULT_QUANT_CONFIG);
        for (const [k, v] of Object.entries(fairBasisCounts(legs))) basisTotals[k] = (basisTotals[k] ?? 0) + v;
        const quoteBlock = num(p.row.quote_block);
        const route = buildRoutes(p.row.route_group_key, legs, DEFAULT_QUANT_CONFIG, {
          maxBlockAgeBlocks: null,
          quoteBlock: quoteBlock != null ? Math.trunc(quoteBlock) : null,
        });
        const principalUsd = num(p.row.principal_usd);
        const grossUsd = num(p.row.gross_usd);
        const finalUsd = principalUsd != null && grossUsd != null ? principalUsd + grossUsd : null;
        const pnl = buildPnl(
          route,
          {
            finalUsd,
            principalUsd,
            costs: rowCosts(p.row),
            measuredNetUsd: num(p.row.net_usd),
          },
          DEFAULT_QUANT_CONFIG,
        );
        routes.push(route);
        pnls.push(pnl);
      }
      const dashboard = buildDashboard(routes, pnls, topN);

      // ── 4. C-16 · reconciliación con denominador explícito ─────────────────
      // La capa deja de ser segunda contabilidad sólo si CADA campo cuadra con
      // el ledger de la fila: net, gross y los cuatro componentes. Δmax ≤ 0.005.
      const pnlByKey = new Map(pnls.map((p) => [p.routeKey, p]));
      const excluded: Record<string, number> = {};
      const reconRows: Array<{
        route_group_key: string;
        net_delta_usd: number | null;
        gross_delta_usd: number | null;
        gas_delta_usd: number | null;
        flash_delta_usd: number | null;
        bribe_delta_usd: number | null;
        other_delta_usd: number | null;
      }> = [];
      let nPresent = 0;
      let netOk = 0;
      let grossOk = 0;
      let gasOk = 0;
      let flashOk = 0;
      let bribeOk = 0;
      let otherOk = 0;
      let maxNetDelta = 0;
      let maxGrossDelta = 0;
      let maxComponentDelta = 0;
      for (const p of parsed) {
        const costs = rowCosts(p.row);
        const costsPresent =
          costs != null &&
          costs.gasUsd != null &&
          costs.flashUsd != null &&
          costs.tipUsd != null &&
          costs.otherCostsUsd != null;
        if (!costsPresent) {
          excluded["cost_components_incomplete"] = (excluded["cost_components_incomplete"] ?? 0) + 1;
          continue;
        }
        const pnl = pnlByKey.get(p.row.route_group_key);
        const grossLedger = num(p.row.gross_usd);
        const netLedger = num(p.row.net_usd);
        if (pnl == null || pnl.netUsd == null || pnl.grossUsd == null || grossLedger == null || netLedger == null) {
          excluded["principal_or_gross_unmeasured"] = (excluded["principal_or_gross_unmeasured"] ?? 0) + 1;
          continue;
        }
        nPresent += 1;
        const netDelta = Math.abs(pnl.netUsd - netLedger);
        const grossDelta = Math.abs(pnl.grossUsd - grossLedger);
        const gasDelta = Math.abs((pnl.gasUsd as number) - (costs.gasUsd as number));
        const flashDelta = Math.abs((pnl.flashUsd as number) - (costs.flashUsd as number));
        const bribeDelta = Math.abs((pnl.tipUsd as number) - (costs.tipUsd as number));
        const otherDelta = Math.abs((pnl.otherCostsUsd as number) - (costs.otherCostsUsd as number));
        if (netDelta <= 0.005) netOk += 1;
        if (grossDelta <= 0.005) grossOk += 1;
        if (gasDelta <= 0.005) gasOk += 1;
        if (flashDelta <= 0.005) flashOk += 1;
        if (bribeDelta <= 0.005) bribeOk += 1;
        if (otherDelta <= 0.005) otherOk += 1;
        maxNetDelta = Math.max(maxNetDelta, netDelta);
        maxGrossDelta = Math.max(maxGrossDelta, grossDelta);
        maxComponentDelta = Math.max(maxComponentDelta, gasDelta, flashDelta, bribeDelta, otherDelta);
        reconRows.push({
          route_group_key: p.row.route_group_key,
          net_delta_usd: netDelta,
          gross_delta_usd: grossDelta,
          gas_delta_usd: gasDelta,
          flash_delta_usd: flashDelta,
          bribe_delta_usd: bribeDelta,
          other_delta_usd: otherDelta,
        });
      }
      for (const nc of notComputed) {
        excluded[nc.reason] = (excluded[nc.reason] ?? 0) + 1;
      }
      const reconciliation = {
        window: "request" as const,
        tolerance_usd: 0.005,
        n_present: nPresent,
        n_excluded: Object.values(excluded).reduce((a, b) => a + b, 0),
        excluded_by_reason: excluded,
        evidence: nPresent >= 10,
        evidence_note:
          nPresent >= 10
            ? "N_present >= 10: el cuadre cuenta como evidencia."
            : "N_present < 10: NO es evidencia — amplía la ventana antes de usarlo.",
        net: { ok: netOk, n: nPresent, max_abs_delta_usd: maxNetDelta },
        gross: { ok: grossOk, n: nPresent, max_abs_delta_usd: maxGrossDelta },
        components: {
          gas_usd: { ok: gasOk, n: nPresent },
          flash_fee_usd: { ok: flashOk, n: nPresent },
          bribe_usd: { ok: bribeOk, n: nPresent },
          other_costs_usd: { ok: otherOk, n: nPresent },
          max_abs_delta_usd: maxComponentDelta,
        },
        rows: reconRows,
      };

      // ── 5. C-16 · supuestos publicados del gas (ESTIMADO, no medido) ───────
      const gasAssumptionsByChain = Array.from(snapshots.entries()).map(([chainId, snap]) => {
        const units = snap.gas_estimate_units;
        const gwei = resolveGasGwei(snap);
        const nativeUsd = snap.base_token_price_usd;
        const sample = parsed.find(
          (p) => p.row.chain_id === chainId && num(p.row.gas_usd) != null,
        );
        const sampleGas = sample != null ? num(sample.row.gas_usd) : null;
        const impliedNativeUsd =
          sampleGas != null && units > 0 && gwei > 0 ? sampleGas / ((units * gwei * 1e9) / 1e18) : null;
        return {
          chain_id: chainId,
          strategy: snap.gas_price_strategy,
          units,
          gwei,
          native_usd: nativeUsd,
          implied_native_usd: impliedNativeUsd,
        };
      });

      res.setHeader("Cache-Control", "private, no-cache, no-store");
      res.json({
        ok: true,
        window_minutes: windowMinutes,
        generated_at: new Date().toISOString(),
        config: DEFAULT_QUANT_CONFIG,
        rows_in_window: q.rows.length,
        fair_basis: {
          chains_con_oraculo: oracleChains,
          tokens_con_precio: priceUsdByToken.size,
          aristas: basisTotals,
        },
        layers: {
          routes,
          pnl: pnls,
          dashboard,
        },
        reconciliation,
        cost_provenance: {
          gas_usd: "estimado_ledger",
          flash_fee_usd: "estimado_kernel",
          bribe_usd: "estimado_kernel",
          other_costs_usd: "estimado_kernel",
          note:
            "Ningún componente es una inclusión on-chain: gas_usd es la estimación del ledger (units×gwei×native, sin receipt) y los otros tres son estimaciones del kernel.",
        },
        gas_assumptions: {
          source:
            "trading_config: gas_estimate_units × resolve_gas_price_gwei(0,0) × base_token_price_usd (misma fórmula que gas_cost_usd() del productor)",
          by_chain: gasAssumptionsByChain,
        },
        out_of_band_count: pnls.filter((p) => p.verdict === "OUT_OF_BAND").length,
        leg_value_jump_count: routes.filter((r) => r.status === "LEG_VALUE_JUMP").length,
        not_computed: notComputed.slice(0, 50),
        not_computed_count: notComputed.length,
        notes: [
          "F_e = spot/fair con el spot MEDIDO (post-fee): el fee ya está dentro del spot.",
          "QUANT-FAIR-01: fair = price_usd(token_in)/price_usd(token_out) con el oráculo del stack soberano (Binance WS + Chainlink vía trading_config.token_prices_usd), procedencia 'oracle_usd'. Donde el oráculo no llega se usa la mediana de las tasas realizadas del mismo par en la ventana, declarada como 'cross_section_median' — y esa mediana, sola, es degenerada (par visto una vez ⇒ F_e = 1 ⇒ sin señal: medido 40/40 rutas así el 2026-09-29). Cada arista publica su fair_basis.",
          "QUANT-DEDUP-01: una fila por ruta (route_group_key), la de mejor net medido — las re-detecciones no inflan el embudo ni el Top.",
          "QUANT-SIZING-NULL-01: sizing_usd = null cuando no hay bound vinculante (no computado), nunca 0 disfrazado de tamaño.",
          "bound_usd = 0.5% × profundidad: en el wire no viaja la profundidad por pata, así que el bound queda 'no computado' y el sizing usa el principal medido.",
          "QUANT-PNL-01: la desviación contra la cadena fair se publica aparte (deviation_vs_fair_usd) y NO suma al coste: el impacto ya está dentro del gross medido, sumarla contaría la misma pérdida dos veces.",
          "C-16: los costes son los cuatro componentes del ledger de la fila (gas_usd + flash_fee_usd + bribe_usd + other_costs_usd), no la constante 6+4/hop. Si la fila no los trae, la capa falla cerrado (measured_incomplete) en vez de inventarlos.",
          "C-16: ningún componente se llama «real». gas_usd es ESTIMADO por el ledger; flash/bribe/other son ESTIMADOS por el kernel. gas_assumptions publica units/gwei/native_usd usados por el productor.",
          "C-16: OUT_OF_BAND = el principal medido cae fuera de la banda [minBoundUsd, capitalUsd×utilizationCap]. Es una bandera de política declarada, no un umbral de coste: se publican principal y gross, y NO se publica ningún bps.",
          "C-16: LEG_VALUE_JUMP = |value_out(i) − value_in(i+1)| / value_in(i+1) > 1 bp. Σw y sizing se publican null; el salto de unidades se arregla aguas arriba, no se esconde en un bps.",
          "verdict: EJECUTAR ⇔ net_bps ≥ 200 (2%), MARGINAL ⇔ 0 < net_bps < 200. RECHAZAR ⇔ coste > gross. OUT_OF_BAND no es un veredicto de viabilidad.",
        ],
      });
    } catch (err) {
      deps.logger?.warn?.({ event: "quant.layers.failed", err: String(err) }, "quant layers query failed");
      res.status(500).json({ ok: false, error: "quant_layers_failed", detail: String(err) });
    }
  });
}
