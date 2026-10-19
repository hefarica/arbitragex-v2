import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import {
  parseBootCensus,
  mountOrchestratorMode,
  BOOT_CENSUS_KEY,
  SEARCHER_BOOT_KEY,
  FRESHNESS_TOLERANCE_SECONDS,
  type OrchestratorModeView,
} from "./orchestrator-mode.js";

const NOW = Date.parse("2026-10-19T12:00:00.000Z");

function census(mode: string, publishedAt = "2026-10-19T11:00:00.000Z"): string {
  return JSON.stringify({ published_at: publishedAt, arbx_orchestrator_mode: mode });
}
function bootMarker(startedAt: string): string {
  return JSON.stringify({ started_at: startedAt });
}

describe("parseBootCensus — the mode itself", () => {
  it.each([
    ["null", null],
    ["undefined", undefined],
    ["empty", ""],
    ["whitespace", "   "],
  ])("an absent census (%s) is NOT_COMPUTED, never a guessed v1", (_l, raw) => {
    const v = parseBootCensus(raw as string | null | undefined, null, NOW, "off");
    expect(v.computed).toBe(false);
    expect(v.mode).toBeNull();
    expect(v.legacy_emitter_reachable).toBeNull();
    expect(v.freshness.verdict).toBe("not_computable");
    expect(v.reason).toBe("boot_census_absent");
  });

  it("an unparseable payload is NOT_COMPUTED with its own reason", () => {
    const v = parseBootCensus("{not json", null, NOW, "off");
    expect(v.computed).toBe(false);
    expect(v.reason).toBe("boot_census_unparseable");
  });

  it("a payload without the mode field is NOT_COMPUTED", () => {
    const v = parseBootCensus(JSON.stringify({ published_at: "2026-10-19T11:00:00Z" }), null, NOW, "off");
    expect(v.computed).toBe(false);
    expect(v.reason).toBe("orchestrator_mode_field_absent");
  });

  it("a foreign mode value is NOT_COMPUTED rather than coerced", () => {
    const v = parseBootCensus(census("turbo"), null, NOW, "off");
    expect(v.computed).toBe(false);
    expect(v.reason).toBe("orchestrator_mode_field_absent");
  });

  it("V2 → legacy emitter NOT reachable (scanner.rs:2016 skips the legacy body)", () => {
    const v = parseBootCensus(census("v2"), null, NOW, "off");
    expect(v.computed).toBe(true);
    expect(v.mode).toBe("v2");
    expect(v.legacy_emitter_reachable).toBe(false);
    expect(v.reason).toBeNull();
  });

  it.each(["v1", "shadow", "off"])("%s → legacy emitter reachable", (m) => {
    const v = parseBootCensus(census(m), null, NOW, "on");
    expect(v.mode).toBe(m);
    expect(v.legacy_emitter_reachable).toBe(true);
  });

  it("normalises case (the searcher parses lowercased)", () => {
    expect(parseBootCensus(census("V2"), null, NOW, "off").mode).toBe("v2");
  });

  it("declares legacy_emitter_reachable as DERIVED, with the Rust line it mirrors", () => {
    const v = parseBootCensus(census("v2"), null, NOW, "off");
    expect(v.derivations.legacy_emitter_reachable).toContain("scanner.rs:2016");
  });

  it("carries published_at as a boot marker with its wall-clock age — never as freshness", () => {
    const v = parseBootCensus(census("v2", "2026-10-19T11:00:00.000Z"), null, NOW, "off");
    expect(v.boot_marker.published_at).toBe("2026-10-19T11:00:00.000Z");
    expect(v.boot_marker.age_seconds).toBe(3600);
    // The old shape promised freshness from age; it must not exist any more.
    expect(v).not.toHaveProperty("age_seconds");
    expect(v).not.toHaveProperty("published_at");
  });

  it("reports the paper-executor half of the pair verbatim, never merged", () => {
    const v = parseBootCensus(census("v2"), null, NOW, "on");
    expect(v.mode).toBe("v2");
    expect(v.paper_executor_mode).toBe("on");
  });
});

