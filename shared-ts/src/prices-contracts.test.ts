// shared-ts/src/prices-contracts.test.ts (t5) — contract-level tests for the
// G-PRICE-1 v4 surface: PriceTickSchema acceptance/rejection and the
// STREAM-SEQ-01 monotonic seq guard. These give the t5 μ(t_seq) measurement a
// reproducible command + literal output (ADM-t5-frontend-bus.md).
import { describe, it, expect } from "vitest";
import { PriceTickSchema, isPriceTick, createSeqGuard } from "./prices-contracts.js";

const GOOD = { pair: "WETH/USDC", mid: 3000.5, ts: 1234567890, src: "binance", seq: 7 };

describe("PriceTickSchema (contract)", () => {
  it("accepts a well-formed tick", () => {
    expect(PriceTickSchema.safeParse(GOOD).success).toBe(true);
    expect(isPriceTick(GOOD)).toBe(true);
  });

  it("rejects each malformed shape (empty pair, non-finite mid, negative ts, missing src, fractional seq)", () => {
    expect(isPriceTick({ ...GOOD, pair: "" })).toBe(false);
    expect(isPriceTick({ ...GOOD, mid: Number.NaN })).toBe(false);
    expect(isPriceTick({ ...GOOD, mid: Number.POSITIVE_INFINITY })).toBe(false);
    expect(isPriceTick({ ...GOOD, ts: -1 })).toBe(false);
    expect(isPriceTick({ ...GOOD, src: "" })).toBe(false);
    expect(isPriceTick({ ...GOOD, seq: 1.5 })).toBe(false);
    expect(isPriceTick({ ...GOOD, seq: -1 })).toBe(false);
    expect(isPriceTick({ pair: GOOD.pair })).toBe(false); // missing fields
    expect(isPriceTick(null)).toBe(false);
    expect(isPriceTick("tick")).toBe(false);
  });
});

describe("createSeqGuard (STREAM-SEQ-01: seq monotónico por par)", () => {
  it("accepts strictly increasing seq per pair", () => {
    const accept = createSeqGuard();
    expect(accept({ ...GOOD, pair: "A/B", seq: 1 })).toBe(true);
    expect(accept({ ...GOOD, pair: "A/B", seq: 2 })).toBe(true);
    expect(accept({ ...GOOD, pair: "A/B", seq: 10 })).toBe(true);
  });

  it("replay (same seq) ⇒ rejected, 0 state effect", () => {
    const accept = createSeqGuard();
    accept({ ...GOOD, pair: "A/B", seq: 5 });
    expect(accept({ ...GOOD, pair: "A/B", seq: 5 })).toBe(false);
  });

  it("out-of-order (lower seq) ⇒ rejected", () => {
    const accept = createSeqGuard();
    accept({ ...GOOD, pair: "A/B", seq: 5 });
    expect(accept({ ...GOOD, pair: "A/B", seq: 4 })).toBe(false);
  });

  it("per-pair independence: interleaved pairs track their own watermark", () => {
    const accept = createSeqGuard();
    expect(accept({ ...GOOD, pair: "A/B", seq: 3 })).toBe(true);
    expect(accept({ ...GOOD, pair: "C/D", seq: 1 })).toBe(true); // lower seq, other pair
    expect(accept({ ...GOOD, pair: "A/B", seq: 4 })).toBe(true);
    expect(accept({ ...GOOD, pair: "C/D", seq: 2 })).toBe(true);
  });

  it("μ(t_seq) bench: 200 live accepted; 1000 replays and 1000 out-of-order ALL rejected (0 state effect)", () => {
    const accept = createSeqGuard();
    let accepted = 0;
    for (let seq = 1; seq <= 200; seq++) {
      if (accept({ ...GOOD, pair: "BENCH/B", seq })) accepted++;
    }
    let replayRejected = 0;
    for (let i = 0; i < 1000; i++) {
      if (!accept({ ...GOOD, pair: "BENCH/B", seq: 1 + (i % 200) })) replayRejected++;
    }
    let outOfOrderRejected = 0;
    for (let i = 0; i < 1000; i++) {
      if (!accept({ ...GOOD, pair: "BENCH/B", seq: 1 + (i % 180) })) outOfOrderRejected++;
    }
    expect(accepted).toBe(200);
    expect(replayRejected).toBe(1000);
    expect(outOfOrderRejected).toBe(1000);
  });
});
