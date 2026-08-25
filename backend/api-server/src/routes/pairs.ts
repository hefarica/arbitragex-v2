/**
 * FE-MASTER EMIT-06 — GET /api/pairs: the effective pair universe as the
 * runtime sees it (P5 §13, `PairsResponseSchema` = EXACTLY `{ entries }`).
 *
 * Sources (all real — RULE 00):
 *   - PG `pools` (is_active, joined to factories/dexes/tokens) — the SAME
 *     registry the Rust side loads (impact_index.rs / scanner / pool workers
 *     all read this table), so the pair universe matches the graph universe.
 *   - Redis `arbx:pool_reserves:<chain>:<pool>` (pool_sync ReservesEntry) —
 *     §62: u256 reserves ALWAYS decimal strings, oriented by the pool's own
 *     token0 onto the pair's canonical (address-ascending) leg order.
 *   - Redis `arbx:dirty_pools:<chain>` (XLS-QB-05 SET) — read via SMEMBERS
 *     WITHOUT draining: `dirty` = "in the current undrained set" as of this
 *     request (the discovery tick drains it; between ticks it re-fills).
 *
 * R8 honesty model:
 *   - `alpha_forward` / `alpha_reverse` ride the EMIT-06b hash
 *     (`arbx:pairs:alpha:<chain>`, field = canonical `<aAddr>|<bAddr>`):
 *     the r15 directed F_e values route discovery publishes per tick under
 *     the `fe_prefilter` knob. Knob OFF / TTL lapsed / pair absent from this
 *     tick's graph / poisoned row ⇒ null (never fabricated, never 0); a row
 *     that fails to parse drops BOTH directions (no half-fabricated pair).
 *   - A pool with no reserves entry (pool_sync absent/TTL'd) is EXCLUDED from
 *     the pair's `pools[]` (a PoolRef without honest reserves cannot exist);
 *     a pair whose every pool lacks reserves drops out of `entries`.
 *   - A registry-incomplete pool (missing token identity/decimals) is skipped.
 *   - Redis ERRORS are 503 `redis_unavailable` (absence ≠ outage — see above).
 *   - `fee_bps` falls back to 30 for NULL `fee_tier` — mirrors graph_builder's
 *     own class constant (`fee.unwrap_or(0.003)`, the V2 CFMM γ); on-chain
 *     per-pool tiers always ride `fee_tier` verbatim.
 *   - `last_reserve_update` = max ReservesEntry.ts across the pair's pools
 *     (epoch seconds → ms); null when no pool carries reserves.
 *
 * No silent caps: the FULL active-pool universe of the chain is grouped and
 * served (bounded by operator discovery config, like /api/v1/pools' 500-row
 * window is bounded by the same registry).
 */
import type { Application, Request, Response } from "express";
import type { Pool as PgPool } from "pg";
import type { Redis } from "ioredis";

export interface PairsDeps {
  pool: PgPool | null;
  redis: Redis | null;
  logger: { warn: (obj: object, msg?: string) => void };
}

/** Mirror of searcher `reserves.rs` key builder + `dirty_signal` constant. */
const POOL_RESERVES_KEY_PREFIX = "arbx:pool_reserves:";
const DIRTY_POOLS_KEY_PREFIX = "arbx:dirty_pools:";
/** Mirror of `pair_alpha_runtime.rs` key builder (EMIT-06b). */
const PAIR_ALPHA_KEY_PREFIX = "arbx:pairs:alpha:";

/** Mirror of `ReservesEntry` (searcher reserves.rs) — the fields consumed here. */
interface ReservesEntryWire {
  r0: string;
  r1: string;
  /** Lowercase token0 address — orients r0/r1 when the pool row lacks it. */
  token0_addr?: string | null;
  ts: number;
}

interface PoolRow {
  address: string;
  dex_name: string;
  fee_tier: number | null;
  token0_symbol: string | null;
  token0_address: string | null;
  token0_decimals: number | null;
  token1_symbol: string | null;
  token1_address: string | null;
  token1_decimals: number | null;
}

