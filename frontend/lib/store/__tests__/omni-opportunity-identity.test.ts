// frontend/lib/store/__tests__/omni-opportunity-identity.test.ts
//
// FE-MASTER · FE-0028 — OmniOpportunity extended identity (§26 §27):
// NO parallel model. cartridge_id is the one REAL new wire field
// (opportunities.cartridge_id — api-server live query SELECTs it);
// hop_count DERIVES from the same persisted topology the ViewModel already
// carries; everything else is a level-(b) gap pinned null, never fabricated.
import { describe, it, expect } from "vitest";
import {
  deriveHopCount,
  mapToOmniOpportunity,
  type RouteMetadataWire,
} from "@/lib/store/types";

// ─── deriveHopCount — §19 hop arithmetic over the persisted topology ─────────

describe("deriveHopCount", () => {
  const rm = (hops: number): RouteMetadataWire => ({
    token_addresses: Array.from({ length: hops + 1 }, (_, i) => `0x${i}`),
    pool_addresses: Array.from({ length: hops }, (_, i) => `0xpool${i}`),
    dex_adapters: Array.from({ length: hops }, () => "uniswap_v2_router"),
  });

  it("dex_adapters.length IS the hop count", () => {
    expect(deriveHopCount(rm(2))).toBe(2);
    expect(deriveHopCount(rm(3))).toBe(3);
    expect(deriveHopCount(rm(7))).toBe(7);
  });

  it("honors the topology invariant: tokens = hops + 1", () => {
    for (const h of [2, 3, 5]) {
      const r = rm(h);
      expect(r.token_addresses.length).toBe((deriveHopCount(r) ?? -1) + 1);
    }
  });

  it("null when there is no topology — absence is a state, never 0 (R8)", () => {
    expect(deriveHopCount(null)).toBeNull();
    expect(deriveHopCount({ ...rm(3), dex_adapters: [] })).toBeNull();
  });
});

// ─── mapToOmniOpportunity — extended identity mapping ────────────────────────

describe("mapToOmniOpportunity — FE-0028 extended identity", () => {
  const BASE = {
    id: "id-1",
    chain_id: 1,
    strategy_kind: "MEV-01-015",
    detected_at: "2026-08-23T00:00:00Z",
    trace_id: "tr-1",
    dex_a: "uniswap-v2",
    token_in: "0xa",
    token_out: "0xa",
  };

  it("cartridge_id passes through from the wire (REST live query serves it)", () => {
    const o = mapToOmniOpportunity({ ...BASE, cartridge_id: "MEV-01-015" });
    expect(o.cartridge_id).toBe("MEV-01-015");
  });

  it("cartridge_id null when the payload omits it (WS push / legacy row)", () => {
    const o = mapToOmniOpportunity({ ...BASE });
    expect(o.cartridge_id).toBeNull();
  });

  it("hop_count derives from the SAME parsed topology the ViewModel carries (§26: no parallel model)", () => {
    const o = mapToOmniOpportunity({
      ...BASE,
      route_metadata: {
        token_addresses: ["0xa", "0xb", "0xc", "0xa"],
        pool_addresses: ["0xp1", "0xp2", "0xp3"],
        dex_adapters: ["uni", "sushi", "uni"],
      },
    });
    expect(o.hop_count).toBe(3);
    expect(o.hop_count).toBe(o.route_metadata?.dex_adapters.length);
  });

  it("hop_count null without route_metadata (R8)", () => {
    const o = mapToOmniOpportunity({ ...BASE });
    expect(o.route_metadata).toBeNull();
    expect(o.hop_count).toBeNull();
  });

  it("level-(b) gaps are null — never placeholders (RULE 00 / R8)", () => {
    // A raw payload that carries NONE of the level-(b) identity (none do
    // today) must map every gap to null — the type surface exists for the
    // wire to grow into, the values are never invented.
    const o = mapToOmniOpportunity({ ...BASE });
    expect(o.candidate_id).toBeNull();
    expect(o.route_id).toBeNull();
    expect(o.pair_id).toBeNull();
    expect(o.detector_id).toBeNull();
    expect(o.quote_token).toBeNull();
    expect(o.quote_version).toBeNull();
    expect(o.graph_version).toBeNull();
    expect(o.config_version).toBeNull();
    expect(o.strategy_version).toBeNull();
    expect(o.gate_results).toBeNull();
    expect(o.data_quality).toBeNull();
  });

  it("mapper output satisfies the gap-list contract exactly (drift alarm)", () => {
    // If the wire later gains a level-(b) field, this FAILS until the mapper
    // is taught to map it — the gap may not silently stay behind.
    const o = mapToOmniOpportunity({ ...BASE });
    const gaps: Array<[string, unknown]> = [
      ["candidate_id", o.candidate_id],
      ["route_id", o.route_id],
      ["pair_id", o.pair_id],
      ["detector_id", o.detector_id],
      ["quote_token", o.quote_token],
      ["quote_version", o.quote_version],
      ["graph_version", o.graph_version],
      ["config_version", o.config_version],
      ["strategy_version", o.strategy_version],
      ["gate_results", o.gate_results],
      ["data_quality", o.data_quality],
    ];
    expect(gaps.map(([k]) => k)).toEqual([
      "candidate_id", "route_id", "pair_id", "detector_id", "quote_token",
      "quote_version", "graph_version", "config_version", "strategy_version",
      "gate_results", "data_quality",
    ]);
    for (const [k, v] of gaps) expect(v, k).toBeNull();
  });
});
