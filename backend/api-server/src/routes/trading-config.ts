// Trading Config routes — operator-tunable strategy parameters per chain.
//
// Architecture:
//   GET  /api/v1/trading-config?chain_id=1   → Public read of effective config
//   GET  /admin/trading-config               → Admin: list all chains' configs
//   PUT  /admin/trading-config/:chain_id     → Admin: upsert + Redis push + audit
//
// Source of truth is PostgreSQL (`trading_config` table, migration 026).
// Every PUT writes to PG, mirrors to Redis (`arbx:trading_config:<chain_id>`),
// and PUBLISHes on `arbx:trading_config:changes` so searcher-rs reacts in <1s.
//
// No-hardcode doctrine: this endpoint is the ONLY way to seed strategy
// parameters. The system stays idle for any chain without a row.

import { Router, type Request, type Response } from "express";
import type { Pool } from "pg";
import type { Redis } from "ioredis";
import { z } from "zod";

export const TRADING_CONFIG_CHANNEL = "arbx:trading_config:changes";
export const tradingConfigKey = (chainId: number): string =>
  `arbx:trading_config:${chainId}`;

const GasStrategy = z.enum(["fixed", "dynamic_basefee_plus_tip", "percentile_75"]);

const TradingConfigSchema = z
  .object({
    capital_usd: z.number().nonnegative(),
    base_token_symbol: z.string().min(1).max(16),
    base_token_price_usd: z.number().positive(),
    allowed_token_symbols: z.array(z.string().min(1).max(16)).max(64),

    // BUG-2 fix (PriceOracle): per-token USD prices, operator-managed.
    token_prices_usd: z.record(z.string().min(1).max(16), z.number().positive()).default({}),

    // Simulation knobs (paper-trade preview, decoupled from operational capital).
    simulation_capital_usd: z.number().positive().nullable().optional(),
    simulation_per_token_amounts_usd: z
      .record(z.string().min(1).max(16), z.number().positive())
      .default({}),
    simulation_per_strategy_caps_usd: z
      .record(z.string().min(1).max(64), z.number().positive())
      .default({}),
    simulation_target_profit_usd: z.number().nonnegative().nullable().optional(),
    simulation_target_roi_pct: z.number().nonnegative().nullable().optional(),

    min_profit_usd: z.number().nonnegative(),
    min_roi_pct: z.number().nonnegative(),
    min_landing_probability: z.number().min(0).max(1).default(0.5),
    min_liquidity_confidence: z.number().min(0).max(1).default(0.7),
    max_token_risk_score: z.number().min(0).max(1).default(1.0),

    gas_price_strategy: GasStrategy.default("dynamic_basefee_plus_tip"),
    fixed_gas_price_gwei: z.number().nonnegative().nullable().optional(),
    gas_estimate_units: z.number().int().positive().default(250_000),
    max_slippage_pct: z.number().min(0).max(50),
    failure_risk_buffer_pct: z.number().min(0).default(0.001),
    flashloan_fee_pct: z.number().min(0).default(0.0009),

    enabled_strategies: z.array(z.string().min(1).max(64)).max(32).default([]),

    // Phase 2 route-finder: per-chain DEX allowlist.
    // NULL = "use all is_active dexes" (backwards-compatible default).
    enabled_dex_ids: z.array(z.string().uuid()).max(256).nullable().optional(),

    // Migration 047 (BE-01 Sprint A): cost-component scalars.
    // Bounds match DB CHECK constraints exactly.
    capital_cost_rate_annual_pct: z.number().min(0.0).max(50.0).default(0.0),
    ops_overhead_usd_per_attempt: z.number().min(0.0).max(100.0).default(0.01),

    // Migration 048 (BE-01 Sprint C): spread sanity gate (Comp. 9) + p_copied heuristic (Comp. 5).
    // Bounds match DB CHECK constraints exactly. p_copied_volume_threshold_usd has no DB CHECK
    // but the heuristic requires > 0 (log10 undefined at 0); enforced via Zod .positive().
    spread_sanity_mult: z.number().min(1.0).max(100.0).default(3.0),
    p_copied_volume_threshold_usd: z.number().positive().default(1_000_000.0),
    p_copied_max: z.number().min(0.0).max(1.0).default(0.5),

    enabled: z.boolean().default(true),
  })
  .superRefine((val, ctx) => {
    if (val.gas_price_strategy === "fixed" && (val.fixed_gas_price_gwei == null || val.fixed_gas_price_gwei <= 0)) {
      ctx.addIssue({
        code: z.ZodIssueCode.custom,
        path: ["fixed_gas_price_gwei"],
        message: "fixed gas strategy requires fixed_gas_price_gwei > 0",
      });
    }
  });