export function mountPairs(app: Application, deps: PairsDeps): void {
  app.get("/api/pairs", async (req: Request, res: Response) => {
    const chainId = Number(req.query["chain_id"] ?? 1);
    if (!Number.isInteger(chainId) || chainId < 1) {
      res.status(400).json({ error: "invalid_chain_id" });
      return;
    }
    if (!deps.pool) {
      res.status(503).json({ error: "db_unavailable", detail: "DATABASE_URL not configured" });
      return;
    }
    if (!deps.redis) {
      res.status(503).json({ error: "redis_unavailable", detail: "no Redis connection" });
      return;
    }

    let rows: PoolRow[];
    try {
      const q = await deps.pool.query(
        `SELECT p.address,
                d.name        AS dex_name,
                p.fee_tier,
                t0.symbol     AS token0_symbol,
                t0.address    AS token0_address,
                t0.decimals   AS token0_decimals,
                t1.symbol     AS token1_symbol,
                t1.address    AS token1_address,
                t1.decimals   AS token1_decimals
           FROM pools p
           JOIN factories f ON f.id = p.factory_id
           JOIN dexes    d  ON d.id  = f.dex_id
           LEFT JOIN tokens t0 ON t0.id = p.token0_id
           LEFT JOIN tokens t1 ON t1.id = p.token1_id
          WHERE p.chain_id = $1
            AND p.is_active = TRUE
          ORDER BY p.address ASC`,
        [chainId],
      );
      rows = q.rows as PoolRow[];
    } catch (e) {
      deps.logger.warn({ event: "pairs.query_failed", err: (e as Error).message });
      res.status(503).json({ error: "query_failed", detail: (e as Error).message });
      return;
    }

    // Live reserves (ONE MGET) + the undrained dirty set (ONE SMEMBERS) +
    // the published per-pair alpha table (ONE HGETALL — EMIT-06b, written
    // by route discovery under `fe_prefilter`; absence is NOT an outage:
    // knob OFF / TTL lapsed simply serves null alphas).
    const poolAddrs = rows.map((r) => r.address.toLowerCase());
    let reserveRaws: (string | null)[];
    let dirtyMembers: string[];
    let alphaRaw: Record<string, string>;
    try {
      reserveRaws = poolAddrs.length
        ? await deps.redis.mget(...poolAddrs.map((a) => `${POOL_RESERVES_KEY_PREFIX}${chainId}:${a}`))
        : [];
      dirtyMembers = await deps.redis.smembers(`${DIRTY_POOLS_KEY_PREFIX}${chainId}`);
      alphaRaw = await deps.redis.hgetall(`${PAIR_ALPHA_KEY_PREFIX}${chainId}`);
    } catch (e) {
      deps.logger.warn({ event: "pairs.redis_read_failed", err: (e as Error).message });
      res.status(503).json({ error: "redis_unavailable", detail: (e as Error).message });
      return;
    }
    // Directed alpha per canonical pair key — {forward, reverse} f_e values
    // or honest nulls. Unparseable/poisoned rows drop to null for BOTH
    // directions (never a half-fabricated direction pair), non-finite
    // numbers are rejected (R8: a poisoned NaN is not an alpha).
    const alphaByPair = new Map<string, { forward: number | null; reverse: number | null }>();
    for (const [field, raw] of Object.entries(alphaRaw)) {
      let fwd: number | null = null;
      let rev: number | null = null;
      try {
        const v = JSON.parse(raw) as { forward?: unknown; reverse?: unknown };
        if (typeof v.forward === "number" && Number.isFinite(v.forward)) fwd = v.forward;
        if (typeof v.reverse === "number" && Number.isFinite(v.reverse)) rev = v.reverse;
      } catch {
        // poisoned row — both stay null
      }
      alphaByPair.set(field, { forward: fwd, reverse: rev });
    }
    const dirty = new Set(dirtyMembers.map((m) => m.toLowerCase()));
    const reserves = new Map<string, ReservesEntryWire | null>();
    poolAddrs.forEach((a, i) => {
      const raw = reserveRaws[i];
      if (raw === null || raw === undefined) {
        reserves.set(a, null);
        return;
      }
      try {
        reserves.set(a, JSON.parse(raw) as ReservesEntryWire);
      } catch {
        reserves.set(a, null); // unparseable = not usable, never fabricated
      }
    });

    // Group pools into canonical (address-ascending) pairs.
    const byPair = new Map<
      string,
      {
        aAddr: string;
        aSymbol: string;
        aDecimals: number;
        bAddr: string;
        bSymbol: string;
        bDecimals: number;
        pools: PoolRow[];
      }
    >();
    for (const r of rows) {
      // Registry-incomplete pools cannot form an honest PairView leg (R8).
      if (
        !r.token0_address || !r.token0_symbol || r.token0_decimals === null ||
        !r.token1_address || !r.token1_symbol || r.token1_decimals === null
      ) {
        continue;
      }
      // R3 7b residual belt: normalize BOTH legs to lowercase before the
      // canonical ordering — if PG ever carries mixed-case addresses the
      // byte-order comparison AND the alpha join would silently disagree
      // with the writer's lowercase fields. One toLowerCase per leg kills
      // the entire assumption (defensive check on a money-data path).
      const t0 = r.token0_address.toLowerCase();
      const t1 = r.token1_address.toLowerCase();
      const [a, b] =
        t0 < t1
          ? [
              { addr: t0, sym: r.token0_symbol, dec: r.token0_decimals },
              { addr: t1, sym: r.token1_symbol, dec: r.token1_decimals },
            ]
          : [
              { addr: t1, sym: r.token1_symbol, dec: r.token1_decimals },
              { addr: t0, sym: r.token0_symbol, dec: r.token0_decimals },
            ];
      const key = `${a.addr}|${b.addr}`;
      const entry = byPair.get(key) ?? {
        aAddr: a.addr,
        aSymbol: a.sym,
        aDecimals: a.dec,
        bAddr: b.addr,
        bSymbol: b.sym,
        bDecimals: b.dec,
        pools: [],
      };
      entry.pools.push(r);
      byPair.set(key, entry);
    }

    const entries = [];
    for (const p of byPair.values()) {
      const poolRefs = [];
      let lastTs: number | null = null;
      let pairDirty = false;
      for (const pool of p.pools) {
        const addr = pool.address.toLowerCase();
        const entry = reserves.get(addr) ?? null;
        if (entry === null) continue; // no live reserves — pool excluded (R8)
        // Orient the pool's r0/r1 onto the pair's canonical a/b legs.
        const poolToken0 = (entry.token0_addr ?? pool.token0_address ?? "").toLowerCase();
        const aIsToken0 = poolToken0 === p.aAddr;
        poolRefs.push({
          pool_address: addr,
          venue: pool.dex_name,
          fee_bps: pool.fee_tier ?? 30, // class constant — see header
          reserves_a: aIsToken0 ? entry.r0 : entry.r1,
          reserves_b: aIsToken0 ? entry.r1 : entry.r0,
        });
        if (typeof entry.ts === "number" && Number.isFinite(entry.ts)) {
          lastTs = lastTs === null ? entry.ts : Math.max(lastTs, entry.ts);
        }
        if (dirty.has(addr)) pairDirty = true;
      }
      if (poolRefs.length === 0) continue; // no live pool — pair not in the effective view
      const alpha = alphaByPair.get(`${p.aAddr}|${p.bAddr}`) ?? { forward: null, reverse: null };
      entries.push({
        chain_id: chainId,
        token_a: { chain_id: chainId, address: p.aAddr, symbol: p.aSymbol, decimals: p.aDecimals },
        token_b: { chain_id: chainId, address: p.bAddr, symbol: p.bSymbol, decimals: p.bDecimals },
        pools: poolRefs,
        venue_count: new Set(poolRefs.map((x) => x.venue)).size,
        alpha_forward: alpha.forward,
        alpha_reverse: alpha.reverse,
        dirty: pairDirty,
        last_reserve_update: lastTs === null ? null : lastTs * 1000,
      });
    }

    res.status(200).json({ entries });
  });
}