describe("parseBootCensus — freshness is a comparison, not an age", () => {
  it("a six-week-old census with a six-week-old process is CURRENT BY CONSTRUCTION", () => {
    const before = NOW - 42 * 24 * 3600 * 1000;
    const v = parseBootCensus(
      census("v1", new Date(before).toISOString()),
      bootMarker(new Date(before).toISOString()),
      NOW,
      "off",
    );
    expect(v.freshness.verdict).toBe("current_by_construction");
    expect(v.freshness.witness).toBe(SEARCHER_BOOT_KEY);
    expect(v.boot_marker.age_seconds).toBe(42 * 24 * 3600);
  });

  it("a census written BEFORE the process started is census_writer_stale", () => {
    const v = parseBootCensus(
      census("v2", "2026-09-07T10:00:00.000Z"),
      bootMarker("2026-10-19T04:53:00.000Z"),
      NOW,
      "off",
    );
    expect(v.freshness.verdict).toBe("census_writer_stale");
    expect(v.freshness.delta_seconds).toBeLessThan(0);
  });

  it("boot and census a few seconds apart are the same boot (tolerance)", () => {
    const v = parseBootCensus(
      census("v1", "2026-10-19T11:59:30.000Z"),
      bootMarker("2026-10-19T12:00:00.000Z"),
      NOW,
      "off",
    );
    expect(Math.abs(v.freshness.delta_seconds ?? 0)).toBeLessThanOrEqual(FRESHNESS_TOLERANCE_SECONDS);
    expect(v.freshness.verdict).toBe("current_by_construction");
  });

  it("without the witness it is NOT_COMPUTED and NAMES what is missing", () => {
    const v = parseBootCensus(census("v2"), null, NOW, "off");
    expect(v.freshness.verdict).toBe("not_computable");
    expect(v.freshness.reason).toBe("searcher_boot_marker_absent");
    expect(String(v.freshness.required_witness)).toContain(SEARCHER_BOOT_KEY);
  });

  it("a malformed witness is NOT_COMPUTED, not a verdict", () => {
    const v = parseBootCensus(census("v2"), "{not json", NOW, "off");
    expect(v.freshness.verdict).toBe("not_computable");
    expect(v.freshness.reason).toBe("searcher_boot_marker_unreadable");
  });

  it("a census NEWER than the boot marker blames the marker, not the census", () => {
    const v = parseBootCensus(
      census("v1", "2026-10-19T11:30:00.000Z"),
      bootMarker("2026-10-19T10:00:00.000Z"),
      NOW,
      "off",
    );
    expect(v.freshness.verdict).toBe("not_computable");
    expect(v.freshness.reason).toBe("boot_marker_predates_census");
  });

  it("an unreadable published_at leaves the freshness NOT_COMPUTED (no delta invented)", () => {
    const v = parseBootCensus(census("v1", "not-a-date"), bootMarker("2026-10-19T10:00:00.000Z"), NOW, "off");
    expect(v.freshness.verdict).toBe("not_computable");
    expect(v.freshness.reason).toBe("boot_census_published_at_unreadable");
  });
});

// ---------------------------------------------------------------------------
// Route — read-only, two Redis GETs, and every failure path is NOT_COMPUTED.
// ---------------------------------------------------------------------------

type Handler = (req: unknown, res: ResponseLike) => Promise<void> | void;
interface ResponseLike {
  status(code: number): ResponseLike;
  json(body: unknown): ResponseLike;
  statusCode?: number;
  body?: unknown;
}

type FakeRedis = { get: (k: string) => Promise<string | null> };

function captureRoute(warn: () => void, redis: FakeRedis | null) {
  const routes: Record<string, Handler> = {};
  const app = { get: (p: string, h: Handler) => { routes[p] = h; } } as unknown as Parameters<typeof mountOrchestratorMode>[0];
  mountOrchestratorMode(app, { redis: redis as never, logger: { warn: warn as never } });
  return routes["/api/v1/readiness/orchestrator-mode"];
}

async function callRoute(handler: Handler) {
  const res: ResponseLike = {
    status(code: number) { this.statusCode = code; return this; },
    json(body: unknown) { this.body = body; return this; },
  };
  await handler({}, res);
  return res.body as OrchestratorModeView & { statusCode?: number };
}

describe("mountOrchestratorMode", () => {
  let orig: string | undefined;
  beforeEach(() => {
    orig = process.env["ARBX_PAPER_EXECUTOR_MODE"];
  });
  afterEach(() => {
    if (orig === undefined) delete process.env["ARBX_PAPER_EXECUTOR_MODE"];
    else process.env["ARBX_PAPER_EXECUTOR_MODE"] = orig;
  });

  it("reads BOTH keys and returns the computed view", async () => {
    process.env["ARBX_PAPER_EXECUTOR_MODE"] = "off";
    const get = vi.fn(async (k: string) => {
      if (k === BOOT_CENSUS_KEY) return census("v2", "2026-10-19T11:00:00.000Z");
      if (k === SEARCHER_BOOT_KEY) return bootMarker("2026-10-19T11:00:10.000Z");
      return null;
    });
    const body = await callRoute(captureRoute(() => {}, { get }));
    expect(get).toHaveBeenCalledTimes(2);
    expect(body.computed).toBe(true);
    expect(body.mode).toBe("v2");
    expect(body.legacy_emitter_reachable).toBe(false);
    expect(body.freshness.verdict).toBe("current_by_construction");
    expect(body.paper_executor_mode).toBe("off");
  });

  it("returns NOT_COMPUTED when the census key is absent", async () => {
    const body = await callRoute(captureRoute(() => {}, { get: async () => null }));
    expect(body.computed).toBe(false);
    expect(body.reason).toBe("boot_census_absent");
    expect(body.freshness.verdict).toBe("not_computable");
  });

  it("returns NOT_COMPUTED with the cause when Redis throws (never a guess, never a 500)", async () => {
    const warn = vi.fn();
    const handler = captureRoute(warn, { get: async () => { throw new Error("ECONNREFUSED"); } });
    const body = await callRoute(handler);
    expect(body.computed).toBe(false);
    expect(String(body.reason)).toContain("redis_unavailable");
    expect(warn).toHaveBeenCalled();
  });

  it("returns NOT_COMPUTED when no Redis client is wired", async () => {
    const body = await callRoute(captureRoute(() => {}, null));
    expect(body.computed).toBe(false);
    expect(String(body.reason)).toContain("redis_unavailable");
  });
});
