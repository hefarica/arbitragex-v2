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
import {
  DEFAULT_QUANT_CONFIG,
  buildDashboard,
  buildLegs,
  buildPnl,
  buildRoutes,
  fairByPair,
  type LegInput,
  type PnlView,
  type RouteView,
} from "../quant/layers.js";

interface Logger {
  info?: (o: unknown, m?: string) => void;
  warn?: (o: unknown, m?: string) => void;
}

interface Row {
  route_group_key: string;
  hops: number;
  pools: string[] | null;
  dexes: string[] | null;
  decimals: Record<string, number> | null;
  legs: Array<{ token_in: string; token_out: string; amount_in_wei: string; amount_out_wei: string }> | null;
  principal_usd: number | string | null;
  gross_usd: number | string | null;
  net_usd: number | string | null;
  detected_at: string;
}

const LIVE_QUERY = `
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
    jsonb_array_length(o.economics->'legs') AS hops,
    o.route_metadata->'pool_addresses' AS pools,
    o.route_metadata->'dex_adapters' AS dexes,
    o.route_metadata->'decimals'->'map' AS decimals,
    o.economics->'legs' AS legs,
    (o.economics->>'amount_in_usd')::float  AS principal_usd,
    (o.economics->>'gross_profit_usd')::float AS gross_usd,
    (o.economics->>'net_profit_usd')::float  AS net_usd,
    to_char(o.detected_at, 'HH24:MI:SS')     AS detected_at
  FROM opportunities o
  WHERE o.detected_at >= NOW() - ($1::int * INTERVAL '1 minute')
    AND (o.economics->>'computation_status') = 'computed'
    AND jsonb_array_length(COALESCE(o.economics->'legs', '[]'::jsonb)) BETWEEN 2 AND 7
  ORDER BY o.detected_at DESC
  LIMIT $2
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

export function mountQuantLayers(app: Express, deps: { pool: Pool | null; logger?: Logger }): void {
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
      const q = await pool.query<Row>(LIVE_QUERY, [windowMinutes, limit]);
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

      // ── 2. fair por par dirigido (mediana del cross-section de la ventana) ──
      const fair = fairByPair(
        parsed.flatMap((p) =>
          p.legs.map((l) => ({ tokenIn: l.tokenIn, tokenOut: l.tokenOut, spot: l.amountOut / l.amountIn })),
        ),
      );

      // ── 3. Capas 05-09 ─────────────────────────────────────────────────────
      const routes: RouteView[] = [];
      const pnls: PnlView[] = [];
      for (const p of parsed) {
        const legs = buildLegs(p.legs as Array<LegInput>, fair, DEFAULT_QUANT_CONFIG);
        const route = buildRoutes(p.row.route_group_key, legs, DEFAULT_QUANT_CONFIG, {
          maxBlockAgeBlocks: null,
        });
        const principalUsd = num(p.row.principal_usd);
        const grossUsd = num(p.row.gross_usd);
        const finalUsd = principalUsd != null && grossUsd != null ? principalUsd + grossUsd : null;
        const pnl = buildPnl(route, { finalUsd, principalUsd }, DEFAULT_QUANT_CONFIG);
        routes.push(route);
        pnls.push(pnl);
      }
      const dashboard = buildDashboard(routes, pnls, topN);

      res.setHeader("Cache-Control", "private, no-cache, no-store");
      res.json({
        ok: true,
        window_minutes: windowMinutes,
        generated_at: new Date().toISOString(),
        config: DEFAULT_QUANT_CONFIG,
        rows_in_window: q.rows.length,
        layers: {
          routes,
          pnl: pnls,
          dashboard,
        },
        not_computed: notComputed.slice(0, 50),
        not_computed_count: notComputed.length,
        notes: [
          "F_e = spot/fair con el spot MEDIDO (post-fee): el fee ya está dentro del spot.",
          "fair = mediana de las tasas realizadas del mismo par dirigido en la ventana (equivalente data-driven de 04_ORACLE).",
          "bound_usd = 0.5% × profundidad: en el wire no viaja la profundidad por pata, así que el bound queda 'no computado' y el sizing usa el principal medido.",
          "verdict: EJECUTAR ⇔ net_bps ≥ 200 (2%), MARGINAL ⇔ 0 < net_bps < 200.",
        ],
      });
    } catch (err) {
      deps.logger?.warn?.({ event: "quant.layers.failed", err: String(err) }, "quant layers query failed");
      res.status(500).json({ ok: false, error: "quant_layers_failed", detail: String(err) });
    }
  });
}
