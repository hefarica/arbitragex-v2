/**
 * Sprint 3 Task 3.3 — Zod schemas for /operations API responses (client side).
 *
 * Mirrors backend/api-server/src/schemas/operations.ts. Server emits
 * snake_case JSON; we keep the same field names here and translate at
 * the component boundary if a camelCase view is needed.
 */
import { z } from "zod";

export const KpisSchema = z.object({
  cpi: z.number(),
  spi: z.number(),
  eac_usd: z.number(),
  etc_usd: z.number(),
  tcpi: z.number(),
  vac_usd: z.number(),
  cv_usd: z.number(),
});

export const KpiPayloadSchema = z.object({
  chain_id: z.number().int(),
  capital_deployed_usd: z.number(),
  profit_realized_usd: z.number(),
  ops_completed: z.number().int(),
  ops_target_per_day: z.number().int(),
  elapsed_day_fraction: z.number().min(0).max(1),
  kpis: KpisSchema,
  computed_at: z.string(),
});

export const SCurveBucketSchema = z.object({
  ts: z.string(),
  ops_cumulative: z.number().int(),
  profit_cumulative_usd: z.number(),
  target_cumulative_usd: z.number(),
});

export const SCurvePayloadSchema = z.object({
  chain_id: z.number().int(),
  bucket_minutes: z.number().int(),
  buckets: z.array(SCurveBucketSchema),
});

export const VarianceRowSchema = z.object({
  bucket_start: z.string(),
  realized_pnl_usd: z.number(),
  planned_pnl_usd: z.number(),
  cv_usd: z.number(),
});

export const VariancePayloadSchema = z.object({
  chain_id: z.number().int(),
  rows: z.array(VarianceRowSchema),
});

// Scanner heartbeat snapshot (Sprint observability arc closure).
// Mirrors searcher-rs::workers::heartbeat_worker::HeartbeatSnapshot one-to-one.
// Backend persists the latest snapshot to Redis with TTL = 3× period_secs;
// when the searcher dies the API returns 404 (R8 fail-honest, no stale data).
export const ScannerHeartbeatSnapshotSchema = z.object({
  period_secs: z.number().int().positive(),
  emitted_at_unix: z.number().int().nonnegative(),
  redis_stream_total: z.number().int().nonnegative(),
  redis_stream_delta: z.number().int(),
  pg_period_inserted: z.number().int().nonnegative(),
  pg_period_profit_pos: z.number().int().nonnegative(),
  pending_received: z.number().int().nonnegative(),
  decoded_ok: z.number().int().nonnegative(),
  enriched_v2: z.number().int().nonnegative(),
  enriched_v3: z.number().int().nonnegative(),
  gate_token_not_allowed: z.number().int().nonnegative(),
  gate_strategy_disabled: z.number().int().nonnegative(),
  gate_no_config: z.number().int().nonnegative(),
  gate_unknown_token_price: z.number().int().nonnegative(),
  gate_anomalous_math: z.number().int().nonnegative(),
  gate_other_rejected: z.number().int().nonnegative(),
  passed_all_gates: z.number().int().nonnegative(),
  db_persisted: z.number().int().nonnegative(),
  db_errors: z.number().int().nonnegative(),
});

export const ScannerHeartbeatResponseSchema = z.object({
  chain_id: z.number().int().positive(),
  snapshot: ScannerHeartbeatSnapshotSchema,
  fetched_at: z.string(),
});

export type KpiPayload = z.infer<typeof KpiPayloadSchema>;
export type SCurvePayload = z.infer<typeof SCurvePayloadSchema>;
export type VariancePayload = z.infer<typeof VariancePayloadSchema>;
export type ScannerHeartbeatSnapshot = z.infer<typeof ScannerHeartbeatSnapshotSchema>;
export type ScannerHeartbeatResponse = z.infer<typeof ScannerHeartbeatResponseSchema>;
export const StrategyRuntimeStatusItemSchema = z.object({
  strategy_kind: z.string(),
  enabled: z.boolean(),
  engine_loaded: z.boolean(),
  engine_invoked: z.boolean(),
  last_invoked_at: z.string().nullable(),
  last_candidate_at: z.string().nullable(),
  candidates_1h: z.number().int(),
  rejections_1h: z.number().int(),
  last_rejection_reason: z.string().nullable(),
  data_dependencies_status: z.string(),
  active_variants: z.array(z.string()).optional(),
  paper_viable_1h: z.number().int().optional(),
  paper_rejected_1h: z.number().int().optional(),
  triangular_pool_map_entries: z.number().int().nullable().optional(),
  pool_index_entries: z.number().int().nullable().optional(),
  reserves_cache_entries: z.number().int().nullable().optional(),
  impacted_cycles_last_1h: z.number().int().nullable().optional(),
  cycles_with_missing_reserves: z.number().int().nullable().optional(),
  base_candidates_seen_1h: z.number().int().optional(),
  wrapped_candidates_1h: z.number().int().optional(),
  no_provider_rejections: z.number().int().optional(),
  negative_after_fee_rejections: z.number().int().optional(),
  lending_watchlist_size: z.number().int().nullable().optional(),
  indexed_positions: z.number().int().nullable().optional(),
  impacted_lending_positions_1h: z.number().int().nullable().optional(),
  hf_below_one_count: z.number().int().nullable().optional(),
  liquidation_candidates_1h: z.number().int().optional(),
});

export const StrategyRuntimeStatusResponseSchema = z.object({
  chain_id: z.number().int().positive(),
  window_seconds: z.number().int(),
  source: z.object({
    postgres: z.string(),
    redis: z.string(),
    logs: z.string(),
  }),
  strategies: z.array(StrategyRuntimeStatusItemSchema),
  ts: z.string(),
});

export type StrategyRuntimeStatusItem = z.infer<typeof StrategyRuntimeStatusItemSchema>;
export type StrategyRuntimeStatusResponse = z.infer<typeof StrategyRuntimeStatusResponseSchema>;

