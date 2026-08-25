// frontend/app/strategies/tabs/__tests__/PairStateFlowPanel.test.tsx
//
// FE-MASTER · FE-0020 — §17 pair lifecycle state flow, SSR-branch tests.
//
// Pure presentational component (props in, markup out — the store wiring
// lives in PairIntelligencePanel). renderToStaticMarkup asserts:
//   - stage counters are VERBATIM wire keys (drain_*/fe_prefilter_*/dirty_seeds);
//   - CLEAN/DIRTY census counts payload booleans; the census stays alive
//     even when the tick is absent (independent surfaces);
//   - QUEUED/HOT/EXPANDING/COOLED render as dimmed gaps labeled
//     FE-0020-pending-EMIT-06c — never a fabricated count;
//   - R8: absent group = "—" / "knob OFF"; present group with 0 = "0"
//     (computed-and-zero ≠ not computed);
//   - tick error surfaces verbatim (role=alert).
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { PairStateFlowPanel } from "../PairStateFlowPanel";
import type { PairView, RouteDiscoveryTickSummary } from "@/lib/apex/schemas";

const A = "0x" + "a".repeat(40);
const B = "0x" + "b".repeat(40);
const C = "0x" + "c".repeat(40);

function pair(dirty: boolean, addrA = A, addrB = B): PairView {
  return {
    chain_id: 1,
    token_a: { chain_id: 1, address: addrA, symbol: "WETH", decimals: 18 },
    token_b: { chain_id: 1, address: addrB, symbol: "USDC", decimals: 6 },
    pools: [],
    venue_count: 0,
    alpha_forward: null,
    alpha_reverse: null,
    dirty,
    last_reserve_update: null,
  };
}

// 2 clean + 1 dirty — the census the panel must count from flags alone.
const PAIRS: PairView[] = [pair(false), pair(false, A, C), pair(true, B, C)];

const TICK: RouteDiscoveryTickSummary = {
  drain_drained: 12,
  drain_unknown_pool: 1,
  drain_invalid_pair: 0,
  drain_already_dirty: 3,
  drain_seeded: 4,
  drain_evicted: 1,
  drain_register_reject: 0,
  dirty_seeds: 2,
  adapter_scoped_skip: 0,
  fe_prefilter_evaluated: 120,
  fe_prefilter_pass: 80,
  fe_prefilter_below_reference: 36,
  fe_prefilter_uncomputed: 4,
  fe_prefilter_map_fail: 0,
};

function render(props: {
  pairs?: PairView[] | null;
  tick?: RouteDiscoveryTickSummary | null;
  tickError?: string | null;
}) {
  return renderToStaticMarkup(
    React.createElement(PairStateFlowPanel, {
      pairs: props.pairs ?? null,
      tick: props.tick ?? null,
      tickError: props.tickError ?? null,
    }),
  );
}

describe("PairStateFlowPanel — SSR branches (FE-0020 · §17)", () => {
  it("stage counters are the verbatim wire keys (drain_* / fe_prefilter_* / dirty_seeds)", () => {
    const html = render({ pairs: PAIRS, tick: TICK });
    // Primaries per stage.
    expect(html).toContain("12"); // Event  — drain_drained
    expect(html).toContain("3"); // PoolDirty — drain_already_dirty
    expect(html).toContain("120"); // Prefilter — fe_prefilter_evaluated
    expect(html).toContain("2"); // HotSeed — dirty_seeds (the census 2 also rides)
    // Secondary detail lines.
    expect(html).toContain("unknown_pool 1");
    expect(html).toContain("invalid_pair 0");
    expect(html).toContain("pass 80");
    expect(html).toContain("below 36");
    expect(html).toContain("uncomputed 4");
    expect(html).toContain("seeded 4");
    expect(html).toContain("evicted 1");
  });

  it("census counts payload flags; pending states are dimmed gaps, never fabricated counts", () => {
    const html = render({ pairs: PAIRS, tick: TICK });
    expect(html).toContain("CLEAN 2");
    expect(html).toContain("DIRTY 1");
    expect(html).toContain("universo servido 3 pares");
    // All four unpublished states render as gap slots with the label…
    expect(html).toContain("QUEUED —");
    expect(html).toContain("HOT —");
    expect(html).toContain("EXPANDING —");
    expect(html).toContain("COOLED —");
    expect(html).toContain("FE-0020-pending-EMIT-06c");
    // …and NEVER as a zero count (a fabricated 0 would read "all cool").
    expect(html).not.toContain("QUEUED 0");
    expect(html).not.toContain("HOT 0");
    expect(html).not.toContain("EXPANDING 0");
    expect(html).not.toContain("COOLED 0");
  });

  it("tick null ⇒ stages honest dash + note; the census stays alive (independent surfaces)", () => {
    const html = render({ pairs: PAIRS, tick: null });
    expect(html).toContain("sin tick servido");
    // Census from the pairs payload is NOT hostage to the tick surface.
    expect(html).toContain("CLEAN 2");
    expect(html).toContain("DIRTY 1");
    // Prefilter dormant message is the knob note, not a zero.
    expect(html).not.toContain("knob OFF");
  });

  it("fe_prefilter group absent ⇒ knob OFF (dormant ≠ 0); drain stages keep their numbers", () => {
    const { fe_prefilter_evaluated: _e, fe_prefilter_pass: _p, fe_prefilter_below_reference: _b, fe_prefilter_uncomputed: _u, fe_prefilter_map_fail: _m, ...drainOnly } = TICK;
    const html = render({ pairs: PAIRS, tick: drainOnly });
    expect(html).toContain("knob OFF");
    expect(html).not.toContain("120");
    expect(html).not.toContain("pass 80");
    // The drain side of the flow is untouched.
    expect(html).toContain("12");
    expect(html).toContain("dirty");
  });

  it("present group with real zeros renders 0 — computed-and-zero, never a dash", () => {
    const quiet: RouteDiscoveryTickSummary = {
      ...TICK,
      drain_drained: 0,
      drain_already_dirty: 0,
      dirty_seeds: 0,
      fe_prefilter_evaluated: 0,
    };
    const html = render({ pairs: PAIRS, tick: quiet });
    expect(html).toContain(">0<"); // a stage cell rendered the zero
    expect(html).not.toContain("knob OFF"); // group present ⇒ not dormant
  });

  it("tick error surfaces verbatim (role=alert)", () => {
    const html = render({ pairs: PAIRS, tick: null, tickError: "503 redis_unavailable" });
    expect(html).toContain("503 redis_unavailable");
    expect(html).toContain('role="alert"');
  });

  it("pairs null ⇒ census honest dash, stages unaffected", () => {
    const html = render({ pairs: null, tick: TICK });
    expect(html).toContain("CLEAN —");
    expect(html).toContain("DIRTY —");
    expect(html).toContain("universo servido — pares");
    expect(html).toContain("120"); // tick side keeps rendering
  });
});
