/**
 * readiness-evidence — append-only evidence registry for gate readiness
 * checklists (G-SIM-1 FASE 2, operator directive 2026-08-16).
 *
 *   POST /admin/readiness-evidence                    (admin-gated, writes)
 *   GET  /admin/readiness-evidence?gate_id=G-SIM-1    (admin-gated, read-only)
 *
 * WHY THIS EXISTS: evidence producers (CI jobs of FASE 4) need a durable place
 * to record each checklist item with provenance. This generalizes the
 * scripts/run_a4_fork_validation.sh mechanism (INSERT into gate_c_validation
 * + marker file) with freshness + provenance:
 *   - freshness  — readers treat rows older than 30 days as NOT fresh
 *                  (computed is_fresh on GET; enforced by the FASE 3 verifier).
 *   - provenance — every row carries evidence_ref (URL) + verified_by
 *                  (ci:rust.yml | operator:<id> | reviewer:<id>).
 *
 * Append-only contract: the POST path inserts into readiness_evidence_history
 * BEFORE upserting the latest-row table, inside ONE transaction. Nothing is
 * ever deleted; the latest table is only ever overwritten forward.
 *
 * Honesty contract (RULE 00 / R8): this route records evidence producers'
 * claims — it does NOT verify them. Verification is FASE 3's job (reads PG
 * directly via its own pool). GET is a read-only convenience for operators.
 */

import type { Application, Request, RequestHandler, Response } from "express";
import type pg from "pg";
import { z } from "zod";

// ---------------------------------------------------------------------------
// Contract constants.
// ---------------------------------------------------------------------------

/** Checklist item keys the G-SIM-1 gate accepts (directive 2026-08-16). */
export const G_SIM_1_ITEM_KEYS = [
  "unit_tests",
  "modules_merged",
  "fork_suite",
  "variance_benchmark",
  "dep_tree",
  "eth_callbundle_staging",
  "second_signoff",
] as const;

/** Rows older than this are flagged is_fresh=false by the GET reader. */
export const FRESHNESS_DAYS = 30;

const ReadinessEvidenceWrite = z.object({
  gate_id: z.string().min(1).max(100),
  item_key: z.string().min(1).max(200),
  status: z.enum(["evidenced", "failed"]),
  evidence_ref: z.string().min(1).max(2000),
  detail: z.record(z.string(), z.unknown()).optional(),
  verified_by: z.string().min(1).max(200),
});

export interface ReadinessEvidenceDeps {
  pool: pg.Pool | null;
  requireAdminToken: (expected: string) => RequestHandler;
  adminToken: string;
  logger: { warn: (obj: object, msg?: string) => void };
  /** Injectable clock for tests; defaults to real time. */
  now?: () => Date;
}

interface EvidenceRow {
  gate_id: string;
  item_key: string;
  status: string;
  evidence_ref: string;
  detail: unknown;
  verified_at: Date | string;
  verified_by: string;
}

/** verified_at > now − 30 days (strict, per directive). Exported for tests. */
export function computeIsFresh(verifiedAt: Date | string, now: Date): boolean {
  const t = verifiedAt instanceof Date ? verifiedAt.getTime() : Date.parse(verifiedAt);
  if (!Number.isFinite(t)) return false;
  return t > now.getTime() - FRESHNESS_DAYS * 24 * 60 * 60 * 1000;
}

function toApiRow(row: EvidenceRow, now: Date) {
  const verifiedIso = (row.verified_at instanceof Date
    ? row.verified_at
    : new Date(row.verified_at)
  ).toISOString();
  return {
    gate_id: row.gate_id,
    item_key: row.item_key,
    status: row.status,
    evidence_ref: row.evidence_ref,
    detail: row.detail ?? null,
    verified_at: verifiedIso,
    verified_by: row.verified_by,
    is_fresh: computeIsFresh(verifiedIso, now),
  };
}

// ---------------------------------------------------------------------------
// Route mounting.
// ---------------------------------------------------------------------------

