/**
 * CHANNELS-01 (2026-09-26) — GET /api/v1/channels
 *
 * Fail-honest (R8/R10) read-only health surface for the Redis Streams bus:
 * per declared stream — exists, XLEN, producer heartbeat (last-entry id
 * timestamp), consumer groups (last-delivered, pending), consumers with
 * idle/pending, zombie detection (pending == 0 && idle > threshold), and
 * oldest-pending age.
 *
 * Doctrine:
 *  - null means "not available" — NEVER coerced to 0 (R8).
 *  - A stream that errors degrades ONLY itself; the envelope stays 200 with
 *    per-stream `error` (partial). Redis entirely unreachable → 503.
 *  - No producer-side fabrication: the producer heartbeat is the timestamp
 *    embedded in the stream's last-generated-id, read from Redis itself.
 *  - Declared stream list is infrastructural topology (system architecture),
 *    overridable without rebuild via ARBX_CHANNELS_STREAMS (comma-separated).
 *  - Mount pattern per arbx-api-server-route-mounting skill (DI, no anonymous
 *    index.ts endpoints); ioredis raw-array replies parsed by pure exported
 *    helpers (WO-15 lesson: XINFO arrives as raw arrays).
 */

import type { Application, Request, Response } from "express";
import type { Redis } from "ioredis";

// CHANNELS-REGISTRY (R10): single source of truth for the declared stream
// list. This route reports the RUNTIME truth; the registry holds the
// DECLARED-WIRING truth — the list is never forked.
import { CANONICAL_CHANNEL_REGISTRY } from "./channels-registry.js";

export const DEFAULT_CHANNELS: readonly string[] = CANONICAL_CHANNEL_REGISTRY.map(
  (e) => e.name,
);

const DEFAULT_ZOMBIE_IDLE_MS = 300_000;

export function channelsStreamList(raw: string | undefined): string[] {
  const parsed = (raw ?? "")
    .split(",")
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
  return parsed.length > 0 ? parsed : [...DEFAULT_CHANNELS];
}

/** ioredis XINFO replies are flat [k1, v1, k2, v2, ...] arrays (WO-15). */
export function parseFlatPairs(raw: unknown): Record<string, unknown> {
  if (!Array.isArray(raw)) return {};
  const out: Record<string, unknown> = {};
  for (let i = 0; i + 1 < raw.length; i += 2) {
    out[String(raw[i])] = raw[i + 1];
  }
  return out;
}

/** "1698000000000-0" → 1698000000000 (ms); null when unparseable. */
export function tsFromId(id: unknown): number | null {
  if (typeof id !== "string") return null;
  const head = Number(id.split("-", 1)[0]);
  return Number.isFinite(head) ? head : null;
}

export interface ChannelConsumerReport {
  name: string;
  pending: number | null;
  idle_ms: number | null;
  zombie: boolean | null;
}

export interface ChannelGroupReport {
  name: string;
  pending: number | null;
  consumers: number | null;
  last_delivered_ms: number | null;
  oldest_pending_ms: number | null;
  consumer_reports: ChannelConsumerReport[];
}

export interface ChannelReport {
  stream: string;
  exists: boolean;
  error: string | null;
  xlen: number | null;
  producer_last_entry_ms: number | null;
  groups: ChannelGroupReport[] | null;
}

function num(v: unknown): number | null {
  const n = Number(v);
  return Number.isFinite(n) ? n : null;
}

async function reportGroup(
  redis: Redis,
  stream: string,
  raw: unknown,
  zombieIdleMs: number,
): Promise<ChannelGroupReport> {
  const g = parseFlatPairs(raw);
  const name = String(g["name"] ?? "unknown");
  const out: ChannelGroupReport = {
    name,
    pending: num(g["pending"]),
    consumers: num(g["consumers"]),
    last_delivered_ms: tsFromId(g["last-delivered-id"]),
    oldest_pending_ms: null,
    consumer_reports: [],
  };
  // Oldest pending: XPENDING summary reply = [count, minId, maxId, [...]].
  try {
    const pend = (await redis.xpending(stream, name)) as unknown;
    if (Array.isArray(pend) && pend.length >= 2) {
      out.oldest_pending_ms = tsFromId(pend[1]);
    }
  } catch {
    // group vanished mid-probe → oldest stays null (fail-honest)
  }
  // Consumers: XINFO CONSUMERS (raw flat-array rows, WO-15).
  try {
    const rows = (await redis.xinfo("CONSUMERS", stream, name)) as unknown[];
    const list = Array.isArray(rows) ? rows : [];
    out.consumer_reports = list.map((row) => {
      const c = parseFlatPairs(row);
      const idle = num(c["idle"]);
      const pending = num(c["pending"]);
      return {
        name: String(c["name"] ?? "unknown"),
        pending,
        idle_ms: idle,
        zombie: idle === null || pending === null ? null : pending === 0 && idle > zombieIdleMs,
      };
    });
  } catch {
    // consumer listing failed → keep empty, do not fabricate
  }
  return out;
}

async function reportStream(
  redis: Redis,
  stream: string,
  zombieIdleMs: number,
): Promise<ChannelReport> {
  const base: ChannelReport = {
    stream,
    exists: false,
    error: null,
    xlen: null,
    producer_last_entry_ms: null,
    groups: null,
  };
  // Missing stream: ioredis throws "ERR no such key" on XLEN/XINFO.
  let xlen: number;
  try {
    xlen = await redis.xlen(stream);
  } catch (e) {
    base.error = (e as Error).message.slice(0, 160);
    return base;
  }
  base.exists = true;
  base.xlen = xlen;
  try {
    const info = parseFlatPairs(await redis.xinfo("STREAM", stream));
    base.producer_last_entry_ms = tsFromId(info["last-generated-id"]);
  } catch {
    // heartbeat stays null (fail-honest), stream still reported
  }
  try {
    const rows = (await redis.xinfo("GROUPS", stream)) as unknown[];
    const list = Array.isArray(rows) ? rows : [];
    base.groups = await Promise.all(
      list.map((row) => reportGroup(redis, stream, row, zombieIdleMs)),
    );
  } catch (e) {
    base.groups = null;
    base.error = `groups: ${(e as Error).message.slice(0, 120)}`;
  }
  return base;
}

export function mountChannels(
  app: Application,
  deps: { redis: Redis | null; logger: { warn: (obj: object, msg?: string) => void } },
): void {
  app.get("/api/v1/channels", async (_req: Request, res: Response) => {
    if (!deps.redis) {
      return res.status(503).json({ error: "redis_unavailable" });
    }
    const streams = channelsStreamList(process.env["ARBX_CHANNELS_STREAMS"]);
    const zombieIdleMs = Math.max(
      1000,
      Number(process.env["ARBX_CHANNELS_ZOMBIE_IDLE_MS"] ?? DEFAULT_ZOMBIE_IDLE_MS) ||
        DEFAULT_ZOMBIE_IDLE_MS,
    );
    try {
      const channels = await Promise.all(
        streams.map((s) => reportStream(deps.redis as Redis, s, zombieIdleMs)),
      );
      const source = { redis: "ok" as const };
      return res.status(200).json({
        source,
        generated_at: new Date().toISOString(),
        zombie_idle_threshold_ms: zombieIdleMs,
        channels,
      });
    } catch (e) {
      deps.logger.warn(
        { event: "channels.redis_failed", err: (e as Error).message },
        "channels endpoint degraded",
      );
      return res.status(503).json({ error: "redis_unavailable", detail: (e as Error).message });
    }
  });
}
