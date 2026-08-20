/**
 * G-PRICE-1 — GET /api/v1/prices/live REST snapshot of USD token prices.
 *
 * Same canonical read as the WS `prices:snapshot` event (HGETALL of
 * `arbx:token_prices:<chain_id>` + TTL), exposed for:
 *   - SSR initial snapshots (R1 mounted-snapshot pattern),
 *   - the frontend's polling fallback when the WS degrades,
 *   - operator L4 verification (curl target).
 *
 * Contract:
 *   - `?chain_id=` REQUIRED, positive integer. Anything else → 400
 *     (fail-fast, no silent mainnet default — RULE 02 mindset).
 *   - 200 `{ chain_id, prices, count, ttl_secs, ts }` — `prices` maps
 *     uppercase symbol → USD price. Empty object = hash absent/empty (R8
 *     honest: worker hasn't ticked yet, NOT a fabricated feed).
 *   - 503 `{ error: "redis_unavailable" }` when Redis is not configured.
 *
 * Proxied through the edge worker as GET /api/prices/live (pass-through,
 * no KV cache — freshness is the point).
 */

import type { Application } from "express";
import type { Redis } from "ioredis";

type Logger = { info: (obj: object, msg?: string) => void; error: (obj: object, msg?: string) => void };

export function mountPricesLive(app: Application, redis: Redis | null, logger?: Logger): void {
    app.get("/api/v1/prices/live", async (req, res) => {
        const rawChain = Number(req.query["chain_id"]);
        if (!Number.isInteger(rawChain) || rawChain <= 0 || rawChain > 2 ** 32 - 1) {
            res.status(400).json({ error: "invalid_chain_id", detail: "chain_id query param (positive integer) is required" });
            return;
        }
        if (!redis) {
            logger?.error({ event: "prices_live.redis_missing" }, "prices/live called without a Redis connection");
            res.status(503).json({ error: "redis_unavailable" });
            return;
        }
        const key = `arbx:token_prices:${rawChain}`;
        try {
            // TTL first: on a missing key HGETALL returns {} and TTL -2 — reading
            // TTL after a possible EXPIRE-refresh race would just be slightly
            // stale, which is acceptable for a worst-case staleness bound.
            const ttl = await redis.ttl(key);
            const raw = (await redis.hgetall(key)) as unknown as Record<string, string>;
            const prices: Record<string, number> = {};
            for (const [sym, valStr] of Object.entries(raw)) {
                const v = Number(valStr);
                if (Number.isFinite(v) && v > 0) {
                    prices[sym.toUpperCase()] = v;
                }
            }
            res.status(200).json({
                chain_id: rawChain,
                prices,
                count: Object.keys(prices).length,
                ttl_secs: ttl > 0 ? ttl : null,
                ts: new Date().toISOString(),
            });
        } catch (e) {
            logger?.error({ event: "prices_live.read_failed", err: (e as Error).message, chain_id: rawChain }, "prices/live Redis read failed");
            res.status(503).json({ error: "redis_unavailable" });
        }
    });
}
