import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { promises as fs } from "node:fs";
import path from "node:path";
import os from "node:os";
import { pending } from "./verifiers/pending.js";
import { verifyVDB1 } from "./verifiers/v-db-1.js";
import { verifyVAT1 } from "./verifiers/v-at-1.js";
import { verifyVNH1 } from "./verifiers/v-nh-1.js";
import { verifyPR1CSP, DEFAULT_URL } from "./verifiers/pr-1-csp.js";
import { verifyPR2Audit } from "./verifiers/pr-2-audit.js";
import { verifyMonitoring } from "./verifiers/monitoring.js";
import { verifyRunbook } from "./verifiers/runbook.js";
import { verifyGSIM1 } from "./verifiers/g-sim-1.js";
import { verifyAll } from "./verifiers/index.js";

const NOW = () => new Date("2026-05-01T12:00:00.000Z");

describe("pending() sentinel factory", () => {
  it("returns status='pending' with default reason", () => {
    const item = pending({ id: "G-X", group: "risk_doctrines", label: "X", now: NOW });
    expect(item.status).toBe("pending");
    expect(item.reason).toBe("feature not yet implemented");
    expect(item.id).toBe("G-X");
  });
  it("uses provided reason when supplied", () => {
    const item = pending({ id: "G-X", group: "risk_doctrines", label: "X", reason: "PR-3 not started", now: NOW });
    expect(item.reason).toBe("PR-3 not started");
  });
  it("includes doctrine when provided", () => {
    const item = pending({ id: "G-X", group: "risk_doctrines", label: "X", doctrine: "arbx-rpc-failover", now: NOW });
    expect(item.doctrine).toBe("arbx-rpc-failover");
  });
});

describe("verifyVDB1()", () => {
  let tmpDir: string;
  beforeEach(async () => { tmpDir = await fs.mkdtemp(path.join(os.tmpdir(), "vdb1-")); });
  afterEach(async () => { await fs.rm(tmpDir, { recursive: true, force: true }); });

  it("returns yellow when directory missing (no repo mount)", async () => {
    const item = await verifyVDB1({ dir: "/nonexistent/path", now: NOW });
    expect(item.status).toBe("yellow");
    expect(item.reason).toMatch(/repo mount/);
  });
  it("returns green when no SQL files contain password literals", async () => {
    await fs.writeFile(path.join(tmpDir, "001.sql"), "ALTER ROLE arbx_rw WITH PASSWORD :'arbx_rw_pw';\n");
    const item = await verifyVDB1({ dir: tmpDir, now: NOW });
    expect(item.status).toBe("green");
  });
  it("returns red when a SQL file contains a literal password", async () => {
    await fs.writeFile(path.join(tmpDir, "leak.sql"), "ALTER ROLE arbx_rw WITH PASSWORD 'literal123';\n");
    const item = await verifyVDB1({ dir: tmpDir, now: NOW });
    expect(item.status).toBe("red");
    expect(item.reason).toMatch(/leak\.sql/);
  });
});

describe("verifyVAT1()", () => {
  let tmpDir: string;
  beforeEach(async () => { tmpDir = await fs.mkdtemp(path.join(os.tmpdir(), "vat1-")); });
  afterEach(async () => { await fs.rm(tmpDir, { recursive: true, force: true }); });

  it("returns yellow when file missing", async () => {
    const item = await verifyVAT1({ file: path.join(tmpDir, "nope.ts"), now: NOW });
    expect(item.status).toBe("yellow");
  });
  it("returns green when file does not use localStorage with admin/token", async () => {
    const safe = `export async function setAdminToken(token: string) { /* httpOnly cookie via /admin/session */ }`;
    const f = path.join(tmpDir, "admin-token.ts");
    await fs.writeFile(f, safe);
    const item = await verifyVAT1({ file: f, now: NOW });
    expect(item.status).toBe("green");
  });
  it("returns red when localStorage admin/token usage is detected", async () => {
    const bad = `localStorage.setItem("admin_token", token);`;
    const f = path.join(tmpDir, "admin-token.ts");
    await fs.writeFile(f, bad);
    const item = await verifyVAT1({ file: f, now: NOW });
    expect(item.status).toBe("red");
  });
});

