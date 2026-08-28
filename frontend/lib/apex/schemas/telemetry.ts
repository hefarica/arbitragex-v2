/**
 * Ω FE-MASTER · Route Discovery telemetry contracts (FE-0001)
 *
 * Frontier mirror — ISOMORPHIC with what route_discovery_worker.rs actually
 * publishes into `tick.tick_summary` (verified in-tree 2026-08-23):
 *   - lat rows        route_discovery_worker.rs:1143-1158 ← latency_budget.rs
 *                     StageSnapshot { target_ms, p50_us, p95_us, headroom_p95_us }
 *                     (rows for lat.decode/state/reprice/pair/expand/refine/
 *                     gates/emit + lat.total; µs, null = not computed)
 *   - lat candidates  lat_candidates.rs rows_value/meta_value (EMIT-09,
 *                     2026-08-24): top-K per-route rows + honesty meta block
 *   - drain/dirty     :1082-1090
 *   - fe_prefilter    :1096-1108 (knob ON only — OFF emits NOTHING)
 *   - scoped re-eval  :1110-1114 (dirty_reeval only)
 *   - multi-hop       :446-498 (19 keys)
 *   - status census   :499-501 (DispatchStatus slugs → counts)
 *   - routes/adapter  :1050-1074
 *   - pair finder     :1468-1475
 *
 * RULE 00: conditional groups are `.optional()` because their absence is
 * a real backend state (knob OFF = dormant is dormant), never a frontend
 * default. `null` inside a row = not computed (no samples) — the §44 rule:
 * never show "PASS" without sufficient sample.
 */
import { z } from 'zod';

// ─── Latency stages (10_LATENCY mirror) ──────────────────────────────────
export const LatencyStageKeySchema = z.enum([
  'lat.decode',
  'lat.state',
  'lat.reprice',
  'lat.pair',
  'lat.expand',
  'lat.refine',
  'lat.gates',
  'lat.emit',
  'lat.total',
]);

export const LatencyStageRowSchema = z.object({
  key: LatencyStageKeySchema,
  /** Stage budget (ms) from the workbook's 10_LATENCY targets. */
  target_ms: z.number().int().min(0),
  /** Windowed percentile (µs). null = no samples yet. */
  p50_us: z.number().int().min(0).nullable(),
  /**
   * FE-LAT-003 extension (2026-08-23, wire keys agreed with d9): p90/p99 are
   * NOT workbook 10_LATENCY columns (those are p50/p95/headroom) — they are
   * the runtime percentile exposure riding the same nearest-rank kernel.
   * n=1 ⇒ every percentile = the single sample (correct, not clamped).
   */
  p90_us: z.number().int().min(0).nullable(),
  p95_us: z.number().int().min(0).nullable(),
  p99_us: z.number().int().min(0).nullable(),
  /** Signed µs (negative = over budget). null when p95 not computed. */
  headroom_p95_us: z.number().int().nullable(),
}).strict();
export type LatencyStageRow = z.infer<typeof LatencyStageRowSchema>;

export const LatencyTelemetrySchema = z.object({
  lat_stages: z.array(LatencyStageRowSchema),
  /** PASS_p95 vs discovery_sla_ms. null = no completed cycles (§44). */
  lat_pass_p95: z.boolean().nullable(),
  lat_cycles: z.number().int().min(0),
}).strict();
export type LatencyTelemetry = z.infer<typeof LatencyTelemetrySchema>;

// ─── Per-candidate latency rows (EMIT-09, FE-0037 §45 unblocker) ──────────
/**
 * One candidate's traversed-stage timings (µs) — the per-row granularity the
 * aggregate `lat_stages` cannot express. Source: worker
 * `lat_candidates.rs::rows_value` (route_discovery, 2026-08-24); the same
 * tick summary carries both, one poll (useRouteTick, FE-0008).
 *
 * R8 presence-of-key: `stages.reprice_us` is ABSENT (not 0, not null) when
 * the route never traversed the adapter this tick — non-triangular by
 * construction, or skipped (scoped-out / F_e-prefiltered / malformed legs).
 * Absence IS the state; `route_kind` tells the consumer which.
 */
export const RouteKindTokenSchema = z.enum([
  'v2v2',
  'v2v3',
  'v3v2',
  'v3v3',
  'triangular',
  'multihop',
]);
export type RouteKindToken = z.infer<typeof RouteKindTokenSchema>;

