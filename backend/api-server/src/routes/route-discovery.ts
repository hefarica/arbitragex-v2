/**
 * QUANTUM FULLSTACK SYMMETRY — read-only REST surface for the OMEGA Route
 * Discovery radar + cartridge telemetry.
 *
 * The Rust searcher PUBLISHes route-discovery telemetry to the Redis channel
 * `arbx:route_discovery:telemetry` (fire-and-forget; nothing persists it). To
 * answer REST polls without inventing data, the WS bridge feeds every event it
 * receives into an in-memory {@link TelemetryCache}; these routers read that
 * cache. R8 fail-honest: when no telemetry has arrived yet, endpoints return
 * `{ ok: false, reason }` (HTTP 200, never 500, never fabricated metrics).
 *
 * RULE 00 / NO-ACTIVE: these endpoints are strictly read-only observation. They
 * never touch `arbx:opps:detected`, never trigger execution, never mutate state.
 */

import { Router } from "express";

/** A single observational telemetry event. Open shape — `event` is the
 *  discriminator the Rust producer always sets; other fields vary per type. */
export interface TelemetryEvent {
  event?: string;
  [k: string]: unknown;
}

/**
 * Bounded in-memory cache of the most recent telemetry events on one channel.
 * Keeps: the last event per `event` type, a rolling ring of recent events, and
 * per-type counters. No persistence, no fabrication — empty until the first
 * real event arrives.
 */
export class TelemetryCache {
  private lastByEvent = new Map<string, TelemetryEvent>();
  private ring: TelemetryEvent[] = [];
  private counts = new Map<string, number>();
  private lastAnyAt: number | null = null;
  private readonly maxRing: number;

  constructor(maxRing = 200) {
    this.maxRing = maxRing;
  }

  record(ev: TelemetryEvent): void {
    const type = typeof ev.event === "string" ? ev.event : "unknown";
    this.lastByEvent.set(type, ev);
    this.counts.set(type, (this.counts.get(type) ?? 0) + 1);
    this.ring.push(ev);
    if (this.ring.length > this.maxRing) this.ring.shift();
    this.lastAnyAt = Date.now();
  }

  lastOf(type: string): TelemetryEvent | null {
    return this.lastByEvent.get(type) ?? null;
  }

  /** Most recent events (newest first), optionally filtered by `event` type. */
  recent(type: string | undefined, limit: number): TelemetryEvent[] {
    const items = type ? this.ring.filter((e) => e.event === type) : this.ring;
    return items.slice(-limit).reverse();
  }

  countsObj(): Record<string, number> {
    return Object.fromEntries(this.counts);
  }

  lastAt(): number | null {
    return this.lastAnyAt;
  }

  isEmpty(): boolean {
    return this.lastAnyAt === null;
  }
}

function isoOrNull(ms: number | null): string | null {
  return ms === null ? null : new Date(ms).toISOString();
}

/**
 * Read-only router for the route-discovery radar. Mounted at `/` (paths are
 * fully qualified). Mode is DERIVED from the last tick's `mode` field (never
 * hardcoded — RULE 00).
 */
