import { describe, expect, it } from "vitest";
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";

/**
 * ARBX-V-014 — worker ↔ dev-local edge route-parity contract test.
 *
 * Doctrine (dev-local :674): "REGLA EDGE as every route: explicit, no generic
 * /api/*". Both proxies declare every route EXPLICITLY — which means the two
 * route sets are comparable by scan. This test pins the CURRENT difference so
 * that any NEW asymmetry fails loudly instead of drifting silently (the
 * FE-0055/FE-0060 class of bugs: endpoint exists in prod worker but the local
 * dev proxy 404s it, or vice versa).
 *
 * Asymmetries are NOT bugs by definition — each entry below carries its
 * documented reason. The contract is: diff == ALLOWED_ASYMMETRIES, EXACTLY.
 * Adding a route to only one proxy requires adding it here WITH a reason;
 * removing an asymmetry (fixing parity) requires removing it here.
 */

const WORKER_SRC = join(__dirname, "..", "..", "..", "edge", "worker", "src", "index.ts");
const DEVLOCAL_SRC = join(__dirname, "..", "..", "..", "edge", "dev-local", "src", "index.ts");

type Route = `${string} ${string}`;

/** Extract `app.<method>("/api/...")` declarations (both proxies use this shape). */
function extractApiRoutes(src: string): Set<Route> {
  const routes = new Set<Route>();
  const re = /app\.(get|post|put|delete|patch|all)\(\s*"((\/api\/)[^"]*)"/g;
  for (const m of src.matchAll(re)) {
    routes.add(`${m[1].toUpperCase()} ${m[2]}` as Route);
  }
  return routes;
}

const workerSrc = readFileSync(WORKER_SRC, "utf8");
const devLocalSrc = readFileSync(DEVLOCAL_SRC, "utf8");
const workerRoutes = extractApiRoutes(workerSrc);
const devLocalRoutes = extractApiRoutes(devLocalSrc);

/**
 * Documented asymmetries (extracted 2026-08-24: worker 116 / dev-local 117 /
 * common 112). Every entry needs a reason; a diff that is NOT in this list
 * fails the test.
 */
const ALLOWED_ASYMMETRIES: ReadonlyArray<{
  route: Route;
  present: "worker" | "dev-local";
  reason: string;
}> = [
  // ── worker-only ──
  {
    route: "GET /api/prices/live",
    present: "worker",
    reason:
      "G-PRICE-1 (worker :488) — USD price snapshot pass-through, NO KV cache " +
      "(freshness is the point). Shipped on the canonical worker; dev-local " +
      "parity pending.",
  },
  {
    route: "GET /api/v1/health",
    present: "worker",
    reason:
      "Latency-optimized pass-through (<30ms target, worker :677); dev-local " +
      "parity pending.",
  },
  {
    route: "GET /api/v1/metrics/entropy",
    present: "worker",
    reason:
      "Latency-optimized pass-through (worker :678); dev-local parity pending.",
  },
  {
    route: "GET /api/recon/timeseries",
    present: "worker",
    reason:
      "Recon timeseries proxy with 15s KV cache (worker :594). dev-local serves " +
      "only /api/recon/summary; parity pending.",
  },
  {
    route: "GET /api/v1/dexes/:id",
    present: "worker",
    reason:
      "Read-side DEX detail (worker :609). dev-local has the WRITE side only " +
      "(admin-session model); read-detail parity pending.",
  },
  {
    route: "GET /api/v1/dexes/:id/active",
    present: "worker",
    reason:
      "Read-side DEX active flag (worker :610); same split as GET :id above.",
  },
  {
    route: "GET /api/live-testnet/events",
    present: "worker",
    reason:
      "DAPP-SURFACE remediation 2026-08-31 (PR #495) — public SSE stream " +
      "proxyPassThrough (api-server /api/live-testnet/events, X-Accel-Buffering: " +
      "no). Worker-only by design: dev-local SSE parity pending.",
  },
  {
    route: "GET /api/operator/me",
    present: "worker",
    reason:
      "DAPP-SURFACE remediation 2026-08-31 (PR #495) — cookie-forwarding " +
      "walletProxy (never cached): the worker translates the httpOnly operator " +
      "session cookie to the upstream identity, the same session model as the " +
      "adminProxy split below. dev-local parity pending.",
  },
  // ── dev-local-only ──
  {
    route: "POST /api/admin/tokens/resolve",
    present: "dev-local",
    reason:
      "INTENTIONAL — admin auth model: dev-local adminProxy translates the " +
      "httpOnly cookie session to the upstream token (dev-local :634); the " +
      "Cloudflare worker does not carry that session.",
  },
  {
    route: "POST /api/v1/dexes",
    present: "dev-local",
    reason:
      "INTENTIONAL — DEX registry CREATE behind adminProxy cookie session " +
      "(dev-local :967); operator write path is dev-local/prod-nginx only.",
  },
  {
    route: "DELETE /api/v1/dexes/:id",
    present: "dev-local",
    reason:
      "INTENTIONAL — DEX registry hard-delete behind adminProxy (dev-local :970).",
  },
  {
    route: "PUT /api/v1/dexes/:id/active",
    present: "dev-local",
    reason:
      "INTENTIONAL — DEX active toggle behind adminProxy (dev-local :960), " +
      "2026-05-10 audit follow-up.",
  },
];

