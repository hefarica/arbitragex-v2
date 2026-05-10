import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { promises as fs } from "node:fs";
import path from "node:path";
import os from "node:os";
import { pending } from "./verifiers/pending.js";
import { verifyVDB1 } from "./verifiers/v-db-1.js";
import { verifyVAT1 } from "./verifiers/v-at-1.js";
import { verifyVNH1 } from "./verifiers/v-nh-1.js";
import { verifyPR1CSP } from "./verifiers/pr-1-csp.js";
import { verifyPR2Audit } from "./verifiers/pr-2-audit.js";
import { verifyMonitoring } from "./verifiers/monitoring.js";
import { verifyRunbook } from "./verifiers/runbook.js";
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
    const item = await verifyVNH1({ script: f, cwd: tmpDir, now: NOW });
    expect(item.status).toBe("green");
  });
  it("returns red when script exits non-zero", async () => {
    const f = path.join(tmpDir, "fail.sh");
    await fs.writeFile(f, "#!/usr/bin/env bash\necho 'leak detected'\nexit 1\n", { mode: 0o755 });
    const item = await verifyVNH1({ script: f, cwd: tmpDir, now: NOW });
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
  it("returns green when CSP + frame-ancestors none present", async () => {
    globalThis.fetch = vi.fn(async () => new Response(null, { status: 200,
      headers: { "content-security-policy-report-only": "default-src 'self'; frame-ancestors 'none'" } })) as any;
    const item = await verifyPR1CSP({ url: "http://x", now: NOW });
    expect(item.status).toBe("green");
  });
  it("returns yellow on fetch failure", async () => {
    globalThis.fetch = vi.fn(async () => { throw new Error("ECONNREFUSED"); }) as any;
    const item = await verifyPR1CSP({ url: "http://x", now: NOW });
    expect(item.status).toBe("yellow");
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

describe("verifyAll() integration", () => {
  it("returns 17 items + summary; flip_blocked=true when any non-green", async () => {
    // Audit re-run #2 (2026-05-10): all 17 verifiers now do real work; no
    // pending sentinels remain. With fetch stubbed and pool=null the
    // network/DB layers fail soft to yellow, so we only assert shape +
    // flip-blocked, not the precise mix of statuses.
    const origFetch = globalThis.fetch;
    globalThis.fetch = vi.fn(async () => { throw new Error("ECONNREFUSED"); }) as any;
    try {
      const report = await verifyAll({ pool: null, now: NOW });
      expect(report.items.length).toBe(17);
      expect(report.summary.total).toBe(17);
      // No pending sentinels left — every gate is verified live.
      expect(report.summary.pending).toBe(0);
      expect(report.flip_blocked).toBe(true);
    } finally {
      globalThis.fetch = origFetch;
    }
  });
});
