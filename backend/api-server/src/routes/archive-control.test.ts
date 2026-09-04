/**
 * Archive control plane — unit tests (DAPP-ARCHIVE-UI-01).
 *
 * Mocks the pg pool (settings reads, counts, keyset export pages) and
 * writeAudit; the archives dir is a real temp dir so statfs/readdir/stat run
 * against the real filesystem (zero-mock on the FS leg). Mirrors the
 * service-control.test.ts harness (vitest + express + supertest + a
 * test-only requireAdminToken).
 *
 * Cases:
 *   (a) no admin token on status → 401
 *   (b) status 200 — disk present, auto mode from db, tables listed, files listed
 *   (c) count query fails → rows_beyond_window null (R8: unknown ≠ 0)
 *   (d) auto toggle bad body → 400; ok → 200 + upsert + audit
 *   (e) export bad table → 400
 *   (f) export happy path → 202, .csv.gz file materializes in the temp dir
 *   (g) export while running → 409 (single-flight)
 */
import { describe, it, expect, beforeEach, vi } from "vitest";
import express, { type Express, type RequestHandler } from "express";
import request from "supertest";
import { mkdtempSync, mkdirSync, writeFileSync, existsSync, readdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const ADMIN_TOKEN = "test-admin-token-32-bytes-of-entropy-aaaa";

/** Test-only requireAdminToken — mirrors @arbx/shared contract. */
function requireAdminToken(expected: string): RequestHandler {
  return (req, res, next) => {
    const got = req.header("x-arbx-admin-token");
    if (!got || got !== expected) {
      res.status(401).json({ error: "unauthorized", source: "admin_token" });
      return;
    }
    next();
  };
}

const writeAudit = vi.fn().mockResolvedValue(undefined);
const logger = { warn: vi.fn(), error: vi.fn(), info: vi.fn() };

/** Fake pg pool: pool.query handles retention_settings + count(*) reads;
 * pool.connect() returns a keyset client whose pages come from `pages`. */
function fakePool(opts: {
  settingsRow?: { enabled?: string; updated_at?: string } | null;
  countsFail?: boolean;
  counts?: Record<string, number>;
  pages?: Array<Array<Record<string, unknown>>>;
  hangExport?: boolean;
}) {
  const client = {
    query: vi.fn(async (q: { text: string }) => {
      if (opts.hangExport) {
        return new Promise<never>(() => {}); // never resolves → running stays true
      }
      // countBeyondWindow runs on the CONNECTED client too (not only pool.query)
      if ((q.text ?? "").includes("count(*)")) {
        if (opts.countsFail) throw new Error("statement timeout");
        const table = /FROM (\w+) WHERE/.exec(q.text ?? "")?.[1] ?? "";
        return { rows: [{ n: opts.counts?.[table] ?? 0 }] };
      }
      const pages = opts.pages ?? [];
      const page = pages.shift() ?? [];
      return {
        rows: page,
        fields: page.length > 0 ? Object.keys(page[0]!).map((name) => ({ name })) : [],
      };
    }),
    release: vi.fn(),
  };
  return {
    query: vi.fn(async (q: unknown) => {
      const text = typeof q === "string" ? q : ((q as { text?: string }).text ?? "");
      if (text.includes("retention_settings")) {
        return { rows: opts.settingsRow ? [opts.settingsRow] : [] };
      }
      if (text.includes("count(*)")) {
        if (opts.countsFail) throw new Error("statement timeout");
        const table = /FROM (\w+) WHERE/.exec(text)?.[1] ?? "";
        return { rows: [{ n: opts.counts?.[table] ?? 0 }] };
      }
      return { rows: [] };
    }),
    connect: vi.fn(async () => client),
  };
}

let tmpArchives = "";

async function buildApp(pool: ReturnType<typeof fakePool>): Promise<Express> {
  // ARCHIVES_DIR is module-level — set the env before the dynamic import so
  // each test batch can point at a fresh temp dir.
  process.env["ARBX_RETENTION_ARCHIVE_DIR"] = tmpArchives;
  vi.resetModules();
  const { buildArchiveControlRouter } = await import("./archive-control.js");
  const app = express();
  app.use(express.json());
  app.use(
    buildArchiveControlRouter({
      // biome-ignore lint/suspicious/noExplicitAny: test-only pool stand-in
      pool: pool as any,
      requireAdminToken,
      adminToken: ADMIN_TOKEN,
      writeAudit,
      logger,
    }),
  );
  return app;
}

beforeEach(() => {
  writeAudit.mockClear();
  writeAudit.mockResolvedValue(undefined);
  tmpArchives = mkdtempSync(join(tmpdir(), "arbx-archive-test-"));
});

describe("archive control plane", () => {
  it("(a) status without admin token → 401", async () => {
    const app = await buildApp(fakePool({}));
    const res = await request(app).get("/api/admin/archive/status");
    expect(res.status).toBe(401);
  });

  it("(b) status 200 — disk, auto mode, tables, files", async () => {
    // a pre-existing archive file must appear in the listing
    mkdirSync(join(tmpArchives, "opportunities"), { recursive: true });
    writeFileSync(join(tmpArchives, "opportunities", "opportunities-manual-20260904-000000.csv.gz"), "x");
    const app = await buildApp(
      fakePool({
        settingsRow: { enabled: "true", updated_at: "2026-09-04T00:00:00Z" },
        counts: { opportunities: 1234 },
      }),
    );
    const res = await request(app)
      .get("/api/admin/archive/status")
      .set("x-arbx-admin-token", ADMIN_TOKEN);
    expect(res.status).toBe(200);
    expect(res.body.kind).toBe("archive_status");
    expect(res.body.disk.total_bytes).toBeGreaterThan(0);
    expect(res.body.disk.used_pct).toBeGreaterThanOrEqual(0);
    expect(res.body.auto_mode.enabled).toBe(true);
    expect(res.body.auto_mode.source).toBe("db");
    expect(res.body.tables).toHaveLength(8);
    const opp = res.body.tables.find((t: { table: string }) => t.table === "opportunities");
    expect(opp.rows_beyond_window).toBe(1234);
    const risk = res.body.tables.find((t: { table: string }) => t.table === "risk_events");
    expect(risk.rows_beyond_window).toBe(0); // computed 0 = real zero
    expect(res.body.archives.files).toHaveLength(1);
    expect(res.body.archives.files[0].table).toBe("opportunities");
  });

  it("(c) count fails → rows_beyond_window null (R8 unknown ≠ 0)", async () => {
    const app = await buildApp(fakePool({ countsFail: true }));
    const res = await request(app)
      .get("/api/admin/archive/status")
      .set("x-arbx-admin-token", ADMIN_TOKEN);
    expect(res.status).toBe(200);
    for (const t of res.body.tables) {
      expect(t.rows_beyond_window).toBeNull();
    }
  });

  it("(d1) auto bad body → 400", async () => {
    const app = await buildApp(fakePool({}));
    const res = await request(app)
      .post("/api/admin/archive/auto")
      .set("x-arbx-admin-token", ADMIN_TOKEN)
      .send({ enabled: "yes" });
    expect(res.status).toBe(400);
    expect(res.body.error).toBe("bad_body");
  });

  it("(d2) auto ok → 200 + upsert + audit", async () => {
    const pool = fakePool({});
    const app = await buildApp(pool);
    const res = await request(app)
      .post("/api/admin/archive/auto")
      .set("x-arbx-admin-token", ADMIN_TOKEN)
      .send({ enabled: true });
    expect(res.status).toBe(200);
    expect(res.body.enabled).toBe(true);
    const upsertText = pool.query.mock.calls[0]?.[0];
    expect(String(typeof upsertText === "string" ? upsertText : upsertText?.text)).toContain(
      "INSERT INTO retention_settings",
    );
    expect(writeAudit).toHaveBeenCalledWith(
      "archive.auto",
      expect.any(String),
      "retention_settings",
      "archive_auto",
      null,
      { enabled: true },
      expect.anything(),
      null,
    );
  });

  it("(e) export bad table → 400 + allowlist", async () => {
    const app = await buildApp(fakePool({}));
    const res = await request(app)
      .post("/api/admin/archive/export")
      .set("x-arbx-admin-token", ADMIN_TOKEN)
      .send({ table: "pg_catalog.pg_tables" });
    expect(res.status).toBe(400);
    expect(res.body.error).toBe("bad_table");
    expect(res.body.allowed).toContain("opportunities");
  });

  it("(f) export happy path → 202 + .csv.gz materialized", async () => {
    const app = await buildApp(
      fakePool({
        pages: [
          [
            { id: 1, detected_at: "2026-07-01T00:00:00Z", strategy_kind: "MEV-01-001", net_profit_wei: "1000" },
            { id: 2, detected_at: "2026-07-02T00:00:00Z", strategy_kind: 'weird,"kind"', net_profit_wei: null },
          ],
          [], // empty page ends the loop
        ],
      }),
    );
    const res = await request(app)
      .post("/api/admin/archive/export")
      .set("x-arbx-admin-token", ADMIN_TOKEN)
      .send({ table: "opportunities" });
    expect(res.status).toBe(202);
    // the detached writer flushes asynchronously — poll briefly for the file
    const dir = join(tmpArchives, "opportunities");
    let ok = false;
    for (let i = 0; i < 40 && !ok; i++) {
      await new Promise((r) => setTimeout(r, 50));
      ok = existsSync(dir) && readdirSync(dir).some((f) => f.endsWith(".csv.gz"));
    }
    expect(ok).toBe(true);
  });

  it("(g) export while running → 409 single-flight", async () => {
    const app = await buildApp(fakePool({ hangExport: true }));
    const r1 = await request(app)
      .post("/api/admin/archive/export")
      .set("x-arbx-admin-token", ADMIN_TOKEN)
      .send({ table: "risk_events" });
    expect(r1.status).toBe(202);
    const r2 = await request(app)
      .post("/api/admin/archive/export")
      .set("x-arbx-admin-token", ADMIN_TOKEN)
      .send({ table: "risk_events" });
    expect(r2.status).toBe(409);
    expect(r2.body.error).toBe("export_in_progress");
  });
});
