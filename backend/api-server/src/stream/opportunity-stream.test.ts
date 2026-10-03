/**
 * STREAM-SEQ-01 (§11.4) — the ordered/idempotent envelope producer.
 *
 * These assertions pin the server half of the contract the frontend consumes
 * (`frontend/lib/store/stream-contract.ts`) and the SQL half the card grid
 * groups by (`opportunities-live.ts:295-303`):
 *
 *   · the stable keys are BIT-IDENTICAL to the LIVE_QUERY `concat_ws` twin,
 *     including the NULL→'' normalisation that `concat_ws` performs via the
 *     query's own COALESCE;
 *   · `seq` is monotonic per strategy and independent across strategies;
 *   · a byte-identical replay reuses (seq, event_id) so the client drops it as a
 *     duplicate instead of applying it twice;
 *   · `progress` never contains a fabricated zero: a metric nobody measured is
 *     simply absent from the group;
 *   · the envelope never rewrites the row — the raw leg's byte-identity is a
 *     documented contract (websocket.ts:506-515).
 */
import { describe, expect, it } from 'vitest';

import {
  createOpportunityStreamProducer,
  declaredSizeWeiOfRow,
  planKeyOfRow,
  routeGroupKeyOfRow,
  strategyKeyOfRow,
  STREAM_SCHEMA_VERSION,
  UNDECLARED_SIZE,
} from './opportunity-stream.js';

function row(over: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    id: 'opp-1',
    chain_id: 1,
    chain_id_out: null,
    strategy_kind: 'dex_arb',
    token_in: '0xaaa',
    token_out: '0xbbb',
    dex_a: 'uniswap_v2',
    dex_b: 'sushiswap',
    detected_at: '2026-10-03T10:00:00Z',
    status: 'rejected',
    rejection_reason: 'spread_zero_equilibrium',
    ...over,
  };
}

describe('opportunity-stream — stable keys mirror the SQL GROUP BY twin', () => {
  it('routeGroupKeyOfRow renders NULLs as empty segments, exactly like concat_ws + COALESCE', () => {
    // concat_ws('|', chain_id::text, COALESCE(chain_id_out::text,''),
    //           COALESCE(strategy_kind,''), token_in, token_out, dex_a,
    //           COALESCE(dex_b,''))
    expect(routeGroupKeyOfRow(row())).toBe('1||dex_arb|0xaaa|0xbbb|uniswap_v2|sushiswap');
  });

  it('strategyKeyOfRow is chain|strategy and degrades to a stable bucket', () => {
    expect(strategyKeyOfRow(row())).toBe('1|dex_arb');
    expect(strategyKeyOfRow(row({ chain_id: null, strategy_kind: null }))).toBe('|');
  });

  it('the declared notional is an EXACT wei string or undeclared — never a parsed float', () => {
    const declared = row({
      route_metadata: { economics_amount_in_wei: '1000000' },
    });
    expect(declaredSizeWeiOfRow(declared)).toBe('1000000');
    // A numeric (float64) declaration is NOT accepted as exact.
    expect(
      declaredSizeWeiOfRow({ route_metadata: { economics_amount_in_wei: 1000000 } }),
    ).toBeNull();
    expect(declaredSizeWeiOfRow({ route_metadata: { economics_amount_in_wei: '1e6' } })).toBeNull();
    expect(declaredSizeWeiOfRow({ route_metadata: {} })).toBeNull();
    expect(planKeyOfRow(row())).toContain(`@${UNDECLARED_SIZE}`);
  });
});

