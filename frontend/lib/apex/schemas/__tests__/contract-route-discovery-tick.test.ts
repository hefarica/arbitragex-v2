/**
 * ARBX-XLANG-01 — CROSS-LANGUAGE WIRE CONTRACT (Rust ↔ Zod mirror).
 *
 * The fixtures in `fixtures/route-discovery-tick/` are NOT authored here:
 * they are emitted by the searcher's REAL tick path (evaluate_tick +
 * inject_scan_telemetry, controlled inputs) and committed by the Rust test
 * `xlang_golden_tick_contract` (backend/searcher-rs/src/route_discovery/
 * route_discovery_worker.rs). This test parses those exact bytes with the
 * mirror — closing the loop that once failed in prod: the mirror declared
 * `multi_hop_capped: z.number()` while the wire carried a boolean, and the
 * `.strict()` aggregate rejected EVERY payload ("schema_reject … nothing
 * has been accepted since").
 *
 * Contract chain (no step may be skipped):
 *   Rust emission changes
 *     → Rust golden test fails until fixtures are regenerated
 *     → fixtures change
 *     → THIS test fails until the mirror follows (.strict() rejects unknown
 *       keys; types must match; nullability must match).
 *
 * Regen (after an INTENTIONAL wire change, mirror update in the same PR):
 *   ARBX_REGEN_GOLDEN=1 cargo test -p searcher-rs xlang_golden
 */
import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { RouteDiscoveryTickSummarySchema } from '../telemetry';

const FIXTURE_DIR = join(
  dirname(fileURLToPath(import.meta.url)),
  'fixtures',
  'route-discovery-tick',
);

function loadFixture(name: string): unknown {
  return JSON.parse(readFileSync(join(FIXTURE_DIR, name), 'utf8'));
}

describe('ARBX-XLANG-01 · route-discovery tick wire contract (Rust-emitted goldens)', () => {
  it('full.json (all groups on) parses under the strict mirror', () => {
    const parsed = RouteDiscoveryTickSummarySchema.parse(loadFixture('full.json'));
    // Regression pins for the four keys that once killed every payload.
    expect(parsed.multi_hop_capped).toBeTypeOf('boolean');
    expect(parsed.multi_hop_do_not).toBeTypeOf('string');
    expect(parsed.fe_prefilter_anchor_dynamic).toBeTypeOf('boolean');
    // Census: non-empty string→count record (slugs belong to the Rust
    // builder — the mirror never names them, backend stays the namer).
    const census = parsed.graph_rejected_reasons ?? {};
    expect(Object.keys(census).length).toBeGreaterThan(0);
    for (const count of Object.values(census)) {
      expect(count).toBeTypeOf('number');
    }
  });

  it('knobs-off.json (dormant tick) parses — absence is a real backend state', () => {
    const parsed = RouteDiscoveryTickSummarySchema.parse(loadFixture('knobs-off.json'));
    // Policy-less tick: the wire carries an honest null, never a string.
    expect(parsed.multi_hop_do_not).toBeNull();
    // Knob OFF ⇒ the whole F_e / scoped groups are ABSENT, not zeroed.
    expect(parsed).not.toHaveProperty('fe_prefilter_evaluated');
    expect(parsed).not.toHaveProperty('scoped_reeval');
    // Empty latency log ⇒ honest nulls, never fabricated zeros.
    expect(parsed.lat_pass_p95).toBeNull();
    expect(parsed.lat_cycles).toBe(0);
    for (const row of parsed.lat_stages ?? []) {
      expect(row.p50_us).toBeNull();
    }
  });

  it('fixture keys are exactly the mirror keys — .strict() rejects any drift', () => {
    // Direct proof the strict aggregate is in force: one injected foreign
    // key must fail the parse (if .strict() ever regresses to strip-mode,
    // new wire keys would die silently — the "se pierde nada" guarantee).
    const full = loadFixture('full.json') as Record<string, unknown>;
    const tampered = { ...full, __future_rust_key__: 1 };
    const result = RouteDiscoveryTickSummarySchema.safeParse(tampered);
    expect(result.success).toBe(false);
  });
});
