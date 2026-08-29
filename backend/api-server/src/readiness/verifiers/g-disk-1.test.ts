import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { verifyGDISK1 } from "./g-disk-1.js";

const NOW = () => new Date("2026-08-29T12:00:00.000Z");

type StatFsSample = { bsize: number; blocks: number; bfree: number; bavail: number };

// Synthetic statfs (tests NEVER hit the real fs for threshold math — CI runs
// on platforms where statfs may throw). Default shape: 1000 blocks × 4096 B,
// 500 free → 50.0% used (a green posture under the 85/95 defaults).
function statfsReturning(over: Partial<StatFsSample>, calls?: string[]) {
  return async (path: string): Promise<StatFsSample> => {
    if (calls) calls.push(path);
    return { bsize: 4096, blocks: 1000, bfree: 500, bavail: 500, ...over };
  };
}

describe("verifyGDISK1()", () => {
  const ENV_KEYS = ["ARBX_DISK_PATH", "ARBX_DISK_WARN_PCT", "ARBX_DISK_CRIT_PCT"] as const;
  let saved: Record<string, string | undefined>;

  beforeEach(() => {
    saved = {};
    for (const k of ENV_KEYS) {
      saved[k] = process.env[k];
      delete process.env[k]; // deterministic 85/95 + "/" defaults per test
    }
  });
  afterEach(() => {
    for (const k of ENV_KEYS) {
      if (saved[k] === undefined) delete process.env[k];
      else process.env[k] = saved[k];
    }
  });

  // ----- status mapping (comparison is >= on both thresholds) -----

  it("green under warn: blocks=1000 bfree=500 bsize=4096 → 50.0% used, 0.0 GB free; base shape + evidence", async () => {
    const item = await verifyGDISK1({ now: NOW, statfs: statfsReturning({}) });
    expect(item.id).toBe("G-DISK-1");
    expect(item.group).toBe("operations");
    expect(item.label).toBe("Host disk usage below critical threshold");
    expect(item.doctrine).toBe("safe-production-observability");
    expect(item.verified_at).toBe("2026-08-29T12:00:00.000Z");
    expect(item.status).toBe("green");
    expect(item.reason).toContain("50.0%");
    expect(item.reason).toContain("0.0 GB free");
    expect(item.evidence).toEqual({ kind: "shell", ref: "statfs(/)" });
  });

  it("yellow at 86% (>= warn 85): reason carries pct + free GB", async () => {
    const item = await verifyGDISK1({
      now: NOW,
      statfs: statfsReturning({ blocks: 10_000_000, bfree: 1_400_000, bsize: 16_384, bavail: 1_400_000 }),
    });
    expect(item.status).toBe("yellow");
    expect(item.reason).toContain("86.0%");
    expect(item.reason).toContain("21.4 GB free");
  });

  it("red at 96% (>= crit 95): reason carries pct + free GB + 2026-08-29 incident hook", async () => {
    const item = await verifyGDISK1({
      now: NOW,
      statfs: statfsReturning({ blocks: 10_000_000, bfree: 400_000, bsize: 16_384, bavail: 400_000 }),
    });
    expect(item.status).toBe("red");
    expect(item.reason).toContain("96.0%");
    expect(item.reason).toContain("6.1 GB free");
    expect(item.reason).toContain("2026-08-29: disk 100% crash-looped postgres for hours");
  });

  it("boundary: 85.0% exactly == warn → YELLOW (>= warn semantics: at-threshold counts as warning)", async () => {
    const item = await verifyGDISK1({ now: NOW, statfs: statfsReturning({ bfree: 150, bavail: 150 }) });
    expect(item.status).toBe("yellow");
    expect(item.reason).toContain("85.0%");
  });

  it("boundary: 95.0% exactly == crit → RED (>= crit semantics: at-critical counts as critical)", async () => {
    const item = await verifyGDISK1({ now: NOW, statfs: statfsReturning({ bfree: 50, bavail: 50 }) });
    expect(item.status).toBe("red");
    expect(item.reason).toContain("95.0%");
  });

  it("boundary: 84.9% just under warn → green", async () => {
    const item = await verifyGDISK1({
      now: NOW,
      statfs: statfsReturning({ blocks: 10_000, bfree: 1_510, bavail: 1_510 }),
    });
    expect(item.status).toBe("green");
    expect(item.reason).toContain("84.9%");
  });

  // ----- measurement failures (R8 fail-honest) -----

  it("statfs throws → yellow with the exact error surfaced, no fabricated numbers", async () => {
    const item = await verifyGDISK1({
      now: NOW,
      statfs: async () => {
        throw new Error("EINVAL: statfs not supported on this platform");
      },
    });
    expect(item.status).toBe("yellow");
    expect(item.reason).toBe("disk usage could not be measured: EINVAL: statfs not supported on this platform");
    // never a fabricated percentage
    expect(item.reason).not.toMatch(/\d+(\.\d+)?%/);
  });

  it("blocks=0 → yellow degenerate statfs result", async () => {
    const item = await verifyGDISK1({
      now: NOW,
      statfs: statfsReturning({ blocks: 0, bfree: 0 }),
    });
    expect(item.status).toBe("yellow");
    expect(item.reason).toMatch(/degenerate statfs result/);
  });

  // ----- env threshold validation (fail-honest, names the env var) -----

  it("warn >= crit → yellow naming both env vars; statfs never called (validate before I/O)", async () => {
    const calls: string[] = [];
    process.env["ARBX_DISK_WARN_PCT"] = "90";
    process.env["ARBX_DISK_CRIT_PCT"] = "80";
    const item = await verifyGDISK1({ now: NOW, statfs: statfsReturning({}, calls) });
    expect(item.status).toBe("yellow");
    expect(item.reason).toContain("ARBX_DISK_WARN_PCT");
    expect(item.reason).toContain("ARBX_DISK_CRIT_PCT");
    expect(calls).toEqual([]);
  });

  it("ARBX_DISK_WARN_PCT=abc (NaN) → yellow naming ARBX_DISK_WARN_PCT", async () => {
    process.env["ARBX_DISK_WARN_PCT"] = "abc";
    const item = await verifyGDISK1({ now: NOW, statfs: statfsReturning({}) });
    expect(item.status).toBe("yellow");
    expect(item.reason).toContain("ARBX_DISK_WARN_PCT");
    expect(item.reason).toContain("abc");
  });

  it("ARBX_DISK_WARN_PCT=0 (<= 0) → yellow naming ARBX_DISK_WARN_PCT", async () => {
    process.env["ARBX_DISK_WARN_PCT"] = "0";
    const item = await verifyGDISK1({ now: NOW, statfs: statfsReturning({}) });
    expect(item.status).toBe("yellow");
    expect(item.reason).toContain("ARBX_DISK_WARN_PCT");
  });

  it("ARBX_DISK_CRIT_PCT=150 (>= 100) → yellow naming ARBX_DISK_CRIT_PCT", async () => {
    process.env["ARBX_DISK_CRIT_PCT"] = "150";
    const item = await verifyGDISK1({ now: NOW, statfs: statfsReturning({}) });
    expect(item.status).toBe("yellow");
    expect(item.reason).toContain("ARBX_DISK_CRIT_PCT");
  });

  it("ARBX_DISK_WARN_PCT=\"\" (empty template line) → yellow naming the env var, never silently defaulted to 85", async () => {
    process.env["ARBX_DISK_WARN_PCT"] = "";
    const item = await verifyGDISK1({ now: NOW, statfs: statfsReturning({}) });
    expect(item.status).toBe("yellow");
    expect(item.reason).toContain("ARBX_DISK_WARN_PCT");
  });

  // ----- path resolution -----

  it("default path is '/' when ARBX_DISK_PATH is unset", async () => {
    const calls: string[] = [];
    const item = await verifyGDISK1({ now: NOW, statfs: statfsReturning({}, calls) });
    expect(calls).toEqual(["/"]);
    expect(item.evidence?.ref).toBe("statfs(/)");
  });

  it("ARBX_DISK_PATH env fallback is honored", async () => {
    process.env["ARBX_DISK_PATH"] = "/mnt/host-data";
    const calls: string[] = [];
    const item = await verifyGDISK1({ now: NOW, statfs: statfsReturning({}, calls) });
    expect(calls).toEqual(["/mnt/host-data"]);
    expect(item.evidence?.ref).toBe("statfs(/mnt/host-data)");
  });

  it("opts.path takes precedence over ARBX_DISK_PATH", async () => {
    process.env["ARBX_DISK_PATH"] = "/mnt/host-data";
    const calls: string[] = [];
    await verifyGDISK1({ now: NOW, path: "/var/lib/postgresql", statfs: statfsReturning({}, calls) });
    expect(calls).toEqual(["/var/lib/postgresql"]);
  });

  // ----- default wiring smoke (the one case allowed to touch the real fs) -----

  it("default statfs wiring: real measurement or honest yellow — always a well-formed item, no NaN/undefined leakage", async () => {
    const item = await verifyGDISK1({ now: NOW });
    expect(item.id).toBe("G-DISK-1");
    // statfs works on Linux/macOS and on recent Windows Node; where it
    // throws the verifier must fail-honest to yellow. Either way: a real
    // status from the taxonomy and a non-empty human-readable reason.
    expect(["green", "yellow", "red"]).toContain(item.status);
    expect(item.reason.length).toBeGreaterThan(0);
    expect(item.reason).not.toMatch(/NaN/);
    expect(item.reason).not.toContain("undefined");
  });
});
