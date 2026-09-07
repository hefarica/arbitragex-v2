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
import type { Server as IoServer } from "socket.io";
import { createHash, randomUUID } from "node:crypto";
import { z } from "zod";
import { broadcastRuntimeAck, type RuntimeAckBroadcast } from "../websocket.js";

export const TRADING_CONFIG_CHANNEL = "arbx:trading_config:changes";

// EMIT-04 (FE-MASTER §4): token-universe version counter — bumped by the admin
// PUT on every EFFECTIVE allowlist change (Redis INCR, monotonic per chain).
// Read by GET /api/v1/trading-config and POST /api/admin/tokens/resolve; the
// runtime_ack event emitted alongside carries resource="token_universe" so the
// frontend (useRuntimeAckSocket) correlates the mutation with the new version.
export const TOKEN_UNIVERSE_VERSION_PREFIX = "arbx:token_universe:version:";

export const tokenUniverseVersionKey = (chainId: number): string =>
  `${TOKEN_UNIVERSE_VERSION_PREFIX}${chainId}`;

/**
 * Canonical universe fingerprint (sha256 hex64) — SET semantics with TW-002
 * form normalization (trim + lowercase identity for both symbols and address
 * entries; the effective universe is what matters, not the declaration form):
 *   - reorder-only edits hash EQUAL → no version bump, no ACK churn;
 *   - `null` only for "no prior row" (first configuration ever).
 * The hex64 form doubles as `config_hash_after` in the runtime_ack wire
 * (RuntimeAckBroadcastSchema requires exactly that shape).
 */
export function universeFingerprint(
  entries: readonly string[] | null | undefined,
): string | null {
  if (entries === null || entries === undefined) return null;
  const norm = [
    ...new Set(
      entries
        .map((s) => s.trim().toLowerCase())
        .filter((s) => s.length > 0),
    ),
  ].sort();
  return createHash("sha256").update(JSON.stringify(norm), "utf8").digest("hex");
}

/** Canal de compatibilidad para sed-core y otros suscriptores legacy.
 *  Publicación dual: tanto TRADING_CONFIG_CHANNEL como HOT_RELOAD_CHANNEL
 *  reciben el mismo payload JSON. Los suscriptores pueden escuchar cualquiera.
 *  Migración futura: deprecar HOT_RELOAD_CHANNEL cuando todos los consumidores
 *  migren a TRADING_CONFIG_CHANNEL.
 */
export const HOT_RELOAD_CHANNEL = "arbx:config:hot_reload";

export const tradingConfigKey = (chainId: number): string =>
  `arbx:trading_config:${chainId}`;

// Dual-channel hot-reload (2026-05-14):
// - TRADING_CONFIG_CHANNEL ("arbx:trading_config:changes"): canal canónico actual
// - HOT_RELOAD_CHANNEL ("arbx:config:hot_reload"): canal de compatibilidad SOP-EDGE-001
// Ambos canales reciben el mismo payload. Los consumidores pueden migrar gradualmente.

const GasStrategy = z.enum(["fixed", "dynamic_basefee_plus_tip", "percentile_75"]);

// Migration 056 — per-strategy fine-grained config schema.
// Mirrors `shared_rs::trading_config::StrategyRuntimeConfig` 1:1.
// All fields default to permissive values so a missing `strategy_configs`
// entry behaves identically to the legacy chain-level toggle (no surprises).
const RouteConstraintsSchema = z
  .object({
    min_legs: z.number().int().min(1).max(8).default(1),
    max_legs: z.number().int().min(1).max(8).default(8),
    require_atomic: z.boolean().default(false),
    allow_cross_chain: z.boolean().default(false),
    allowed_base_tokens: z.array(z.string().min(1).max(16)).default([]),
    allowed_quote_tokens: z.array(z.string().min(1).max(16)).default([]),
    min_pool_tvl_usd: z.number().nonnegative().nullable().optional(),
    min_pool_volume_24h_usd: z.number().nonnegative().nullable().optional(),
  })
  .superRefine((val, ctx) => {
    if (val.min_legs > val.max_legs) {
      ctx.addIssue({
        code: z.ZodIssueCode.custom,
        path: ["max_legs"],
        message: "max_legs must be >= min_legs",
      });
    }
  });

