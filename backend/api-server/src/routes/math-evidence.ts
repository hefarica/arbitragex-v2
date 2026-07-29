/**
 * GET /api/math/evidence?chain_id=1&strategy_kind=Dex
 *
 * Serves the LIVE math-evidence snapshots the searcher persists to Redis
 * (`arbx:math_evidence:<chain>:<strategy_kind>`) — the detected market regime
 * plus the per-operator computed values for a strategy, in real time.
 *
 * R8 fail-honest: absent key (searcher quiet / TTL expired) →
 * { ok:false, reason:"no_recent_evidence" } — never a fabricated regime.
 */

import { Router, type Request, type Response } from "express";
import type { Redis } from "ioredis";

interface Deps {
  redis?: Redis;
  logger: { warn: (obj: object, msg?: string) => void };
}

export function buildMathEvidenceRouter(deps: Deps): Router {
  const r = Router();

  r.get("/api/math/evidence", async (req: Request, res: Response): Promise<void> => {
    if (!deps.redis) {
      res.status(503).json({ ok: false, reason: "redis_unavailable" });
      return;
    }
    const chainId = Number(req.query["chain_id"] ?? 1);
    const strategy = String(req.query["strategy_kind"] ?? "");
    if (!Number.isInteger(chainId) || chainId < 1) {
      res.status(400).json({ ok: false, reason: "invalid_chain_id" });
      return;
    }
    if (!/^[\w:.-]{1,64}$/.test(strategy)) {
      res.status(400).json({ ok: false, reason: "invalid_strategy_kind" });
      return;
    }
    try {
      const raw = await deps.redis.get(`arbx:math_evidence:${chainId}:${strategy}`);
      if (!raw) {
        res.json({
          ok: false,
          reason: "no_recent_evidence",
          detail:
            "searcher has not persisted math evidence for this strategy recently (quiet, boot pending, or TTL expired)",
          data: null,
        });
        return;
      }
      res.json({ ok: true, source: "searcher_math_evidence", data: JSON.parse(raw) });
    } catch (e) {
      deps.logger.warn({ event: "math_evidence.read_failed", err: (e as Error).message });
      res.status(503).json({ ok: false, reason: "read_failed", detail: (e as Error).message });
    }
  });

  // List all strategies with recent evidence for a chain (SCAN over the prefix).
  r.get("/api/math/evidence/all", async (req: Request, res: Response): Promise<void> => {
    if (!deps.redis) {
      res.status(503).json({ ok: false, reason: "redis_unavailable" });
      return;
    }
    const chainId = Number(req.query["chain_id"] ?? 1);
    if (!Number.isInteger(chainId) || chainId < 1) {
      res.status(400).json({ ok: false, reason: "invalid_chain_id" });
      return;
    }
    try {
      const prefix = `arbx:math_evidence:${chainId}:`;
      const keys: string[] = [];
      let cursor = "0";
      do {
        const [next, batch] = await deps.redis.scan(cursor, "MATCH", `${prefix}*`, "COUNT", 200);
        cursor = next;
        keys.push(...batch);
      } while (cursor !== "0");

      const items: Array<Record<string, unknown>> = [];
      for (const key of keys) {
        const raw = await deps.redis.get(key);
        if (raw) {
          try {
            items.push(JSON.parse(raw));
          } catch {
            /* skip malformed */
          }
        }
      }
      res.json({ ok: true, chain_id: chainId, count: items.length, items });
    } catch (e) {
      deps.logger.warn({ event: "math_evidence.scan_failed", err: (e as Error).message });
      res.status(503).json({ ok: false, reason: "scan_failed", detail: (e as Error).message });
    }
  });

  return r;
}