describe("verifyVAT1() endpoint probe (prod path — no repo mount)", () => {
  const origFetch = globalThis.fetch;
  afterEach(() => { globalThis.fetch = origFetch; });

  it("returns green when the probe answers 400 token_required", async () => {
    globalThis.fetch = vi.fn(async () => new Response(null, { status: 400 })) as any;
    const item = await verifyVAT1({ file: "/nonexistent/vat1-probe-test.ts", probeUrl: "http://vat1-test/admin/session", now: NOW });
    expect(item.status).toBe("green");
    expect(item.reason).toMatch(/endpoint probe/);
    expect(item.evidence?.ref).toContain("400");
  });
  it("returns green when the route limiter answers 429 with its header (route alive + rate-limit active)", async () => {
    globalThis.fetch = vi.fn(async () => new Response(null, { status: 429,
      headers: { "x-ratelimit-admin-session-remaining": "0" } })) as any;
    const item = await verifyVAT1({ file: "/nonexistent/vat1-probe-test.ts", probeUrl: "http://vat1-test/admin/session", now: NOW });
    expect(item.status).toBe("green");
    expect(item.reason).toContain("429");
  });
  it("returns yellow on a bare 429 without the route header (global limiter or lockout — not route-attributable)", async () => {
    globalThis.fetch = vi.fn(async () => new Response(null, { status: 429 })) as any;
    const item = await verifyVAT1({ file: "/nonexistent/vat1-probe-test.ts", probeUrl: "http://vat1-test/admin/session", now: NOW });
    expect(item.status).toBe("yellow");
    expect(item.reason).toMatch(/429/);
  });
  it("returns yellow when the probe answers 404 (route missing)", async () => {
    globalThis.fetch = vi.fn(async () => new Response(null, { status: 404 })) as any;
    const item = await verifyVAT1({ file: "/nonexistent/vat1-probe-test.ts", probeUrl: "http://vat1-test/admin/session", now: NOW });
    expect(item.status).toBe("yellow");
    expect(item.reason).toMatch(/404/);
  });
  it("returns yellow on an unexpected 500", async () => {
    globalThis.fetch = vi.fn(async () => new Response(null, { status: 500 })) as any;
    const item = await verifyVAT1({ file: "/nonexistent/vat1-probe-test.ts", probeUrl: "http://vat1-test/admin/session", now: NOW });
    expect(item.status).toBe("yellow");
  });
  it("returns yellow when the probe throws (ECONNREFUSED)", async () => {
    globalThis.fetch = vi.fn(async () => { throw new Error("ECONNREFUSED"); }) as any;
    const item = await verifyVAT1({ file: "/nonexistent/vat1-probe-test.ts", probeUrl: "http://vat1-test/admin/session", now: NOW });
    expect(item.status).toBe("yellow");
  });
});

describe("verifyVNH1()", () => {
  let tmpDir: string;
  beforeEach(async () => { tmpDir = await fs.mkdtemp(path.join(os.tmpdir(), "vnh1-")); });
  afterEach(async () => { await fs.rm(tmpDir, { recursive: true, force: true }); });

  it("returns yellow when script missing", async () => {
    const item = await verifyVNH1({ script: path.join(tmpDir, "no-script.sh"), now: NOW });
    expect(item.status).toBe("yellow");
  });
  it("returns green when script exits 0", async () => {
    const f = path.join(tmpDir, "ok.sh");
    await fs.writeFile(f, "#!/usr/bin/env bash\nexit 0\n", { mode: 0o755 });
    
    const execFn = async () => ({ stdout: "", stderr: "" });
    const item = await verifyVNH1({ script: f, cwd: tmpDir, now: NOW, execFn });
    expect(item.status).toBe("green");
  });
  it("returns red when script exits non-zero", async () => {
    const f = path.join(tmpDir, "fail.sh");
    await fs.writeFile(f, "#!/usr/bin/env bash\necho 'leak detected'\nexit 1\n", { mode: 0o755 });
    
    const execFn = async () => { throw new Error("leak detected"); };
    const item = await verifyVNH1({ script: f, cwd: tmpDir, now: NOW, execFn });
    expect(item.status).toBe("red");
  });
});

