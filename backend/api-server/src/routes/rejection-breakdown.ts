/**
 * REJECT-BREAKDOWN-EXPORT-01 — grouped rejection-reason breakdown surface.
 *
 * The operator asked (2026-09-06) for the FULL rejection-reason information
 * downloadable from the DApp, to attack the problems one by one. Production
 * evidence that motivated it: 48.4K opportunities/24h, 100% rejected, and
 * 78.2% of the flood was TokenNotAllowed for exactly TWO spam tokens
 * (XEN 0x0645…, AGLD 0x3235…) — invisible without a grouped view.
 *
 * Route (public read — LESS sensitive than /api/v1/opportunities/live,
 * which already exposes rejection_reason per row):
 *   GET /api/v1/rejections/breakdown?hours=24&chain_id=1
 *
 * Contract (RULE 00 / R8 fail-honest):
 *   - Families are RAW reasons grouped by the segment before ':' and
 *     NORMALIZED camelCase→snake_case read-side only (writers are NOT
 *     touched — changing emitted labels would break Prometheus/dashboard
 *     compat). e.g. `UnknownTokenPrice` (writer A) and `unknown_token_price`
 *     (writer B) merge into ONE family; both raw forms stay visible in
 *     `top_raw` for traceability.
 *   - Averages read the REAL schema columns (mig 003 + 049):
 *     gross = `expected_profit_usd`, net = `net_expected_profit_usd` (NULL
 *     for pre-spine rows — R8: the average is over the rows that HAVE the
 *     value, never a filled zero).
 *   - Family averages MERGE across raw groups weighted by the per-group
 *     count of rows that actually had the value (COUNT(col)), not by COUNT(*).
 *   - `token_flood`: TokenNotAllowed:<address> suffixes ranked by count with
 *     symbol resolved from the `tokens` table; unknown address → symbol null
 *     (never fabricated). Two chains sharing an address → lexicographically
 *     smallest symbol (deterministic, documented).
 *   - Anti-hang (RDO-SUMMARY-HANG #502 pattern): every aggregate runs in its
 *     own transaction with SET LOCAL statement_timeout — a heavy window fails
 *     honest (503 query_failed) instead of stacking on the shared pool.
 *   - 503 db_unavailable / 503 query_failed / 400 invalid_hours|invalid_chain_id.
 */
import { Router, type Request, type Response } from "express";
import type { Pool } from "pg";

interface Deps {
  /** Null when PG is absent at boot (R6 pattern) — routes then 503 honestly. */
  pool: Pool | null;
  logger: { warn: (obj: object, msg?: string) => void; error: (obj: object, msg?: string) => void };
}

const DEFAULT_HOURS = 24;
/** Retention keeps opportunities 60d; cap reads at 30d so the aggregate scan
 * never walks the whole cold tail. */
const MAX_HOURS = 720;
/** Distinct raw reasons returned by the group-by (payload bound — the token
 * floods observed in prod have a handful of addresses each). */
const RAW_REASON_LIMIT = 500;
const TOP_RAW_PER_FAMILY = 5;
const TOKEN_FLOOD_LIMIT = 20;
/** Per-statement budget (RDO-SUMMARY-HANG #502 pattern): a heavy 30d window
 * fails honest instead of hanging a pool client. */
const BREAKDOWN_STATEMENT_TIMEOUT_MS = 15_000;
/** PG int4 ceiling — anything above can never match a real chain_id and would
 * only produce an int4-out-of-range 503; reject it as 400 up front. */
const MAX_CHAIN_ID = 2_147_483_647;
/** `TokenNotAllowed:0x<40hex>` (case-insensitive family — writer casing varies). */
const TOKEN_ADDR_RE = /^tokennotallowed:(0x[0-9a-fA-F]{40})$/i;

/**
 * camelCase→snake_case, mirroring `camel_to_snake` in
 * backend/searcher-rs/src/opportunity_emitter.rs so TS-normalized families
 * match the emitter's own label transform ("UnknownTokenPrice" →
 * "unknown_token_price"). Already-snake input passes through unchanged.
 */
export function normalizeRejectionFamily(rawFamily: string): string {
  return rawFamily
    .replace(/([a-z0-9])([A-Z])/g, "$1_$2")
    .replace(/([A-Z]+)([A-Z][a-z])/g, "$1_$2")
    .toLowerCase();
}

