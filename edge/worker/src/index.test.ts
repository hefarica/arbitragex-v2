/**
 * PERF-STACK WO-10 regression tests (2026-09-20).
 *
 * Covers the deferred-write rework of proxy()/checkRl():
 *   1. Cache semantics unchanged: MISS then HIT with identical body, and the
 *      KV fill happens even though the put is deferred (fire-and-forget in
 *      this harness — app.request() without an ExecutionContext mirrors the
 *      node-server production entry, where c.executionCtx throws).
 *   2. Rate-limit semantics unchanged: the counter still increments (deferred
 *      put lands) and the max+1-th request in a window gets 429.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import app from "./index.js";

// Hono's app.request signature: (input, requestInit, Env, executionCtx).
// Passing env as the 3rd arg and NO executionCtx mirrors the node-server
// production entry, where c.executionCtx throws and deferWrite falls back
// to fire-and-forget.
type Env = Parameters<typeof app.request>[2];

function makeKV(putDelayMs = 0) {
  const store = new Map<string, string>();
  return {
    store,
    async get(key: string): Promise<string | null> {
      return store.get(key) ?? null;
    },
    async put(key: string, value: string, _opts?: { expirationTtl?: number }): Promise<void> {
      if (putDelayMs > 0) await new Promise((r) => setTimeout(r, putDelayMs));
      store.set(key, value);
    },
    async delete(key: string): Promise<void> {
      store.delete(key);
    },
  };
}

function makeEnv(kv = makeKV(), rl = makeKV(), allowedOrigins = ""): Env {
  return {
    ARBX_ENV: "test",
    API_SERVER_URL: "http://upstream.invalid",
    ALLOWED_ORIGINS: allowedOrigins,
    ARBX_EDGE_TOKEN: "edge-secret",
    JWT_SECRET: "jwt-secret",
    ARBX_CACHE: kv,
    RATE_LIMIT: rl,
  } as unknown as Env;
}

function stubUpstream(body: unknown, status = 200) {
  return vi.fn(async () =>
    Promise.resolve(
      new Response(JSON.stringify(body), {
        status,
        headers: { "content-type": "application/json" },
      }),
    ),
  );
}

async function flushDeferred(ms = 20): Promise<void> {
  await new Promise((r) => setTimeout(r, ms));
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("WO-10: proxy() deferred cache fill", () => {
  it("first call MISS, second call HIT with identical body (node-style, no executionCtx)", async () => {
    const upstream = stubUpstream({ success: true, data: [{ id: "p1" }] });
    vi.stubGlobal("fetch", upstream);
    const kv = makeKV(5);
    const env = makeEnv(kv);

    const req1 = new Request("http://edge.invalid/api/dexes");
    const res1 = await app.request(req1, undefined, env);
    expect(res1.headers.get("x-arbx-cache")).toBe("MISS");
    expect(res1.status).toBe(200);
    const body1 = await res1.json();

    await flushDeferred();

    const req2 = new Request("http://edge.invalid/api/dexes");
    const res2 = await app.request(req2, undefined, env);
    expect(res2.headers.get("x-arbx-cache")).toBe("HIT");
    expect(res2.status).toBe(200);
    expect(await res2.json()).toEqual(body1);
    expect(upstream).toHaveBeenCalledTimes(1);
  });

  it("deferred cache put does not crash when it rejects (fail-soft, logged)", async () => {
    const errSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    const failingKv = {
      ...makeKV(),
      put: () => Promise.reject(new Error("redis down")),
    };
    vi.stubGlobal("fetch", stubUpstream({ ok: true }));

    const res = await app.request(new Request("http://edge.invalid/api/dexes"), undefined, makeEnv(failingKv));
    expect(res.status).toBe(200);
    expect(res.headers.get("x-arbx-cache")).toBe("MISS");

    await flushDeferred();
    // Unhandled rejection would fail the process; reaching here proves the catch.
    expect(errSpy).toHaveBeenCalled();
    errSpy.mockRestore();
  });
});

describe("WO-LR22.2: proxyPassThrough() carries CORS headers on success AND error", () => {
  // Incident shape (2026-09-22): proxyPassThrough returned a raw
  // `new Response(...)`, which Hono returns verbatim — the CORS headers set
  // via c.header() in the app.use("*") middleware were dropped, so the
  // honest 503 `quote_anchor_not_published` surfaced in the browser console
  // as a CORS error instead of a readable 503 (R8 observability defect).
  const ORIGIN = "https://arbx.ape-tv.net";

  it("success (200): allowlisted origin gets ACAO echoed on /api/quote/anchor", async () => {
    vi.stubGlobal("fetch", stubUpstream({ pair: "WETH/USDC", price_usd: 1 }, 200));
    const res = await app.request(
      new Request("http://edge.invalid/api/quote/anchor", { headers: { origin: ORIGIN } }),
      undefined,
      makeEnv(makeKV(), makeKV(), ORIGIN),
    );
    expect(res.status).toBe(200);
    expect(res.headers.get("x-arbx-cache")).toBe("PASS");
    expect(res.headers.get("access-control-allow-origin")).toBe(ORIGIN);
  });

  it("error (503 quote_anchor_not_published): same ACAO + real status visible to the browser", async () => {
    vi.stubGlobal("fetch", stubUpstream({ error: "quote_anchor_not_published" }, 503));
    const res = await app.request(
      new Request("http://edge.invalid/api/quote/anchor", { headers: { origin: ORIGIN } }),
      undefined,
      makeEnv(makeKV(), makeKV(), ORIGIN),
    );
    expect(res.status).toBe(503);
    expect(res.headers.get("access-control-allow-origin")).toBe(ORIGIN);
    const body = (await res.json()) as { error?: string };
    expect(body.error).toBe("quote_anchor_not_published");
  });

  it("non-allowlisted origin: ACAO empty string (no wildcard leak), body still passes through", async () => {
    vi.stubGlobal("fetch", stubUpstream({ ok: true }, 200));
    const res = await app.request(
      new Request("http://edge.invalid/api/quote/anchor", { headers: { origin: "https://evil.example" } }),
      undefined,
      makeEnv(makeKV(), makeKV(), ORIGIN),
    );
    expect(res.status).toBe(200);
    expect(res.headers.get("access-control-allow-origin") ?? "").toBe("");
  });
});

describe("WO-10: checkRl() deferred counter", () => {
  it("counter still increments and 429s after max requests in the window", async () => {
    vi.stubGlobal("fetch", stubUpstream({ ok: true }));
    const env = makeEnv();

    // Drain the whole public bucket for one IP.
    const iterations = 121; // RL_GENERAL_MAX floor = 120
    let last = 0;
    for (let i = 0; i < iterations; i++) {
      const res = await app.request(
        new Request("http://edge.invalid/health", {
          headers: { "cf-connecting-ip": "203.0.113.7" },
        }),
        undefined,
        env,
      );
      last = res.status;
    }

    expect(last).toBe(429);
  });
});