describe("verifyPR1CSP()", () => {
  const origFetch = globalThis.fetch;
  afterEach(() => { globalThis.fetch = origFetch; });

  it("returns red when no CSP header present", async () => {
    globalThis.fetch = vi.fn(async () => new Response(null, { status: 200, headers: {} })) as any;
    const item = await verifyPR1CSP({ url: "http://x", now: NOW });
    expect(item.status).toBe("red");
  });
  it("returns yellow when CSP present but missing frame-ancestors none", async () => {
    globalThis.fetch = vi.fn(async () => new Response(null, { status: 200,
      headers: { "content-security-policy-report-only": "default-src 'self'" } })) as any;
    const item = await verifyPR1CSP({ url: "http://x", now: NOW });
    expect(item.status).toBe("yellow");
  });
  it("returns yellow when CSP + frame-ancestors ok but HSTS missing", async () => {
    globalThis.fetch = vi.fn(async () => new Response(null, { status: 200,
      headers: { "content-security-policy-report-only": "default-src 'self'; frame-ancestors 'none'" } })) as any;
    const item = await verifyPR1CSP({ url: "http://x", now: NOW });
    expect(item.status).toBe("yellow");
    expect(item.reason).toMatch(/strict-transport-security/);
  });
  it("returns green when CSP + frame-ancestors none + HSTS present", async () => {
    globalThis.fetch = vi.fn(async () => new Response(null, { status: 200,
      headers: {
        "content-security-policy-report-only": "default-src 'self'; frame-ancestors 'none'",
        "strict-transport-security": "max-age=31536000; includeSubDomains",
      } })) as any;
    const item = await verifyPR1CSP({ url: "http://x", now: NOW });
    expect(item.status).toBe("green");
  });
  it("returns yellow on fetch failure", async () => {
    globalThis.fetch = vi.fn(async () => { throw new Error("ECONNREFUSED"); }) as any;
    const item = await verifyPR1CSP({ url: "http://x", now: NOW });
    expect(item.status).toBe("yellow");
  });
  // READY-CIRC-01 regression: the compose-internal DEFAULT_URL must target a
  // static asset (/favicon.ico), never `/`. `/` triggers force-dynamic SSR →
  // getReadinessDecision() → verifyAll() → PR-1 HEAD `/` self-deadlock (8s
  // abort every ~39s TTL beat, observed live 2026-08-22). Env overrides keep
  // their exact target (operator-controlled CSP_PROBE_URL / dev
  // FRONTEND_INTERNAL_URL), so only the built-in default is asserted.
  it("READY-CIRC-01: DEFAULT_URL probes /favicon.ico, never the SSR root", async () => {
    expect(DEFAULT_URL).toBe("http://frontend:5173/favicon.ico");
  });
});

describe("verifyPR2Audit()", () => {
  it("returns yellow when pool is null", async () => {
    const item = await verifyPR2Audit({ pool: null, now: NOW });
    expect(item.status).toBe("yellow");
  });
  it("returns green when audit_log has rows in window", async () => {
    const pool = { query: vi.fn(async () => ({ rows: [{ cnt: 5 }] })) } as any;
    const item = await verifyPR2Audit({ pool, now: NOW });
    expect(item.status).toBe("green");
    expect(item.reason).toMatch(/5 audit/);
  });
  it("returns yellow when 0 rows in window", async () => {
    const pool = { query: vi.fn(async () => ({ rows: [{ cnt: 0 }] })) } as any;
    const item = await verifyPR2Audit({ pool, now: NOW });
    expect(item.status).toBe("yellow");
  });
  it("returns red when query throws", async () => {
    const pool = { query: vi.fn(async () => { throw new Error("table not found"); }) } as any;
    const item = await verifyPR2Audit({ pool, now: NOW });
    expect(item.status).toBe("red");
  });
});

describe("verifyMonitoring()", () => {
  const origFetch = globalThis.fetch;
  afterEach(() => { globalThis.fetch = origFetch; });

  it("returns green when both healthy", async () => {
    globalThis.fetch = vi.fn(async () => new Response(null, { status: 200 })) as any;
    const item = await verifyMonitoring({ promUrl: "http://p", grafanaUrl: "http://g", now: NOW });
    expect(item.status).toBe("green");
  });
  it("returns yellow when one healthy", async () => {
    let calls = 0;
    globalThis.fetch = vi.fn(async () => {
      calls++;
      if (calls === 1) return new Response(null, { status: 200 });
      return new Response(null, { status: 503 });
    }) as any;
    const item = await verifyMonitoring({ promUrl: "http://p", grafanaUrl: "http://g", now: NOW });
    expect(item.status).toBe("yellow");
  });
  it("returns red when both fail", async () => {
    globalThis.fetch = vi.fn(async () => { throw new Error("ECONNREFUSED"); }) as any;
    const item = await verifyMonitoring({ promUrl: "http://p", grafanaUrl: "http://g", now: NOW });
    expect(item.status).toBe("red");
  });
});

