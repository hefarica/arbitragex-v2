/** Bounded scoring observations, NOT a trading/activation policy.
 * Uses migration 097's created_at index; no scan of the entire scoring ledger.
 * Small populations are exact, large populations are an explicit lower bound.
 */
import type pg from "pg";

export const SCORING_ACQUIRE_MS = 500;
export const SCORING_READ_MS = 2500;
export const SCORING_STATEMENT_MS = 1500;

export interface ScoringReader {
  query<T extends pg.QueryResultRow = pg.QueryResultRow>(text: string, values?: unknown[]): Promise<pg.QueryResult<T>>;
}

export function scoringCountLimit(minScored: number): number {
  const required = Math.ceil(minScored);
  if (!Number.isFinite(minScored) || minScored < 0 || !Number.isSafeInteger(required) || required >= Number.MAX_SAFE_INTEGER) {
    throw new Error("scoring_threshold_invalid");
  }
  // At least the operator's calibration threshold: a display cap must not
  // silently change whether that threshold has actually been reached.
  return Math.max(1000, required);
}

export const SCORING_SUMMARY_SQL = `SELECT
  (SELECT COUNT(*)::bigint FROM
    (SELECT 1 FROM scored_opportunities ORDER BY created_at DESC LIMIT $1) observed) AS total,
  (SELECT created_at FROM scored_opportunities ORDER BY created_at DESC LIMIT 1) AS last,
  (SELECT created_at FROM scored_opportunities ORDER BY created_at ASC LIMIT 1) AS first`;

export async function readScoringSummary(db: ScoringReader, minScored: number) {
  const limit = scoringCountLimit(minScored);
  // One look-ahead row distinguishes exactly limit from strictly more.
  // Count and full-history date bounds are read from ONE statement snapshot.
  const result = await db.query<{ total: string; first: Date | null; last: Date | null }>(SCORING_SUMMARY_SQL, [limit + 1]);
  const row = result.rows[0];
  if (!row) throw new Error("scoring_summary_missing");
  if (typeof row.total !== "string" || !/^[0-9]+$/.test(row.total)) throw new Error("scoring_summary_count_invalid");
  const observed = Number(row.total);
  if (!Number.isSafeInteger(observed) || observed < 0 || observed > limit + 1) {
    throw new Error("scoring_summary_count_invalid");
  }
  if ((observed === 0) !== (row.first === null && row.last === null)
      || (observed > 0 && (row.first === null || row.last === null))) {
    throw new Error("scoring_summary_bounds_invalid");
  }
  const first = row.first === null ? null : new Date(row.first).toISOString();
  const last = row.last === null ? null : new Date(row.last).toISOString();
  if (first !== null && last !== null && first > last) throw new Error("scoring_summary_bounds_invalid");
  return { count: Math.min(observed, limit), exact: observed <= limit, limit, first, last };
}

function acquire(pool: pg.Pool): Promise<pg.PoolClient> {
  return new Promise((resolve, reject) => {
    let pending = true;
    const timer = setTimeout(() => {
      pending = false;
      reject(new Error("scoring_pool_timeout"));
    }, SCORING_ACQUIRE_MS);
    // A timed-out waiter can still receive a connection later. Always release
    // that late connection, or pool exhaustion becomes permanent.
    void Promise.resolve().then(() => pool.connect()).then((client) => {
      if (!pending) { client.release(); return; }
      pending = false;
      clearTimeout(timer);
      resolve(client);
    }, (error: unknown) => {
      if (!pending) return;
      pending = false;
      clearTimeout(timer);
      reject(error);
    });
  });
}

/** Same-client READ ONLY transaction, bounded both in PostgreSQL and in Node.
 * SET LOCAL never changes shared pool defaults. On failure/expiry the physical
 * connection is destroyed (including any open transaction), never recycled.
 * The wrapper prevents a late continuation from issuing another SQL statement.
 */
export async function withScoringRead<T>(pool: pg.Pool, work: (db: ScoringReader) => Promise<T>): Promise<T> {
  const client = await acquire(pool);
  let active = true;
  let reusable = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const check = () => { if (!active) throw new Error("scoring_read_timeout"); };
  const db: ScoringReader = {
    async query<R extends pg.QueryResultRow = pg.QueryResultRow>(text: string, values?: unknown[]): Promise<pg.QueryResult<R>> {
      check();
      const result = await client.query<R>(text, values);
      check();
      return result;
    },
  };
  try {
    const deadline = new Promise<never>((_resolve, reject) => {
      timer = setTimeout(() => {
        active = false;
        reject(new Error("scoring_read_timeout"));
      }, SCORING_READ_MS);
    });
    const task = (async () => {
      await db.query("BEGIN READ ONLY");
      await db.query(`SET LOCAL lock_timeout='250ms'; SET LOCAL statement_timeout='${SCORING_STATEMENT_MS}ms'`);
      const result = await work(db);
      check();
      await db.query("COMMIT");
      return result;
    })();
    const result = await Promise.race([task, deadline]);
    reusable = true;
    return result;
  } finally {
    active = false;
    if (timer !== undefined) clearTimeout(timer);
    client.release(!reusable);
  }
}