const StrategyRuntimeConfigSchema = z.object({
  enabled: z.boolean().default(true),
  allowed_chain_ids: z.array(z.number().int().positive()).max(64).default([]),
  // null = inherit chain-level enabled_dex_ids, [] = block-all, [...] = strict allowlist.
  enabled_dex_ids: z.array(z.string().uuid()).max(256).nullable().optional(),
  // Audit 2026-05-10 follow-up: per-strategy pool allowlist by pools.id UUID.
  // Same null/[]/[...] semantics as enabled_dex_ids. Backed by Rust
  // StrategyRuntimeConfig.enabled_pool_ids; PoolsTab UI writeback in a
  // future sprint, contract reserved here.
  enabled_pool_ids: z.array(z.string().uuid()).max(2048).nullable().optional(),
  enabled_protocol_types: z.array(z.string().min(1).max(32)).max(32).default([]),
  min_profit_usd: z.number().nonnegative().nullable().optional(),
  min_roi_pct: z.number().nonnegative().nullable().optional(),
  max_slippage_pct: z.number().min(0).max(50).nullable().optional(),
  max_price_impact_pct: z.number().min(0).max(50).nullable().optional(),
  max_gas_usd: z.number().nonnegative().nullable().optional(),
  simulation_capital_usd: z.number().positive().nullable().optional(),
  per_token_caps_usd: z.record(z.string().min(1).max(16), z.number().positive()).default({}),
  route_constraints: RouteConstraintsSchema.default({
    min_legs: 1,
    max_legs: 8,
    require_atomic: false,
    allow_cross_chain: false,
    allowed_base_tokens: [],
    allowed_quote_tokens: [],
    min_pool_tvl_usd: null,
    min_pool_volume_24h_usd: null,
  }),
});

export const StrategyConfigsSchema = z
  .record(z.string().min(1).max(64), StrategyRuntimeConfigSchema)
  .default({});

export type StrategyRuntimeConfig = z.infer<typeof StrategyRuntimeConfigSchema>;