export type TradingConfigBody = z.infer<typeof TradingConfigSchema>;

interface Deps {
  pool: Pool | null;
  redis: Redis;
  requireAdminToken: (token: string) => (req: Request, res: Response, next: () => void) => void;
  adminToken: string;
  writeAudit: (
    action: string,
    actor: string,
    targetKind: string | null,
    targetId: string | null,
    before: unknown,
    after: unknown,
    ip: string | null,
    traceId: string | null,
  ) => Promise<void>;
  logger: { warn: (obj: object, msg?: string) => void; info: (obj: object, msg?: string) => void };
}

interface DbRow {
  chain_id: number;
  capital_usd: string;
  base_token_symbol: string;
  base_token_price_usd: string;
  allowed_token_symbols: string[];
  // BUG-2 fix + simulation knobs (migration 032).
  // pg returns JSONB as already-parsed objects; numeric NULLABLEs as string|null.
  token_prices_usd: Record<string, number>;
  simulation_capital_usd: string | null;
  simulation_per_token_amounts_usd: Record<string, number>;
  simulation_per_strategy_caps_usd: Record<string, number>;
  simulation_target_profit_usd: string | null;
  simulation_target_roi_pct: string | null;
  min_profit_usd: string;
  min_roi_pct: string;
  min_landing_probability: string;
  min_liquidity_confidence: string;
  max_token_risk_score: string;
  gas_price_strategy: "fixed" | "dynamic_basefee_plus_tip" | "percentile_75";
  fixed_gas_price_gwei: string | null;
  gas_estimate_units: number;
  max_slippage_pct: string;
  failure_risk_buffer_pct: string;
  flashloan_fee_pct: string;
  enabled_strategies: string[];
  enabled_dex_ids: string[] | null;
  // Migration 047 + 048: pg returns NUMERIC as string; cast in rowToRedisState.
  capital_cost_rate_annual_pct: string;
  ops_overhead_usd_per_attempt: string;
  spread_sanity_mult: string;
  p_copied_volume_threshold_usd: string;
  p_copied_max: string;
  enabled: boolean;
  updated_at: Date;
  updated_by: string;
  created_at: Date;
}

function rowToRedisState(row: DbRow): Record<string, unknown> {
  // Numerics stringified by pg → cast to number for Redis JSON consumed by Rust.
  // JSONB columns come back as already-parsed objects; pass through verbatim.
  // Nullable numerics: preserve null when absent (Rust deserialises as None).
  return {
    chain_id: row.chain_id,
    capital_usd: Number(row.capital_usd),
    base_token_symbol: row.base_token_symbol,
    base_token_price_usd: Number(row.base_token_price_usd),
    allowed_token_symbols: row.allowed_token_symbols,
    token_prices_usd: row.token_prices_usd ?? {},
    simulation_capital_usd:
      row.simulation_capital_usd == null ? null : Number(row.simulation_capital_usd),
    simulation_per_token_amounts_usd: row.simulation_per_token_amounts_usd ?? {},
    simulation_per_strategy_caps_usd: row.simulation_per_strategy_caps_usd ?? {},
    simulation_target_profit_usd:
      row.simulation_target_profit_usd == null ? null : Number(row.simulation_target_profit_usd),
    simulation_target_roi_pct:
      row.simulation_target_roi_pct == null ? null : Number(row.simulation_target_roi_pct),
    min_profit_usd: Number(row.min_profit_usd),
    min_roi_pct: Number(row.min_roi_pct),
    min_landing_probability: Number(row.min_landing_probability),
    min_liquidity_confidence: Number(row.min_liquidity_confidence),
    max_token_risk_score: Number(row.max_token_risk_score),
    gas_price_strategy: row.gas_price_strategy,
    fixed_gas_price_gwei: row.fixed_gas_price_gwei == null ? null : Number(row.fixed_gas_price_gwei),
    gas_estimate_units: row.gas_estimate_units,
    max_slippage_pct: Number(row.max_slippage_pct),
    failure_risk_buffer_pct: Number(row.failure_risk_buffer_pct),
    flashloan_fee_pct: Number(row.flashloan_fee_pct),
    enabled_strategies: row.enabled_strategies,
    enabled_dex_ids: row.enabled_dex_ids ?? null,
    // Migration 047 + 048: cast NUMERIC strings to numbers for Redis JSON / Rust deserialise.
    capital_cost_rate_annual_pct: Number(row.capital_cost_rate_annual_pct),
    ops_overhead_usd_per_attempt: Number(row.ops_overhead_usd_per_attempt),
    spread_sanity_mult: Number(row.spread_sanity_mult),
    p_copied_volume_threshold_usd: Number(row.p_copied_volume_threshold_usd),
    p_copied_max: Number(row.p_copied_max),
    enabled: row.enabled,
    updated_at: row.updated_at.toISOString(),
    updated_by: row.updated_by,
  };
}