export const LatCandidateRowSchema = z
  .object({
    route_hash: z.string().min(1),
    /** `RouteKind::as_str()` closed vocabulary (types.rs:37-59). */
    route_kind: RouteKindTokenSchema,
    /** Closed hop envelope (finder bounds clamp 2..=7). */
    hops: z.number().int().min(2).max(7),
    stages: z
      .object({
        /** Annotation + dispatch planning (measured). Always present. */
        gates_us: z.number().int().min(0),
        /** Adapter pass (measured-UPPER-BOUND: F_e math + backfill ride
         * inside). Optional = traversed-or-not is a real state (R8). */
        reprice_us: z.number().int().min(0).optional(),
      })
      .strict(),
    /** Σ traversed stages — NOT the tick's wall-clock. */
    total_us: z.number().int().min(0),
  })
  .strict()
  .superRefine((row, ctx) => {
    // Producer coherence (same cross-field pattern as DetectorMask):
    // total_us is derived — it must equal the sum of the keys present.
    const sum =
      row.stages.gates_us + (row.stages.reprice_us ?? 0);
    if (row.total_us !== sum) {
      ctx.addIssue({
        code: z.ZodIssueCode.custom,
        path: ['total_us'],
        message: `total_us (${row.total_us}) must equal Σ stages (${sum})`,
      });
    }
  });
export type LatCandidateRow = z.infer<typeof LatCandidateRowSchema>;

/**
 * Once-per-tick honesty block. `attribution` is a closed literal vocabulary —
 * a new token is producer drift and fails here. The cut counters make any
 * top-K truncation observable (`truncated`/`dropped` vs `sampled`) — a recorte
 * is never silent.
 */
export const LatCandidatesTelemetrySchema = z.object({
  lat_candidates: z.array(LatCandidateRowSchema),
  lat_candidates_meta: z
    .object({
      attribution: z
        .object({
          gates: z.literal('measured'),
          reprice: z.literal('measured-upper-bound'),
        })
        .strict(),
      /** Top-K cap applied (env knob, default 10; clamped >= 1). */
      cap: z.number().int().min(1),
      /** Candidates captured BEFORE the cut. */
      sampled: z.number().int().min(0),
      truncated: z.boolean(),
      dropped: z.number().int().min(0),
    })
    .strict()
    .superRefine((meta, ctx) => {
      // dropped == sampled − len(rows) == sampled − min(sampled, cap):
      // the counters must agree or the block lies about its own cut.
      const kept = Math.min(meta.sampled, meta.cap);
      if (meta.dropped !== meta.sampled - kept) {
        ctx.addIssue({
          code: z.ZodIssueCode.custom,
          path: ['dropped'],
          message: `dropped (${meta.dropped}) must equal sampled − kept (${meta.sampled - kept})`,
        });
      }
      if (meta.truncated !== (meta.dropped > 0)) {
        ctx.addIssue({
          code: z.ZodIssueCode.custom,
          path: ['truncated'],
          message: 'truncated must be true exactly when dropped > 0',
        });
      }
    }),
}).strict();
export type LatCandidatesTelemetry = z.infer<
  typeof LatCandidatesTelemetrySchema
>;

// ─── Dirty pool/pair drain (Event→PoolDirty→PairDirty→HotSeed, §17-18) ───
export const DirtyDrainTelemetrySchema = z.object({
  drain_drained: z.number().int().min(0),
  drain_unknown_pool: z.number().int().min(0),
  drain_invalid_pair: z.number().int().min(0),
  drain_already_dirty: z.number().int().min(0),
  drain_seeded: z.number().int().min(0),
  drain_evicted: z.number().int().min(0),
  drain_register_reject: z.number().int().min(0),
  dirty_seeds: z.number().int().min(0),
  adapter_scoped_skip: z.number().int().min(0),
}).strict();
export type DirtyDrainTelemetry = z.infer<typeof DirtyDrainTelemetrySchema>;

// ─── F_e prefilter histogram (ARBX-0024; knob ON only) ───────────────────
/**
 * Signal-never-proof: `below_reference` routes were signaled out BEFORE the
 * net gate proved anything — PASS authority never moved (worker :1091-1095).
 */
export const FePrefilterTelemetrySchema = z.object({
  fe_prefilter_evaluated: z.number().int().min(0),
  fe_prefilter_pass: z.number().int().min(0),
  fe_prefilter_below_reference: z.number().int().min(0),
  fe_prefilter_uncomputed: z.number().int().min(0),
  fe_prefilter_map_fail: z.number().int().min(0),
  /** ARBX-0023: true = dynamic anchor chosen this tick; false = raw units. */
  fe_prefilter_anchor_dynamic: z.boolean(),
}).strict();
export type FePrefilterTelemetry = z.infer<typeof FePrefilterTelemetrySchema>;

// ─── Graph rejection census (worker :560-567, non-empty rejects only) ────
/**
 * Per-reason census of pools that yielded no edges this tick (BTreeMap on the
 * wire → string→count record). Absent when the graph accepted every pool.
 */
