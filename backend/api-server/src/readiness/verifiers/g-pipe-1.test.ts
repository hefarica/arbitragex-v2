import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { verifyGPIPE1, type PipeRedisDeps } from "./g-pipe-1.js";

const NOW = new Date("2026-08-29T16:00:00.000Z");

function fakeRedis(opts: {
  killswitch?: string | null;
  selectorGroup?: { entriesRead?: number | null; lag?: number | null } | "missing";
  simGroup?: { entriesRead?: number | null; lag?: number | null } | "missing";
  detectedLen?: number;
  validatedLen?: number;
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
      return [flat];
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
    expect(item.reason).toContain("selector lag 0");
    expect(item.reason).toContain("sim-ctl lag 12");
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
