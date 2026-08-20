/**
 * G-PRICE-1 — Price stream bridge: Redis pub/sub → WebSocket (snapshot + push).
 *
 * Exchange-style market-data pattern for USD token prices:
 *   1. Client connects and emits `subscribe:prices` with `{ chain_id }`.
 *   2. Server joins the socket into room `prices:<chain_id>` and immediately
 *      emits `prices:snapshot` (HGETALL of `arbx:token_prices:<chain_id>`).
 *   3. Every writer that persists prices (searcher-rs price_worker,
 *      token-enricher DexScreener / GeckoTerminal tiers) PUBLISHes a notice on
 *      `arbx:prices:updated:<chain_id>` inside its atomic write pipeline.
 *   4. This bridge re-reads the canonical hash and broadcasts `prices:update`
 *      to the room. The Redis hash stays the single source of truth — the
 *      pub/sub payload carries only a source label, never the data (no drift).
 *
 * Fail-honest (R8): a missing/empty hash yields `prices: []` — never
 * fabricated. A Redis failure emits `prices:error` to the requesting socket.
 * Non-fatal by design: the rest of the WS gateway keeps working.
 *
 * Wire contract:
 *   client → server : `subscribe:prices`  payload `{ chain_id: number }`
 *   server → client : `prices:snapshot`   `{ chain_id, prices, count, ttl_secs, ts, seq }`
 *   server → client : `prices:update`     same shape (full-map replace)
 *   server → client : `prices:error`      `{ code, chain_id? }`
 *
 * `seq` is a per-chain monotonic counter incremented on every broadcast —
 * clients can detect missed frames after a reconnect (snapshot resets it).
 */

import type { Server } from "socket.io";
import { Redis } from "ioredis";

const PRICES_ROOM_PREFIX = "prices:";
const PRICES_UPDATED_CHANNEL_PATTERN = "arbx:prices:updated:*";
/** Redis hash key — MUST match `sharedRs::price_oracle::redis_token_prices_key`. */
const tokenPricesKey = (chainId: number): string => `arbx:token_prices:${chainId}`;
const roomFor = (chainId: number): string => `${PRICES_ROOM_PREFIX}${chainId}`;

/** Per-chain monotonic sequence stamped on every broadcast. */
const seqByChain = new Map<number, number>();
const nextSeq = (chainId: number): number => {
    const n = (seqByChain.get(chainId) ?? 0) + 1;
    seqByChain.set(chainId, n);
    return n;
};

export interface PricesSnapshot {
    chain_id: number;
    /** Uppercase symbol → USD price. Empty object = hash absent/empty (R8). */
    prices: Record<string, number>;
    count: number;
    /** Remaining key TTL in seconds (worst-case staleness bound for the client). */
    ttl_secs: number | null;
    /** Server wall-clock ISO timestamp of the read. */
    ts: string;
    seq: number;
}

/** Parse + validate a raw `{ symbol: string }` hash into `{ symbol: number }`.
 * Mirrors the Rust reader's `RedisCachedPriceOracle` validation: non-finite /
 * non-positive prices are dropped (never surfaced, never defaulted). */
function parsePricesHash(raw: Record<string, string>): Record<string, number> {
    const out: Record<string, number> = {};
    for (const [sym, valStr] of Object.entries(raw)) {
        const v = Number(valStr);
        if (Number.isFinite(v) && v > 0) {
            out[sym.toUpperCase()] = v;
        }
    }
    return out;
}

/** Read the canonical hash and build the snapshot payload. `null` ttl means
 * the key does not exist (worker hasn't ticked yet / expired). */
async function buildSnapshot(cmdRedis: Redis, chainId: number): Promise<PricesSnapshot> {
    const key = tokenPricesKey(chainId);
    const raw = (await cmdRedis.hgetall(key)) as unknown as Record<string, string>;
    const ttl = await cmdRedis.ttl(key);
    const prices = parsePricesHash(raw);
    return {
        chain_id: chainId,
        prices,
        count: Object.keys(prices).length,
        ttl_secs: ttl > 0 ? ttl : null,
        ts: new Date().toISOString(),
        seq: nextSeq(chainId),
    };
}

function isValidChainId(v: unknown): v is number {
    return typeof v === "number" && Number.isInteger(v) && v > 0 && v <= 2 ** 32 - 1;
}

/**
 * Register the `subscribe:prices` room handler. Call ONCE after
 * `setupWebSocketGateway()` — multiple `io.on('connection')` listeners are
 * additive by design (EventEmitter semantics), so this never disturbs the
 * gateway's own handlers.
 *
 * @param io    — Socket.IO Server instance
 * @param redis — shared command connection (regular commands; NOT in subscriber mode)
 */
