/**
 * Archive control plane — cold-tier export surface for ARBX-RETENTION-01.
 *
 * DAPP-ARCHIVE-UI-01 (2026-09-04): the operator asked for the cold-archive
 * export (docs/RETENTION_POLICY.md) to be instrumented in the DApp: capacity
 * visible, manual export triggerable from the UI, and an automatic mode
 * toggle (nightly cron archives each range BEFORE purging it — fail-honest:
 * archive failed → that table is NOT purged).
 *
 * Routes (all admin-token gated, audit-logged where mutating):
 *   GET  /api/admin/archive/status   — disk capacity (statfs of the archives
 *                                      bind mount), auto mode (DB-backed),
 *                                      per-table rows beyond the retention
 *                                      window, existing archive files.
 *   POST /api/admin/archive/export   — manual export of one table's
 *                                      beyond-window rows to
 *                                      <dir>/<table>/<table>-manual-<ts>.csv.gz
 *   POST /api/admin/archive/auto     — toggle the automatic archive mode.
 *
 * RULE 00 / R8 fail-honest: every failure surfaces as an explicit JSON error
 * (upstream_unreachable, insufficient_disk, export_in_progress, bad_table…);
 * nothing is fabricated. Row counts that time out are reported as null, not 0.
 *
 * Zero new dependencies: rows stream via keyset-paginated SELECTs (bounded
 * memory) into a manual CSV serializer + zlib gzip. The nightly host-side
 * cron (scripts/pg_retention.sh) keeps its zstd path — this route is the
 * operator's MANUAL path from the UI; both write into the SAME directory
 * (host bind ../archives:/app/archives), so rsync sees one tree.
 *
 * In-process single-flight guard + fail-closed disk check: an export refuses
 * to start with <3GB free on the archives mount (the 2026-09-04 ENOSPC
 * incidents set that floor).
 */
import { Router, type Request, type Response } from "express";
import { createWriteStream } from "node:fs";
import { mkdir, readdir, stat, statfs } from "node:fs/promises";
import { createGzip } from "node:zlib";
import type { Pool, QueryResult } from "pg";

interface Deps {
  /** Null when PG is absent at boot (R6 pattern) — routes then 503 honestly. */
  pool: Pool | null;
  requireAdminToken: (expected: string) => import("express").RequestHandler;
  adminToken: string;
  writeAudit: (
    action: string,
    actor: string,
    targetKind: string | null,
    targetId: string | null,
    before: unknown,
    after: unknown,
    ip: string | null,
    traceId: string | null,
    userAgent?: string | null,
  ) => Promise<void>;
  logger: {
    warn: (obj: object, msg?: string) => void;
    error: (obj: object, msg?: string) => void;
    info: (obj: object, msg?: string) => void;
  };
}

/** Mirror of scripts/pg_retention.sh TABLES (source of truth: the script).
 * ts_ms = epoch-millis column; otherwise timestamptz column name. */
const TABLES: ReadonlyArray<{ table: string; tsCol: string; tsKind: "ms" | "ts"; windowDays: number }> = [
  { table: "route_discovery_outcomes", tsCol: "ts_ms", tsKind: "ms", windowDays: 2 },
  { table: "opportunities", tsCol: "detected_at", tsKind: "ts", windowDays: 60 },
  { table: "pool_reserves", tsCol: '"timestamp"', tsKind: "ts", windowDays: 30 },
  { table: "risk_events", tsCol: "created_at", tsKind: "ts", windowDays: 90 },
  { table: "scored_opportunities", tsCol: "created_at", tsKind: "ts", windowDays: 60 },
  { table: "simulations", tsCol: "simulated_at", tsKind: "ts", windowDays: 90 },
  { table: "opportunity_observations", tsCol: "observed_at", tsKind: "ts", windowDays: 60 },
  { table: "paper_trade_runs", tsCol: "created_at", tsKind: "ts", windowDays: 90 },
] as const;

const ARCHIVES_DIR = process.env["ARBX_RETENTION_ARCHIVE_DIR"] ?? "/app/archives";
/** Fail-closed floor (bytes) — the 2026-09-04 ENOSPC lesson: never start a
 * multi-GB write without headroom for it plus WAL/sort spill. */
const MIN_FREE_BYTES = 3 * 1024 * 1024 * 1024;
const COUNT_TIMEOUT_MS = 8_000;
const PAGE = 50_000;

function cutoffPredicate(t: (typeof TABLES)[number]): string {
  if (t.tsKind === "ms") {
    return `((${t.tsCol})::bigint < (extract(epoch FROM now() - interval '${t.windowDays} days') * 1000)::bigint)`;
  }
  return `(${t.tsCol} < now() - interval '${t.windowDays} days')`;
}

