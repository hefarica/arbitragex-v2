import type { Application, Request, Response } from "express";
import type { Pool } from "pg";
import type { Redis } from "ioredis";

async function countKeysScan(redis: Redis, pattern: string): Promise<number> {
  let count = 0;
  let cursor = "0";
  do {
    const res = await redis.scan(cursor, "MATCH", pattern, "COUNT", 1000);
    cursor = res[0];
    count += res[1].length;
  } while (cursor !== "0");
  return count;
}
export function mountStrategyRuntimeStatus(
  app: Application,
  deps: { pool: Pool | null; redis: Redis; logger: { warn: (obj: object, msg?: string) => void } }
): void {
  app.get("/api/v1/strategies/runtime-status", async (req: Request, res: Response) => {
    const chainId = Number(req.query["chain_id"] ?? 1);
    const windowSeconds = Number(req.query["window_seconds"] ?? 3600);

    const sourceStatus = {
      postgres: "ok",
      redis: "ok",
      logs: "not_used",
    };

    if (!deps.pool) {
      sourceStatus.postgres = "unavailable";
      return res.status(503).json({ error: "db_unavailable" });
    }

    try {
      // 1. Redis Heartbeat and Keys
      let heartbeat: any = {};
      let poolIndexEntries = 0;
      let reservesCacheEntries = 0;
      let lendingWatchlistSize = 0;
      let redisOk = false;

      try {
        const heartbeatKey = `arbx:heartbeat:scanner:${chainId}:latest`;
        const heartbeatRaw = await deps.redis.get(heartbeatKey);
        if (heartbeatRaw) {
          heartbeat = JSON.parse(heartbeatRaw);
        }

        // SCAN counts the whole keyspace per pattern; run the 4 independent
        // patterns concurrently (Promise.all) instead of 4 serial awaits.
        const [poolIndexKeysCount, poolIndexV3KeysCount, reservesV2KeysCount, reservesV3KeysCount] =
          await Promise.all([
            countKeysScan(deps.redis, `arbx:pool_index:${chainId}:*`),
            countKeysScan(deps.redis, `arbx:pool_index_v3:${chainId}:*`),
            countKeysScan(deps.redis, `arbx:pool_reserves:${chainId}:*`),
            countKeysScan(deps.redis, `arbx:v3_slot0:${chainId}:*`),
          ]);
        poolIndexEntries = poolIndexKeysCount + poolIndexV3KeysCount;
        reservesCacheEntries = reservesV2KeysCount + reservesV3KeysCount;

        redisOk = true;
      } catch (e) {
        deps.logger.warn({ event: "strategy_status.redis_failed", err: (e as Error).message });
        sourceStatus.redis = "unavailable";
      }

      // Base engine loaded inference
      const engineLoaded = redisOk && Object.keys(heartbeat).length > 0;
      const lastInvokedAt = engineLoaded && heartbeat.emitted_at_unix 
        ? new Date(heartbeat.emitted_at_unix * 1000).toISOString() 
        : null;

      // 2. Postgres Opportunities
      // We read from opportunities directly for candidates_1h and rejections_1h.
      // We normalize variants (dex_arb_v2v2 -> dex_arb).
      const dbStats: Record<string, any> = {
        dex_arb: { candidates_1h: 0, rejections_1h: 0, last_candidate_at: null, last_rejection_reason: null, active_variants: new Set<string>() },
        triangular_arb: { candidates_1h: 0, rejections_1h: 0, last_candidate_at: null, last_rejection_reason: null },
        flashloan_arb: { candidates_1h: 0, rejections_1h: 0, last_candidate_at: null, last_rejection_reason: null },
        liquidation: { candidates_1h: 0, rejections_1h: 0, last_candidate_at: null, last_rejection_reason: null },
      };

      let flashloanNoProviderRejections: number | null = 0;
      try {
        // GROUP BY in PG: ~4 rows (one per family) cross the wire instead of
        // ~15k. The all-statuses time-range scan is still index-free (no covering
        // index exists — see 054 partial index excludes rejected/failed), but
        // aggregation + 4-row transfer is ~100x cheaper than 15k-row transfer +
        // node-postgres parse + JS loop.
        const query = `
          SELECT
            CASE
              WHEN strategy_kind LIKE 'dex_arb%' THEN 'dex_arb'
              WHEN strategy_kind = 'triangular'  THEN 'triangular_arb'
              ELSE strategy_kind
            END AS family,
            COUNT(*) AS candidates_1h,
            COUNT(*) FILTER (WHERE rejection_reason IS NOT NULL OR status = 'rejected') AS rejections_1h,
            MAX(detected_at) FILTER (WHERE rejection_reason IS NULL AND status <> 'rejected') AS last_candidate_at,
            (ARRAY_AGG(rejection_reason ORDER BY detected_at DESC)
               FILTER (WHERE rejection_reason IS NOT NULL OR status = 'rejected'))[1] AS last_rejection_reason,
            ARRAY_AGG(DISTINCT strategy_kind) AS variants,
            COUNT(*) FILTER (WHERE strategy_kind = 'flashloan_arb'
                                 AND rejection_reason = 'flashloan_no_provider') AS fl_no_provider
          FROM opportunities
          WHERE detected_at >= NOW() - ($2::int * INTERVAL '1 second')
            AND chain_id = $1
          GROUP BY 1
        `;
        const q = await deps.pool.query(query, [chainId, windowSeconds]);

        for (const row of q.rows) {
          const fam = row.family;
          if (!dbStats[fam]) continue; // backrun not surfaced in this view
          dbStats[fam].candidates_1h = Number(row.candidates_1h);
          dbStats[fam].rejections_1h = Number(row.rejections_1h);
          dbStats[fam].last_candidate_at = row.last_candidate_at
            ? new Date(row.last_candidate_at).toISOString()
            : null;
          dbStats[fam].last_rejection_reason = row.last_rejection_reason ?? null;
          if (fam === "dex_arb" && Array.isArray(row.variants)) {
            for (const v of row.variants) dbStats.dex_arb.active_variants.add(v);
          }
          if (fam === "flashloan_arb") {
            flashloanNoProviderRejections = Number(row.fl_no_provider);
          }
        }
      } catch (e) {
        deps.logger.warn({ event: "strategy_status.pg_failed", err: (e as Error).message });
        sourceStatus.postgres = "unavailable";
        return res.status(503).json({
          error: "opportunities_query_failed",
          detail: (e as Error).message,
          source: sourceStatus,
        });
      }

      // Assemble DEX
      const dexArb = {
        strategy_kind: "dex_arb",
        enabled: true,
        engine_loaded: engineLoaded,
        engine_invoked: engineLoaded, // heartbeat always pulses for DEX
        last_invoked_at: lastInvokedAt,
        last_candidate_at: dbStats.dex_arb.last_candidate_at,
        candidates_1h: dbStats.dex_arb.candidates_1h,
        rejections_1h: dbStats.dex_arb.rejections_1h,
        last_rejection_reason: dbStats.dex_arb.last_rejection_reason,
        data_dependencies_status: "ok",
        active_variants: Array.from(dbStats.dex_arb.active_variants),
        paper_viable_1h: dbStats.dex_arb.candidates_1h - dbStats.dex_arb.rejections_1h,
        paper_rejected_1h: dbStats.dex_arb.rejections_1h,
      };

      // Assemble Triangular
      const triCandidates = dbStats.triangular_arb.candidates_1h;
      let triDataStatus = "ok";
      if (!redisOk || poolIndexEntries === 0) {
        triDataStatus = "unknown_pool_map_entries";
      } else if (poolIndexEntries > 0 && reservesCacheEntries > 0 && triCandidates === 0) {
        triDataStatus = "armed_waiting_for_impact";
      }
      const triInvoked = heartbeat.triangular_cycles_scanned > 0;

      const triangularArb = {
        strategy_kind: "triangular_arb",
        enabled: true,
        engine_loaded: engineLoaded,
        engine_invoked: triInvoked,
        last_invoked_at: triInvoked ? lastInvokedAt : null,
        last_candidate_at: dbStats.triangular_arb.last_candidate_at,
        candidates_1h: triCandidates,
        rejections_1h: dbStats.triangular_arb.rejections_1h,
        last_rejection_reason: dbStats.triangular_arb.last_rejection_reason,
        data_dependencies_status: triDataStatus,
        triangular_pool_map_entries: redisOk ? poolIndexEntries : null,
        pool_index_entries: redisOk ? poolIndexEntries : null,
        reserves_cache_entries: redisOk ? reservesCacheEntries : null,
        impacted_cycles_last_1h: null, // Hard to infer without observation logs
        cycles_with_missing_reserves: heartbeat.triangular_v3_quote_failures || 0,
      };

      // Assemble Flashloan
      const baseCandidates = heartbeat.flashloan_arb_pairs_scanned || 0;
      const wrappedCandidates = heartbeat.flashloan_arb_opps_emitted || 0;
      let flDataStatus = "ok";
      if (baseCandidates > 0 && wrappedCandidates === 0) {
        flDataStatus = "waiting_for_profitable_base";
      }
      const flInvoked = baseCandidates > 0;

      const flashloanArb = {
        strategy_kind: "flashloan_arb",
        enabled: true,
        engine_loaded: engineLoaded,
        engine_invoked: flInvoked,
        last_invoked_at: flInvoked ? lastInvokedAt : null,
        last_candidate_at: dbStats.flashloan_arb.last_candidate_at,
        candidates_1h: dbStats.flashloan_arb.candidates_1h,
        rejections_1h: dbStats.flashloan_arb.rejections_1h,
        last_rejection_reason: dbStats.flashloan_arb.last_rejection_reason,
        data_dependencies_status: flDataStatus,
        base_candidates_seen_1h: baseCandidates,
        wrapped_candidates_1h: wrappedCandidates,
        no_provider_rejections: flashloanNoProviderRejections,
        negative_after_fee_rejections: heartbeat.flashloan_arb_sanity_reject ?? null,
      };

      // Assemble Liquidation
      let liqDataStatus = "ok";
      lendingWatchlistSize = heartbeat.liquidation_positions_scanned || 0;
      if (lendingWatchlistSize === 0) {
        liqDataStatus = "missing_lending_watchlist";
      }
      const liqInvoked = lendingWatchlistSize > 0;

      const liquidation = {
        strategy_kind: "liquidation",
        enabled: false,
        engine_loaded: engineLoaded,
        engine_invoked: liqInvoked,
        last_invoked_at: liqInvoked ? lastInvokedAt : null,
        last_candidate_at: dbStats.liquidation.last_candidate_at,
        candidates_1h: dbStats.liquidation.candidates_1h,
        rejections_1h: dbStats.liquidation.rejections_1h,
        last_rejection_reason: dbStats.liquidation.last_rejection_reason,
        data_dependencies_status: liqDataStatus,
        lending_watchlist_size: lendingWatchlistSize,
        indexed_positions: lendingWatchlistSize,
        impacted_lending_positions_1h: null,
        hf_below_one_count: null,
        liquidation_candidates_1h: dbStats.liquidation.candidates_1h,
      };

      res.status(200).json({
        chain_id: chainId,
        window_seconds: windowSeconds,
        source: sourceStatus,
        strategies: [dexArb, triangularArb, flashloanArb, liquidation],
        ts: new Date().toISOString(),
      });

    } catch (e) {
      // Fail-honest: surface as 503 (degraded/retryable), not a silent 500.
      // The inner PG path already returns 503; this backstop matches it. Cause
      // is logged; body stays detail-free (no raw error leak to the client).
      deps.logger.warn({ event: "strategy_status.unhandled_error", err: (e as Error).message });
      res.status(503).json({ error: "runtime_status_unavailable" });
    }
  });

  app.get("/api/v1/strategies/readiness", async (req: Request, res: Response) => {
    const chainId = Number(req.query["chain_id"] ?? 1);
    
    const sourceStatus: Record<string, string> = {
      postgres: "unavailable",
      redis: "ok",
      logs: "not_used",
    };

    // R8: probe postgres with a real query, not just pool existence.
    if (deps.pool) {
      try {
        await deps.pool.query("SELECT 1");
        sourceStatus.postgres = "queried_ok";
      } catch (e) {
        sourceStatus.postgres = "configured_unreachable";
        deps.logger.warn({ event: "strategy_readiness.pg_probe_failed", err: (e as Error).message });
      }
    }

    let poolIndexEntries: number | null = null;
    let poolIndexV3Entries: number | null = null;
    let reservesCacheEntries: number | null = null;
    
    let dexArbHeartbeat: string | null = null;
    let triArbHeartbeat: string | null = null;
    let flArbHeartbeat: string | null = null;
    let liqHeartbeat: string | null = null;
    let heartbeatReason: string | null = "no_per_strategy_heartbeat_source";

    try {
      // SCAN counts the whole keyspace per pattern; run the 4 independent
      // patterns concurrently instead of 4 serial awaits.
      const scanCounts = await Promise.all([
        countKeysScan(deps.redis, `arbx:pool_index:${chainId}:*`),
        countKeysScan(deps.redis, `arbx:pool_index_v3:${chainId}:*`),
        countKeysScan(deps.redis, `arbx:pool_reserves:${chainId}:*`),
        countKeysScan(deps.redis, `arbx:v3_slot0:${chainId}:*`),
      ]);
      poolIndexEntries = scanCounts[0];
      poolIndexV3Entries = scanCounts[1];
      reservesCacheEntries = scanCounts[2] + scanCounts[3];

      const heartbeatKey = `arbx:heartbeat:scanner:${chainId}:latest`;
      const heartbeatRaw = await deps.redis.get(heartbeatKey);
      if (heartbeatRaw) {
        const heartbeat = JSON.parse(heartbeatRaw);
        // Map available telemetry to approximate readiness. Real per-strategy heartbeat is not yet emitted.
        if (heartbeat.emitted_at_unix) {
          dexArbHeartbeat = new Date(heartbeat.emitted_at_unix * 1000).toISOString();
        }
        if (heartbeat.triangular_cycles_scanned > 0) {
          triArbHeartbeat = dexArbHeartbeat;
        }
        if (heartbeat.flashloan_arb_pairs_scanned > 0) {
          flArbHeartbeat = dexArbHeartbeat;
        }
        if (heartbeat.liquidation_positions_scanned > 0) {
          liqHeartbeat = dexArbHeartbeat;
        }
      }
    } catch (e) {
      deps.logger.warn({ event: "strategy_readiness.redis_failed", err: (e as Error).message });
      sourceStatus.redis = "unavailable";
      heartbeatReason = "redis_unavailable";
    }

    res.status(200).json({
      chain_id: chainId,
      source: sourceStatus,
      readiness: {
        pool_index: {
          entries: poolIndexEntries,
          source: "redis",
          reason: poolIndexEntries === null ? "redis_unavailable" : null,
        },
        pool_index_v3: {
          entries: poolIndexV3Entries,
          source: "redis",
          reason: poolIndexV3Entries === null ? "redis_unavailable" : null,
        },
        reserves_cache: {
          entries: reservesCacheEntries,
          source: "redis",
          reason: reservesCacheEntries === null ? "redis_unavailable" : null,
        },
        strategy_heartbeat: {
          dex_arb: dexArbHeartbeat,
          triangular_arb: triArbHeartbeat,
          flashloan_arb: flArbHeartbeat,
          liquidation: liqHeartbeat,
          source: "redis",
          reason: heartbeatReason,
        }
      },
      ts: new Date().toISOString()
    });
  });
}