interface RawGroupRow {
  family_raw: string;
  raw_reason: string;
  n: number;
  /** Rows in this raw group that actually HAVE the value (PG AVG ignores
   * NULLs — these are the correct merge weights, not n). */
  gross_n: number;
  net_n: number;
  avg_gross: string | null;
  avg_net: string | null;
}

interface FamilyOut {
  family: string;
  count: number;
  share_pct_of_rejected: number;
  avg_gross_usd: number | null;
  avg_net_usd: number | null;
  top_raw: Array<{ reason: string; count: number }>;
}

/** Mutable accumulator while merging raw groups (weighted-average state). */
interface FamilyAcc extends FamilyOut {
  /** Rows that actually had gross_usd / net_usd (AVG ignores NULLs). */
  grossN: number;
  netN: number;
}

interface FloodOut {
  address: string;
  symbol: string | null;
  count: number;
}

function readHours(req: Request): number | null {
  const raw = req.query["hours"];
  if (raw === undefined) return DEFAULT_HOURS;
  const n = Number(raw);
  if (!Number.isInteger(n) || n < 1 || n > MAX_HOURS) return null;
  return n;
}

function readChainId(req: Request): number | null | undefined {
  const raw = req.query["chain_id"];
  if (raw === undefined) return undefined;
  const n = Number(raw);
  if (!Number.isInteger(n) || n < 1 || n > MAX_CHAIN_ID) return null;
  return n;
}

