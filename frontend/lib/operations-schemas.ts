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

export type KpiPayload = z.infer<typeof KpiPayloadSchema>;
export type SCurvePayload = z.infer<typeof SCurvePayloadSchema>;
export type VariancePayload = z.infer<typeof VariancePayloadSchema>;
