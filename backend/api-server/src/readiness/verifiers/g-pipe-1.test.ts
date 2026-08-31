import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { verifyGPIPE1, type PipeRedisDeps } from "./g-pipe-1.js";

const NOW = new Date("2026-08-29T16:00:00.000Z");

function fakeRedis(opts: {
  killswitch?: string | null;
  selectorGroup?: { entriesRead?: number | null; lag?: number | null; lastDeliveredId?: string | null } | "missing";
  simGroup?: { entriesRead?: number | null; lag?: number | null; lastDeliveredId?: string | null } | "missing";
  detectedLen?: number;
  validatedLen?: number;
  /** Entries physically present after last-delivered-id (the deliverable backlog). */
  selectorBacklog?: number;
  simBacklog?: number;
}): PipeRedisDeps {
  return {
    get: async (key: string) => {
      if (key === "arbx:killswitch") return opts.killswitch ?? null;
      return null;
    },
    xlen: async (key: string) =>
      key === "arbx:opps:detected" ? (opts.detectedLen ?? 0) : (opts.validatedLen ?? 0),
    xinfo: async (kind: "GROUPS", key: string) => {
      const group =
        key === "arbx:opps:detected" ? opts.selectorGroup : opts.simGroup;
      if (group === "missing" || group === undefined) return [];
      const flat: unknown[] = ["name", key === "arbx:opps:detected" ? "selector-g0" : "sim-ctl-g0"];
      if (group.entriesRead !== undefined) flat.push("entries-read", group.entriesRead);
      if (group.lag !== undefined) flat.push("lag", group.lag);
      if (group.lastDeliveredId !== undefined) flat.push("last-delivered-id", group.lastDeliveredId);
      return [flat];
    },
    xrange: async (key: string, _start: string, _end: string, count: number) => {
      const backlog =
        key === "arbx:opps:detected" ? (opts.selectorBacklog ?? 0) : (opts.simBacklog ?? 0);
      const n = Math.min(backlog, count);
      // Only the LENGTH matters to the verifier; ids need not be realistic.
      return Array.from({ length: n }, (_, i) => [`${1700000000000 + i}-0`, []]);
    },
  };
}

describe("G-PIPE-1 (paper pipeline stream flow)", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(NOW);
    process.env["ARBX_PIPE_LAG_MAX"] = "500";
  });
  afterEach(() => {
    delete process.env["ARBX_PIPE_LAG_MAX"];
    vi.useRealTimers();
  });

  it("id/group/label are stable for the panel contract", async () => {
    const item = await verifyGPIPE1({
      now: () => NOW,
      redis: fakeRedis({ selectorGroup: { lag: 3 }, simGroup: { lag: 0 } }),
    });
    expect(item.id).toBe("G-PIPE-1");
    expect(item.group).toBe("operations");
    expect(item.status).toBe("green");
  });

  it("green when both consumer groups report low lag and kill-switch is off", async () => {
    const item = await verifyGPIPE1({
      now: () => NOW,
      redis: fakeRedis({
        killswitch: JSON.stringify({ enabled: false }),
        selectorGroup: { lag: 0 },
        simGroup: { lag: 12 },
      }),
    });
    expect(item.status).toBe("green");
    // G-PIPE-1b contract: the reason labels each backlog with its measurement
    // mode — these fakes omit last-delivered-id, so the legacy raw-lag path ran.
    expect(item.reason).toContain("selector 0 (raw-lag)");
    expect(item.reason).toContain("sim-ctl 12 (raw-lag)");
  });

  it("red when kill-switch key explicitly enabled (halt by design)", async () => {
    const item = await verifyGPIPE1({
      now: () => NOW,
      redis: fakeRedis({
        killswitch: JSON.stringify({ enabled: true, reason: "test" }),
        selectorGroup: { lag: 0 },
        simGroup: { lag: 0 },
      }),
    });
    expect(item.status).toBe("red");
    expect(item.reason).toContain("kill-switch ENABLED");
  });

  it("red when selector group lag ≥ threshold (A5-STALL signature)", async () => {
    const item = await verifyGPIPE1({
      now: () => NOW,
      redis: fakeRedis({
        selectorGroup: { lag: 1781 },
        simGroup: { lag: 0 },
      }),
    });
    expect(item.status).toBe("red");
    expect(item.reason).toContain("selector consumer stalled: 1781");
    expect(item.reason).toContain("A5-STALL");
  });

  it("red when sim-ctl group lag ≥ threshold", async () => {
    const item = await verifyGPIPE1({
      now: () => NOW,
      redis: fakeRedis({
        selectorGroup: { lag: 10 },
        simGroup: { lag: 900 },
      }),
    });
    expect(item.status).toBe("red");
    expect(item.reason).toContain("sim-ctl consumer stalled: 900");
  });

  it("lag falls back to entries-added − entries-read when server omits lag", async () => {
    const item = await verifyGPIPE1({
      now: () => NOW,
      redis: fakeRedis({
        selectorGroup: { entriesRead: 100 },
        simGroup: { entriesRead: 50 },
        detectedLen: 1600,
        validatedLen: 60,
      }),
    });
    // 1600 − 100 = 1500 ≥ 500 → selector stalled
    expect(item.status).toBe("red");
    expect(item.reason).toContain("1500");
  });

  it("yellow when Redis omits both lag and entries-read (older server)", async () => {
    const item = await verifyGPIPE1({
      now: () => NOW,
      redis: fakeRedis({
        selectorGroup: {},
        simGroup: {},
      }),
    });
    expect(item.status).toBe("yellow");
    expect(item.reason).toContain("lag/entries-read");
  });

  it("yellow when a consumer group is missing entirely", async () => {
    const item = await verifyGPIPE1({
      now: () => NOW,
      redis: fakeRedis({
        selectorGroup: "missing",
        simGroup: { lag: 0 },
      }),
    });
    expect(item.status).toBe("yellow");
    expect(item.reason).toContain("selector-g0");
  });

  it("yellow naming the env var when ARBX_PIPE_LAG_MAX is malformed", async () => {
    process.env["ARBX_PIPE_LAG_MAX"] = "abc";
    const item = await verifyGPIPE1({ now: () => NOW, redis: fakeRedis({}) });
    expect(item.status).toBe("yellow");
    expect(item.reason).toContain('ARBX_PIPE_LAG_MAX="abc"');
  });

  it("custom lagMax threshold is honored (injectable, no env needed)", async () => {
    const item = await verifyGPIPE1({
      now: () => NOW,
      redis: fakeRedis({ selectorGroup: { lag: 60 }, simGroup: { lag: 0 } }),
      lagMax: 50,
    });
    expect(item.status).toBe("red");
    expect(item.reason).toContain("60");
  });

  it("kill-switch key absent does NOT red by itself (fail-closed default engages at consumers; lag catches it)", async () => {
    const item = await verifyGPIPE1({
      now: () => NOW,
      redis: fakeRedis({
        killswitch: null,
        selectorGroup: { lag: 0 },
        simGroup: { lag: 0 },
      }),
    });
    expect(item.status).toBe("green");
    expect(item.reason).toContain("key absent");
  });

  it("unparseable kill-switch payload falls through to lag evaluation, not a crash", async () => {
    const item = await verifyGPIPE1({
      now: () => NOW,
      redis: fakeRedis({
        killswitch: "not-json{{{",
        selectorGroup: { lag: 2 },
        simGroup: { lag: 2 },
      }),
    });
    expect(item.status).toBe("green");
    expect(item.reason).toContain("unparseable");
  });
});