export function mountReadinessEvidence(app: Application, deps: ReadinessEvidenceDeps): void {
  const now = deps.now ?? (() => new Date());
  const auth = deps.requireAdminToken(deps.adminToken);

  app.post("/admin/readiness-evidence", auth, async (req: Request, res: Response) => {
    const parsed = ReadinessEvidenceWrite.safeParse(req.body);
    if (!parsed.success) {
      res.status(400).json({ error: "invalid_request", details: parsed.error.flatten() });
      return;
    }
    const b = parsed.data;
    // G-SIM-1 has a closed checklist; other gates may record any item key.
    if (b.gate_id === "G-SIM-1" && !(G_SIM_1_ITEM_KEYS as readonly string[]).includes(b.item_key)) {
      res.status(400).json({ error: "invalid_item_key", allowed: G_SIM_1_ITEM_KEYS });
      return;
    }
    if (!deps.pool) {
      res.status(503).json({ error: "db_unavailable" });
      return;
    }

    // One timestamp for BOTH rows so the history PK matches the latest row.
    const verifiedAt = now();
    const params = [
      b.gate_id,
      b.item_key,
      b.status,
      b.evidence_ref,
      b.detail !== undefined ? JSON.stringify(b.detail) : null,
      verifiedAt,
      b.verified_by,
    ];

    // Connect failures (DB unreachable) must fail-honest as a 5xx response:
    // under Express 4 an awaited throw outside the try/catch below would be an
    // unhandled promise rejection with no response at all.
    let client: pg.PoolClient;
    try {
      client = await deps.pool.connect();
    } catch (e) {
      deps.logger.warn({ event: "admin.readiness_evidence.connect_failed", err: (e as Error).message });
      res.status(503).json({ error: "db_unavailable", detail: (e as Error).message });
      return;
    }
    try {
      await client.query("BEGIN");
      // History FIRST — the append-only trail must never miss an accepted write.
      await client.query(
        `INSERT INTO readiness_evidence_history
           (gate_id, item_key, status, evidence_ref, detail, verified_at, verified_by)
         VALUES ($1,$2,$3,$4,$5::jsonb,$6,$7)`,
        params,
      );
      const up = await client.query(
        `INSERT INTO readiness_evidence
           (gate_id, item_key, status, evidence_ref, detail, verified_at, verified_by)
         VALUES ($1,$2,$3,$4,$5::jsonb,$6,$7)
         ON CONFLICT (gate_id, item_key) DO UPDATE SET
           status       = EXCLUDED.status,
           evidence_ref = EXCLUDED.evidence_ref,
           detail       = EXCLUDED.detail,
           verified_at  = EXCLUDED.verified_at,
           verified_by  = EXCLUDED.verified_by
         RETURNING verified_at`,
        params,
      );
      await client.query("COMMIT");
      const stored = up.rows[0]?.verified_at ?? verifiedAt;
      deps.logger.warn(
        { event: "admin.readiness_evidence.write", gate_id: b.gate_id, item_key: b.item_key, status: b.status, verified_by: b.verified_by },
        "readiness evidence recorded",
      );
      res.status(201).json({
        ok: true,
        gate_id: b.gate_id,
        item_key: b.item_key,
        verified_at: (stored instanceof Date ? stored : new Date(stored as string)).toISOString(),
      });
    } catch (e) {
      await client.query("ROLLBACK").catch(() => {});
      deps.logger.warn({ event: "admin.readiness_evidence.write_failed", err: (e as Error).message });
      res.status(500).json({ error: "readiness_evidence_write_failed", detail: (e as Error).message });
    } finally {
      client.release();
    }
  });

  app.get("/admin/readiness-evidence", auth, async (req: Request, res: Response) => {
    const gateId = typeof req.query["gate_id"] === "string" ? (req.query["gate_id"] as string).trim() : "";
    if (gateId.length === 0) {
      res.status(400).json({ error: "missing_gate_id" });
      return;
    }
    if (!deps.pool) {
      res.status(503).json({ error: "db_unavailable" });
      return;
    }
    try {
      // Latest row per item_key = max verified_at (DISTINCT ON + DESC).
      const r = await deps.pool.query(
        `SELECT DISTINCT ON (item_key)
           gate_id, item_key, status, evidence_ref, detail, verified_at, verified_by
         FROM readiness_evidence
        WHERE gate_id = $1
        ORDER BY item_key, verified_at DESC`,
        [gateId],
      );
      const at = now();
      res.status(200).json({
        gate_id: gateId,
        generated_at: at.toISOString(),
        freshness_days: FRESHNESS_DAYS,
        items: (r.rows as EvidenceRow[]).map((row) => toApiRow(row, at)),
      });
    } catch (e) {
      deps.logger.warn({ event: "admin.readiness_evidence.read_failed", err: (e as Error).message });
      res.status(500).json({ error: "readiness_evidence_query_failed", detail: (e as Error).message });
    }
  });
}
