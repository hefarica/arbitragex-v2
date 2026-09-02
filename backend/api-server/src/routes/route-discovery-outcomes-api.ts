/**
 * Route-Discovery Outcomes API — READ-ONLY analytics over the durable
 * `route_discovery_outcomes` table (FASE B Gate-C hit-rate series; the shadow
 * emitter's resolved outcomes, with the Paso 9 `reason` column).
 *
 * This is the missing read-side for the passive sink (route-discovery-outcome-sink.ts):
 * the sink WRITES the table; nothing READ it until now. 100% read-only / NO-ACTIVE —
 * never touches arbx:opps:detected, capital, signers, or execution. R8 fail-honest:
 * pool absent -> 503; empty -> ok:true with empty data (never fabricates).
 *
 *   GET /api/v1/route-discovery-outcomes/summary?hours=24
 *       -> { ok, source, window_hours, data: { totals, by_reason, by_chain,
 *            by_cartridge, by_pair }, rollup }   // FE-0038 §47 groupings
 *   GET /api/v1/route-discovery-outcomes?limit=100
 *       -> { ok, source, count, data: [ ...rows ] }
 *
 * RDO-SUMMARY-503 (2026-09-02): at 76M rows / 37 GB (~1.32M rows/h) an
 * on-demand GROUP BY over a 24h window = 35.6M rows ≈ 90s measured — the
 * 15s honest budget can never hold (miration 115 header). The summary now
 * aggregates the 5-minute rollup (`route_discovery_outcome_rollup_5m`,
 * migration 115) for COMPLETE buckets and scans RAW only for the two ≤5-min
 * window edges. ensureRollups() tops the rollup up oldest-missing-first
 * (≤24 buckets/request, INSERT .. ON CONFLICT DO NOTHING — idempotent); if
 * the requested window still has coverage gaps the route answers the SAME
 * honest 503 (reason=rollup_backfilling) rather than silently undercounting.
 */

import { Router, type Request, type Response } from "express";
import pg from "pg";