export function buildRejectionBreakdownRouter(deps: Deps): Router {
  const router = Router();

  /** RDO-SUMMARY-HANG (#502) pattern: one short-lived transaction per
   * statement with SET LOCAL statement_timeout — transaction-scoped, so the
   * bound never leaks to other pool users; heavy windows fail honest. */
  const timedQuery = <T>(sql: string, params: unknown[]): Promise<{ rows: T[] }> => {
    const run = async (): Promise<{ rows: T[] }> => {
      // pool is non-null — the route guards `if (!deps.pool)` before here.
      const client = await deps.pool!.connect();
      try {
        await client.query("BEGIN");
        await client.query(`SET LOCAL statement_timeout = ${BREAKDOWN_STATEMENT_TIMEOUT_MS}`);
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

  router.get("/api/v1/rejections/breakdown", async (req: Request, res: Response) => {
    const hours = readHours(req);
    if (hours === null) {
      res.status(400).json({ error: "invalid_hours", min: 1, max: MAX_HOURS });
      return;
    }
    const chainId = readChainId(req);
    if (chainId === null) {
      res.status(400).json({ error: "invalid_chain_id" });
      return;
    }
    if (!deps.pool) {
      res.status(503).json({ error: "db_unavailable" });
      return;
    }
    // $2 exists only when the chain filter does — params must match EXACTLY
    // (a dangling $2 with a 1-element array makes pg reject the query).
    const params: unknown[] = chainId === undefined ? [hours] : [hours, chainId];
    const chainClause = chainId === undefined ? "" : " AND chain_id = $2";

    try {
      const raws = await timedQuery<RawGroupRow>(
        `SELECT split_part(rejection_reason, ':', 1) AS family_raw,
                rejection_reason               AS raw_reason,
                COUNT(*)::int                  AS n,
                COUNT(expected_profit_usd)::int       AS gross_n,
                COUNT(net_expected_profit_usd)::int   AS net_n,
                AVG(expected_profit_usd)::text        AS avg_gross,
                AVG(net_expected_profit_usd)::text    AS avg_net
           FROM opportunities
          WHERE detected_at > now() - ($1::int * interval '1 hour')
            AND rejection_reason IS NOT NULL${chainClause}
          GROUP BY 1, 2
          ORDER BY n DESC
          LIMIT ${RAW_REASON_LIMIT}`,
        params,
      );
      const totals = await timedQuery<{ total: number; rejected: number }>(
        `SELECT COUNT(*)::int AS total,
                COUNT(rejection_reason)::int AS rejected
           FROM opportunities
          WHERE detected_at > now() - ($1::int * interval '1 hour')${chainClause}`,
        params,
      );

      const totalRows = Number(totals.rows[0]?.total ?? 0);
      const rejectedRows = Number(totals.rows[0]?.rejected ?? 0);

      // Merge raw→family (weighted averages; raw groups are disjoint).
      const families = new Map<string, FamilyAcc>();
      const flood = new Map<string, number>();
      for (const r of raws.rows) {
        const fam = normalizeRejectionFamily(r.family_raw);
        let f = families.get(fam);
        if (!f) {
          f = {
            family: fam,
            count: 0,
            share_pct_of_rejected: 0,
            avg_gross_usd: null,
            avg_net_usd: null,
            top_raw: [],
            grossN: 0,
            netN: 0,
          };
          families.set(fam, f);
        }
        const n = Number(r.n);
        f.count += n;
        // Keep the biggest raw forms for drill-down traceability.
        if (f.top_raw.length < TOP_RAW_PER_FAMILY || n < f.top_raw[f.top_raw.length - 1]!.count) {
          f.top_raw.push({ reason: r.raw_reason, count: n });
          f.top_raw.sort((a, b) => b.count - a.count);
          f.top_raw = f.top_raw.slice(0, TOP_RAW_PER_FAMILY);
        }
        // Weighted avg accumulation over the rows that HAD the value:
        // combined = Σ(avg_i × colN_i) / ΣcolN_i (PG AVG ignores NULLs, so
        // COUNT(col) — not COUNT(*) — is the merge weight). First group seeds
        // the running mean with ITS avg (mean of its colN rows).
        if (r.avg_gross !== null) {
          const g = Number(r.avg_gross);
          const gn = Number(r.gross_n);
          f.avg_gross_usd = f.avg_gross_usd === null ? g : (f.avg_gross_usd * f.grossN + g * gn) / (f.grossN + gn);
          f.grossN += gn;
        }
        if (r.avg_net !== null) {
          const v = Number(r.avg_net);
          const vn = Number(r.net_n);
          f.avg_net_usd = f.avg_net_usd === null ? v : (f.avg_net_usd * f.netN + v * vn) / (f.netN + vn);
          f.netN += vn;
        }
        const m = TOKEN_ADDR_RE.exec(r.raw_reason);
        if (m?.[1]) {
          const addr = m[1].toLowerCase();
          flood.set(addr, (flood.get(addr) ?? 0) + n);
        }
      }
      const familyList = [...families.values()].map((f) => ({
        family: f.family,
        count: f.count,
        share_pct_of_rejected: rejectedRows > 0 ? Math.round((f.count / rejectedRows) * 1000) / 10 : 0,
        avg_gross_usd: f.avg_gross_usd,
        avg_net_usd: f.avg_net_usd,
        top_raw: f.top_raw,
      }));
      familyList.sort((a, b) => b.count - a.count);

      // Symbol resolution for the flood addresses (honest null on unknown).
      let tokenFlood: FloodOut[] = [];
      const floodAddrs = [...flood.entries()]
        .sort((a, b) => b[1] - a[1])
        .slice(0, TOKEN_FLOOD_LIMIT);
      if (floodAddrs.length > 0) {
        const sym = await timedQuery<{ addr: string; symbol: string }>(
          `SELECT lower(address) AS addr, symbol
             FROM tokens
            WHERE lower(address) = ANY($1::text[])`,
          [floodAddrs.map(([a]) => a)],
        );
        const symByAddr = new Map<string, string>();
        for (const row of sym.rows) {
          const prev = symByAddr.get(row.addr);
          // Deterministic pick when one address carries symbols on several chains.
          if (prev === undefined || row.symbol < prev) symByAddr.set(row.addr, row.symbol);
        }
        tokenFlood = floodAddrs.map(([address, count]) => ({
          address,
          symbol: symByAddr.get(address) ?? null,
          count,
        }));
      }

      res.json({
        ok: true,
        kind: "rejection_breakdown",
        read_only: true,
        window_hours: hours,
        chain_id: chainId ?? null,
        generated_at: new Date().toISOString(),
        total_rows: totalRows,
        rejected_rows: rejectedRows,
        // R8 honesty signal: the group-by is LIMIT-bounded; when this is true
        // family counts/token_flood are computed over the TOP raw groups only
        // while rejected_rows is exact — surface it, never hide it.
        raw_groups_truncated: raws.rows.length === RAW_REASON_LIMIT,
        families: familyList,
        token_flood: tokenFlood,
      });
    } catch (e) {
      deps.logger.error({ err: (e as Error).message, route: "rejection_breakdown" }, "query_failed");
      res.status(503).json({ error: "query_failed" });
      return;
    }
  });

  return router;
}
