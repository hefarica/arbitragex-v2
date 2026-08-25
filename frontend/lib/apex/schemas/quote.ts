/**
 * Ω FE-MASTER · Quote/Base contracts (FE-0001)
 *
 * Materialized from `.ai-work/FE-P3-P4-DOMAIN-SHAPES.md` §4 (input d9 →
 * reviewed by 7b, crossed R7). Derived from backend truth:
 *   - quote_score.rs (QB-06) — score forma 05 r11, 0..100
 *   - fe_normalization.rs StateVersion — quote_version / graph_version
 *   - canonical_knobs quote_w_* ×5 — weights are a RUNTIME-CONFIG mirror;
 *     Σ=1.0±1e-9 validation lives backend-side (§9: the component NEVER
 *     duplicates weight values — it renders what the payload carries).
 *
 * EMIT-02/03 LANDED (2026-08-24, d9): GET /api/quote/anchor (8-key flattened)
 * + POST /api/admin/quote/preview (4-key envelope) serve these schemas —
 * `backend/api-server/src/routes/quote-anchor.ts` + tests 14/14.
 */
import { z } from 'zod';
import { ChainIdSchema, EvmAddressSchema } from './_primitives';

// ─── QuoteScore components (§9 explainable score) ────────────────────────
export const QuoteScoreComponentsSchema = z.object({
  prior: z.number(),
  liquidity: z.number(),
  venues: z.number(),
  stability: z.number(),
  cross_dex: z.number(),
}).strict();
export type QuoteScoreComponents = z.infer<typeof QuoteScoreComponentsSchema>;

/**
 * Mirror of the runtime knobs `quote_w_*` (§9 weights table). Same shape as
 * components by design: w_PP·prior + w_LL·liquidity + w_VV·venues +
 * w_SS·stability + w_CC·cross_dex = score. The Σ≈1 invariant is enforced by
 * the backend knob validation; FE-0045 asserts it as a CONTRACT test, not
 * at parse time (parse stays a pure mirror).
 */
export const QuoteWeightsSchema = z.object({
  prior: z.number().min(0),
  liquidity: z.number().min(0),
  venues: z.number().min(0),
  stability: z.number().min(0),
  cross_dex: z.number().min(0),
}).strict();
export type QuoteWeights = z.infer<typeof QuoteWeightsSchema>;

// ─── Anchor view (§8 Current Quote Anchor) ───────────────────────────────
export const QuoteAnchorViewSchema = z.object({
  chain_id: ChainIdSchema,
  quote_symbol: z.string().min(1),
  /** 0..100, workbook 05 r11 form. */
  quote_score: z.number().min(0).max(100),
  /** StateVersion::quote_version — the EFFECTIVE runtime version. */
  quote_version: z.number().int().min(0),
  graph_version: z.number().int().min(0),
  components: QuoteScoreComponentsSchema,
  weights: QuoteWeightsSchema,
}).strict();
export type QuoteAnchorView = z.infer<typeof QuoteAnchorViewSchema>;

// ─── Per-token score table (§9) ──────────────────────────────────────────
export const QuoteTokenRowSchema = z.object({
  symbol: z.string().min(1),
  address: EvmAddressSchema,
  components: QuoteScoreComponentsSchema,
  score: z.number().min(0).max(100),
}).strict();
export type QuoteTokenRow = z.infer<typeof QuoteTokenRowSchema>;

// ─── EMIT-02 Layer-2 endpoint response (agreed flattened, 2026-08-24) ────
/**
 * GET /api/quote/anchor serves the view FLATTENED with the §9 per-token
 * table appended (d9 proposal accepted — one object, no nesting). Exactly
 * 8 keys: the 7 view keys + `tokens`. Redis-only snapshot internals
 * (pairs_by_token / pools_by_token) NEVER ride this response — they are
 * preview-impact INPUTS resolved server-side. Row order is backend-fixed
 * (score desc, tie symbol asc); the consumer renders payload order.
 */
export const QuoteAnchorResponseSchema = QuoteAnchorViewSchema.extend({
  tokens: z.array(QuoteTokenRowSchema),
}).strict();
export type QuoteAnchorResponse = z.infer<typeof QuoteAnchorResponseSchema>;

// ─── Preview before apply (§10, CORRECTED by operator ruling 2026-08-23) ──
export const QuotePreviewRequestSchema = z.object({
  chain_id: ChainIdSchema,
  weights: QuoteWeightsSchema,
}).strict();
export type QuotePreviewRequest = z.infer<typeof QuotePreviewRequestSchema>;

/**
 * INVARIANT QB-TOPOLOGY-01 (operator ruling 2026-08-23 — doctrine of
 * workbook 05 r12/r15 + 09 r25 wins over the §10 brief example, which is
 * expressly corrected and NOT an invariant):
 *
 * Quote/Base is orientation & valuation metadata over a numeraire-agnostic
 * graph. Changing the quote anchor MUST NOT add/remove/reverse an edge,
 * mutate PairIndex/TokenId/PoolId/adjacency, or mutate graph_version.
 * It MAY increment quote_version, invalidate QuoteVersionedCell caches,
 * change display orientation, normalized valuation, inefficiency score,
 * ranking, and which existing route becomes economically attractive.
 *
 * Consequently `graph_rebuild_required` is a TYPE-LEVEL LITERAL `false`:
 * no payload can ever carry `true` through this schema. Topology rebuild
 * is reserved for universe/pool/venue/DEX/chain/bridge/identity changes.
 */
export const QuotePreviewImpactSchema = z.object({
  graph_rebuild_required: z.literal(false),
  quote_revaluation_required: z.boolean(),
  quote_cache_invalidation_required: z.boolean(),
  affected_pairs: z.number().int().min(0),
  affected_edges: z.number().int().min(0),
  affected_cached_routes: z.number().int().min(0),
  current_quote_version: z.number().int().min(0),
  proposed_quote_version: z.number().int().min(0),
  topology_version_unchanged: z.literal(true),
}).strict();
export type QuotePreviewImpact = z.infer<typeof QuotePreviewImpactSchema>;

// ─── EMIT-03 response envelope (d9 co-landing, 2026-08-24) ───────────────
/**
 * POST /api/admin/quote/preview response: EXACTLY 4 keys — `impact` (the
 * 9-key frozen contract above) + the three §10 sketch fields. The FE renders
 * `proposed_tokens` payload order (backend-fixed: score desc, tie symbol
 * asc → address asc — §79: never recomputed client-side). `proposed_tokens`
 * is `min(1)`: the writer invariant "the anchor row always heads `tokens`"
 * means an empty table is a corrupted snapshot (503), never a 200.
 */
export const QuotePreviewResponseSchema = z.object({
  impact: QuotePreviewImpactSchema,
  proposed_quote_symbol: z.string().min(1),
  proposed_quote_score: z.number().min(0).max(100),
  proposed_tokens: z.array(QuoteTokenRowSchema).min(1),
}).strict();
export type QuotePreviewResponse = z.infer<typeof QuotePreviewResponseSchema>;
