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
 * Wire contract (legacy, unchanged):
 *   client → server : `subscribe:prices`  payload `{ chain_id: number }`
 *   server → client : `prices:snapshot`   `{ chain_id, prices, count, ttl_secs, ts, seq }`
 *   server → client : `prices:update`     same shape (full-map replace)
 *   server → client : `prices:error`      `{ code, chain_id? }`
 *
 * `seq` is a per-chain monotonic counter incremented on every broadcast —
 * clients can detect missed frames after a reconnect (snapshot resets it).
 *
 * ── WO-PRICE-EXCHANGE-V1 (frozen wire contract) ────────────────────────────
 * Additive events on the SAME room, backed by an in-process versioned mirror
 * (`price-delta.ts`) fed from the canonical hash + the Rust meta sidecar
 * (`arbx:token_prices:meta:<chain>`, source/ts per token):
 *   server → client : `price_snapshot`  `{ chain_id, prices, count, ttl_secs, ts, seq, version }`
 *                      — emitted on subscribe, on `resync:prices`, and on the
 *                        subscriber's Redis reconnect (re-read covers notices
 *                        missed while disconnected; verdict e: keyspace-notify
 *                        is REJECTED, resync+re-read covers pub/sub gaps).
 *   server → client : `price_delta`     `{ chain_id, seq, version, ts, deltas: [{ token, prev, next, source, ts, seq }] }`
 *                      — ε = 1bp relative gates EMISSION never the value;
 *                        source-switch emits even at identical price.
 *   client → server : `resync:prices`   `{ chain_id }` → fresh `price_snapshot`.
 * A client that observes `seq !== lastSeq + 1` (gap) emits `resync:prices`.
 *
 * OHLC write-behind (verdict d): every notice feeds the 1-minute aggregator
 * (`price-history.ts`) which flushes closed buckets into `price_history`
 * (migration 122) on a low-frequency timer — no volume column (RULE 00).
 */

import type { Server } from "socket.io";
import type { Pool } from "pg";
import { Redis } from "ioredis";
import {
    type MirrorState,
    type TokenMeta,
    applyNotice,
    mirrorFromSnapshot,
    parseMetaHash,
} from "./price-delta.js";
import { OhlcAggregator, flushPriceHistoryBuckets } from "./price-history.js";

const PRICES_ROOM_PREFIX = "prices:";
const PRICES_UPDATED_CHANNEL_PATTERN = "arbx:prices:updated:*";
/** Redis hash key — MUST match `sharedRs::price_oracle::redis_token_prices_key`. */
const tokenPricesKey = (chainId: number): string => `arbx:token_prices:${chainId}`;
/** Sidecar hash key — MUST match `sharedRs::price_oracle::token_prices_meta_key`. */
const tokenPricesMetaKey = (chainId: number): string => `arbx:token_prices:meta:${chainId}`;
const roomFor = (chainId: number): string => `${PRICES_ROOM_PREFIX}${chainId}`;

/** Per-chain monotonic sequence stamped on every broadcast. */
const seqByChain = new Map<number, number>();
const nextSeq = (chainId: number): number => {
    const n = (seqByChain.get(chainId) ?? 0) + 1;
    seqByChain.set(chainId, n);
    return n;
};

/** WO-PRICE-EXCHANGE-V1 in-process versioned mirror, per chain. Shared by the
 * subscribe path and the pub/sub bridge (same process — verdict c: mirror in
 * api-server, NOT a separate service). */
const mirrors = new Map<number, MirrorState>();

/** OHLC 1-min write-behind aggregator (fed on every notice, flushed by timer). */
const ohlc = new OhlcAggregator();
const OHLC_FLUSH_INTERVAL_MS = 30_000;

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

