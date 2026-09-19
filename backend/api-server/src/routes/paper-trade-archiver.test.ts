/**
 * paper-trade-archiver tests — pure-function assertions (no Redis, no PG).
 *
 * LATLED-01: `detectionToLedgerMs` computes the detection→ledger latency that
 * populates `paper_trade_runs.execution_time_ms`. It must mirror relays-client
 * `detection_to_ledger_ms` (persistence.rs) exactly: elapsed ms, clock-skew
 * clamped to 0, i32 saturation for the INTEGER column.
 *
 * ARBX-R-0001: `archiverRejectionSkip` is the paper-side twin of relays-client
 * `SubmitEngine::rejection_refusal` — the d9 6h JOIN showed 434/434
 * paper_trade_runs rows were REJECTED opps (this archiver inserted them
 * because it only gated on sim-profit presence). The predicate must treat
 * ANY non-null rejection_reason as never-a-trade.
 */
import { describe, it, expect, vi, afterEach } from "vitest";
import {
  detectionToLedgerMs,
  archiverRejectionSkip,
  SKIP_REJECTED_SUMMARY_WINDOW_MS,
  PaperTradeArchiver,
  type PaperTradeArchiverDeps,
} from "./paper-trade-archiver.js";

const NOW = Date.parse("2026-08-23T12:00:00.000Z");

describe("detectionToLedgerMs (LATLED-01)", () => {
  it("measures elapsed ms from detected_at to now", () => {
    expect(detectionToLedgerMs("2026-08-23T11:59:58.500Z", NOW)).toBe(1500);
  });

  it("accepts offset-qualified ISO timestamps (zod schema allows offsets)", () => {
    // 12:00Z == 14:00+02:00 → same instant, 0 ms elapsed.
    expect(detectionToLedgerMs("2026-08-23T14:00:00.000+02:00", NOW)).toBe(0);
  });

  it("clock skew (detected_at in the future) clamps to 0 — never negative (R8)", () => {
    expect(detectionToLedgerMs("2026-08-23T12:00:05.000Z", NOW)).toBe(0);
  });

  it("saturates at i32 MAX for the INTEGER column (parity with Rust clamp)", () => {
    const ancient = "2016-08-23T12:00:00.000Z"; // 10 years back ≈ 3.15e11 ms
    expect(detectionToLedgerMs(ancient, NOW)).toBe(2_147_483_647);
  });

  it("defends NaN from unparseable timestamps to 0 (impossible post-zod, defended anyway)", () => {
    expect(detectionToLedgerMs("not-a-date", NOW)).toBe(0);
  });
});

describe("archiverRejectionSkip (ARBX-R-0001)", () => {
  it("a rejected opportunity is never a paper trade — reason returned for the skip log", () => {
    // The exact rejection from the flood: SizeOptimizer verdict copied verbatim,
    // never relabeled.
    expect(archiverRejectionSkip({ rejection_reason: "NegativeNetProfit:gas_floor_breach" })).toBe(
      "NegativeNetProfit:gas_floor_breach",
    );
  });

  it("a viable opportunity (rejection_reason null) proceeds to the insert path", () => {
    expect(archiverRejectionSkip({ rejection_reason: null })).toBe(null);
  });
});

// WO-NO-WS-LOGS-01 (2026-09-17): R9 aggregation — per-item skip_rejected moves
// to debug; ONE info summary per window carries the reason histogram. The
// production flood was ~30 info-lines/s (QA-WS §5.4), drowning ws.* lifecycle
// events. These tests pin the aggregation semantics WITHOUT Redis/PG (the
// recorder path never touches them).
type LogLine = { level: "info" | "warn" | "error" | "debug"; obj: Record<string, unknown>; msg?: string };

function fakeDeps(lines: LogLine[], opts: { withDebug?: boolean } = {}): PaperTradeArchiverDeps {
  const logger = {
    info: (obj: object, msg?: string) => lines.push({ level: "info", obj: obj as Record<string, unknown>, msg }),
    warn: (obj: object, msg?: string) => lines.push({ level: "warn", obj: obj as Record<string, unknown>, msg }),
    error: (obj: object, msg?: string) => lines.push({ level: "error", obj: obj as Record<string, unknown>, msg }),
    ...(opts.withDebug === false
      ? {}
      : {
          debug: (obj: object, msg?: string) => lines.push({ level: "debug", obj: obj as Record<string, unknown>, msg }),
        }),
  };
  return { redisUrl: "redis://unused", pool: {} as never, logger: logger as never };
}

