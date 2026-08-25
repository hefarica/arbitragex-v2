/**
 * Ω ARBX-FE-EMIT-09 · RouteDiscoveryTickSummary — lat_candidates vectors.
 *
 * The per-candidate group EMIT-09 added to the flat tick_summary wire
 * (source: route_discovery/lat_candidates.rs rows_value/meta_value):
 *
 *   lat_candidates: top-K rows by total_us desc — {route_hash, route_kind,
 *     hops, stages{gates_us, reprice_us?}, total_us}. R8 presence-of-key:
 *     `stages.reprice_us` is ABSENT (not 0, not null) when the route never
 *     traversed the adapter this tick — absence IS the state.
 *
 *   lat_candidates_meta: {attribution{gates, reprice}, cap, sampled,
 *     truncated, dropped} — the once-per-tick honesty block; attribution is a
 *     closed literal vocabulary and the cut counters must agree.
 *
 * Vectors mirror the Rust json! literals. RULE 00: absence of the whole group
 * is legal (.partial() — pure/clock-free ticks emit nothing); a malformed
 * shape is NOT.
 */
import { describe, expect, it } from 'vitest';

import { RouteDiscoveryTickSummarySchema } from '../telemetry';

const ROW_TRIANGULAR = {
  route_hash: '0xabc123def4567890abc123def4567890abc123def4567890abc123def4567890',
  route_kind: 'triangular',
  hops: 3,
  stages: { gates_us: 96, reprice_us: 1240 },
  total_us: 1336,
} as const;

const ROW_V2V2_GATES_ONLY = {
  route_hash: '0xfeed00000000000000000000000000000000000000000000000000000000beef',
  route_kind: 'v2v2',
  hops: 2,
  // reprice_us ABSENT: a 2-leg route never traverses the triangular adapter.
  stages: { gates_us: 41 },
  total_us: 41,
} as const;

const META_HONEST = {
  attribution: { gates: 'measured', reprice: 'measured-upper-bound' },
  cap: 10,
  sampled: 2,
  truncated: false,
  dropped: 0,
} as const;

const META_TRUNCATED = {
  ...META_HONEST,
  sampled: 12,
  truncated: true,
  dropped: 2,
} as const;

describe('lat_candidates rows (EMIT-09)', () => {
  it('full vector parses: traversed + non-traversed rows together', () => {
    const r = RouteDiscoveryTickSummarySchema.safeParse({
      lat_candidates: [ROW_TRIANGULAR, ROW_V2V2_GATES_ONLY],
      lat_candidates_meta: META_HONEST,
    });
    expect(r.success).toBe(true);
    if (r.success) {
      expect(r.data.lat_candidates).toHaveLength(2);
      // Presence-of-key is the R8 signal — the v2v2 row carries NO key.
      expect(r.data.lat_candidates![1]!.stages).not.toHaveProperty('reprice_us');
    }
  });

  it('absence of the whole group parses (pure tick, knob off) — RULE 00', () => {
    const r = RouteDiscoveryTickSummarySchema.safeParse({});
    expect(r.success).toBe(true);
  });

  it('empty rows + zero-count meta parse — honest empty tick', () => {
    const r = RouteDiscoveryTickSummarySchema.safeParse({
      lat_candidates: [],
      lat_candidates_meta: { ...META_HONEST, sampled: 0 },
    });
    expect(r.success).toBe(true);
  });

  it('reprice_us null REJECTS — absence is undefined, null is a lie about shape', () => {
    const r = RouteDiscoveryTickSummarySchema.safeParse({
      lat_candidates: [
        { ...ROW_V2V2_GATES_ONLY, stages: { gates_us: 41, reprice_us: null } },
      ],
      lat_candidates_meta: META_HONEST,
    });
    expect(r.success).toBe(false);
  });

  it('route_kind outside the closed vocabulary REJECTS (e.g. the "pair" shorthand)', () => {
    const r = RouteDiscoveryTickSummarySchema.safeParse({
      lat_candidates: [{ ...ROW_V2V2_GATES_ONLY, route_kind: 'pair' }],
      lat_candidates_meta: META_HONEST,
    });
    expect(r.success).toBe(false);
  });

  it('total_us ≠ Σ traversed stages REJECTS — producer coherence', () => {
    const r = RouteDiscoveryTickSummarySchema.safeParse({
      lat_candidates: [{ ...ROW_TRIANGULAR, total_us: 9999 }],
      lat_candidates_meta: META_HONEST,
    });
    expect(r.success).toBe(false);
  });

  it('hops outside the closed envelope [2,7] REJECTS', () => {
    const r = RouteDiscoveryTickSummarySchema.safeParse({
      lat_candidates: [{ ...ROW_TRIANGULAR, hops: 8 }],
      lat_candidates_meta: META_HONEST,
    });
    expect(r.success).toBe(false);
  });
});

describe('lat_candidates_meta (the honesty block)', () => {
  it('truncated cut with agreeing counters parses', () => {
    const r = RouteDiscoveryTickSummarySchema.safeParse({
      lat_candidates: [ROW_TRIANGULAR],
      lat_candidates_meta: META_TRUNCATED,
    });
    expect(r.success).toBe(true);
  });

  it('dropped disagreeing with sampled − kept REJECTS', () => {
    const r = RouteDiscoveryTickSummarySchema.safeParse({
      lat_candidates: [ROW_TRIANGULAR],
      lat_candidates_meta: { ...META_TRUNCATED, dropped: 5 }, // sampled 12 − cap 10 ⇒ 2
    });
    expect(r.success).toBe(false);
  });

  it('truncated lying about the cut REJECTS', () => {
    const r = RouteDiscoveryTickSummarySchema.safeParse({
      lat_candidates: [ROW_TRIANGULAR],
      lat_candidates_meta: { ...META_TRUNCATED, truncated: false }, // dropped 2 > 0
    });
    expect(r.success).toBe(false);
  });

  it('attribution outside the closed vocabulary REJECTS (producer drift)', () => {
    const r = RouteDiscoveryTickSummarySchema.safeParse({
      lat_candidates: [ROW_TRIANGULAR],
      lat_candidates_meta: {
        ...META_HONEST,
        attribution: { gates: 'measured', reprice: 'estimated' },
      },
    });
    expect(r.success).toBe(false);
  });

  it('cap 0 REJECTS — the cap is a payload bound clamped >= 1, never an off switch', () => {
    const r = RouteDiscoveryTickSummarySchema.safeParse({
      lat_candidates: [ROW_TRIANGULAR],
      lat_candidates_meta: { ...META_HONEST, cap: 0 },
    });
    expect(r.success).toBe(false);
  });
});