// ── G-PIPE-1b — ghost-lag regression (workbook 2026-08-30, fix 2026-08-31) ──
//
// The 2026-08-29 killswitch-wipe + stream recreation left the server-side lag
// counter permanently inflated: entries-added − entries-read includes wiped
// entries the consumer can NEVER read. The gate must use the DELIVERABLE
// backlog (entries present after last-delivered-id), not the raw counter.

describe("G-PIPE-1b deliverable backlog (ghost-lag fix)", () => {
  it("green when server lag is ghost-inflated but deliverable backlog is 0", async () => {
    const item = await verifyGPIPE1({
      now: () => NOW,
      redis: fakeRedis({
        selectorGroup: { lag: 1781, lastDeliveredId: "1700-0" }, // A5-STALL-sized ghost
        simGroup: { lag: 0, lastDeliveredId: "1700-0" },
        selectorBacklog: 0,
        simBacklog: 0,
      }),
    });
    expect(item.status).toBe("green");
    expect(item.reason).toContain("selector 0 (deliverable)");
    expect(item.reason).toContain("phantom");
  });

  it("red when the deliverable backlog is real, regardless of server lag", async () => {
    const item = await verifyGPIPE1({
      now: () => NOW,
      redis: fakeRedis({
        selectorGroup: { lag: 50, lastDeliveredId: "1700-0" }, // server counter LOW
        simGroup: { lag: 0, lastDeliveredId: "1700-0" },
        selectorBacklog: 500, // but 500 real entries waiting — A5-STALL shape
      }),
    });
    expect(item.status).toBe("red");
    expect(item.reason).toContain("selector consumer stalled");
  });

  it("reports the exact deliverable count below threshold", async () => {
    const item = await verifyGPIPE1({
      now: () => NOW,
      redis: fakeRedis({
        selectorGroup: { lag: 3, lastDeliveredId: "1700-0" },
        simGroup: { lag: 1, lastDeliveredId: "1700-0" },
        selectorBacklog: 7,
        simBacklog: 0,
      }),
    });
    expect(item.status).toBe("green");
    expect(item.reason).toContain("selector 7 (deliverable)");
  });

  it("XRANGE failure falls back to raw server lag without crashing", async () => {
    const base = fakeRedis({
      selectorGroup: { lag: 600, lastDeliveredId: "1700-0" },
      simGroup: { lag: 0, lastDeliveredId: "1700-0" },
    });
    const broken: typeof base = {
      ...base,
      xrange: async () => {
        throw new Error("XRANGE denied");
      },
    };
    const item = await verifyGPIPE1({ now: () => NOW, redis: broken });
    expect(item.status).toBe("red"); // raw-lag fallback preserves the stall signal
    expect(item.reason).toContain("raw");
  });

  it("missing last-delivered-id keeps the legacy raw-lag gate (older Redis)", async () => {
    const item = await verifyGPIPE1({
      now: () => NOW,
      redis: fakeRedis({
        selectorGroup: { lag: 700 }, // no lastDeliveredId → fallback path
        simGroup: { lag: 0 },
      }),
    });
    expect(item.status).toBe("red");
    expect(item.reason).toContain("700");
  });
});