describe("ARBX-V-014 — edge route parity (worker ↔ dev-local)", () => {
  it("both proxies declare a substantial explicit /api route set", () => {
    // Guards against a regex/extraction regression silently matching nothing.
    expect(workerRoutes.size).toBeGreaterThan(100);
    expect(devLocalRoutes.size).toBeGreaterThan(100);
  });

  it("route-set difference is EXACTLY the documented asymmetry allowlist", () => {
    const workerOnly = [...workerRoutes].filter((r) => !devLocalRoutes.has(r)).sort();
    const devLocalOnly = [...devLocalRoutes].filter((r) => !workerRoutes.has(r)).sort();

    const expectedWorker = ALLOWED_ASYMMETRIES.filter((a) => a.present === "worker")
      .map((a) => a.route)
      .sort();
    const expectedDevLocal = ALLOWED_ASYMMETRIES.filter((a) => a.present === "dev-local")
      .map((a) => a.route)
      .sort();

    // toEqual gives a precise diff message for either direction of drift.
    expect(workerOnly).toEqual(expectedWorker);
    expect(devLocalOnly).toEqual(expectedDevLocal);
  });

  it("every allowlist entry still exists where it is claimed (no stale entries)", () => {
    for (const a of ALLOWED_ASYMMETRIES) {
      const claimed = a.present === "worker" ? workerRoutes : devLocalRoutes;
      const other = a.present === "worker" ? devLocalRoutes : workerRoutes;
      expect(claimed.has(a.route), `${a.route} must exist in ${a.present}`).toBe(true);
      expect(other.has(a.route), `${a.route} must NOT exist in the other proxy`).toBe(false);
    }
  });

  it("program endpoints (FE-0055/FE-0056 canon) are declared in BOTH proxies", () => {
    const programEndpoints: Route[] = [
      "GET /api/config/canonical-knobs",
      "GET /api/strategies/catalog",
      "GET /api/detectors/catalog",
      "GET /api/quote/anchor",
      "GET /api/paper/history",
      "GET /api/paper/history/summary",
    ];
    for (const ep of programEndpoints) {
      expect(workerRoutes.has(ep), `worker must declare ${ep}`).toBe(true);
      expect(devLocalRoutes.has(ep), `dev-local must declare ${ep}`).toBe(true);
    }
  });

  it("doctrine: NEITHER proxy declares a generic /api/* catch-all", () => {
    // "REGLA EDGE as every route: explicit, no generic /api/*" (dev-local :674).
    // A catch-all would silently swallow 404s and defeat this parity contract.
    for (const [name, src] of [
      ["worker", workerSrc],
      ["dev-local", devLocalSrc],
    ] as const) {
      expect(
        /app\.(get|all|use)\(\s*['"](\/api\/\*|\/api)['"]/m.test(src),
        `${name} must not declare a generic /api or /api/* route`,
      ).toBe(false);
    }
  });

  it("pinned upstream paths exist in api-server source (proxies never 404 on healthy api-server)", () => {
    // The public edge path and the upstream path can differ (e.g.
    // /api/config/canonical-knobs → /api/v1/config/canonical-knobs), so pin
    // the UPSTREAM path strings against api-server's own route declarations.
    const upstreams = [
      "/api/v1/config/canonical-knobs",
      "/api/strategies/catalog",
      "/api/detectors/catalog",
      "/api/quote/anchor",
      "/api/v1/paper/history",
    ];
    // Collect api-server source once (src/**, .ts only).
    const srcDir = join(__dirname);
    const files: string[] = [];
    const walk = (dir: string) => {
      for (const entry of readdirSync(dir)) {
        const p = join(dir, entry);
        if (statSync(p).isDirectory()) walk(p);
        else if (entry.endsWith(".ts")) files.push(readFileSync(p, "utf8"));
      }
    };
    walk(srcDir);
    const allSource = files.join("\n");
    for (const up of upstreams) {
      expect(
        allSource.includes(`"${up}"`),
        `api-server must declare upstream ${up} (edge proxies point at it)`,
      ).toBe(true);
    }
  });
});
