// frontend/lib/store/__tests__/mapper-fail-honest.test.ts
//
// FE-MASTER · FE-0029 — mapper fail-honest audit (§28): the wire-mandatory
// identity fields map to null when a (malformed) payload omits them. The OLD
// semantic defaults — missing→"dex_arb", missing→now(), missing→"detected",
// missing→chain 0, missing→"0" wei — fabricated a coherent-looking row out of
// a broken one. Worst case pinned below: a missing detected_at stamped NOW
// made the card immortal (TTL prune saw age 0 on every remap).
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, it, expect } from "vitest";
import { mapToOmniOpportunity } from "@/lib/store/types";
import { StrategyBadge } from "@/components/StrategyBadge";
import { ChainBadge } from "@/components/ChainBadge";
import { StatusPill } from "@/components/StatusPill";

// The wire-mandatory fields, absent on purpose (malformed WS/poll payload).
const MALFORMED = {
  id: "broken-1",
  trace_id: "tr",
  dex_a: "uniswap-v2",
  token_in: "0xa",
  token_out: "0xb",
};

describe("mapToOmniOpportunity — FE-0029 semantic defaults ELIMINATED (§28)", () => {
  it("missing strategy_kind → null, NEVER dex_arb", () => {
    const o = mapToOmniOpportunity({ ...MALFORMED });
    expect(o.strategy_kind).toBeNull();
  });

  it("missing detected_at → null, NEVER now() (the immortal-card bug)", () => {
    const o = mapToOmniOpportunity({ ...MALFORMED });
    expect(o.detected_at).toBeNull();
    // And the store's prune sees NaN (keeps the undated row — its documented
    // semantics) instead of a fresh now() that defeated the TTL.
    expect(o.detected_at == null ? NaN : Date.parse(o.detected_at)).toBeNaN();
  });

  it("missing status → null, NEVER detected", () => {
    const o = mapToOmniOpportunity({ ...MALFORMED });
    expect(o.status).toBeNull();
  });

  it("missing chain_id → null, NEVER chain 0", () => {
    const o = mapToOmniOpportunity({ ...MALFORMED });
    expect(o.chain_id).toBeNull();
  });

  it("missing amount_in_wei → null, NEVER \"0\" (0 = computed-exactly-zero, R8)", () => {
    const o = mapToOmniOpportunity({ ...MALFORMED });
    expect(o.amount_in_wei).toBeNull();
  });

  it("an entirely empty payload maps all five to null", () => {
    const o = mapToOmniOpportunity({});
    expect(o.strategy_kind).toBeNull();
    expect(o.detected_at).toBeNull();
    expect(o.status).toBeNull();
    expect(o.chain_id).toBeNull();
    expect(o.amount_in_wei).toBeNull();
  });

  it("PRESENT values pass through verbatim (no coercion drift)", () => {
    const o = mapToOmniOpportunity({
      ...MALFORMED,
      strategy_kind: "MEV-01-015",
      detected_at: "2026-08-24T00:00:00Z",
      status: "rejected",
      chain_id: 8453,
      amount_in_wei: "1230000000000000000",
    });
    expect(o.strategy_kind).toBe("MEV-01-015");
    expect(o.detected_at).toBe("2026-08-24T00:00:00Z");
    expect(o.status).toBe("rejected");
    expect(o.chain_id).toBe(8453);
    expect(o.amount_in_wei).toBe("1230000000000000000");
  });
});

// ─── Display primitives render absence honestly ──────────────────────────────

describe("FE-0029 — badges render null as UNKNOWN/—, never a claim", () => {
  it("StrategyBadge(null) → UNKNOWN, no family colour", () => {
    const html = renderToStaticMarkup(
      React.createElement(StrategyBadge, { strategy_kind: null }),
    );
    expect(html).toContain("UNKNOWN");
    expect(html).not.toContain("DEX");
  });

  it("ChainBadge(null) → dash, title explains §28", () => {
    const html = renderToStaticMarkup(
      React.createElement(ChainBadge, { chain_id: null }),
    );
    expect(html).toContain("—");
    expect(html).toContain("§28");
  });

  it("StatusPill(null) → UNKNOWN, never DETECTED", () => {
    const html = renderToStaticMarkup(
      React.createElement(StatusPill, { status: null }),
    );
    expect(html).toContain("UNKNOWN");
    expect(html).not.toContain("DETECTED");
  });

  it("strategyLabel(null) → dash", async () => {
    const { strategyLabel } = await import("@/components/StrategyBadge");
    expect(strategyLabel(null)).toBe("—");
  });

  it("R1: null-arm renders are byte-identical (pure, deterministic)", () => {
    const r = () =>
      renderToStaticMarkup(
        React.createElement(StrategyBadge, { strategy_kind: null }),
      );
    expect(r()).toBe(r());
  });
});