/** Extended snapshot of the frozen WO-PRICE-EXCHANGE-V1 contract. */
export interface PriceSnapshotFrame extends PricesSnapshot {
    /** Mirror version — increments whenever the token map changes. */
    version: number;
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

interface ChainRead {
    prices: Record<string, number>;
    meta: Record<string, TokenMeta>;
    ttl: number;
    tsMs: number;
}

/** One canonical read of both hashes (prices + meta sidecar) for a chain. */
async function readChain(cmdRedis: Redis, chainId: number): Promise<ChainRead> {
    const [raw, rawMeta, ttl] = await Promise.all([
        cmdRedis.hgetall(tokenPricesKey(chainId)) as unknown as Promise<Record<string, string>>,
        cmdRedis.hgetall(tokenPricesMetaKey(chainId)) as unknown as Promise<Record<string, string>>,
        cmdRedis.ttl(tokenPricesKey(chainId)),
    ]);
    return { prices: parsePricesHash(raw), meta: parseMetaHash(rawMeta), ttl, tsMs: Date.now() };
}

/** Read the canonical hash and build the legacy snapshot payload. `null` ttl
 * means the key does not exist (worker hasn't ticked yet / expired). */
function legacyFrameFrom(read: ChainRead, chainId: number): PricesSnapshot {
    return {
        chain_id: chainId,
        prices: read.prices,
        count: Object.keys(read.prices).length,
        ttl_secs: read.ttl > 0 ? read.ttl : null,
        ts: new Date(read.tsMs).toISOString(),
        seq: nextSeq(chainId),
    };
}

/** WO-PRICE-EXCHANGE-V1: rebuild the mirror from a read (seed prev-values, no
 * deltas — the snapshot is the authoritative paint) and return the frame. */
function framedSnapshotFrom(read: ChainRead, chainId: number): PriceSnapshotFrame {
    const prevMirror = mirrors.get(chainId);
    const mirror = mirrorFromSnapshot(chainId, read.prices, read.meta, read.tsMs, prevMirror?.noticeSeq ?? 0);
    mirror.version = (prevMirror?.version ?? 0) + 1;
    mirrors.set(chainId, mirror);
    return {
        chain_id: chainId,
        prices: read.prices,
        count: Object.keys(read.prices).length,
        ttl_secs: read.ttl > 0 ? read.ttl : null,
        ts: new Date(read.tsMs).toISOString(),
        seq: mirror.noticeSeq,
        version: mirror.version,
    };
}

function isValidChainId(v: unknown): v is number {
    return typeof v === "number" && Number.isInteger(v) && v > 0 && v <= 2 ** 32 - 1;
}

/**
 * Register the `subscribe:prices` + `resync:prices` room handlers. Call ONCE
 * after `setupWebSocketGateway()` — multiple `io.on('connection')` listeners
 * are additive by design (EventEmitter semantics), so this never disturbs the
 * gateway's own handlers.
 *
 * @param io    — Socket.IO Server instance
 * @param redis — shared command connection (regular commands; NOT in subscriber mode)
 */
export function attachPriceRooms(io: Server, redis: Redis): void {
    io.on("connection", (socket) => {
        const chainIdFrom = (payload: unknown): number | null => {
            const chainId = (payload as { chain_id?: unknown } | null | undefined)?.chain_id;
            return isValidChainId(chainId) ? chainId : null;
        };
        socket.on("subscribe:prices", async (payload: unknown) => {
            const chainId = chainIdFrom(payload);
            if (chainId === null) {
                socket.emit("prices:error", { code: "invalid_chain_id", detail: "expected integer chain_id" });
                return;
            }
            socket.join(roomFor(chainId));
            try {
                const read = await readChain(redis, chainId);
                const framed = framedSnapshotFrom(read, chainId);
                socket.emit("prices:snapshot", legacyFrameFrom(read, chainId));
                socket.emit("price_snapshot", framed);
                console.log(`[PricesStream] socket ${socket.id} subscribed to ${roomFor(chainId)} (${framed.count} prices)`);
            } catch (err) {
                // R8: surface the failure, never a fabricated snapshot.
                socket.emit("prices:error", { code: "redis_unavailable", chain_id: chainId });
                console.error(`[PricesStream] snapshot failed for chain ${chainId}:`, (err as Error).message);
            }
        });
        // WO-PRICE-EXCHANGE-V1: client-detected seq gap → full re-read + repaint.
        socket.on("resync:prices", async (payload: unknown) => {
            const chainId = chainIdFrom(payload);
            if (chainId === null) {
                socket.emit("prices:error", { code: "invalid_chain_id", detail: "expected integer chain_id" });
                return;
            }
            try {
                const read = await readChain(redis, chainId);
                socket.emit("price_snapshot", framedSnapshotFrom(read, chainId));
            } catch (err) {
                socket.emit("prices:error", { code: "redis_unavailable", chain_id: chainId });
                console.error(`[PricesStream] resync failed for chain ${chainId}:`, (err as Error).message);
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
 * WO-PRICE-EXCHANGE-V1: every notice advances the in-process mirror (emitting
 * `price_delta` frames when ε/source rules fire) and feeds the OHLC 1-min
 * write-behind aggregator; on subscriber reconnect the bridge re-reads every
 * known chain and pushes a fresh `price_snapshot` (verdict e — resync +
 * re-read instead of keyspace-notifications).
 *
 * @param io    — Socket.IO Server instance
 * @param redisUrl — Redis URL for the dedicated subscriber + command clients
 * @param opts  — `pool` (optional): pg.Pool for the OHLC write-behind flush.
 *                Absent/null ⇒ OHLC disabled (no consumer, honest no-op).
 * @returns the subscriber instance (for shutdown cleanup)
 */
export function subscribeToPriceUpdates(io: Server, redisUrl: string, opts: { pool?: Pool | null } = {}): Redis {
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
        // Verdict e: notices missed while disconnected are recovered by a
        // re-read of every known chain — no keyspace-notify dependency.
        void resyncAllKnownChains(io, cmdRedis);
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

    // OHLC write-behind flush timer (verdict d). Low frequency by design: the
    // hot path never blocks on PG; closed buckets flush within ~30s of close.
    const ohlcPool = opts.pool ?? null;
    let ohlcTimer: ReturnType<typeof setInterval> | null = null;
    if (ohlcPool) {
        ohlcTimer = setInterval(() => {
            const closed = ohlc.rollover(Date.now());
            if (closed.length === 0) return;
            flushPriceHistoryBuckets(ohlcPool, closed)
                .then((n) => console.log(`[PricesStream] price_history write-behind flushed ${n} bucket(s)`))
                .catch((err: Error) =>
                    console.error(`[PricesStream] price_history flush failed (bucket(s) dropped, R8):`, err.message),
                );
        }, OHLC_FLUSH_INTERVAL_MS);
        ohlcTimer.unref?.();
    }

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
        if (clients === 0 && !ohlcPool) {
            return; // Nobody listening, no history consumer — skip the HGETALL.
        }
        try {
            // Single canonical read feeds BOTH the mirror (delta engine) and
            // the OHLC aggregator — one HGETALL pair per notice, not two.
            const read = await readChain(cmdRedis, chainId);
            const { state, frame } = applyNotice(mirrors.get(chainId), chainId, read.prices, read.meta, read.tsMs);
            mirrors.set(chainId, state);
            // OHLC (verdict d): EVERY observed price feeds history — the ε
            // emission gate must NOT create blind spots in the record.
            for (const [sym, price] of Object.entries(read.prices)) {
                ohlc.apply(chainId, sym, price, read.tsMs);
            }
            if (clients > 0) {
                const legacy = legacyFrameFrom(read, chainId);
                io.to(room).emit("prices:update", legacy);
                if (frame.deltas.length > 0) {
                    io.to(room).emit("price_delta", frame);
                }
                console.log(`[PricesStream] ${room} <- ${source} update (${legacy.count} prices, ${frame.deltas.length} delta(s), ${clients} client(s))`);
            }
        } catch (err) {
            console.error(`[PricesStream] broadcast failed for chain ${chainId}:`, (err as Error).message);
        }
    });

    return subscriber;
}

/** Re-read every chain the mirror has seen and push a fresh `price_snapshot`
 * to its room (subscriber-reconnect recovery; also collapses any drift). */
async function resyncAllKnownChains(io: Server, cmdRedis: Redis): Promise<void> {
    for (const chainId of Array.from(mirrors.keys())) {
        const room = roomFor(chainId);
        const clients = io.sockets.adapter.rooms.get(room)?.size ?? 0;
        if (clients === 0) continue;
        try {
            const read = await readChain(cmdRedis, chainId);
            io.to(room).emit("price_snapshot", framedSnapshotFrom(read, chainId));
            console.log(`[PricesStream] ${room} resync re-read pushed after subscriber ready`);
        } catch (err) {
            console.error(`[PricesStream] resync re-read failed for chain ${chainId}:`, (err as Error).message);
        }
    }
}