describe("verifyRunbook()", () => {
  let tmpDir: string;
  beforeEach(async () => { tmpDir = await fs.mkdtemp(path.join(os.tmpdir(), "rb-")); });
  afterEach(async () => { await fs.rm(tmpDir, { recursive: true, force: true }); });

  it("returns yellow when dir missing", async () => {
    const item = await verifyRunbook({ dir: "/nonexistent", now: NOW });
    expect(item.status).toBe("yellow");
  });
  it("returns red when no .md files", async () => {
    await fs.writeFile(path.join(tmpDir, "README.txt"), "x");
    const item = await verifyRunbook({ dir: tmpDir, now: NOW });
    expect(item.status).toBe("red");
  });
  it("returns yellow when fewer than min .md files", async () => {
    await fs.writeFile(path.join(tmpDir, "a.md"), "x");
    const item = await verifyRunbook({ dir: tmpDir, minCount: 3, now: NOW });
    expect(item.status).toBe("yellow");
  });
  it("returns green when ≥ min .md files", async () => {
    for (const n of ["a.md", "b.md", "c.md"]) await fs.writeFile(path.join(tmpDir, n), "x");
    const item = await verifyRunbook({ dir: tmpDir, minCount: 3, now: NOW });
    expect(item.status).toBe("green");
  });
});