export function buildRouteDiscoveryOutcomesRouter(pool: pg.Pool | null): Router {
  const router = Router();

  const unavailable = (res: Response, reason: string) =>
    res.status(503).json({ ok: false, reason, source: "postgres", data: null });
  const failed = (res: Response, e: unknown) =>
    res.status(503).json({ ok: false, reason: "query_failed", source: "postgres", error: (e as Error).message, data: null });

  const clampInt = (v: unknown, def: number, lo: number, hi: number) => {
    const n = parseInt(String(v ?? def), 10);
    return Number.isFinite(n) ? Math.min(Math.max(n, lo), hi) : def;
  };

  // RDO-SUMMARY-HANG (2026-08-31): each aggregation runs inside its own
  // short-lived transaction with a 15s statement timeout, and the groupings
  // execute in PARALLEL. With this bound a heavy window either completes
  // within one query budget or fails honest (503 query_failed via `failed()`)
  // — never stacks, never starves the pool, never blocks ALTER TABLE.
  const RDO_STATEMENT_TIMEOUT_MS = 15_000;

  // RDO-SUMMARY-503: rollup bucket grain (ms) — mirrors migration 115.
  const ROLLUP_BUCKET_MS = 300_000;
  // Rollup top-up work bound per request: 24 buckets ≈ ≤7s of one-time
  // per-bucket aggregation inside the same 15s budget; steady state is 0-1.
  const ROLLUP_TOPUP_BUCKETS = 24;

  const timedQuery = (sql: string, params: unknown[]) => {
    const run = async (): Promise<pg.QueryResult> => {
      // pool is non-null — the route guards `if (!pool)` before reaching here.
      const client = await (pool as pg.Pool).connect();
      try {
        await client.query("BEGIN");
        await client.query(`SET LOCAL statement_timeout = ${RDO_STATEMENT_TIMEOUT_MS}`);
        const r = await client.query(sql, params);
        await client.query("COMMIT");
        return r;
      } catch (e) {
        await client.query("ROLLBACK").catch(() => {});
        throw e;
      } finally {
        client.release();
      }
    };
    return run();
  };

  // ── Rollup maintenance (RDO-SUMMARY-503) ─────────────────────────────────
  // Inserts any MISSING COMPLETE buckets, oldest gap first, ≤24 per call.
  // Idempotent (ON CONFLICT DO NOTHING); only ever touches buckets strictly
  // below the current in-progress bucket, so it can never race a partial row.
  // "Missing" = no `__totals__` row — an empty bucket (detection paused ≥5min,
  // e.g. a deploy restart) gets a zero-row marker, so interior gaps self-heal
  // instead of tripping the coverage check forever.
  const ENSURE_ROLLUP_SQL = `
    WITH oldest AS (
      -- bigint / bigint division already truncates (= floor for positive ms);
      -- wrapping it in floor() would resolve to DOUBLE and break
      -- generate_series(bigint, bigint, int) below.
      SELECT min(ts_ms) / ${ROLLUP_BUCKET_MS}::bigint * ${ROLLUP_BUCKET_MS} AS b
      FROM route_discovery_outcomes
    ),
    last_complete AS (
      SELECT floor(extract(epoch FROM now()) * 1000)::bigint
             / ${ROLLUP_BUCKET_MS} * ${ROLLUP_BUCKET_MS} - ${ROLLUP_BUCKET_MS} AS b
    ),
    todo AS (
      SELECT g.bucket
      FROM generate_series(
             -- Empty table: COALESCE to last_complete so the zero-marker for
             -- that one bucket still gets created (coverage below requires it).
             COALESCE((SELECT b FROM oldest), (SELECT b FROM last_complete)),
             (SELECT b FROM last_complete),
             ${ROLLUP_BUCKET_MS}) AS g(bucket)
      WHERE NOT EXISTS (
        SELECT 1 FROM route_discovery_outcome_rollup_5m rr
        WHERE rr.dim = '__totals__' AND rr.bucket_ms = g.bucket
      )
      ORDER BY 1
      LIMIT ${ROLLUP_TOPUP_BUCKETS}
    )
    INSERT INTO route_discovery_outcome_rollup_5m (dim, key, bucket_ms, n, opportunities, with_reserves, profit_gt0)
    SELECT agg.dim, agg.key, t.bucket, agg.n, agg.opportunities, agg.with_reserves, agg.profit_gt0
    FROM todo t
    CROSS JOIN LATERAL (
      SELECT '__totals__' AS dim, '' AS key,
             count(*)::bigint AS n,
             count(*) FILTER (WHERE r.is_opportunity)::bigint AS opportunities,
             count(*) FILTER (WHERE r.had_reserves)::bigint AS with_reserves,
             count(*) FILTER (WHERE r.estimated_profit > 0)::bigint AS profit_gt0
      FROM route_discovery_outcomes r
      WHERE r.ts_ms >= t.bucket AND r.ts_ms < t.bucket + ${ROLLUP_BUCKET_MS}
      UNION ALL
      SELECT 'reason', COALESCE(NULLIF(r.reason, ''), '(null)'),
             count(*)::bigint, count(*) FILTER (WHERE r.is_opportunity)::bigint, 0::bigint, 0::bigint
      FROM route_discovery_outcomes r
      WHERE r.ts_ms >= t.bucket AND r.ts_ms < t.bucket + ${ROLLUP_BUCKET_MS}
      GROUP BY 2
      UNION ALL
      SELECT 'chain', r.chain_id::text,
             count(*)::bigint, count(*) FILTER (WHERE r.is_opportunity)::bigint, 0::bigint, 0::bigint
      FROM route_discovery_outcomes r
      WHERE r.ts_ms >= t.bucket AND r.ts_ms < t.bucket + ${ROLLUP_BUCKET_MS}
      GROUP BY 2
      UNION ALL
      SELECT 'cartridge', COALESCE(NULLIF(r.cartridge_id, ''), '(null)'),
             count(*)::bigint, count(*) FILTER (WHERE r.is_opportunity)::bigint, 0::bigint, 0::bigint
      FROM route_discovery_outcomes r
      WHERE r.ts_ms >= t.bucket AND r.ts_ms < t.bucket + ${ROLLUP_BUCKET_MS}
      GROUP BY 2
      UNION ALL
      SELECT 'pair', COALESCE(r.token_in, '') || '|' || COALESCE(r.token_out, ''),
             count(*)::bigint, count(*) FILTER (WHERE r.is_opportunity)::bigint, 0::bigint, 0::bigint
      FROM route_discovery_outcomes r
      WHERE r.ts_ms >= t.bucket AND r.ts_ms < t.bucket + ${ROLLUP_BUCKET_MS}
      GROUP BY 2
    ) agg
    ON CONFLICT DO NOTHING`;

  const ensureRollups = () => timedQuery(ENSURE_ROLLUP_SQL, []);

  // How many COMPLETE buckets the window expects but the rollup still lacks.
  // The demand starts at the OLDEST DATA BUCKET, not at `since` — a window
  // reaching back before any data exists (e.g. hours=336 on a 57h-old table)
  // must not demand markers for buckets that can never have any; an EMPTY
  // table demands nothing beyond the zero-marker ensureRollups() creates.
  const coverageGaps = (fromBucket: number, toBucket: number) =>
    timedQuery(
      `WITH oldest AS (
         SELECT min(ts_ms) / ${ROLLUP_BUCKET_MS}::bigint * ${ROLLUP_BUCKET_MS} AS b
         FROM route_discovery_outcomes
       )
       SELECT count(*)::int AS missing,
              COALESCE(min(g.b), 0)::bigint AS oldest_missing
       FROM generate_series(
              GREATEST($1::bigint, COALESCE((SELECT b FROM oldest), $2::bigint)),
              $2::bigint,
              ${ROLLUP_BUCKET_MS}) AS g(b)
       WHERE NOT EXISTS (
         SELECT 1 FROM route_discovery_outcome_rollup_5m rr
         WHERE rr.dim = '__totals__' AND rr.bucket_ms = g.b
       )`,
      [fromBucket, toBucket],
    );

  // ── Window math ──────────────────────────────────────────────────────────
  // rollup serves buckets [firstAligned, lastComplete] (complete + fully
  // inside the window); the two raw edges are [since, firstAligned) and
  // [tailFrom, now) — each at most one bucket wide.
  const windowEdges = (since: number, now: number) => {
    const lastComplete = Math.floor(now / ROLLUP_BUCKET_MS) * ROLLUP_BUCKET_MS - ROLLUP_BUCKET_MS;
    const tailFrom = lastComplete + ROLLUP_BUCKET_MS;
    const firstAligned = Math.ceil(since / ROLLUP_BUCKET_MS) * ROLLUP_BUCKET_MS;
    const useRollup = firstAligned <= lastComplete;
    return {
      rollFrom: firstAligned,
      rollTo: lastComplete,
      headFrom: since,
      headTo: useRollup ? firstAligned : tailFrom,
      tailFrom,
      useRollup,
    };
  };

  // The three-source merge every grouping uses: complete-bucket rollup +
  // raw head edge + raw tail edge. Output columns are aliased to the SAME
  // names the raw-only query always returned (FE contract unchanged).
  const groupingSQL = (dim: "reason" | "chain" | "cartridge" | "pair", selectExpr: string) => `
    WITH roll AS (
      SELECT key, n, opportunities FROM route_discovery_outcome_rollup_5m
      WHERE dim = '${dim}' AND bucket_ms >= $1 AND bucket_ms <= $2
    ),
    rawh AS (
      SELECT ${selectExpr} AS key, count(*)::bigint AS n,
             count(*) FILTER (WHERE is_opportunity)::bigint AS opportunities
      FROM route_discovery_outcomes WHERE ts_ms >= $3 AND ts_ms < $4 GROUP BY 1
    ),
    rawt AS (
      SELECT ${selectExpr} AS key, count(*)::bigint AS n,
             count(*) FILTER (WHERE is_opportunity)::bigint AS opportunities
      FROM route_discovery_outcomes WHERE ts_ms >= $5 GROUP BY 1
    )
    SELECT key, sum(n)::bigint AS n, sum(opportunities)::bigint AS opportunities
    FROM (SELECT * FROM roll UNION ALL SELECT * FROM rawh UNION ALL SELECT * FROM rawt) u
    GROUP BY 1 ORDER BY 2 DESC LIMIT 25`;

  const TOTALS_SQL = `
    WITH roll AS (
      -- UNION ALL aligns BY POSITION: rename n -> total so the merged
      -- column keeps the raw query's name (FE contract) and sum(total)
      -- resolves. Same shape as rawh/rawt below.
      SELECT n AS total, opportunities, with_reserves, profit_gt0
      FROM route_discovery_outcome_rollup_5m
      WHERE dim = '__totals__' AND bucket_ms >= $1 AND bucket_ms <= $2
    ),
    rawh AS (
      SELECT count(*)::bigint AS total,
             count(*) FILTER (WHERE is_opportunity)::bigint AS opportunities,
             count(*) FILTER (WHERE had_reserves)::bigint AS with_reserves,
             count(*) FILTER (WHERE estimated_profit > 0)::bigint AS profit_gt0
      FROM route_discovery_outcomes WHERE ts_ms >= $3 AND ts_ms < $4
    ),
    rawt AS (
      SELECT count(*)::bigint AS total,
             count(*) FILTER (WHERE is_opportunity)::bigint AS opportunities,
             count(*) FILTER (WHERE had_reserves)::bigint AS with_reserves,
             count(*) FILTER (WHERE estimated_profit > 0)::bigint AS profit_gt0
      FROM route_discovery_outcomes WHERE ts_ms >= $5
    ),
    merged AS (SELECT * FROM roll UNION ALL SELECT * FROM rawh UNION ALL SELECT * FROM rawt)
    SELECT sum(total)::bigint AS total,
           sum(opportunities)::bigint AS opportunities,
           sum(with_reserves)::bigint AS with_reserves,
           sum(profit_gt0)::bigint AS profit_gt0,
           (SELECT count(DISTINCT k)::int FROM (
              SELECT key AS k FROM route_discovery_outcome_rollup_5m
              WHERE dim = 'chain' AND bucket_ms >= $1 AND bucket_ms <= $2
              UNION SELECT chain_id::text FROM route_discovery_outcomes
                WHERE ts_ms >= $3 AND ts_ms < $4
              UNION SELECT chain_id::text FROM route_discovery_outcomes WHERE ts_ms >= $5
            ) c) AS chains,
           (SELECT count(DISTINCT k)::int FROM (
              SELECT key AS k FROM route_discovery_outcome_rollup_5m
              WHERE dim = 'cartridge' AND bucket_ms >= $1 AND bucket_ms <= $2
              UNION SELECT COALESCE(NULLIF(cartridge_id, ''), '(null)') FROM route_discovery_outcomes
                WHERE ts_ms >= $3 AND ts_ms < $4
              UNION SELECT COALESCE(NULLIF(cartridge_id, ''), '(null)') FROM route_discovery_outcomes WHERE ts_ms >= $5
            ) x) AS cartridges
    FROM merged`;

  // Aggregate hit-rate / reason / coverage over a time window (default 24h, max 14d).
  router.get("/api/v1/route-discovery-outcomes/summary", async (req: Request, res: Response) => {
    if (!pool) return unavailable(res, "db_unavailable");
    const hours = clampInt(req.query.hours, 24, 1, 336);
    const since = Date.now() - hours * 3600 * 1000;
    try {
      // 1) Top the rollup up (idempotent, oldest gap first) BEFORE reading it.
      await ensureRollups();

      // 2) Window edges. If the rollup serves part of the window, verify the
      //    coverage is complete — a gapped window would UNDERCOUNT silently,
      //    so it answers the same honest 503 instead (recoverable: the top-up
      //    converges across polls and the next request succeeds).
      const edges = windowEdges(since, Date.now());
      if (edges.useRollup) {
        const gaps = await coverageGaps(edges.rollFrom, edges.rollTo);
        const missing = Number(gaps.rows[0]?.missing ?? 0);
        if (missing > 0) {
          return res.status(503).json({
            ok: false,
            reason: "rollup_backfilling",
            source: "postgres",
            detail: {
              missing_buckets: missing,
              oldest_missing_ms: gaps.rows[0]?.oldest_missing ?? null,
              buckets_per_request: ROLLUP_TOPUP_BUCKETS,
              retry_after_s: 30,
            },
            data: null,
          });
        }
      }

      const p = [edges.rollFrom, edges.rollTo, edges.headFrom, edges.headTo, edges.tailFrom];

      const [totals, byReason, byChain, byCartridge, byPair] = await Promise.all([
        timedQuery(TOTALS_SQL, p),
        // FE-0038 (§47): by-strategy (cartridge_id) and by-pair groupings over the
        // SAME window — the dimensions the sink already persists. hop / detector /
        // DEX are NOT columns of this table; the FE renders them as honest gaps
        // (nivel-(b)) rather than this route inventing joins.
        timedQuery(groupingSQL("reason", `COALESCE(NULLIF(reason, ''), '(null)')`), p).then((r) =>
          // passthrough rename key -> reason (same column name the raw query served)
          r.rows.map((row: { key: string; n: string; opportunities: string }) => ({
            reason: row.key,
            n: row.n,
            opportunities: row.opportunities,
          })),
        ),
        timedQuery(groupingSQL("chain", `chain_id::text`), p).then((r) =>
          r.rows.map((row: { key: string; n: string; opportunities: string }) => ({
            chain_id: row.key,
            n: row.n,
            opportunities: row.opportunities,
          })),
        ),
        timedQuery(groupingSQL("cartridge", `COALESCE(NULLIF(cartridge_id, ''), '(null)')`), p).then((r) =>
          r.rows.map((row: { key: string; n: string; opportunities: string }) => ({
            cartridge_id: row.key,
            n: row.n,
            opportunities: row.opportunities,
          })),
        ),
        timedQuery(groupingSQL("pair", `COALESCE(token_in, '') || '|' || COALESCE(token_out, '')`), p).then((r) =>
          r.rows.map((row: { key: string; n: string; opportunities: string }) => ({
            token_in: row.key.split("|")[0] || null,
            token_out: row.key.split("|")[1] || null,
            n: row.n,
            opportunities: row.opportunities,
          })),
        ),
      ]);
      res.json({
        ok: true,
        source: "postgres",
        window_hours: hours,
        data: {
          totals: totals.rows[0],
          by_reason: byReason,
          by_chain: byChain,
          by_cartridge: byCartridge,
          by_pair: byPair,
        },
        rollup: {
          bucket_ms: ROLLUP_BUCKET_MS,
          served_buckets:
            edges.useRollup ? (edges.rollTo - edges.rollFrom) / ROLLUP_BUCKET_MS + 1 : 0,
          raw_edge_head_ms: Math.max(0, edges.headTo - edges.headFrom),
          raw_edge_tail_ms: Date.now() - edges.tailFrom,
        },
      });
    } catch (e) {
      failed(res, e);
    }
  });

  // Recent raw outcome rows (newest first).
  router.get("/api/v1/route-discovery-outcomes", async (req: Request, res: Response) => {
    if (!pool) return unavailable(res, "db_unavailable");
    const limit = clampInt(req.query.limit, 100, 1, 500);
    try {
      const r = await pool.query(
        `SELECT stream_id, ts_ms, schema_ver, chain_id, cartridge_id, tx_hash,
                source_event, pool_hint, token_in, token_out, is_opportunity,
                estimated_profit, confidence, urgency, had_reserves, mode, reason, inserted_at
         FROM route_discovery_outcomes ORDER BY id DESC LIMIT $1`,
        [limit],
      );
      res.json({ ok: true, source: "postgres", count: r.rowCount, data: r.rows });
    } catch (e) {
      failed(res, e);
    }
  });

  return router;
}