export function buildRouteDiscoveryRouter(cache: TelemetryCache): Router {
  const router = Router();

  const mode = (): string => {
    const tick = cache.lastOf("route_discovery.tick") as
      | { mode?: unknown }
      | null;
    return typeof tick?.mode === "string" ? tick.mode : "unknown";
  };

  const ok = (data: unknown) => ({
    ok: true as const,
    source: "redis_pubsub_cache",
    mode: mode(),
    updated_at: isoOrNull(cache.lastAt()),
    data,
  });

  const empty = (reason: string) => ({
    ok: false as const,
    reason,
    source: "redis_pubsub_cache",
    mode: "off_or_idle",
    updated_at: null,
    data: null,
  });

  // GET /api/route-discovery/status — last tick + per-event counts.
  router.get("/api/route-discovery/status", (_req, res) => {
    if (cache.isEmpty()) {
      res.json(empty("no_telemetry_yet"));
      return;
    }
    res.json(
      ok({
        last_tick: cache.lastOf("route_discovery.tick"),
        counts: cache.countsObj(),
      }),
    );
  });

  // GET /api/route-discovery/latest — last tick + the most recent N events.
  router.get("/api/route-discovery/latest", (_req, res) => {
    if (cache.isEmpty()) {
      res.json(empty("no_telemetry_yet"));
      return;
    }
    res.json(
      ok({
        tick: cache.lastOf("route_discovery.tick"),
        recent: cache.recent(undefined, 20),
      }),
    );
  });

  // GET /api/route-discovery/metrics — flattened tick gauges (honest nulls).
  router.get("/api/route-discovery/metrics", (_req, res) => {
    if (cache.isEmpty()) {
      res.json(empty("no_telemetry_yet"));
      return;
    }
    const tick = (cache.lastOf("route_discovery.tick") ?? {}) as Record<
      string,
      unknown
    >;
    res.json(
      ok({
        counts: cache.countsObj(),
        algorithm: tick["algorithm"] ?? null,
        pools_total: tick["pools_total"] ?? null,
        edges_built: tick["edges_built"] ?? null,
        edges_rejected: tick["edges_rejected"] ?? null,
        routes_found: tick["routes_found"] ?? null,
        routes_dispatched: tick["routes_dispatched"] ?? null,
        routes_dropped_for_cap: tick["routes_dropped_for_cap"] ?? null,
        routes_capped: tick["routes_capped"] ?? null,
        telemetry_emitted: tick["telemetry_emitted"] ?? null,
        latency_ms: tick["latency_ms"] ?? null,
      }),
    );
  });

  // GET /api/route-discovery/routes?limit=100 — recent discovered candidates.
  router.get("/api/route-discovery/routes", (req, res) => {
    if (cache.isEmpty()) {
      res.json(empty("no_telemetry_yet"));
      return;
    }
    const raw = Number(req.query["limit"] ?? 100);
    const limit = Number.isFinite(raw) ? Math.min(Math.max(raw, 1), 200) : 100;
    res.json(
      ok({
        routes: cache.recent("route_discovery.route_candidate", limit),
      }),
    );
  });

  return router;
}

/**
 * Read-only router for cartridge telemetry (the `arbx:cartridge:telemetry`
 * channel — the primary path is the WS room `telemetry`; this is a REST
 * snapshot for polling/health UIs).
 */
export function buildCartridgesRouter(cache: TelemetryCache): Router {
  const router = Router();

  const ok = (data: unknown) => ({
    ok: true as const,
    source: "redis_pubsub_cache",
    mode: "shadow",
    updated_at: isoOrNull(cache.lastAt()),
    data,
  });

  const empty = (reason: string) => ({
    ok: false as const,
    reason,
    source: "redis_pubsub_cache",
    mode: "off_or_idle",
    updated_at: null,
    data: null,
  });

  // GET /api/cartridges/status — last message + counts.
  router.get("/api/cartridges/status", (_req, res) => {
    if (cache.isEmpty()) {
      res.json(empty("no_telemetry_yet"));
      return;
    }
    res.json(
      ok({
        last: cache.recent(undefined, 1)[0] ?? null,
        counts: cache.countsObj(),
      }),
    );
  });

  // GET /api/cartridges/telemetry/latest — most recent N cartridge messages.
  router.get("/api/cartridges/telemetry/latest", (req, res) => {
    if (cache.isEmpty()) {
      res.json(empty("no_telemetry_yet"));
      return;
    }
    const raw = Number(req.query["limit"] ?? 50);
    const limit = Number.isFinite(raw) ? Math.min(Math.max(raw, 1), 200) : 50;
    res.json(ok({ messages: cache.recent(undefined, limit) }));
  });

  return router;
}