/** Private-method access for the R9 recorder (no Redis/PG needed). */
function recorder(deps: PaperTradeArchiverDeps) {
  const a = new PaperTradeArchiver(deps);
  return {
    record: (reason: string, id: string) => (a as unknown as { recordSkipRejected(r: string, o: string): void }).recordSkipRejected(reason, id),
    flush: () => (a as unknown as { flushSkipRejectedSummary(now: number): void }).flushSkipRejectedSummary(Date.now()),
    stop: () => a.stop(),
  };
}

describe("WO-NO-WS-LOGS-01 — skip_rejected R9 aggregation", () => {
  afterEach(() => vi.useRealTimers());

  it("per-item skip logs at DEBUG (not info) with the reason VERBATIM", () => {
    const lines: LogLine[] = [];
    const r = recorder(fakeDeps(lines));
    r.record("NonPositiveProfit:spot_product_le_one", "opp-1");
    const item = lines.filter((l) => l.obj["event"] === "paper_archiver.skip_rejected");
    expect(item).toHaveLength(1);
    expect(item[0]!.level).toBe("debug");
    expect(item[0]!.obj["reason"]).toBe("NonPositiveProfit:spot_product_le_one"); // never relabeled (R8)
    expect(lines.filter((l) => l.level === "info")).toHaveLength(0); // no info flood
  });

  it("first skip anchors the window — no near-empty instant summary", () => {
    const lines: LogLine[] = [];
    const r = recorder(fakeDeps(lines));
    vi.useFakeTimers({ now: 1_000_000 });
    r.record("r1", "opp-1");
    expect(lines.filter((l) => l.obj["event"] === "paper_archiver.skip_rejected_summary")).toHaveLength(0);
  });

  it("ONE info summary per window with the reason histogram (counts by reason)", () => {
    const lines: LogLine[] = [];
    const r = recorder(fakeDeps(lines));
    vi.useFakeTimers({ now: 1_000_000 });
    // Window 1: 3 skips, 2 distinct reasons.
    r.record("v3_quote_unavailable", "a");
    r.record("v3_quote_unavailable", "b");
    r.record("spot_product_le_one", "c");
    // Window 2 opens past the window edge → flush of window 1 + anchor reset.
    vi.setSystemTime(1_000_000 + SKIP_REJECTED_SUMMARY_WINDOW_MS + 5);
    r.record("v3_quote_unavailable", "d");
    const summaries = lines.filter((l) => l.obj["event"] === "paper_archiver.skip_rejected_summary");
    expect(summaries).toHaveLength(1);
    expect(summaries[0]!.level).toBe("info");
    expect(summaries[0]!.obj["total"]).toBe(3);
    expect(summaries[0]!.obj["reasons"]).toEqual({ v3_quote_unavailable: 2, spot_product_le_one: 1 });
    expect(summaries[0]!.obj["window_ms"]).toBe(SKIP_REJECTED_SUMMARY_WINDOW_MS);
  });

  it("stop() flushes the partial final window so shutdown never loses counts", async () => {
    const lines: LogLine[] = [];
    const r = recorder(fakeDeps(lines));
    vi.useFakeTimers({ now: 1_000_000 });
    r.record("non_positive_profit", "a");
    expect(lines.filter((l) => l.obj["event"] === "paper_archiver.skip_rejected_summary")).toHaveLength(0);
    await r.stop();
    const summaries = lines.filter((l) => l.obj["event"] === "paper_archiver.skip_rejected_summary");
    expect(summaries).toHaveLength(1);
    expect(summaries[0]!.obj["total"]).toBe(1);
  });

  it("a logger shim without debug() omits the per-item line without crashing (interface optional)", () => {
    const lines: LogLine[] = [];
    const r = recorder(fakeDeps(lines, { withDebug: false }));
    expect(() => r.record("r", "opp")).not.toThrow();
    // The count still lands in the histogram via stop()'s flush.
    return r.stop().then(() => {
      const summaries = lines.filter((l) => l.obj["event"] === "paper_archiver.skip_rejected_summary");
      expect(summaries).toHaveLength(1);
      expect(summaries[0]!.obj["total"]).toBe(1);
    });
  });
});
