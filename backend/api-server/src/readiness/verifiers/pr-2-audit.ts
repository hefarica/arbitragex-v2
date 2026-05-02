import type pg from "pg";
import type { ReadinessItem } from "../types.js";

/**
 * PR-2 — Audit log live + populated within last 7 days.
 *
 * Verification: SQL count over audit_log table.
 * - Table missing → red (regression: migration 011 should have created it).
 * - Reachable, count > 0 in last 7d → green.
 * - Reachable, 0 rows in last 7d → yellow ("no admin activity recently"; not necessarily bad).
 * - DB pool not configured → yellow.
 */
export async function verifyPR2Audit(opts: {
  pool: pg.Pool | null;
  now?: () => Date;
  windowDays?: number;
}): Promise<ReadinessItem> {
  const verified_at = (opts.now ?? (() => new Date()))().toISOString();
  const days = opts.windowDays ?? 7;
  const base = {
    id: "PR-2",
    group: "audit_trail" as const,
    label: "Audit log: killswitch + auth events with tamper-evident DB",
    verified_at,
  };

  if (!opts.pool) {
    return {
      ...base,
      status: "yellow",
      reason: "DATABASE_URL not configured on api-server; cannot verify audit_log",
    };
  }

  try {
    const r = await opts.pool.query(
      `SELECT COUNT(*)::int AS cnt
         FROM audit_log
        WHERE created_at > NOW() - ($1::int || ' days')::interval`,
      [days],
    );
    const count = Number(r.rows[0]?.cnt ?? 0);
    if (count === 0) {
      return {
        ...base,
        status: "yellow",
        reason: `audit_log reachable but 0 rows in last ${days}d (no admin activity yet)`,
        evidence: { kind: "db_query", ref: `SELECT COUNT(*) FROM audit_log WHERE created_at > NOW() - '${days} days'` },
      };
    }
    return {
      ...base,
      status: "green",
      reason: `${count} audit rows in last ${days}d`,
      evidence: { kind: "db_query", ref: `SELECT COUNT(*) FROM audit_log WHERE created_at > NOW() - '${days} days'` },
    };
  } catch (e) {
    const msg = (e as Error).message.split("\n")[0]?.slice(0, 200) ?? "unknown";
    return {
      ...base,
      status: "red",
      reason: `audit_log query failed: ${msg}`,
    };
  }
}