describe('opportunity-stream — ordering and idempotency', () => {
  it('allocates a monotonic seq per strategy, independently per strategy', () => {
    let t = 0;
    const producer = createOpportunityStreamProducer({ now: () => (t += 1000) });
    const a1 = producer.build(row({ id: 'a1', detected_at: '2026-10-03T10:00:00Z' }))!;
    const a2 = producer.build(row({ id: 'a2', detected_at: '2026-10-03T10:01:00Z' }))!;
    const b1 = producer.build(
      row({ id: 'b1', chain_id: 10, strategy_kind: 'triangular_atomic' }),
    )!;
    expect(a1.seq).toBe(1);
    expect(a2.seq).toBe(2);
    expect(b1.seq).toBe(3);
    expect(a2.strategy_key).toBe(a1.strategy_key);
    expect(b1.strategy_key).not.toBe(a1.strategy_key);
    expect(b1.schema_version).toBe(STREAM_SCHEMA_VERSION);
  });

  it('a byte-identical replay REUSES (seq, event_id) — the client then drops it as a duplicate', () => {
    const producer = createOpportunityStreamProducer({ now: () => 1_700_000_000_000 });
    const first = producer.build(row())!;
    const replay = producer.build(row())!;
    expect(replay.seq).toBe(first.seq);
    expect(replay.event_id).toBe(first.event_id);
  });

  it('a REAL change allocates a NEW seq (a replay guard must not swallow updates)', () => {
    const producer = createOpportunityStreamProducer({ now: () => 1_700_000_000_000 });
    const first = producer.build(row({ economics: { computation_status: 'computed', net_profit_usd: -7.91 } }))!;
    const second = producer.build(row({ economics: { computation_status: 'computed', net_profit_usd: -3.5 } }))!;
    expect(second.seq).toBeGreaterThan(first.seq);
    expect(second.event_id).not.toBe(first.event_id);
  });

  it('a status transition is a real change too (plan.rejected vs plan.updated)', () => {
    const producer = createOpportunityStreamProducer({ now: () => 1 });
    const rejected = producer.build(row({ status: 'rejected' }))!;
    const validated = producer.build(row({ status: 'validated' }))!;
    expect(rejected.kind).toBe('plan.rejected');
    expect(validated.kind).toBe('plan.updated');
    expect(validated.seq).toBeGreaterThan(rejected.seq);
  });

  it('returns NULL for a non-row instead of stamping a fabricated event', () => {
    const producer = createOpportunityStreamProducer();
    expect(producer.build(null)).toBeNull();
    expect(producer.build('nope')).toBeNull();
    expect(producer.build([])).toBeNull();
  });

  it('never rewrites the payload (the raw leg stays byte-identical)', () => {
    const producer = createOpportunityStreamProducer({ now: () => 1 });
    const r = row();
    const before = JSON.stringify(r);
    const event = producer.build(r)!;
    expect(event.payload).toBe(r); // same object, not a copy with edits
    expect(JSON.stringify(r)).toBe(before);
  });

  it('carries the snapshot id so a resync can tell "behind" from "current"', () => {
    const producer = createOpportunityStreamProducer({ now: () => 1 });
    producer.setSnapshotId('snap-42');
    const event = producer.build(row())!;
    expect(event.snapshot_id).toBe('snap-42');
    expect(event.progress?.snapshot_id).toBe('snap-42');
  });
});

describe('opportunity-stream — progress never fabricates a zero (R10)', () => {
  it('an unobserved strategy has NO progress keys at all', () => {
    const producer = createOpportunityStreamProducer();
    expect(producer.progressOf('9|never_seen', null)).toEqual({});
  });

  it('the quote census is emitted ONLY over rows that carried an economics object', () => {
    const producer = createOpportunityStreamProducer({ now: () => 1 });
    producer.build(row({ id: 'no-econ' }));
    const progress = producer.progressOf('1|dex_arb', null);
    expect(progress.routes_explored).toBe(1);
    // Nobody emitted `economics` ⇒ the census is ABSENT, not 0/0/0.
    expect(progress.quotes_exact).toBeUndefined();
    expect(progress.quotes_incomplete).toBeUndefined();
    expect(progress.quotes_error).toBeUndefined();

    producer.build(
      row({ id: 'with-econ', economics: { computation_status: 'computed' } }),
    );
    const after = producer.progressOf('1|dex_arb', null);
    expect(after.quotes_exact).toBe(1);
    expect(after.quotes_incomplete).toBe(0); // a real census of zero IS a measurement
    expect(after.quotes_error).toBe(0);
  });

  it('routes_explored counts distinct ROUTES, sizes_quoted distinct declared notions', () => {
    const producer = createOpportunityStreamProducer({ now: () => 1 });
    const rm = (wei: string) => ({ route_metadata: { economics_amount_in_wei: wei } });
    producer.build(row({ id: 'a', ...rm('1000') }));
    producer.build(row({ id: 'b', detected_at: '2026-10-03T10:01:00Z', ...rm('1000') }));
    producer.build(row({ id: 'c', token_out: '0xccc', ...rm('2000') }));
    const p = producer.progressOf('1|dex_arb', null);
    expect(p.routes_explored).toBe(2);
    expect(p.sizes_quoted).toBe(2);
  });

  it('budget exhaustion is declared ONLY from the explicit rejection vocabulary', () => {
    const producer = createOpportunityStreamProducer({ now: () => 1 });
    producer.build(row({ id: 'a', rejection_reason: 'non_positive_profit' }));
    expect(producer.progressOf('1|dex_arb', null).budget_exhausted).toBeUndefined();

    const event = producer.build(
      row({ id: 'b', detected_at: '2026-10-03T10:02:00Z', rejection_reason: 'expansion_budget_exhausted' }),
    )!;
    expect(event.progress?.budget_exhausted).toBe(true);
    expect(event.progress?.budget_reason).toBe('expansion_budget_exhausted');
  });
});
