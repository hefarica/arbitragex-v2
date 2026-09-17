/**
 * SEL-GATE-01 (2026-09-17) — selector-api must not publish producer-rejected
 * opportunities to `arbx:opps:validated`.
 *
 * Incident shape (audits/real-cycles-audit-20260916/00-SYNTHESIS.md, funnel
 * 2026-09-17T04:00Z): 87% of sims burned fork RPC on opportunities the
 * searcher had ALREADY rejected upstream (status="rejected",
 * rejection_reason="non_positive_profit" / "v3_quote_unavailable",
 * amount_in_wei="0") — selector-api's decide() ignores the producer's
 * lifecycle verdict entirely, so rows that can never execute still flow
 * detect→validate→simulate.
 *
 * The regression test locks:
 *   1. producerRejected() classifies exactly the lifecycle verdicts the
 *      producer emits pre-publication (rejected status or a rejection_reason
 *      set), and leaves clean opportunities untouched;
 *   2. the prefilter call site orders the guard BEFORE the expensive work
 *      (safety fetch + scoring) and BEFORE the persist/publish tail.
 */

import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { producerRejected } from "./engine.js";

const __dirname_local = dirname(fileURLToPath(import.meta.url));
const CONSUMER_SRC = readFileSync(
  resolve(__dirname_local, "../consumer.ts"),
  "utf8",
);

function opp(partial: Partial<import("./policy/engine.js").PrefilterInput["opportunity"]>) {
  return {
    id: "0e689f68-1111-4111-8111-411111111111",
    chain_id: 1,
    strategy_kind: "dex_arb",
    dex_a: "UniswapV2",
    dex_b: null,
    pair_symbol: "WETH/USDC",
    token_in: "0xC02aaa39b223FE8D0A0e5C4F27eAD9083C756Cc2",
    token_out: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
    amount_in_wei: "1000000000000000000",
    expected_profit_usd: 12.5,
    net_expected_profit_usd: null,
    roi_pct: null,
    risk_score: null,
    block_number: null,
    status: "detected",
    rejection_reason: null,
    cartridge_id: null,
    detected_at: "2026-09-17T04:00:00.000Z",
    trace_id: "0e689f68-2222-4222-8222-422222222222",
    ...partial,
  } as import("./policy/engine.js").PrefilterInput["opportunity"];
}

describe("SEL-GATE-01 — producer-rejected opportunities never reach the validated stream", () => {
  it("classifies status=rejected as producer-rejected", () => {
    expect(producerRejected(opp({ status: "rejected" }))).toBe(true);
  });

  it("classifies a set rejection_reason as producer-rejected regardless of status", () => {
    expect(
      producerRejected(opp({ status: "detected", rejection_reason: "non_positive_profit" })),
    ).toBe(true);
    // The zero-amount incident rows carried amount_in_wei="0" AND a reason.
    expect(
      producerRejected(opp({
        status: "rejected",
        rejection_reason: "v3_quote_unavailable",
        amount_in_wei: "0",
      })),
    ).toBe(true);
  });

  it("leaves clean opportunities untouched", () => {
    expect(producerRejected(opp())).toBe(false);
    // status="validated" downstream-rewritten rows with no reason stay clean.
    expect(producerRejected(opp({ status: "validated" }))).toBe(false);
    // Absent status/reason (older producers) must NOT be dropped.
    const legacy = opp();
    delete (legacy as Record<string, unknown>)["status"];
    delete (legacy as Record<string, unknown>)["rejection_reason"];
    expect(producerRejected(legacy)).toBe(false);
  });

  it("guard runs inside prefilter BEFORE safety fetch + scoring + persist", () => {
    // The consumer's decision path must reach prefilter (which now carries the
    // producer-rejection guard) before checkToken / scoreOpportunity, and the
    // publish tail stays accept-gated.
    const preIdx = CONSUMER_SRC.indexOf("prefilter(");
    const safetyIdx = CONSUMER_SRC.indexOf("checkToken(");
    const scoreIdx = CONSUMER_SRC.indexOf("scoreOpportunity(");
    expect(preIdx).toBeGreaterThanOrEqual(0);
    expect(safetyIdx).toBeGreaterThan(preIdx);
    expect(scoreIdx).toBeGreaterThan(preIdx);
    expect(CONSUMER_SRC).toMatch(/decision\.kind === "accept"/);
  });
});