const TradingConfigSchema = z
  .object({
    capital_usd: z.number().nonnegative(),
    base_token_symbol: z.string().min(1).max(16),
    base_token_price_usd: z.number().positive(),
    // ARBX-TW-002: entries may be symbols (≤16 chars, retro-compat UI) OR
    // EVM addresses in `0x`+40-hex form (42 chars) — the canonical TokenKey
    // identity (ARBX-0018). Address entries gate directly at runtime; symbol
    // entries resolve through the chain universe as before.
    allowed_token_symbols: z.array(z.string().min(1).max(42)).max(64),

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
    // WO-04 (2026-09-06, migration 119): LP-fee proxy default for the live
    // simulation (computeSimulatedNet) when a route's per-leg fee tiers are
    // unknown to the api-server hot path. Bounds match the DB CHECK exactly
    // (0 ≤ v ≤ 0.5) — invalid input fails fast with 400 here, not in the sim.
    lp_fee_default_pct: z.number().min(0.0).max(0.5).default(0.003),

    enabled_strategies: z.array(z.string().min(1).max(64)).max(32).default([]),

    // Phase 2 route-finder: per-chain DEX allowlist.
    // NULL = "use all is_active dexes" (backwards-compatible default).
    enabled_dex_ids: z.array(z.string().uuid()).max(256).nullable().optional(),

    // Migration 056 — per-strategy fine-grained config (Decision A: JSONB).
    // Keyed by strategy_kind. Default {} = no per-strategy overrides; the
    // chain-level enabled_strategies / enabled_dex_ids govern instead.
    // See StrategyConfigsSchema above for field-level shape.
    strategy_configs: StrategyConfigsSchema,

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
    userAgent?: string | null,
  ) => Promise<void>;
  logger: { warn: (obj: object, msg?: string) => void; info: (obj: object, msg?: string) => void };
  /** EMIT-04: optional so router-level tests keep compiling without a WSS
   *  gateway — when absent the ack is still INSERTed (the frontend GET
   *  /api/system/runtime-ack/:event_id fallback recovers it), only the live
   *  broadcast is skipped, mirroring the POST-INSERT fail-honest shape. */
  io?: IoServer;
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
  // WO-04 (2026-09-06): NUMERIC(6,4) NOT NULL DEFAULT 0.0030 (migration 119).
  lp_fee_default_pct: string;
  enabled_strategies: string[];
  enabled_dex_ids: string[] | null;
  // Migration 056 — JSONB returned as already-parsed object by node-postgres.
  strategy_configs: Record<string, unknown>;
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

/**
 * Hotfix (2026-05-10): normalise each `strategy_configs` entry so every
 * canonical field is present in the wire payload with proper null defaults.
 *
 * Why this matters: when a new field is added to `StrategyRuntimeConfig`
 * (e.g. `enabled_pool_ids`), existing JSONB entries persisted before the
 * migration do NOT have that field. The Rust struct uses `#[serde(default)]`
 * so it deserialises tolerantly, but the frontend Zod schema (strict by
 * default to avoid TS2719 record-value inference issues) had treated the
 * missing field as a parse error, leaving the /strategies page rendering
 * a red "endpoint error" banner.
 *
 * This helper guarantees the GET response shape is stable regardless of
 * when the entry was first persisted. Adding a new field in the future
 * means adding one more line here — the frontend keeps working.
 *
 * R8 fail-honest: never invents a value. All defaults are `null` (or `[]`
 * for collections) — the universal "not configured" sentinel.
 */
function normalizeStrategyConfigEntry(entry: unknown): Record<string, unknown> {
  const e = (entry ?? {}) as Record<string, unknown>;
  return {
    enabled: e["enabled"] ?? true,
    allowed_chain_ids: e["allowed_chain_ids"] ?? [],
    enabled_dex_ids: e["enabled_dex_ids"] ?? null,
    enabled_pool_ids: e["enabled_pool_ids"] ?? null,
    enabled_protocol_types: e["enabled_protocol_types"] ?? [],
    min_profit_usd: e["min_profit_usd"] ?? null,
    min_roi_pct: e["min_roi_pct"] ?? null,
    max_slippage_pct: e["max_slippage_pct"] ?? null,
    max_price_impact_pct: e["max_price_impact_pct"] ?? null,
    max_gas_usd: e["max_gas_usd"] ?? null,
    simulation_capital_usd: e["simulation_capital_usd"] ?? null,
    per_token_caps_usd: e["per_token_caps_usd"] ?? {},
    route_constraints: normalizeRouteConstraints(e["route_constraints"]),
  };
}

function normalizeRouteConstraints(rc: unknown): Record<string, unknown> {
  const r = (rc ?? {}) as Record<string, unknown>;
  return {
    min_legs: r["min_legs"] ?? 1,
    max_legs: r["max_legs"] ?? 8,
    require_atomic: r["require_atomic"] ?? false,
    allow_cross_chain: r["allow_cross_chain"] ?? false,
    allowed_base_tokens: r["allowed_base_tokens"] ?? [],
    allowed_quote_tokens: r["allowed_quote_tokens"] ?? [],
    min_pool_tvl_usd: r["min_pool_tvl_usd"] ?? null,
    min_pool_volume_24h_usd: r["min_pool_volume_24h_usd"] ?? null,
  };
}

function normalizeStrategyConfigs(
  configs: Record<string, unknown> | null | undefined,
): Record<string, unknown> {
  if (!configs) return {};
  const out: Record<string, unknown> = {};
  for (const [kind, entry] of Object.entries(configs)) {
    out[kind] = normalizeStrategyConfigEntry(entry);
  }
  return out;
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
    lp_fee_default_pct: Number(row.lp_fee_default_pct), // WO-04 (2026-09-06)
    enabled_strategies: row.enabled_strategies,
    enabled_dex_ids: row.enabled_dex_ids ?? null,
    // Migration 056 + 2026-05-10 hotfix — normalise every entry so legacy
    // rows missing newer fields (e.g. enabled_pool_ids) round-trip as null
    // and the strict frontend Zod parses cleanly.
    strategy_configs: normalizeStrategyConfigs(row.strategy_configs),
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

/**
 * Boot re-hydration of the PG→Redis trading_config mirror.
 *
 * PG `trading_config` is the source of truth, but the Redis mirror key
 * `arbx:trading_config:<chain_id>` is written ONLY by the admin PUT handler.
 * A Redis restart / flush / eviction therefore silently drops the mirror with
 * nothing to repopulate it → searcher-rs reads None → has_config=false → the
 * orchestrator falls to observe-only and emits 0 opportunities until the next
 * PUT. (Root-caused 2026-06-04: the live feed sat at 0 for exactly this reason.)
 *
 * This runs once at api-server boot: it re-mirrors every ENABLED row from PG to
 * Redis (idempotent SET) and publishes the SAME dual-channel reload the PUT
 * emits, so searcher-rs picks the config back up in <1s. It introduces NO new
 * values — it replays exactly what the operator already persisted in PG
 * (no-hardcode safe). Fail-honest: it never throws (must not break boot) and
 * logs precisely what it mirrored.
 */
export async function rehydrateTradingConfigMirror(deps: {
  pool: Pool | null;
  redis: Redis;
  logger: { warn: (obj: object, msg?: string) => void; info: (obj: object, msg?: string) => void };
}): Promise<void> {
  if (!deps.pool) {
    deps.logger.warn({ event: "trading_config.rehydrate_skipped", reason: "db_unavailable" });
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
              lp_fee_default_pct,
              enabled_strategies, enabled_dex_ids, strategy_configs,
              capital_cost_rate_annual_pct, ops_overhead_usd_per_attempt,
              spread_sanity_mult, p_copied_volume_threshold_usd, p_copied_max,
              enabled,
              updated_at, updated_by, created_at
         FROM trading_config
        WHERE enabled = TRUE`,
    );
    let mirrored = 0;
    for (const row of q.rows) {
      try {
        const json = JSON.stringify(rowToRedisState(row));
        await deps.redis.set(tradingConfigKey(row.chain_id), json);
        await deps.redis.publish(TRADING_CONFIG_CHANNEL, json);
        try {
          await deps.redis.publish(HOT_RELOAD_CHANNEL, json);
        } catch (err) {
          deps.logger.warn({
            event: "trading_config.rehydrate_hot_reload_failed",
            chain_id: row.chain_id,
            err: (err as Error).message,
          });
        }
        mirrored += 1;
      } catch (err) {
        deps.logger.warn({
          event: "trading_config.rehydrate_row_failed",
          chain_id: row.chain_id,
          err: (err as Error).message,
        });
      }
    }
    deps.logger.info(
      {
        event: "trading_config.rehydrated",
        enabled_rows: q.rowCount,
        mirrored,
        channels: [TRADING_CONFIG_CHANNEL, HOT_RELOAD_CHANNEL],
      },
      "trading_config Redis mirror re-hydrated from Postgres at boot",
    );
  } catch (e) {
    deps.logger.warn(
      { event: "trading_config.rehydrate_failed", err: (e as Error).message },
      "trading_config boot re-hydration failed (non-fatal)",
    );
  }
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
                lp_fee_default_pct,
                enabled_strategies, enabled_dex_ids, strategy_configs,
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
      // EMIT-04: universe_version — Redis counter bumped by the admin PUT on
      // every EFFECTIVE allowlist change. Absent key = the universe was never
      // changed since deployment = honest null (never a fabricated 0).
      let universeVersion: number | null = null;
      try {
        const v = await deps.redis.get(tokenUniverseVersionKey(chainId));
        const n = Number(v);
        if (v !== null && Number.isSafeInteger(n) && n >= 0) universeVersion = n;
      } catch {
        // absent/unreadable counter stays null — honest absence
      }
      res.status(200).json({
        chain_id: chainId,
        configured: true,
        universe_version: universeVersion,
        ...rowToRedisState(q.rows[0]!),
      });
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
              lp_fee_default_pct,
              enabled_strategies, enabled_dex_ids, strategy_configs, enabled, updated_by,
              capital_cost_rate_annual_pct, ops_overhead_usd_per_attempt,
              spread_sanity_mult, p_copied_volume_threshold_usd, p_copied_max
            )
            VALUES (
              $1,$2,$3,$4,$5,$6::jsonb,$7,$8::jsonb,$9::jsonb,$10,$11,
              $12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,$23,$24,$25::uuid[],$26::jsonb,$27,$28,
              $29,$30,$31,$32,$33
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
              lp_fee_default_pct = EXCLUDED.lp_fee_default_pct,
              enabled_strategies = EXCLUDED.enabled_strategies,
              enabled_dex_ids = EXCLUDED.enabled_dex_ids,
              strategy_configs = EXCLUDED.strategy_configs,
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
                      lp_fee_default_pct,
                      enabled_strategies, enabled_dex_ids, strategy_configs,
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
            body.lp_fee_default_pct, // WO-04 (2026-09-06) → $23
            body.enabled_strategies,
            body.enabled_dex_ids ?? null,
            // Migration 056 — JSONB stringified for $25::jsonb cast.
            JSON.stringify(body.strategy_configs ?? {}),
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

        // Dual-channel publish: sed-core y otros consumidores legacy escuchan
        // HOT_RELOAD_CHANNEL. El fallo de esta publicación no afecta el canal
        // canónico ni la respuesta al cliente (fail-honest).
        let subsHotReload = 0;
        try {
          subsHotReload = await deps.redis.publish(HOT_RELOAD_CHANNEL, json);
        } catch (err) {
          deps.logger.warn(
            { event: "trading_config.hot_reload_publish_failed", chain_id: chainId, err: (err as Error).message },
            "HOT_RELOAD_CHANNEL publish failed; TRADING_CONFIG_CHANNEL succeeded",
          );
        }

        await deps.writeAudit(
          "trading_config.upsert",
          actor,
          "trading_config",
          String(chainId),
          before,
          body,
          req.ip ?? null,
          (req as Request & { traceId?: string }).traceId ?? null,
          // R3: pass UA so audit_log.user_agent is hashed via arbx_hash_user_agent.
          req.header("user-agent") ?? null,
        );

        // EMIT-04 (FE-MASTER §4): the ONLY mutation that changes the effective
        // token universe is an edit of allowed_token_symbols — every other
        // knob in this upsert leaves the universe untouched (no bump, no ACK
        // churn; set-semantics fingerprint makes reorder-only edits a no-op).
        // On a real change: INCR the per-chain version counter, INSERT a
        // runtime_ack row with the STABLE resource "token_universe" (the
        // correlation key useRuntimeAckSocket consumers filter on, alongside
        // the event_id bijection) and broadcast post-INSERT — same invariant
        // I-2 as POST /api/system/runtime-ack. layer=persistence /
        // status=applied is honest: the durable portion (PG row + Redis
        // mirror + dual-channel publish) is fully confirmed above; the
        // searcher's index rebuild rides its own ≤30s TTL cycle and never
        // blocks this response.
        const beforeFp = universeFingerprint(
          (before as { allowed_token_symbols?: string[] } | null)?.allowed_token_symbols,
        );
        const afterFp = universeFingerprint(body.allowed_token_symbols);
        let universeVersion: number | null = null;
        let runtimeAckEventId: string | null = null;
        if (beforeFp !== afterFp) {
          universeVersion = await deps.redis.incr(tokenUniverseVersionKey(chainId));
          runtimeAckEventId = randomUUID();
          const ack: RuntimeAckBroadcast = {
            event_id: runtimeAckEventId,
            resource: "token_universe",
            chain_id: chainId,
            idempotency_key: `token_universe:${chainId}:v${universeVersion}`,
            config_hash_before: beforeFp,
            config_hash_after: afterFp as string,
            worker_id: "api-server:trading-config",
            layer: "persistence",
            status: "applied",
          };
          try {
            await deps.pool.query("BEGIN");
            try {
              // Same advisory-lock + ON CONFLICT dedup convention as the
              // system-manifest POST (P1-1/P1-5): concurrent producers
              // serialize per (resource, chain_id); a replayed event_id is a
              // no-op INSERT that does not re-broadcast.
              await deps.pool.query(
                "SELECT pg_advisory_xact_lock(hashtext($1)::int)",
                [`runtime_ack:token_universe:${chainId}`],
              );
              const ins = await deps.pool.query(
                `INSERT INTO runtime_ack (
                   event_id, resource, chain_id, idempotency_key,
                   config_hash_before, config_hash_after,
                   worker_id, layer, status
                 ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
                 ON CONFLICT (event_id, layer) DO NOTHING
                 RETURNING id`,
                [
                  ack.event_id,
                  ack.resource,
                  ack.chain_id,
                  ack.idempotency_key,
                  ack.config_hash_before,
                  ack.config_hash_after,
                  ack.worker_id,
                  ack.layer,
                  ack.status,
                ],
              );
              await deps.pool.query("COMMIT");
              if ((ins.rowCount ?? 0) > 0 && deps.io) {
                broadcastRuntimeAck(deps.io, ack);
              }
            } catch (err) {
              await deps.pool.query("ROLLBACK").catch(() => {});
              throw err;
            }
          } catch (err) {
            // The config itself is already durable — an ack failure never
            // fails the PUT. The counter bump stands; the frontend correlates
            // through universe_version on the next GET even without the WSS
            // event (honest degradation, not a silent fabricate).
            deps.logger.warn(
              {
                event: "trading_config.universe_ack_failed",
                chain_id: chainId,
                universe_version: universeVersion,
                err: (err as Error).message,
              },
              "token_universe runtime_ack INSERT failed (config already persisted)",
            );
          }
        }

        deps.logger.info(
          {
            event: "trading_config.upsert",
            chain_id: chainId,
            actor,
            subscribers_trading_config: subs,
            subscribers_hot_reload: subsHotReload,
            channels: [TRADING_CONFIG_CHANNEL, HOT_RELOAD_CHANNEL],
            universe_version: universeVersion,
            universe_changed: universeVersion !== null,
          },
          "trading config persisted and broadcast",
        );
        res.status(200).json({
          ok: true,
          chain_id: chainId,
          subscribers_trading_config: subs,
          subscribers_hot_reload: subsHotReload,
          channels: [TRADING_CONFIG_CHANNEL, HOT_RELOAD_CHANNEL],
          universe_version: universeVersion,
          runtime_ack_event_id: runtimeAckEventId,
          ...redisState,
        });
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
                  lp_fee_default_pct,
                  enabled_strategies, enabled_dex_ids, strategy_configs,
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