export const GraphRejectedReasonsSchema = z.record(
  z.string().min(1),
  z.number().int().min(0),
);
export type GraphRejectedReasons = z.infer<typeof GraphRejectedReasonsSchema>;

// ─── Scoped re-evaluation (R9 re-eval, dirty_reeval only) ────────────────
export const ScopedReevalTelemetrySchema = z.object({
  scoped_reeval: z.literal(true),
  scoped_reeval_cycles: z.number().int().min(0),
  scoped_reeval_routes: z.number().int().min(0),
  scoped_cycle_map_fail: z.number().int().min(0),
}).strict();
export type ScopedReevalTelemetry = z.infer<typeof ScopedReevalTelemetrySchema>;

// ─── Multi-hop dispatch signal (19 verified keys, worker :446-498) ───────
/** Hop envelope `[min, max]` (inclusive, workbook Allowed_Hops). */
export const HopBoundsSchema = z.tuple([
  z.number().int().min(2).max(7),
  z.number().int().min(2).max(7),
]);

export const MultiHopTelemetrySchema = z.object({
  multi_hop_profitable_cycles: z.number().int().min(0),
  multi_hop_v3_skipped: z.number().int().min(0),
  /** Truncation FLAG (MultiHopResult.capped: bool) — parallel to routes_capped. */
  multi_hop_capped: z.boolean(),
  multi_hop_noise_dropped: z.number().int().min(0),
  /** Selected strategy id when the StrategyMask gated the pass. */
  multi_hop_mask_strategy: z.string().nullable(),
  multi_hop_hops_effective: HopBoundsSchema.nullable(),
  multi_hop_mask_skip: z.boolean(),
  /** DispatchStatus reason slugs (§22 canonical states). */
  multi_hop_status: z.string().nullable(),
  multi_hop_status_skip_reason: z.string().nullable(),
  /** Execution_Class (e.g. NONATOMIC_BRIDGE_REQUIRED) — null when N/A. */
  multi_hop_execution_class: z.string().nullable(),
  multi_hop_needs_class: z.string().nullable(),
  multi_hop_detector: z.string().nullable(),
  multi_hop_graph_policy: z.string().nullable(),
  multi_hop_family_hops: HopBoundsSchema.nullable(),
  multi_hop_hot_seed: z.string().nullable(),
  multi_hop_may_seed: z.boolean().nullable(),
  /** Wire is `mh_policy.map(…)` — null when no policy gated the tick. */
  multi_hop_do_not: z.string().nullable(),
  multi_hop_family_clamped: z.boolean(),
  multi_hop_family_skip_reason: z.string().nullable(),
}).strict();
export type MultiHopTelemetry = z.infer<typeof MultiHopTelemetrySchema>;

/**
 * Per-status census over the 264-strategy registry (COMPUTED from the
 * generated table — workbook drift changes the numbers). Keys are
 * DispatchStatus slugs (§22: route_ready / needs_route_data /
 * observe_only / no_compatible_route); kept as a record so the backend
 * remains the namer, the frontend never hardcodes the four (§21).
 */
export const StrategyStatusCountsSchema = z.record(
  z.string().min(1),
  z.number().int().min(0),
);
export type StrategyStatusCounts = z.infer<typeof StrategyStatusCountsSchema>;

// ─── RequiredDataGate block (ARBX-DP-002 + DP-003 tier field) ─────────────
/**
 * The tick's data-availability gate for the SELECTED detector (worker writes
 * `tick_summary["required_data_gate"] = json!(Option<…>)` — null when no
 * multi-hop policy was selected this tick). `tier` is the DP-003 emission
 * tier ("observation" | "signal" | "candidate" | "executable"); null = the
 * class drifted outside the closed 29-token vocabulary (honest unknown).
 */
export const RequiredDataGateTelemetrySchema = z.object({
  detector: z.string().min(1),
  surface: z.string(),
  verdict: z.enum(['ready', 'needs_data', 'not_tracked']),
  reason: z.string().nullable(),
  required_data: z.string(),
  /**
   * Closed four-token vocabulary — SignalTier::as_str() on the Rust side
   * (lib/apex/signal-tier.ts mirrors the same fold). A fifth token is
   * producer drift and FAILS here; null = class outside the closed
   * Execution_Class vocabulary (honest unknown, never a default tier).
   */
  tier: z.enum(['observation', 'signal', 'candidate', 'executable']).nullable(),
}).strict();
export type RequiredDataGateTelemetry = z.infer<typeof RequiredDataGateTelemetrySchema>;

