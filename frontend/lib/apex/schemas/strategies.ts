/**
 * Ω FE-MASTER · QuoteBase strategy catalog contract (FE-0002 tramo 2 — P6 §21-§24)
 *
 * Wire mirror of GET /api/strategies/catalog (EMIT-07): the generated table
 * `backend/api-server/src/generated/quotebase_catalog.ts`
 * (QUOTEBASE_STRATEGY_CATALOG) served VERBATIM inside `{ entries }`.
 * Static-per-canon: it only changes with a new workbook ingestion — cached
 * client-side, never refetched per render.
 *
 * §79: `allowed_hops` arrives ALREADY EXPANDED from HopMask_u8 — TypeScript
 * never decodes bits. The hot-path hop cap (7) is RUNTIME policy, NOT
 * catalog metadata: `max_legs` legitimately spans the workbook canon up to
 * 16 and the schema must not clamp it.
 *
 * Contract invariants (264 rows; status counts 79/174/8/3;
 * DETERMINISTIC_EXECUTABLE ⊆ ROUTE_READY; OBSERVE_ONLY ⟺ class) are
 * WORKBOOK-VERSION facts — they belong to differential tests against the
 * generated fixture, NEVER to this schema (no hardcoded counts, §28).
 *
 * Runtime enabled/active state lives in trading_config.enabled_strategies
 * (join done by the consumer) — `active` is NOT a catalog field (§28).
 */
import { z } from 'zod';

// ─── DispatchStatus — col Status of 11_STRATEGY_HOP_MAP (ARBX-0021) ──────
export const DispatchStatusSchema = z.enum([
  'ROUTE_READY', // expand + candidate + telemetry
  'NEEDS_ROUTE_DATA', // no fabricated route — the absence IS the datum (R8)
  'OBSERVE_ONLY', // telemetry yes, execution never
  'NO_COMPATIBLE_ROUTE',
]);
export type DispatchStatus = z.infer<typeof DispatchStatusSchema>;

// ─── Catalog row — one per workbook strategy (§21-§24) ────────────────────
export const StrategyCatalogRowSchema = z
  .object({
    /** "MEV-01-001" — ascending, unique, byte-exact workbook id. */
    mev_id: z.string().regex(/^MEV-\d{2,3}-\d{3}$/, 'INV-2: workbook strategy id'),
    /** Numeric family of the workbook (MEV-<group>-…). */
    group: z.number().int().min(1),
    /** Col Strategy ("DEX–DEX arbitrage"). */
    name: z.string().min(1),
    /** Col Family. */
    family: z.string().min(1),
    /** Col Surface (DEX_AMM, …). */
    surface: z.string().min(1),
    /** Col Backend_Module (route_graph_engine…). */
    backend_module: z.string().min(1),
    /** Link to P7 (60 detector family ids). */
    detector_id: z.string().min(1),
    /** Canon: min∈{1..4}. */
    min_legs: z.number().int().min(1).max(16),
    /** Canon: max∈{2,3,4,6,8,12,16} — NOT clamped to the hot-path cap. */
    max_legs: z.number().int().min(2).max(16),
    /** Expanded HopMask_u8, e.g. [2,3,4,5,6,7] — bits decoded backend-side. */
    allowed_hops: z.array(z.number().int().min(1).max(16)).min(1),
    /** TOKEN_MULTIGRAPH | … */
    graph_model: z.string().min(1),
    /** PRIMARY_PAIR+NUMERAIRE | … */
    quotebase_role: z.string().min(1),
    /** Workbook sentence. */
    search_policy: z.string().min(1),
    /** 29 execution classes (TW-005). */
    execution_class: z.string().min(1),
    /** ["op_27 Path Ordering", …] */
    primary_ops: z.array(z.string().min(1)),
    /** LaTeX-ish workbook equation — DISPLAY ONLY, never evaluated. */
    discovery_equation: z.string(),
    /** Gate_LIVE sentence. */
    gate_live: z.string(),
    status: DispatchStatusSchema,
  })
  .strict()
  .superRefine((row, ctx) => {
    if (row.min_legs > row.max_legs) {
      ctx.addIssue({
        code: z.ZodIssueCode.custom,
        path: ['min_legs'],
        message: 'min_legs must not exceed max_legs',
      });
    }
  });
export type StrategyCatalogRow = z.infer<typeof StrategyCatalogRowSchema>;

// ─── EMIT-07 envelope (GET /api/strategies/catalog) ───────────────────────
export const StrategyCatalogResponseSchema = z.object({
  entries: z.array(StrategyCatalogRowSchema),
}).strict();
export type StrategyCatalogResponse = z.infer<typeof StrategyCatalogResponseSchema>;
