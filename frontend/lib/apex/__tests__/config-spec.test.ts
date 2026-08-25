import { describe, expect, it } from 'vitest';

import {
  buildKnobRows,
  CANONICAL_KNOB_BINDINGS,
  EXCEL_CONFIG_SPEC,
} from '../config-spec';
import { CanonicalKnobsResponseSchema } from '../schemas/knobs';

/**
 * Pin of the canonical_knobs.rs published field set (validated 2026-08-24).
 * Every binding key MUST exist in the searcher's boot snapshot — a binding
 * to a non-existent knob would render a permanent NOT EXPOSED lie.
 */
const PUBLISHED_KNOB_KEYS = new Set([
  'max_hops', 'min_hops', 'selected_financing', 'min_pool_liquidity_usd',
  'max_pool_utilization_pct', 'min_gross_edge_bps', 'max_gas_usd',
  'max_freshness_s', 'max_state_age_blocks', 'min_size_usd', 'min_ev_usd',
  'risk_haircut_pct', 'slippage_factor', 'min_net_bps', 'beam_k',
  'dirty_reeval_enabled', 'fe_prefilter_enabled', 'quote_w_prior',
  'quote_w_liquidity', 'quote_w_venue_coverage', 'quote_w_stability',
  'quote_w_cross_dex', 'discovery_sla_ms', 'emission_budget_routes_block',
  'candidate_budget_routes_block', 'block_cadence_s', 'cpu_op_budget_block',
  'estimated_cycles', 'execution_mode', 'enable_2v2',
]);

const SNAPSHOT = {
  generated_at: '2026-08-24T00:00:00.000Z',
  source: 'searcher-rs CanonicalKnobs (boot snapshot)',
  knobs: {
    beam_k: 4,
    max_hops: 7,
    min_hops: 2,
    min_pool_liquidity_usd: 150000,
    min_net_bps: 5,
    max_state_age_blocks: 2,
    quote_w_prior: 0.3,
    quote_w_liquidity: 0.3,
    quote_w_venue_coverage: 0.2,
    quote_w_stability: 0.1,
    quote_w_cross_dex: 0.1,
    discovery_sla_ms: 30,
  },
};

describe('EXCEL_CONFIG_SPEC (workbook 01_CONFIG canon)', () => {
  it('carries the 17 workbook parameters', () => {
    expect(EXCEL_CONFIG_SPEC).toHaveLength(17);
    expect(new Set(EXCEL_CONFIG_SPEC.map((r) => r.Parameter)).size).toBe(17);
  });

  it('12 of 17 parameters bind to a published canonical knob', () => {
    expect(Object.keys(CANONICAL_KNOB_BINDINGS)).toHaveLength(12);
  });

  it('every binding targets a knob the searcher actually publishes', () => {
    for (const key of Object.values(CANONICAL_KNOB_BINDINGS)) {
      expect(PUBLISHED_KNOB_KEYS.has(key)).toBe(true);
    }
  });
});

describe('buildKnobRows classification (FE-CFG-002/003)', () => {
  it('full snapshot: 12 EFFECTIVE + 1 DERIVED + 4 NOT EXPOSED', () => {
    const rows = buildKnobRows(CanonicalKnobsResponseSchema.parse(SNAPSHOT));
    const counts = { EFFECTIVE: 0, DERIVED: 0, NOT_EXPOSED: 0 };
    for (const r of rows) counts[r.status] += 1;
    expect(counts).toEqual({ EFFECTIVE: 12, DERIVED: 1, NOT_EXPOSED: 4 });
  });

  it('formula parameter (Allowed_Symbol_Count) is DERIVED, never fabricated', () => {
    const rows = buildKnobRows(CanonicalKnobsResponseSchema.parse(SNAPSHOT));
    const row = rows.find((r) => r.spec.Parameter === 'Allowed_Symbol_Count');
    expect(row?.status).toBe('DERIVED');
    expect(row?.effective).toBeUndefined();
  });

  it('dynamic graph metrics (N_Active_Chain…) are NOT EXPOSED — absence IS the datum', () => {
    const rows = buildKnobRows(CanonicalKnobsResponseSchema.parse(SNAPSHOT));
    for (const p of ['N_Active_Chain', 'Avg_Active_Degree', 'Avg_Parallel_Pools', 'Dirty_Seeds']) {
      expect(rows.find((r) => r.spec.Parameter === p)?.status).toBe('NOT_EXPOSED');
    }
  });

  it('null snapshot (503 pre-boot): zero EFFECTIVE claims — absence never over-claims (R8)', () => {
    const rows = buildKnobRows(null);
    expect(rows.filter((r) => r.status === 'EFFECTIVE')).toHaveLength(0);
    expect(rows.filter((r) => r.status === 'DERIVED')).toHaveLength(1);
    expect(rows.filter((r) => r.status === 'NOT_EXPOSED')).toHaveLength(16);
  });

  it('bound but unpublished knob (partial snapshot) classifies NOT_EXPOSED, not a zero', () => {
    const partial = CanonicalKnobsResponseSchema.parse({ ...SNAPSHOT, knobs: { beam_k: 8 } });
    const rows = buildKnobRows(partial);
    expect(rows.find((r) => r.spec.Parameter === 'Beam_K')?.effective).toBe(8);
    expect(rows.find((r) => r.spec.Parameter === 'Max_Hops')?.status).toBe('NOT_EXPOSED');
    expect(rows.find((r) => r.spec.Parameter === 'Max_Hops')?.effective).toBeUndefined();
  });
});

describe('CanonicalKnobsResponseSchema (wire)', () => {
  it('parses the envelope (ISO timestamp, source, knob record)', () => {
    expect(CanonicalKnobsResponseSchema.safeParse(SNAPSHOT).success).toBe(true);
  });

  it('strict: unknown envelope keys fail', () => {
    expect(
      CanonicalKnobsResponseSchema.safeParse({ ...SNAPSHOT, extra: 1 }).success,
    ).toBe(false);
  });

  it('non-ISO generated_at fails (INV-2 hermiticity)', () => {
    expect(
      CanonicalKnobsResponseSchema.safeParse({ ...SNAPSHOT, generated_at: 'yesterday' }).success,
    ).toBe(false);
  });
});
