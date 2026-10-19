import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import {
  parseBootCensus,
  mountOrchestratorMode,
  BOOT_CENSUS_KEY,
  type OrchestratorModeView,
} from "./orchestrator-mode.js";

const NOW = Date.parse("2026-10-19T12:00:00.000Z");

function census(mode: string, publishedAt = "2026-10-19T11:00:00.000Z"): string {
  return JSON.stringify({ published_at: publishedAt, arbx_orchestrator_mode: mode });
}

describe("parseBootCensus", () => {
  it.each([
    ["null", null],
    ["undefined", undefined],
    ["empty", ""],
    ["whitespace", "   "],
  ])("an absent census (%s) is NOT_COMPUTED, never a guessed v1", (_l, raw) => {
    const v = parseBootCensus(raw as string | null | undefined, NOW, "off");
    expect(v.computed).toBe(false);
    expect(v.mode).toBeNull();
    expect(v.legacy_emitter_reachable).toBeNull();
    expect(v.reason).toBe("boot_census_absent");
  });

  it("an unparseable payload is NOT_COMPUTED with its own reason", () => {
    const v = parseBootCensus("{not json", NOW, "off");
    expect(v.computed).toBe(false);
    expect(v.reason).toBe("boot_census_unparseable");
  });

  it("a payload without the mode field is NOT_COMPUTED", () => {
    const v = parseBootCensus(JSON.stringify({ published_at: "2026-10-19T11:00:00Z" }), NOW, "off");
    expect(v.computed).toBe(false);
    expect(v.reason).toBe("orchestrator_mode_field_absent");
  });

  it("a foreign mode value is NOT_COMPUTED rather than coerced", () => {
    const v = parseBootCensus(census("turbo"), NOW, "off");
    expect(v.computed).toBe(false);
    expect(v.reason).toBe("orchestrator_mode_field_absent");
  });

  it("V2 → legacy emitter NOT reachable (scanner.rs:2016-2019 skips the legacy body)", () => {
    const v = parseBootCensus(census("v2"), NOW, "off");
    expect(v.computed).toBe(true);
    expect(v.mode).toBe("v2");
    expect(v.legacy_emitter_reachable).toBe(false);
    expect(v.reason).toBeNull();
  });

  it.each(["v1", "shadow", "off"])("%s → legacy emitter reachable (the paper ledger can accumulate)", (m) => {
    const v = parseBootCensus(census(m), NOW, "on");
    expect(v.computed).toBe(true);
    expect(v.mode).toBe(m);
    expect(v.legacy_emitter_reachable).toBe(true);
  });

  it("normalises case (the searcher parses lowercased)", () => {
    expect(parseBootCensus(census("V2"), NOW, "off").mode).toBe("v2");
  });

  it("carries published_at and its age, so a stale boot census is visible", () => {
    const v = parseBootCensus(census("v2", "2026-10-19T11:00:00.000Z"), NOW, "off");
    expect(v.published_at).toBe("2026-10-19T11:00:00.000Z");
    expect(v.age_seconds).toBe(3600);
  });

  it("an unparseable published_at leaves age null without failing the computation", () => {
    const v = parseBootCensus(census("v1", "not-a-date"), NOW, "off");
    expect(v.computed).toBe(true);
    expect(v.age_seconds).toBeNull();
  });

  it("reports the paper-executor half of the pair verbatim, never merged", () => {
    const v = parseBootCensus(census("v2"), NOW, "on");
    expect(v.mode).toBe("v2");
    expect(v.paper_executor_mode).toBe("on");
  });
});

// ---------------------------------------------------------------------------
// Route — read-only, one Redis GET, and every failure path is NOT_COMPUTED.
// ---------------------------------------------------------------------------

type Handler = (req: unknown, res: ResponseLike) => Promise<void> | void;
interface ResponseLike {
  status(code: number): ResponseLike;
  json(body: unknown): ResponseLike;
  statusCode?: number;
  body?: unknown;
}

function captureRoute(deps: { redis: { get: (k: string) => Promise<string | null> } | null; warn: () => void }) {
  const routes: Record<string, Handler> = {};
  const app = { get: (p: string, h: Handler) => { routes[p] = h; } } as unknown as Parameters<typeof mountOrchestratorMode>[0];
  mountOrchestratorMode(app, {
    redis: deps.redis as never,
    logger: { warn: deps.warn as never },
  });
  return routes["/api/v1/readiness/orchestrator-mode"];
}

async function callRoute(handler: Handler, deps: { redis: { get: (k: string) => Promise<string | null> } | null; warn?: () => void }) {
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

  it("returns the computed view for a present census", async () => {
    process.env["ARBX_PAPER_EXECUTOR_MODE"] = "off";
    const body = await callRoute(
      captureRoute({ redis: { get: async (k) => (k === BOOT_CENSUS_KEY ? census("v2") : null) }, warn: () => {} }),
      { redis: { get: async () => census("v2") } },
    );
    expect(body.computed).toBe(true);
    expect(body.mode).toBe("v2");
    expect(body.legacy_emitter_reachable).toBe(false);
    expect(body.paper_executor_mode).toBe("off");
  });

  it("returns NOT_COMPUTED when the key is absent", async () => {
    const body = await callRoute(
      captureRoute({ redis: { get: async () => null }, warn: () => {} }),
      { redis: { get: async () => null } },
    );
    expect(body.computed).toBe(false);
    expect(body.reason).toBe("boot_census_absent");
  });

  it("returns NOT_COMPUTED with the cause when Redis throws (never a guess, never a 500)", async () => {
    const warn = vi.fn();
    const handler = captureRoute({ redis: { get: async () => { throw new Error("ECONNREFUSED"); } }, warn });
    const body = await callRoute(handler, { redis: { get: async () => { throw new Error("ECONNREFUSED"); } } });
    expect(body.computed).toBe(false);
    expect(String(body.reason)).toContain("redis_unavailable");
    expect(warn).toHaveBeenCalled();
  });

  it("returns NOT_COMPUTED when no Redis client is wired", async () => {
    const handler = captureRoute({ redis: null, warn: () => {} });
    const body = await callRoute(handler, { redis: null });
    expect(body.computed).toBe(false);
    expect(String(body.reason)).toContain("redis_unavailable");
  });
});