export function buildTradingConfigRouter(deps: Deps): Router {
  const r = Router();

  // ── Public read ────────────────────────────────────────────────────────
  // Frontend /config/trading page calls this to render the form's initial values.
  r.get("/api/v1/trading-config", async (req: Request, res: Response) => {
    if (!deps.pool) {
      res.status(503).json({ error: "db_unavailable" });
      return;
    }
    const chainId = Number(req.query["chain_id"] ?? 1);
    if (!Number.isFinite(chainId) || chainId < 1) {
      res.status(400).json({ error: "invalid_chain_id" });
      return;
    }
    try {
      const q = await deps.pool.query<DbRow>(
        `SELECT chain_id, capital_usd, base_token_symbol, base_token_price_usd,
                allowed_token_symbols, token_prices_usd,
                simulation_capital_usd, simulation_per_token_amounts_usd,
                simulation_per_strategy_caps_usd,
                simulation_target_profit_usd, simulation_target_roi_pct,
                min_profit_usd, min_roi_pct,
                min_landing_probability, min_liquidity_confidence, max_token_risk_score,
                gas_price_strategy, fixed_gas_price_gwei, gas_estimate_units,
                max_slippage_pct, failure_risk_buffer_pct, flashloan_fee_pct,
                enabled_strategies, enabled_dex_ids,
                capital_cost_rate_annual_pct, ops_overhead_usd_per_attempt,
                spread_sanity_mult, p_copied_volume_threshold_usd, p_copied_max,
                enabled,
                updated_at, updated_by, created_at
           FROM trading_config
          WHERE chain_id = $1`,
        [chainId],
      );
      if (q.rowCount === 0) {
        res.status(200).json({ chain_id: chainId, configured: false });
        return;
      }
      res.status(200).json({ chain_id: chainId, configured: true, ...rowToRedisState(q.rows[0]!) });
    } catch (e) {
      deps.logger.warn({ event: "trading_config.read_failed", err: (e as Error).message });
      res.status(503).json({ error: "query_failed", detail: (e as Error).message });
    }
  });

  // ── Admin upsert ───────────────────────────────────────────────────────
  // Writes PG row, mirrors to Redis, publishes change. searcher-rs picks
  // up the new state on its next cache miss (≤1s).
  r.put(
    "/admin/trading-config/:chain_id",
    deps.requireAdminToken(deps.adminToken),
    async (req: Request, res: Response) => {
      if (!deps.pool) {
        res.status(503).json({ error: "db_unavailable" });
        return;
      }
      const chainId = Number(req.params["chain_id"]);
      if (!Number.isFinite(chainId) || chainId < 1) {
        res.status(400).json({ error: "invalid_chain_id" });
        return;
      }
      const parsed = TradingConfigSchema.safeParse(req.body);
      if (!parsed.success) {
        res.status(400).json({ error: "invalid_request", details: parsed.error.flatten() });
        return;
      }
      const body = parsed.data;
      const actor = req.header("x-arbx-actor") ?? "admin";
      try {
        const beforeQ = await deps.pool.query(
          `SELECT * FROM trading_config WHERE chain_id = $1`, [chainId],
        );
        const before = beforeQ.rows[0] ?? null;

        const upsertQ = await deps.pool.query<DbRow>(
          `INSERT INTO trading_config (
              chain_id, capital_usd, base_token_symbol, base_token_price_usd,
              allowed_token_symbols, token_prices_usd,
              simulation_capital_usd, simulation_per_token_amounts_usd,
              simulation_per_strategy_caps_usd,
              simulation_target_profit_usd, simulation_target_roi_pct,
              min_profit_usd, min_roi_pct,
              min_landing_probability, min_liquidity_confidence, max_token_risk_score,
              gas_price_strategy, fixed_gas_price_gwei, gas_estimate_units,
              max_slippage_pct, failure_risk_buffer_pct, flashloan_fee_pct,
              enabled_strategies, enabled_dex_ids, enabled, updated_by,
              capital_cost_rate_annual_pct, ops_overhead_usd_per_attempt,
              spread_sanity_mult, p_copied_volume_threshold_usd, p_copied_max
            )
            VALUES (
              $1,$2,$3,$4,$5,$6::jsonb,$7,$8::jsonb,$9::jsonb,$10,$11,
              $12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,$23,$24::uuid[],$25,$26,
              $27,$28,$29,$30,$31
            )
            ON CONFLICT (chain_id) DO UPDATE SET
              capital_usd = EXCLUDED.capital_usd,
              base_token_symbol = EXCLUDED.base_token_symbol,
              base_token_price_usd = EXCLUDED.base_token_price_usd,
              allowed_token_symbols = EXCLUDED.allowed_token_symbols,
              token_prices_usd = EXCLUDED.token_prices_usd,
              simulation_capital_usd = EXCLUDED.simulation_capital_usd,
              simulation_per_token_amounts_usd = EXCLUDED.simulation_per_token_amounts_usd,
              simulation_per_strategy_caps_usd = EXCLUDED.simulation_per_strategy_caps_usd,
              simulation_target_profit_usd = EXCLUDED.simulation_target_profit_usd,
              simulation_target_roi_pct = EXCLUDED.simulation_target_roi_pct,
              min_profit_usd = EXCLUDED.min_profit_usd,
              min_roi_pct = EXCLUDED.min_roi_pct,
              min_landing_probability = EXCLUDED.min_landing_probability,
              min_liquidity_confidence = EXCLUDED.min_liquidity_confidence,
              max_token_risk_score = EXCLUDED.max_token_risk_score,
              gas_price_strategy = EXCLUDED.gas_price_strategy,
              fixed_gas_price_gwei = EXCLUDED.fixed_gas_price_gwei,
              gas_estimate_units = EXCLUDED.gas_estimate_units,
              max_slippage_pct = EXCLUDED.max_slippage_pct,
              failure_risk_buffer_pct = EXCLUDED.failure_risk_buffer_pct,
              flashloan_fee_pct = EXCLUDED.flashloan_fee_pct,
              enabled_strategies = EXCLUDED.enabled_strategies,
              enabled_dex_ids = EXCLUDED.enabled_dex_ids,
              enabled = EXCLUDED.enabled,
              capital_cost_rate_annual_pct = EXCLUDED.capital_cost_rate_annual_pct,
              ops_overhead_usd_per_attempt = EXCLUDED.ops_overhead_usd_per_attempt,
              spread_sanity_mult = EXCLUDED.spread_sanity_mult,
              p_copied_volume_threshold_usd = EXCLUDED.p_copied_volume_threshold_usd,
              p_copied_max = EXCLUDED.p_copied_max,
              updated_by = EXCLUDED.updated_by,
              updated_at = NOW()
            RETURNING chain_id, capital_usd, base_token_symbol, base_token_price_usd,
                      allowed_token_symbols, token_prices_usd,
                      simulation_capital_usd, simulation_per_token_amounts_usd,
                      simulation_per_strategy_caps_usd,
                      simulation_target_profit_usd, simulation_target_roi_pct,
                      min_profit_usd, min_roi_pct,
                      min_landing_probability, min_liquidity_confidence, max_token_risk_score,
                      gas_price_strategy, fixed_gas_price_gwei, gas_estimate_units,
                      max_slippage_pct, failure_risk_buffer_pct, flashloan_fee_pct,
                      enabled_strategies, enabled_dex_ids,
                      capital_cost_rate_annual_pct, ops_overhead_usd_per_attempt,
                      spread_sanity_mult, p_copied_volume_threshold_usd, p_copied_max,
                      enabled,
                      updated_at, updated_by, created_at`,
          [
            chainId,
            body.capital_usd,
            body.base_token_symbol,
            body.base_token_price_usd,
            body.allowed_token_symbols,
            JSON.stringify(body.token_prices_usd ?? {}),
            body.simulation_capital_usd ?? null,
            JSON.stringify(body.simulation_per_token_amounts_usd ?? {}),
            JSON.stringify(body.simulation_per_strategy_caps_usd ?? {}),
            body.simulation_target_profit_usd ?? null,
            body.simulation_target_roi_pct ?? null,
            body.min_profit_usd,
            body.min_roi_pct,
            body.min_landing_probability,
            body.min_liquidity_confidence,
            body.max_token_risk_score,
            body.gas_price_strategy,
            body.fixed_gas_price_gwei ?? null,
            body.gas_estimate_units,
            body.max_slippage_pct,
            body.failure_risk_buffer_pct,
            body.flashloan_fee_pct,
            body.enabled_strategies,
            body.enabled_dex_ids ?? null,
            body.enabled,
            actor,
            body.capital_cost_rate_annual_pct,
            body.ops_overhead_usd_per_attempt,
            body.spread_sanity_mult,
            body.p_copied_volume_threshold_usd,
            body.p_copied_max,
          ],
        );
        const row = upsertQ.rows[0]!;
        const redisState = rowToRedisState(row);
        const json = JSON.stringify(redisState);
        await deps.redis.set(tradingConfigKey(chainId), json);
        const subs = await deps.redis.publish(TRADING_CONFIG_CHANNEL, json);

        await deps.writeAudit(
          "trading_config.upsert",
          actor,
          "trading_config",
          String(chainId),
          before,
          body,
          req.ip ?? null,
          (req as Request & { traceId?: string }).traceId ?? null,
        );

        deps.logger.info(
          { event: "trading_config.upsert", chain_id: chainId, actor, subscribers_notified: subs },
          "trading config persisted and broadcast",
        );
        res.status(200).json({ ok: true, chain_id: chainId, subscribers_notified: subs, ...redisState });
      } catch (e) {
        deps.logger.warn({ event: "trading_config.upsert_failed", err: (e as Error).message });
        res.status(500).json({ error: "db_error", detail: (e as Error).message });
      }
    },
  );

  // ── Admin list ─────────────────────────────────────────────────────────
  r.get(
    "/admin/trading-config",
    deps.requireAdminToken(deps.adminToken),
    async (_req: Request, res: Response) => {
      if (!deps.pool) {
        res.status(503).json({ error: "db_unavailable" });
        return;
      }
      try {
        const q = await deps.pool.query<DbRow>(
          `SELECT chain_id, capital_usd, base_token_symbol, base_token_price_usd,
                  allowed_token_symbols, token_prices_usd,
                  simulation_capital_usd, simulation_per_token_amounts_usd,
                  simulation_per_strategy_caps_usd,
                  simulation_target_profit_usd, simulation_target_roi_pct,
                  min_profit_usd, min_roi_pct,
                  min_landing_probability, min_liquidity_confidence, max_token_risk_score,
                  gas_price_strategy, fixed_gas_price_gwei, gas_estimate_units,
                  max_slippage_pct, failure_risk_buffer_pct, flashloan_fee_pct,
                  enabled_strategies, enabled_dex_ids,
                  capital_cost_rate_annual_pct, ops_overhead_usd_per_attempt,
                  spread_sanity_mult, p_copied_volume_threshold_usd, p_copied_max,
                  enabled,
                  updated_at, updated_by, created_at
             FROM trading_config
            ORDER BY chain_id`,
        );
        res.status(200).json({
          count: q.rowCount,
          items: q.rows.map(rowToRedisState),
        });
      } catch (e) {
        deps.logger.warn({ event: "trading_config.list_failed", err: (e as Error).message });
        res.status(503).json({ error: "query_failed", detail: (e as Error).message });
      }
    },
  );

  return r;
}