describe("verifyGSIM1()", () => {
  const SIM = "http://sim-ctl.test";
  const PROM = "http://prom.test";
  // /capabilities shape per FASE 1 (sim-ctl src/capabilities.rs): 4 modules
  // is the real simulator-v2::capabilities() set.
  const CAPS = {
    simulator_backend: "v2",
    build: { sha: "abc1234", features: [], revm_version: "19.0.0", alloy_primitives_version: "0.7.7" },
    modules: ["bellman_ford", "lazy_db", "revm_runner", "sequence_runner"],
    dispatch_gate: { env: "ARBX_USE_SIMULATOR_V2", active: true },
    fork_suite: null,
  };
  // The closed 7-key checklist (directive 2026-08-16) — mirror of
  // G_SIM_1_ITEM_KEYS in routes/readiness-evidence.ts.
  const KEYS = [
    "unit_tests",
    "modules_merged",
    "fork_suite",
    "variance_benchmark",
    "dep_tree",
    "eth_callbundle_staging",
    "second_signoff",
  ];
  // Registry rows: all evidenced + fresh (NOW − 1 day). NOW is
  // 2026-05-01T12:00:00Z; the strict freshness boundary is 2026-04-01.
  function freshRows(overrides: Record<string, string> = {}) {
    return KEYS.map((k) => ({
      item_key: k,
      status: "evidenced",
      verified_at: overrides[k] ?? "2026-04-30T12:00:00.000Z",
    }));
  }
  function mockPool(rows: object[]) {
    return { query: vi.fn(async () => ({ rows })) } as any;
  }

  let origReady: string | undefined;
  let origFetch: typeof globalThis.fetch;

  beforeEach(() => {
    origReady = process.env["ARBX_SIMULATOR_V2_READY"];
    origFetch = globalThis.fetch;
  });
  afterEach(() => {
    if (origReady === undefined) delete process.env["ARBX_SIMULATOR_V2_READY"];
    else process.env["ARBX_SIMULATOR_V2_READY"] = origReady;
    globalThis.fetch = origFetch;
  });

  // fetch stub: /health → ok|throw; /capabilities → caps JSON | throw | non-ok;
  // prom count()/increase() → sample strings. promThrows proves the prom query
  // is NOT reached (layer-2 red short-circuits before any metric lookup).
  function stubFetch(opts: {
    healthOk: boolean;
    caps?: object | null;
    capsThrow?: boolean;
    count?: string;
    increase?: string;
    promThrows?: boolean;
  }) {
    globalThis.fetch = vi.fn(async (url: string) => {
      const u = String(url);
      if (u.includes("/health")) {
        if (!opts.healthOk) throw new Error("ECONNREFUSED");
        return { ok: true, json: async () => ({}) };
      }
      if (u.includes("/capabilities")) {
        if (opts.capsThrow) throw new Error("ECONNREFUSED");
        if (opts.caps === null) return { ok: false, json: async () => ({}) };
        return { ok: true, json: async () => opts.caps ?? CAPS };
      }
      if (opts.promThrows) throw new Error("prom should not be queried for a red layer-2 verdict");
      if (u.includes("count(")) {
        return { ok: true, json: async () => ({ data: { result: [{ value: [0, opts.count ?? "0"] }] } }) };
      }
      return { ok: true, json: async () => ({ data: { result: [{ value: [0, opts.increase ?? "0"] }] } }) };
    }) as any;
  }

  // (e) layer-1 unchanged.
  it("(e) returns red when sim-ctl /health is unreachable", async () => {
    stubFetch({ healthOk: false });
    const item = await verifyGSIM1({ simCtlUrl: SIM, promUrl: PROM, pool: null, now: NOW });
    expect(item.status).toBe("red");
    expect(item.reason).toMatch(/unreachable/);
    expect(item.reason).toMatch(/bypass simulation/);
  });

  // (a) directive: truthful IMPLEMENTADO language from the real topology.
  it("(a) flag=false + 4 modules + 0/7 evidenced → red with IMPLEMENTADO language, no 'stub', prom never queried", async () => {
    process.env["ARBX_SIMULATOR_V2_READY"] = "false";
    stubFetch({ healthOk: true, caps: CAPS, promThrows: true });
    const item = await verifyGSIM1({ simCtlUrl: SIM, promUrl: PROM, pool: mockPool([]), now: NOW });
    expect(item.status).toBe("red");
    expect(item.reason).toMatch(/IMPLEMENTADO/);
    expect(item.reason).toMatch(/4 módulos/);
    expect(item.reason).toMatch(/backend v2 disponible/);
    expect(item.reason).toMatch(/0\/7/);
    expect(item.reason).toContain("unit_tests"); // pending keys named
    expect(item.reason).not.toMatch(/stub/);
  });

  // (b) checklist complete but flag still false: red, and the pending list is
  // EMPTY — no "pendiente" wording, checklist-resolved language instead.
  it("(b) flag=false + 7/7 fresh evidence → red with EMPTY pending list (checklist resolved, flag is the only step left)", async () => {
    process.env["ARBX_SIMULATOR_V2_READY"] = "false";
    stubFetch({ healthOk: true, caps: CAPS, promThrows: true });
    const item = await verifyGSIM1({ simCtlUrl: SIM, promUrl: PROM, pool: mockPool(freshRows()), now: NOW });
    expect(item.status).toBe("red");
    expect(item.reason).toMatch(/7\/7/);
    expect(item.reason).toMatch(/hard blocker de checklist resuelto/);
    expect(item.reason).toMatch(/ARBX_SIMULATOR_V2_READY=false/);
    expect(item.reason).not.toMatch(/pendiente/);
    expect(item.reason).not.toMatch(/hasta completar/);
  });

  // (c) flag=true + full fresh checklist + prometheus flow → green (layer 3).
  it("(c) flag=true + 7/7 fresh + simulations flowing → green", async () => {
    process.env["ARBX_SIMULATOR_V2_READY"] = "true";
    stubFetch({ healthOk: true, caps: CAPS, count: "1", increase: "42" });
    const item = await verifyGSIM1({ simCtlUrl: SIM, promUrl: PROM, pool: mockPool(freshRows()), now: NOW });
    expect(item.status).toBe("green");
    expect(item.reason).toMatch(/42 simulations/);
  });

  // Layer 3 yellow (idle market) still applies with a complete checklist.
  it("returns yellow when flag=true + 7/7 fresh but no recent samples (idle market)", async () => {
    process.env["ARBX_SIMULATOR_V2_READY"] = "true";
    stubFetch({ healthOk: true, caps: CAPS, count: "0", increase: "0" });
    const item = await verifyGSIM1({ simCtlUrl: SIM, promUrl: PROM, pool: mockPool(freshRows()), now: NOW });
    expect(item.status).toBe("yellow");
    expect(item.reason).toMatch(/idle|no recent samples/);
  });

  // FIX regression (capital-gate integrity): series presence alone must
  // NEVER yield green. arbx_simulation_total is a lazily-created counter
  // child that persists on every scrape, so series_present=1 with
  // recent_count=0 is zero flow → yellow ("alive but quiet"), never green.
  it("series_present=1 + recent_count=0 → yellow, never green without flow", async () => {
    process.env["ARBX_SIMULATOR_V2_READY"] = "true";
    stubFetch({ healthOk: true, caps: CAPS, count: "1", increase: "0" });
    const item = await verifyGSIM1({ simCtlUrl: SIM, promUrl: PROM, pool: mockPool(freshRows()), now: NOW });
    expect(item.status).toBe("yellow");
    expect(item.reason).toMatch(/alive but quiet/);
    expect(item.reason).not.toMatch(/simulations in last 24h/);
  });

  // FIX regression (NaN): a malformed Prometheus value string ("NaN") must
  // fall back to 0 → yellow. Under parseInt/parseFloat the NaN broke every
  // ===0 guard (NaN === 0 is false) and produced a GREEN with "NaN
  // simulations in last 24h".
  it("Prometheus value string 'NaN' → falls to 0 → yellow, never green", async () => {
    process.env["ARBX_SIMULATOR_V2_READY"] = "true";
    stubFetch({ healthOk: true, caps: CAPS, count: "NaN", increase: "NaN" });
    const item = await verifyGSIM1({ simCtlUrl: SIM, promUrl: PROM, pool: mockPool(freshRows()), now: NOW });
    expect(item.status).toBe("yellow");
    expect(item.reason).toMatch(/no recent samples/);
    expect(item.reason).not.toMatch(/NaN/);
  });

  // (d) 31-day-old evidenced item: pending AND flagged stale.
  it("(d) an evidenced item 31 days old counts as pending and is flagged stale", async () => {
    process.env["ARBX_SIMULATOR_V2_READY"] = "false";
    stubFetch({ healthOk: true, caps: CAPS, promThrows: true });
    const item = await verifyGSIM1({
      simCtlUrl: SIM, promUrl: PROM,
      pool: mockPool(freshRows({ fork_suite: "2026-03-31T12:00:00.000Z" })),
      now: NOW,
    });
    expect(item.status).toBe("red");
    expect(item.reason).toMatch(/6\/7/);
    expect(item.reason).toMatch(/hard blocker hasta completar: \[fork_suite/);
    expect(item.reason).toMatch(/stale \(>30d\): fork_suite/);
  });

  // (f) CONTRACT: with modules.length >= 1 the language NEVER says "stub".
  it("(f) CONTRACT flag=false branch: modules >= 1 → reason never says 'stub'", async () => {
    process.env["ARBX_SIMULATOR_V2_READY"] = "false";
    stubFetch({ healthOk: true, caps: CAPS, promThrows: true });
    const item = await verifyGSIM1({ simCtlUrl: SIM, promUrl: PROM, pool: mockPool([]), now: NOW });
    expect(item.status).toBe("red");
    expect(item.reason).not.toMatch(/stub/);
  });

  it("(f) CONTRACT flag=true premature branch: modules >= 1 → SECURE_BOOT red, never 'stub'", async () => {
    process.env["ARBX_SIMULATOR_V2_READY"] = "true";
    stubFetch({ healthOk: true, caps: CAPS, promThrows: true });
    const item = await verifyGSIM1({ simCtlUrl: SIM, promUrl: PROM, pool: mockPool([]), now: NOW });
    expect(item.status).toBe("red");
    expect(item.reason).toMatch(/premature flag — SECURE_BOOT violated/);
    expect(item.reason).toContain("unit_tests"); // missing keys named
    expect(item.reason).not.toMatch(/stub/);
  });

  // (g) capabilities unreachable while /health alive: flag-only truthful
  // reasoning, capabilities unavailability noted, no enumeration, no "stub".
  it("(g) capabilities unreachable + health alive + flag=false → red, notes capabilities unavailable, no module enumeration, no 'stub'", async () => {
    process.env["ARBX_SIMULATOR_V2_READY"] = "false";
    stubFetch({ healthOk: true, capsThrow: true, promThrows: true });
    const item = await verifyGSIM1({ simCtlUrl: SIM, promUrl: PROM, pool: mockPool([]), now: NOW });
    expect(item.status).toBe("red");
    expect(item.reason).toMatch(/\/capabilities unavailable/);
    expect(item.reason).not.toMatch(/\d+ módulos/); // never enumerate a topology it could not read
    expect(item.reason).not.toMatch(/IMPLEMENTADO/);
    expect(item.reason).not.toMatch(/stub/);
  });

  // (h) pool null: 0/7 and the reason SAYS the registry is unavailable —
  // never a silent 0/7 of a possibly-healthy registry.
  it("(h) pool null → 0/7 evidenced with the reason stating the registry is unavailable", async () => {
    process.env["ARBX_SIMULATOR_V2_READY"] = "false";
    stubFetch({ healthOk: true, caps: CAPS, promThrows: true });
    const item = await verifyGSIM1({ simCtlUrl: SIM, promUrl: PROM, pool: null, now: NOW });
    expect(item.status).toBe("red");
    expect(item.reason).toMatch(/0\/7/);
    expect(item.reason).toMatch(/readiness_evidence registry unavailable/);
  });

  it("registry query error → 0/7 with the reason stating the registry is unreadable", async () => {
    process.env["ARBX_SIMULATOR_V2_READY"] = "false";
    stubFetch({ healthOk: true, caps: CAPS, promThrows: true });
    const pool = { query: vi.fn(async () => { throw new Error('relation "readiness_evidence" does not exist'); }) } as any;
    const item = await verifyGSIM1({ simCtlUrl: SIM, promUrl: PROM, pool, now: NOW });
    expect(item.status).toBe("red");
    expect(item.reason).toMatch(/0\/7/);
    expect(item.reason).toMatch(/readiness_evidence registry unreadable/);
  });

  // Kept hard-blocker property: flag=false is red BEFORE any Prometheus
  // lookup, even while simulations are flowing (structural, not traffic-based).
  it("returns red on flag=false even while simulations are flowing", async () => {
    process.env["ARBX_SIMULATOR_V2_READY"] = "false";
    stubFetch({ healthOk: true, caps: CAPS, count: "1", increase: "42" });
    const item = await verifyGSIM1({ simCtlUrl: SIM, promUrl: PROM, pool: mockPool([]), now: NOW });
    expect(item.status).toBe("red");
    expect(item.reason).toMatch(/ARBX_SIMULATOR_V2_READY=false|hard blocker/);
  });

  // The word "stub" remains permitted ONLY when /capabilities reports
  // backend v1 (or the caps fetch itself failed) — honest degraded language.
  it("flag=false with /capabilities reporting backend v1 → honest degraded language may say stub", async () => {
    process.env["ARBX_SIMULATOR_V2_READY"] = "false";
    stubFetch({ healthOk: true, caps: { simulator_backend: "v1", modules: [] }, promThrows: true });
    const item = await verifyGSIM1({ simCtlUrl: SIM, promUrl: PROM, pool: mockPool([]), now: NOW });
    expect(item.status).toBe("red");
    expect(item.reason).toMatch(/stub/);
    expect(item.reason).not.toMatch(/IMPLEMENTADO/);
  });

  // FIX (stub-permission directive): backend v2 with an empty modules field
  // is a self-INCONSISTENT payload — truthful inconsistency language, red,
  // no "stub", no module enumeration.
  it("backend v2 + modules [] → red with inconsistency wording and NO 'stub'", async () => {
    process.env["ARBX_SIMULATOR_V2_READY"] = "false";
    stubFetch({ healthOk: true, caps: { simulator_backend: "v2", modules: [] }, promThrows: true });
    const item = await verifyGSIM1({ simCtlUrl: SIM, promUrl: PROM, pool: mockPool([]), now: NOW });
    expect(item.status).toBe("red");
    expect(item.reason).toMatch(/inconsistencia en \/capabilities/);
    expect(item.reason).toMatch(/backend v2 con 0 módulos/);
    expect(item.reason).not.toMatch(/stub/);
    expect(item.reason).not.toMatch(/IMPLEMENTADO/);
  });

  it("uses the capabilitiesPath override for the /capabilities fetch", async () => {
    process.env["ARBX_SIMULATOR_V2_READY"] = "true";
    const urls: string[] = [];
    globalThis.fetch = vi.fn(async (url: unknown) => {
      const u = String(url);
      urls.push(u);
      if (u.includes("/health")) return { ok: true, json: async () => ({}) };
      if (u.includes("/caps-custom")) return { ok: true, json: async () => CAPS };
      if (u.includes("count(")) return { ok: true, json: async () => ({ data: { result: [{ value: [0, "1"] }] } }) };
      return { ok: true, json: async () => ({ data: { result: [{ value: [0, "42"] }] } }) };
    }) as any;
    const item = await verifyGSIM1({
      simCtlUrl: SIM, promUrl: PROM, capabilitiesPath: "/caps-custom",
      pool: mockPool(freshRows()), now: NOW,
    });
    expect(item.status).toBe("green");
    expect(urls).toContain(`${SIM}/caps-custom`);
  });
});

describe("verifyAll() integration", () => {
  it("returns 18 items + summary; flip_blocked=true when any non-green", async () => {
    // Audit re-run #2 (2026-05-10): all verifiers do real work; no pending
    // sentinels remain. 17 → 18 on 2026-08-29: G-DISK-1 (host disk usage,
    // ARBX-GDISK1) joined the gate after the 100%-disk incident crash-looped
    // postgres for hours. With fetch stubbed and pool=null the network/DB
    // layers fail soft to yellow, so we only assert shape + flip-blocked,
    // not the precise mix of statuses.
    const origFetch = globalThis.fetch;
    globalThis.fetch = vi.fn(async () => { throw new Error("ECONNREFUSED"); }) as any;
    try {
      const report = await verifyAll({ pool: null, now: NOW });
      expect(report.items.length).toBe(18);
      expect(report.summary.total).toBe(18);
      // G-DISK-1 (ARBX-GDISK1) is wired into the rollup.
      expect(report.items.some((i) => i.id === "G-DISK-1")).toBe(true);
      // No pending sentinels left — every gate is verified live.
      expect(report.summary.pending).toBe(0);
      expect(report.flip_blocked).toBe(true);
    } finally {
      globalThis.fetch = origFetch;
    }
  });
});

describe("verifyAll() wires the pg pool into verifyGSIM1", () => {
  const origFetch = globalThis.fetch;
  afterEach(() => { globalThis.fetch = origFetch; });

  // G-SIM-1 only reaches the readiness_evidence SELECT after sim-ctl
  // /health answers ok, so every fetch answers 200 with an empty JSON body
  // (the caps shape only affects the reason wording, not whether the
  // registry query runs).
  it("runs the readiness_evidence SELECT against the pool passed to verifyAll", async () => {
    const queries: Array<{ text: string; values: unknown[] }> = [];
    const pool = {
      query: vi.fn(async (text: string, values?: unknown[]) => {
        queries.push({ text: String(text), values: values ?? [] });
        return { rows: [] };
      }),
    } as any;
    globalThis.fetch = vi.fn(async () => new Response("{}", { status: 200 })) as any;
    try {
      await verifyAll({ pool, now: NOW });
      const evidence = queries.find((q) => q.text.includes("FROM readiness_evidence"));
      expect(evidence).toBeDefined();
      expect(evidence?.values).toEqual(["G-SIM-1"]);
    } finally {
      globalThis.fetch = origFetch;
    }
  });
});

describe("verifyAll() wires V_AT_1_PROBE_URL to the V-AT-1 probe", () => {
  const origFetch = globalThis.fetch;
  let origProbeUrl: string | undefined;

  beforeEach(() => { origProbeUrl = process.env["V_AT_1_PROBE_URL"]; });
  afterEach(() => {
    if (origProbeUrl === undefined) delete process.env["V_AT_1_PROBE_URL"];
    else process.env["V_AT_1_PROBE_URL"] = origProbeUrl;
    globalThis.fetch = origFetch;
  });

  // Records every requested URL and answers 400 (V-AT-1 green posture).
  function recordingFetch(urls: string[]) {
    globalThis.fetch = vi.fn(async (url: unknown) => {
      urls.push(String(url));
      return new Response(null, { status: 400 });
    }) as any;
  }

  it("probes the URL from V_AT_1_PROBE_URL when set", async () => {
    process.env["V_AT_1_PROBE_URL"] = "http://vat1-env-test/admin/session";
    const urls: string[] = [];
    recordingFetch(urls);
    await verifyAll({ pool: null, now: NOW });
    expect(urls).toContain("http://vat1-env-test/admin/session");
  });

  it("probes the compose-internal edge URL when V_AT_1_PROBE_URL is unset", async () => {
    delete process.env["V_AT_1_PROBE_URL"];
    const urls: string[] = [];
    recordingFetch(urls);
    await verifyAll({ pool: null, now: NOW });
    expect(urls).toContain("http://edge:8787/admin/session");
  });

  it("falls back to the default edge URL when V_AT_1_PROBE_URL is empty or whitespace", async () => {
    process.env["V_AT_1_PROBE_URL"] = "  ";
    const urls: string[] = [];
    recordingFetch(urls);
    await verifyAll({ pool: null, now: NOW });
    expect(urls).toContain("http://edge:8787/admin/session");
  });
});
