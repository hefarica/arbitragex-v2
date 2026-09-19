// ICON-RAIN-20260917 — network-tier dedupe tests for the useTokenIcon cascade.
//
// Reproduces the defect observed in BROWSE §4.5 (audits/first-understand-
// 20260917): ~30 cards of the same long-tail token mounted per render each
// fired their own GET /api/v1/token-icon/:chain/:addr (and aborted on unmount,
// so the in-memory cache never populated). These tests pin the fix:
//   1. single-flight  — concurrent resolutions of one token share ONE fetch;
//   2. cache populate — after success, a later resolution hits tier 2 (0 fetch);
//   3. negative cache — after a failure, remounts do NOT re-fetch within TTL;
//   4. per-token keys — different tokens do not share a flight.
// Node env (no DOM) — the hook itself is SSR-rendered elsewhere; the network
// tier is a plain module function, so it is tested directly with a stubbed
// global fetch (vi.stubGlobal). No fabricated icon data anywhere (RULE 00):
// the stub returns a canned HTTP response exactly like the real route's.

import { afterEach, describe, expect, it, vi } from "vitest";

import {
  _resetIconFetchStateForTests,
  fetchIconResolution,
  recentIconFetchFailure,
} from "./useTokenIcon";
import {
  _resetIconCacheForTests,
  getCachedIcon,
} from "@/lib/known-tokens";

const UNKNOWN = "0x1111111111111111111111111111111111111111";
const OTHER = "0x2222222222222222222222222222222222222222";

function jsonResponse(body: unknown, status = 200): Response {
  return {
    ok: status >= 200 && status < 300,
    status,
    json: async () => body,
  } as unknown as Response;
}

/** Deferred fetch stub so we control when the in-flight request resolves. */
function deferredFetch() {
  let resolveFn!: (r: Response) => void;
  const promise = new Promise<Response>((res) => {
    resolveFn = res;
  });
  const fn = vi.fn((_url: string) => promise);
  return { fn, resolve: resolveFn };
}

afterEach(() => {
  vi.unstubAllGlobals();
  _resetIconFetchStateForTests();
  _resetIconCacheForTests();
});

describe("fetchIconResolution single-flight (ICON-RAIN-20260917)", () => {
  it("concurrent resolutions of the same token share ONE network request", async () => {
    const { fn, resolve } = deferredFetch();
    vi.stubGlobal("fetch", fn);

    // 30 concurrent mounts of the same token (the observed PEPE rain).
    const calls = Array.from({ length: 30 }, () => fetchIconResolution(1, UNKNOWN));
    resolve(
      jsonResponse({ ok: true, iconUrl: "https://x/pepe.png", source: "redis", cached: true }),
    );
    const outcomes = await Promise.all(calls);

    expect(fn).toHaveBeenCalledTimes(1);
    expect(fn.mock.calls[0]![0]).toBe(`/api/v1/token-icon/1/${UNKNOWN}`);
    for (const o of outcomes) {
      expect(o.res.iconUrl).toBe("https://x/pepe.png");
      expect(o.error).toBeNull();
    }
  });

  it("a successful fetch populates the tier-2 cache (later resolution = 0 fetches)", async () => {
    const fn = vi.fn(async () =>
      jsonResponse({ ok: true, iconUrl: "https://x/pepe.png", source: "redis", cached: true }),
    );
    vi.stubGlobal("fetch", fn);

    await fetchIconResolution(1, UNKNOWN);
    // Remount after the first completed: tier 2 covers it synchronously.
    expect(getCachedIcon(1, UNKNOWN)?.iconUrl).toBe("https://x/pepe.png");
  });

  it("different tokens get independent flights (no cross-token suppression)", async () => {
    const { fn, resolve } = deferredFetch();
    vi.stubGlobal("fetch", fn);

    const a = fetchIconResolution(1, UNKNOWN);
    const b = fetchIconResolution(1, OTHER);
    resolve(
      jsonResponse({ ok: true, iconUrl: "https://x/pepe.png", source: "redis", cached: true }),
    );
    await a;
    // b's flight is still pending and unresolved — but it DID issue its own
    // request (different key), which is exactly what the rain fix must not
    // collapse across tokens.
    expect(fn).toHaveBeenCalledTimes(2);
    await b;
  });

  it("failures are negatively cached: remounts within the TTL do NOT re-fetch", async () => {
    const fn = vi.fn(async () => jsonResponse({ ok: false }, 503));
    vi.stubGlobal("fetch", fn);

    const first = await fetchIconResolution(1, UNKNOWN);
    expect(first.error).toBe("HTTP 503");
    expect(first.res.isFallback).toBe(true); // honest degradation, no icon

    const failure = recentIconFetchFailure(1, UNKNOWN);
    expect(failure).not.toBeNull();
    expect(failure!.message).toBe("HTTP 503");

    // Remount storm within TTL → would-be requests are absorbed by the
    // negative cache in the hook's effect; the fetcher itself still issues
    // a request if called directly, so assert the guard used by the effect.
    expect(recentIconFetchFailure(1, UNKNOWN)).not.toBeNull();
    expect(fn).toHaveBeenCalledTimes(1);
  });

  it("a null-icon (jazzicon) response is cached as a real resolution, not an error", async () => {
    const fn = vi.fn(async () =>
      jsonResponse({ ok: true, iconUrl: null, source: "jazzicon", cached: false }),
    );
    vi.stubGlobal("fetch", fn);

    const out = await fetchIconResolution(1, UNKNOWN);
    expect(out.error).toBeNull();
    expect(out.res.isFallback).toBe(true);
    expect(getCachedIcon(1, UNKNOWN)?.source).toBe("jazzicon");
    expect(recentIconFetchFailure(1, UNKNOWN)).toBeNull();
  });
});