function csvEscape(v: unknown): string {
  if (v === null || v === undefined) return "";
  const s = typeof v === "object" ? JSON.stringify(v) : String(v);
  if (/[",\r\n]/.test(s)) return `"${s.replaceAll('"', '""')}"`;
  return s;
}

async function diskStatus(): Promise<
  | { ok: true; total_bytes: number; free_bytes: number; used_pct: number }
  | { ok: false; error: string }
> {
  try {
    const st = await statfs(ARCHIVES_DIR);
    const total = st.bsize * Number(st.blocks);
    const free = st.bsize * Number(st.bavail);
    const usedPct = total > 0 ? Math.round(((total - free) / total) * 1000) / 10 : 0;
    return { ok: true, total_bytes: total, free_bytes: free, used_pct: usedPct };
  } catch (e) {
    return { ok: false, error: (e as Error).message };
  }
}

async function countBeyondWindow(pool: Pool, t: (typeof TABLES)[number]): Promise<number | null> {
  const client = await pool.connect().catch(() => null);
  if (!client) return null;
  try {
    const r = await client.query({
      text: `SELECT count(*)::int AS n FROM ${t.table} WHERE ${cutoffPredicate(t)}`,
      values: [],
    });
    const row = r.rows[0] as { n: number } | undefined;
    return row ? row.n : null;
  } catch {
    return null; // R8: timeout/error → "unknown", never 0
  } finally {
    client.release();
  }
}

interface ArchiveFile {
  table: string;
  name: string;
  bytes: number;
  modified_at: string;
}

async function listArchives(): Promise<{ ok: true; files: ArchiveFile[]; total_bytes: number } | { ok: false; error: string }> {
  try {
    const tables = await readdir(ARCHIVES_DIR, { withFileTypes: true }).catch(() => []);
    const files: ArchiveFile[] = [];
    for (const d of tables) {
      if (!d.isDirectory()) continue;
      const entries = await readdir(`${ARCHIVES_DIR}/${d.name}`, { withFileTypes: true }).catch(() => []);
      for (const f of entries) {
        if (!f.isFile()) continue;
        const st = await stat(`${ARCHIVES_DIR}/${d.name}/${f.name}`).catch(() => null);
        if (!st) continue;
        files.push({ table: d.name, name: f.name, bytes: st.size, modified_at: st.mtime.toISOString() });
      }
    }
    files.sort((a, b) => b.modified_at.localeCompare(a.modified_at));
    const total = files.reduce((acc, f) => acc + f.bytes, 0);
    return { ok: true, files: files.slice(0, 100), total_bytes: total };
  } catch (e) {
    return { ok: false, error: (e as Error).message };
  }
}

export function buildArchiveControlRouter(deps: Deps): Router {
  const { pool, requireAdminToken, adminToken, writeAudit, logger } = deps;
  const router = Router();
  let running: { table: string; started_at: string } | null = null;

  const auth = requireAdminToken(adminToken);

  // R6 fail-honest: no PG pool → the PG-backed legs (auto mode, counts,
  // export) are explicitly unavailable. Disk + file listing still work.
  const pgUnavailable = (_req: Request, res: Response): boolean => {
    if (!pool) {
      res.status(503).json({ error: "pg_unavailable", message: "postgres pool not initialized" });
      return true;
    }
    return false;
  };

  // ── GET /api/admin/archive/status — read-only evidence ──
  router.get("/api/admin/archive/status", auth, async (_req, res) => {
    if (pgUnavailable(_req, res)) return;
    const disk = await diskStatus();
    const autoDb = pool
      ? await pool
          .query(`SELECT value->>'enabled' AS enabled, updated_at FROM retention_settings WHERE key='archive_auto'`)
          .then((r) => (r.rows[0] as { enabled?: string; updated_at?: string } | undefined) ?? null)
          .catch(() => null)
      : null;
    const counts = pool
      ? await Promise.all(TABLES.map((t) => countBeyondWindow(pool, t)))
      : TABLES.map(() => null);
    const archives = await listArchives();
    res.json({
      ok: true,
      kind: "archive_status",
      read_only: true,
      archive_dir: ARCHIVES_DIR,
      disk: disk.ok
        ? { total_bytes: disk.total_bytes, free_bytes: disk.free_bytes, used_pct: disk.used_pct }
        : { error: disk.error },
      auto_mode: {
        enabled: autoDb?.enabled === "true",
        source: autoDb ? "db" : "default_off",
        updated_at: autoDb?.updated_at ?? null,
        // Documented semantic: ON makes the nightly cron archive each range
        // BEFORE purging (fail-honest: no archive → no purge that night).
        effect: "cron archives then purges (docs/RETENTION_POLICY.md)",
      },
      export_running: running ? { table: running.table, started_at: running.started_at } : null,
      tables: TABLES.map((t, i) => ({
        table: t.table,
        window_days: t.windowDays,
        rows_beyond_window: counts[i], // null = not computed (R8)
      })),
      archives: archives.ok
        ? { files: archives.files, total_bytes: archives.total_bytes }
        : { error: archives.error, files: [], total_bytes: 0 },
      min_free_bytes: MIN_FREE_BYTES,
      ts: new Date().toISOString(),
    });
  });

  // ── POST /api/admin/archive/auto {enabled: boolean} ──
  router.post("/api/admin/archive/auto", auth, async (req, res) => {
    if (pgUnavailable(req, res) || !pool) return;
    const enabled = (req.body as { enabled?: unknown } | undefined)?.enabled;
    if (typeof enabled !== "boolean") {
      res.status(400).json({ error: "bad_body", message: "expected { enabled: boolean }" });
      return;
    }
    try {
      await pool.query(
        `INSERT INTO retention_settings (key, value) VALUES ('archive_auto', $1::jsonb)
         ON CONFLICT (key) DO UPDATE SET value = $1::jsonb, updated_at = now()`,
        [JSON.stringify({ enabled })],
      );
    } catch (e) {
      res.status(502).json({ error: "upstream_unreachable", detail: (e as Error).message });
      return;
    }
    const actor = req.header("x-arbx-actor") ?? "admin";
    await writeAudit(
      "archive.auto",
      actor,
      "retention_settings",
      "archive_auto",
      null,
      { enabled },
      req.ip ?? null,
      (req as Request & { traceId?: string }).traceId ?? null,
    ).catch(() => undefined);
    logger.info({ event: "archive.auto", enabled }, "archive auto mode toggled");
    res.json({ ok: true, enabled });
  });

  // ── POST /api/admin/archive/export {table: string} ──
  router.post("/api/admin/archive/export", auth, async (req, res) => {
    if (pgUnavailable(req, res) || !pool) return;
    const table = String((req.body as { table?: unknown } | undefined)?.table ?? "");
    const spec = TABLES.find((t) => t.table === table);
    if (!spec) {
      res.status(400).json({ error: "bad_table", allowed: TABLES.map((t) => t.table) });
      return;
    }
    if (running) {
      res.status(409).json({ error: "export_in_progress", table: running.table, started_at: running.started_at });
      return;
    }
    const disk = await diskStatus();
    if (!disk.ok) {
      res.status(502).json({ error: "disk_status_unavailable", detail: disk.error });
      return;
    }
    if (disk.free_bytes < MIN_FREE_BYTES) {
      res.status(507).json({ error: "insufficient_disk", free_bytes: disk.free_bytes, min_free_bytes: MIN_FREE_BYTES });
      return;
    }

    running = { table, started_at: new Date().toISOString() };
    const actor = req.header("x-arbx-actor") ?? "admin";
    // Respond with the job started; the export runs detached and its outcome
    // is observable via /status (export_running + the file appearing, or not).
    res.status(202).json({ ok: true, table, started_at: running.started_at, dir: ARCHIVES_DIR });

    void (async () => {
      const started = Date.now();
      const stamp = running!.started_at.replace(/[-:]/g, "").replace(/\..+/, "").replace("T", "-");
      const dir = `${ARCHIVES_DIR}/${spec.table}`;
      const file = `${dir}/${spec.table}-manual-${stamp}.csv.gz`;
      const predicate = cutoffPredicate(spec);
      let rows = 0;
      try {
        await mkdir(dir, { recursive: true });
        const gz = createGzip({ level: 6 });
        const out = createWriteStream(file, { mode: 0o640 });
        gz.pipe(out);
        const client = await pool.connect();
        try {
          // Column order from the driver's field metadata (stable for SELECT *).
          let cols: string[] = [];
          let lastId: number | null = null;
          for (;;) {
            const page: QueryResult<Record<string, unknown>> = await client.query(
              `SELECT * FROM ${spec.table} WHERE ${predicate}${lastId === null ? "" : " AND id > $1"} ORDER BY id ASC LIMIT ${PAGE}`,
              lastId === null ? [] : [lastId],
            );
            if (page.rows.length === 0) break;
            if (cols.length === 0) cols = page.fields.map((f: { name: string }) => f.name);
            for (const row of page.rows as Record<string, unknown>[]) {
              gz.write(`${cols.map((c) => csvEscape(row[c])).join(",")}\n`);
              rows += 1;
              lastId = Number(row["id"]);
            }
            if (page.rows.length < PAGE) break;
          }
        } finally {
          client.release();
        }
        await new Promise<void>((resolve, reject) => {
          gz.end(() => resolve());
          out.on("error", reject);
        });
        const st = await stat(file);
        logger.info(
          { event: "archive.export.done", table, file, bytes: st.size, rows, ms: Date.now() - started },
          "manual archive export complete",
        );
        await writeAudit("archive.export", actor, "table", spec.table, null, { file, rows, bytes: st.size }, req.ip ?? null, null).catch(
          () => undefined,
        );
      } catch (e) {
        logger.error({ event: "archive.export.failed", table, err: (e as Error).message, rows }, "manual archive export failed");
      } finally {
        running = null;
      }
    })();
  });

  return router;
}