export function attachPriceRooms(io: Server, redis: Redis): void {
    io.on("connection", (socket) => {
        socket.on("subscribe:prices", async (payload: unknown) => {
            const chainId = (payload as { chain_id?: unknown } | null | undefined)?.chain_id;
            if (!isValidChainId(chainId)) {
                socket.emit("prices:error", { code: "invalid_chain_id", detail: "expected integer chain_id" });
                return;
            }
            socket.join(roomFor(chainId));
            try {
                const snapshot = await buildSnapshot(redis, chainId);
                socket.emit("prices:snapshot", snapshot);
                console.log(`[PricesStream] socket ${socket.id} subscribed to ${roomFor(chainId)} (${snapshot.count} prices)`);
            } catch (err) {
                // R8: surface the failure, never a fabricated snapshot.
                socket.emit("prices:error", { code: "redis_unavailable", chain_id: chainId });
                console.error(`[PricesStream] snapshot failed for chain ${chainId}:`, (err as Error).message);
            }
        });
    });
}

/**
 * Bridge writer notifications → room broadcasts. Call ONCE from `index.ts`
 * with a DEDICATED subscriber connection (ioredis forbids mixing SUBSCRIBE
 * mode with regular commands on one connection, hence the separate command
 * client created here). Fail-honest: on Redis errors the bridge logs and
 * relies on auto-reconnect; subscribed sockets simply stop receiving
 * `prices:update` until recovery (the REST snapshot remains available).
 *
 * @returns the subscriber instance (for shutdown cleanup)
 */
export function subscribeToPriceUpdates(io: Server, redisUrl: string): Redis {
    const subscriber = new Redis(redisUrl, {
        lazyConnect: false,
        maxRetriesPerRequest: 1,
        retryStrategy(times: number) {
            const delay = Math.min(times * 50, 2000);
            console.log(`[PricesStream] Redis subscriber reconnect attempt ${times}, retrying in ${delay}ms`);
            return delay;
        },
        reconnectOnError(err) {
            const targetErrors = ["ECONNREFUSED", "ETIMEDOUT", "ECONNRESET", "EHOSTUNREACH"];
            return targetErrors.some((code) => err.message.includes(code)) ? 2 : false;
        },
    });
    const cmdRedis = new Redis(redisUrl, {
        lazyConnect: false,
        maxRetriesPerRequest: 1,
        retryStrategy(times: number) {
            return Math.min(times * 50, 2000);
        },
    });

    subscriber.on("ready", () => {
        console.log("[PricesStream] Redis subscriber ready");
    });
    const onErr = (role: string) => (err: Error) => {
        // Non-fatal — the rest of the WSS gateway keeps working.
        console.error(`[PricesStream] Redis ${role} error (non-fatal):`, err.message);
    };
    subscriber.on("error", onErr("subscriber"));
    cmdRedis.on("error", onErr("command"));

    subscriber.psubscribe(PRICES_UPDATED_CHANNEL_PATTERN).then(() => {
        console.log(`[PricesStream] Subscribed to Redis channel pattern: ${PRICES_UPDATED_CHANNEL_PATTERN}`);
    }).catch((err: Error) => {
        console.error(`[PricesStream] Failed to psubscribe ${PRICES_UPDATED_CHANNEL_PATTERN}:`, err.message);
    });

    subscriber.on("pmessage", async (_pattern: string, channel: string, message: string) => {
        const chainId = Number(channel.split(":").pop());
        if (!isValidChainId(chainId)) {
            console.warn(`[PricesStream] Unparseable channel "${channel}", skipping`);
            return;
        }
        // Log the notice at debug-level volume: writers tick every 15-60s so
        // this is a handful of lines per minute across all sources.
        let source = "unknown";
        try {
            const parsed = JSON.parse(message) as { source?: string };
            if (typeof parsed.source === "string") source = parsed.source;
        } catch {
            // Notice payload is informational only — a malformed one still
            // means the hash changed; proceed with the canonical re-read.
        }
        const room = roomFor(chainId);
        const clients = io.sockets.adapter.rooms.get(room)?.size ?? 0;
        if (clients === 0) {
            return; // Nobody listening — skip the HGETALL (hot-path discipline).
        }
        try {
            const snapshot = await buildSnapshot(cmdRedis, chainId);
            io.to(room).emit("prices:update", snapshot);
            console.log(`[PricesStream] ${room} <- ${source} update (${snapshot.count} prices, ${clients} client(s))`);
        } catch (err) {
            console.error(`[PricesStream] broadcast failed for chain ${chainId}:`, (err as Error).message);
        }
    });

    return subscriber;
}