// ─── DetectorMask block (ARBX-DP-004 HotSeedClassifier→DetectorMask) ──────
/**
 * The tick's dispatch-selectivity mask: which of the 60 detectors its event
 * evidence (the dirty-pool drain = "pool_reserve_update") may wake. Written
 * unconditionally as an object every tick; `selected_admitted` is null when
 * no multi-hop policy was selected.
 */
export const DetectorMaskTelemetrySchema = z.object({
  event: z.string(),
  admitted: z.number().int().min(0),
  total: z.number().int().min(0),
  selected_admitted: z.boolean().nullable(),
})
  .strict()
  .superRefine((mask, ctx) => {
    // admitted = popcount over `total` rows — it can never exceed it. Same
    // cross-field pattern as HopEnvelopeSchema (detectors.ts).
    if (mask.admitted > mask.total) {
      ctx.addIssue({
        code: z.ZodIssueCode.custom,
        path: ['admitted'],
        message: 'detector_mask.admitted must not exceed total',
      });
    }
  });
export type DetectorMaskTelemetry = z.infer<typeof DetectorMaskTelemetrySchema>;

// ─── Route dispatch + adapter cache ──────────────────────────────────────
export const RouteDispatchTelemetrySchema = z.object({
  routes_dispatched: z.number().int().min(0),
  adapter_cache_hit: z.number().int().min(0),
  adapter_backfill_ok: z.number().int().min(0),
  adapter_backfill_fail: z.number().int().min(0),
  adapter_budget_exhausted: z.number().int().min(0),
}).strict();
export type RouteDispatchTelemetry = z.infer<typeof RouteDispatchTelemetrySchema>;

// ─── Pair-finder tick (base tick_event wire, telemetry.rs tick_event) ────
/**
 * The 14 base keys `tick_event()` ALWAYS writes (verified against the worker
 * wire during EMIT-05 cross-check 2026-08-23). The 6 added beyond the original
 * :1468-1475 test-assert block (chain_id, edges_built, telemetry_emitted,
 * routes_dropped_for_cap, routes_capped, pools_truncated) are unconditional
 * in the builder — without them the `.strict()` aggregate rejected EVERY real
 * worker payload (unrecognized-key error), so the tick surface could never
 * have validated end-to-end.
 */
export const PairFinderTickSchema = z.object({
  event: z.string(),
  chain_id: z.number().int().min(1),
  algorithm: z.string(),
  routes_found: z.number().int().min(0),
  pools_total: z.number().int().min(0),
  edges_built: z.number().int().min(0),
  edges_rejected: z.number().int().min(0),
  telemetry_emitted: z.number().int().min(0),
  /** Honest truncation signal: routes_found is a lower bound when true. */
  routes_dropped_for_cap: z.number().int().min(0),
  routes_capped: z.boolean(),
  /** Per-pair branching cap dropped a parallel pool (set not exhaustive). */
  pools_truncated: z.boolean(),
  latency_ms: z.number().min(0),
  mode: z.string(),
}).strict();
export type PairFinderTick = z.infer<typeof PairFinderTickSchema>;

// ─── Aggregate tick summary (all groups optional = real backend states) ──
/**
 * FLAT mirror of the wire `tick_summary` object — every key lives at the
 * top level exactly as the worker sets it (no nested groups: the worker
 * writes `tick_summary["fe_prefilter_pass"]`, not
 * `tick_summary.fe_prefilter.pass`). All keys optional because emission is
 * path/knob-conditional (fe_prefilter needs knob ON, scoped_reeval needs
 * dirty_reeval, the pair-finder block is a separate path): absence is a
 * real backend state, never a frontend default (RULE 00).
 */
export const RouteDiscoveryTickSummarySchema = z
  .object({
    ...LatencyTelemetrySchema.shape,
    ...LatCandidatesTelemetrySchema.shape,
    ...DirtyDrainTelemetrySchema.shape,
    ...RouteDispatchTelemetrySchema.shape,
    ...MultiHopTelemetrySchema.shape,
    ...PairFinderTickSchema.shape,
    ...FePrefilterTelemetrySchema.shape,
    ...ScopedReevalTelemetrySchema.shape,
    strategy_status_counts: StrategyStatusCountsSchema,
    graph_rejected_reasons: GraphRejectedReasonsSchema,
    // DP-002/003/004 blocks — `required_data_gate` is Option on the wire
    // (null when no policy selected); `detector_mask` is always an object.
    required_data_gate: RequiredDataGateTelemetrySchema.nullable(),
    detector_mask: DetectorMaskTelemetrySchema,
  })
  .partial()
  .strict();
export type RouteDiscoveryTickSummary = z.infer<
  typeof RouteDiscoveryTickSummarySchema
>;
