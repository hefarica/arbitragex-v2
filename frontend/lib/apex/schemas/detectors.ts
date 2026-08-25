/**
 * Ω FE-MASTER · QuoteBase detector policy contract (FE-0002 tramo 2 — P7 §25)
 *
 * Wire mirror of GET /api/detectors/catalog (EMIT-08): the generated table
 * `backend/api-server/src/generated/quotebase_catalog.ts`
 * (QUOTEBASE_DETECTOR_CATALOG) served VERBATIM inside `{ entries }`.
 * Source canon: docs/quotebase_detector_policy.json → detector_policy.rs.
 *
 * ENMIENDA (d9, 2026-08-24, pre-Zod — incorporated here): the workbook's
 * Frontend_Config column is FREEFORM ("solver timeout", "reserve safety
 * floor", …), NOT a snake_key list. `frontend_config` is therefore
 * `string[]` — the EXACT phrases (split ';', trim, strip '.', backend-side).
 * FrontendKnobSpec {key,kind,unit} is ELIMINATED: deriving kind/unit
 * heuristically would be fabrication (§28/R8). The knob VALUES live in
 * runtime config, never in this catalog — the strings are display-only.
 *
 * `hop_envelope` is the FAMILY envelope; it INTERSECTS each member
 * strategy's min/max_legs (a strategy never escapes its family — backend
 * invariant; the frontend only displays the intersection).
 *
 * Contract invariants (60 rows; Σ strategies_count == 264) belong to
 * differential tests, NEVER to this schema (no hardcoded counts, §28).
 */
import { z } from 'zod';

// ─── may_seed() projection (detector_policy.rs, 2-valued) ────────────────
export const HotSeedSchema = z.enum(['SEED_CANDIDATE', 'OBSERVE_EVIDENCE']);
export type HotSeed = z.infer<typeof HotSeedSchema>;

// ─── Family hop envelope — intersects member min/max_legs ────────────────
export const HopEnvelopeSchema = z
  .object({
    min: z.number().int().min(1).max(16),
    max: z.number().int().min(1).max(16),
  })
  .strict()
  .superRefine((env, ctx) => {
    if (env.min > env.max) {
      ctx.addIssue({
        code: z.ZodIssueCode.custom,
        path: ['min'],
        message: 'hop_envelope.min must not exceed max',
      });
    }
  });
export type HopEnvelope = z.infer<typeof HopEnvelopeSchema>;

// ─── Policy row — one per detector family (§25) ───────────────────────────
export const DetectorPolicyViewSchema = z.object({
  /** "R_CLOSED_CYCLE" — 60 values; links P6 rows via detector_id. */
  detector_id: z.string().min(1),
  /** Σ == 264 across the catalog — contract test, not a schema constant. */
  strategies_count: z.number().int().min(0),
  example_surface: z.string(),
  /**
   * DP-006 (d9, 2026-08-24): col `Example_MEV` — one canonical MEV_ID whose
   * strategy this detector executes. Position mirrors the wire (after
   * example_surface); the generator fail-fasts the join against the 264
   * canon. Co-landing: a pre-DP-006 payload WITHOUT this key FAILS
   * (.strict()) — same pattern as the p90/p99 LatencyStageRow extension.
   */
  example_mev: z.string().regex(/^MEV-\d{2,3}-\d{3}$/),
  execution_class: z.string(),
  primary_ops: z.array(z.string().min(1)),
  secondary_ops: z.array(z.string().min(1)),
  /** EXACT workbook sentence. */
  exact_discovery_criterion: z.string(),
  required_data: z.string(),
  /** ENMIENDA d9: exact workbook phrases, display-only (see file header). */
  frontend_config: z.array(z.string().min(1)),
  /** GraphPolicy::as_str() — 12 sentences. */
  graph_policy: z.string().min(1),
  hop_envelope: HopEnvelopeSchema,
  hot_seed: HotSeedSchema,
  /** DO_NOT_RULES — the single universal rule. */
  do_not_do: z.string(),
}).strict();
export type DetectorPolicyView = z.infer<typeof DetectorPolicyViewSchema>;

// ─── EMIT-08 envelope (GET /api/detectors/catalog) ────────────────────────
export const DetectorCatalogResponseSchema = z.object({
  entries: z.array(DetectorPolicyViewSchema),
}).strict();
export type DetectorCatalogResponse = z.infer<typeof DetectorCatalogResponseSchema>;
