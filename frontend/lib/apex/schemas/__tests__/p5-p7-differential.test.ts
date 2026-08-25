import { describe, expect, it } from 'vitest';

// Differential fixture = the REAL generated wire table (EMIT-07/08 source),
// imported cross-package on purpose: these tests pin the WORKBOOK-VERSION
// invariants the runtime schemas deliberately do NOT hardcode (§28). If a
// new workbook ingestion changes any of these numbers, this file is the
// alarm — the schema itself stays count-agnostic.
import {
  QUOTEBASE_DETECTOR_CATALOG,
  QUOTEBASE_STRATEGY_CATALOG,
} from '../../../../../backend/api-server/src/generated/quotebase_catalog';

import { DetectorCatalogResponseSchema, StrategyCatalogResponseSchema } from '../index';

describe('P6/P7 differential — frozen Zod vs generated wire table', () => {
  it('ALL strategy rows parse through StrategyCatalogResponseSchema', () => {
    const r = StrategyCatalogResponseSchema.safeParse({ entries: QUOTEBASE_STRATEGY_CATALOG });
    if (!r.success) {
      console.log(JSON.stringify(r.error.issues.slice(0, 5), null, 2));
    }
    expect(r.success).toBe(true);
  });

  it('ALL detector rows parse through DetectorCatalogResponseSchema', () => {
    const r = DetectorCatalogResponseSchema.safeParse({ entries: QUOTEBASE_DETECTOR_CATALOG });
    if (!r.success) {
      console.log(JSON.stringify(r.error.issues.slice(0, 5), null, 2));
    }
    expect(r.success).toBe(true);
  });

  it('264 rows (workbook-version invariant)', () => {
    expect(QUOTEBASE_STRATEGY_CATALOG).toHaveLength(264);
  });

  it('60 detector families (workbook-version invariant)', () => {
    expect(QUOTEBASE_DETECTOR_CATALOG).toHaveLength(60);
  });

  it('status counts 79 ROUTE_READY / 174 NEEDS_ROUTE_DATA / 8 OBSERVE_ONLY / 3 NO_COMPATIBLE_ROUTE', () => {
    const counts = { ROUTE_READY: 0, NEEDS_ROUTE_DATA: 0, OBSERVE_ONLY: 0, NO_COMPATIBLE_ROUTE: 0 };
    for (const row of QUOTEBASE_STRATEGY_CATALOG) counts[row.status] += 1;
    expect(counts).toEqual({ ROUTE_READY: 79, NEEDS_ROUTE_DATA: 174, OBSERVE_ONLY: 8, NO_COMPATIBLE_ROUTE: 3 });
  });

  it('DETERMINISTIC_EXECUTABLE ⊆ ROUTE_READY (37/37)', () => {
    const det = QUOTEBASE_STRATEGY_CATALOG.filter((r) => r.execution_class === 'DETERMINISTIC_EXECUTABLE');
    expect(det).toHaveLength(37);
    expect(det.every((r) => r.status === 'ROUTE_READY')).toBe(true);
  });

  it('OBSERVE_ONLY status ⟺ OBSERVE_ONLY class (8/8)', () => {
    const byStatus = QUOTEBASE_STRATEGY_CATALOG.filter((r) => r.status === 'OBSERVE_ONLY');
    const byClass = QUOTEBASE_STRATEGY_CATALOG.filter((r) => r.execution_class === 'OBSERVE_ONLY');
    expect(byStatus).toHaveLength(8);
    expect(byClass).toHaveLength(8);
    expect(new Set(byStatus.map((r) => r.mev_id))).toEqual(new Set(byClass.map((r) => r.mev_id)));
  });

  it('Σ strategies_count across detectors == 264', () => {
    const sum = QUOTEBASE_DETECTOR_CATALOG.reduce((acc, d) => acc + d.strategies_count, 0);
    expect(sum).toBe(QUOTEBASE_STRATEGY_CATALOG.length);
  });

  it('every strategy detector_id links to a catalog family', () => {
    const ids = new Set(QUOTEBASE_DETECTOR_CATALOG.map((d) => d.detector_id));
    for (const row of QUOTEBASE_STRATEGY_CATALOG) {
      expect(ids.has(row.detector_id)).toBe(true);
    }
  });

  it('mev_id ascending + unique', () => {
    const ids = QUOTEBASE_STRATEGY_CATALOG.map((r) => r.mev_id);
    expect(new Set(ids).size).toBe(ids.length);
    expect([...ids].sort()).toEqual(ids);
  });
});

// ─── EMIT-06 differential — the route's pinned wire vector vs the schema ──
// Vector = the EXACT response asserted by backend/api-server
// src/routes/pairs.test.ts ("serves { entries } EXACTLY…"): 3-pool universe →
// 1 canonical pair (AB) with 2 re-oriented live pools, P2 dirty, PAIR-AC
// dropped (no reserves), alpha honest-null (EMIT-06b), NULL tier → 30 class
// constant, last_reserve_update = max ts in ms.
import { PairsResponseSchema } from '../index';

const A41 = `0x${'a'.repeat(40)}`;
const B41 = `0x${'b'.repeat(40)}`;
const P1 = `0x${'1'.repeat(40)}`;
const P2 = `0x${'2'.repeat(40)}`;

const PAIRS_ROUTE_VECTOR = {
  entries: [
    {
      chain_id: 1,
      token_a: { chain_id: 1, address: A41, symbol: 'AAA', decimals: 18 },
      token_b: { chain_id: 1, address: B41, symbol: 'BBB', decimals: 6 },
      pools: [
        // P1 token0=A ⇒ a=r0 "1000"; NULL tier ⇒ fee 30 (class constant).
        { pool_address: P1, venue: 'DexOne', fee_bps: 30, reserves_a: '1000', reserves_b: '2000' },
        // P2 token0=B ⇒ a=r1 "77" (re-oriented onto canonical legs).
        { pool_address: P2, venue: 'DexTwo', fee_bps: 100, reserves_a: '77', reserves_b: '88' },
      ],
      venue_count: 2,
      alpha_forward: null,
      alpha_reverse: null,
      dirty: true,
      last_reserve_update: 1_700_000_050_000,
    },
  ],
};

describe('P5 differential — EMIT-06 pairs route vector vs PairsResponseSchema', () => {
  it('the pinned route vector parses (oriented string reserves §62, honest nulls)', () => {
    const r = PairsResponseSchema.safeParse(PAIRS_ROUTE_VECTOR);
    if (!r.success) {
      console.log(JSON.stringify(r.error.issues.slice(0, 5), null, 2));
    }
    expect(r.success).toBe(true);
    expect(r.data!.entries[0]!.pools).toHaveLength(2);
    expect(r.data!.entries[0]!.alpha_forward).toBeNull();
  });

  it('a pair-level sidecar riding the envelope FAILS .strict()', () => {
    expect(
      PairsResponseSchema.safeParse({
        ...PAIRS_ROUTE_VECTOR,
        total_pairs: 1,
      }).success,
    ).toBe(false);
  });

  it('numeric reserves (float drift of §62 decimal strings) FAIL', () => {
    const drifted = {
      entries: [
        {
          ...PAIRS_ROUTE_VECTOR.entries[0],
          pools: [
            { ...PAIRS_ROUTE_VECTOR.entries[0]!.pools[0], reserves_a: 1000 },
          ],
        },
      ],
    };
    expect(PairsResponseSchema.safeParse(drifted).success).toBe(false);
  });
});
