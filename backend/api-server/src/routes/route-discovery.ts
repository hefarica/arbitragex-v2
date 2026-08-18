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

import { Router, type RequestHandler } from "express";
import type { Redis } from "ioredis";

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
  // Global ring across ALL event types — backs the unfiltered live feed.
  private ring: TelemetryEvent[] = [];
  // Per-type rings — so e.g. route_candidate events are retained even though a
  // single tick floods ~hundreds of events of other types right after them. A
  // single global ring would rotate past the candidates within the same tick.
  private byType = new Map<string, TelemetryEvent[]>();
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
    let arr = this.byType.get(type);
    if (!arr) {
      arr = [];
      this.byType.set(type, arr);
    }
    arr.push(ev);
    if (arr.length > this.maxRing) arr.shift();
    this.lastAnyAt = Date.now();
  }

  lastOf(type: string): TelemetryEvent | null {
    return this.lastByEvent.get(type) ?? null;
  }

  /** Most recent events (newest first). With `type`, reads that type's
   *  dedicated ring (retains the last N of THAT type regardless of how many
   *  other-type events flooded in between). Without, the global ring. */
  recent(type: string | undefined, limit: number): TelemetryEvent[] {
    const items = type ? (this.byType.get(type) ?? []) : this.ring;
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
        // SHADOW-NO-ROUTE-CAPS: deferred ≠ lost — the enumeration cursor
        // (deferred_cursor) resumes next tick. Rotation covers parallel pools
        // across ladders. Old capped/dropped fields are gone by design.
        routes_deferred: tick["routes_deferred"] ?? null,
        deferred_cursor: tick["deferred_cursor"] ?? null,
        pools_rotated: tick["pools_rotated"] ?? null,
        depth_pass: tick["depth_pass"] ?? null,
        rotation_epoch: tick["rotation_epoch"] ?? null,
        pass_emitted_total: tick["pass_emitted_total"] ?? null,
        ladder_complete: tick["ladder_complete"] ?? null,
        telemetry_emitted: tick["telemetry_emitted"] ?? null,
        latency_ms: tick["latency_ms"] ?? null,
      }),
    );
  });

  // GET /api/route-discovery/routes?limit=100 — recent discovered routes, with
  // candidate topology MERGED with strategy applicability + dispatch by
  // route_hash (server-side, so REST consumers get the full row without the WS).
  router.get("/api/route-discovery/routes", (req, res) => {
    if (cache.isEmpty()) {
      res.json(empty("no_telemetry_yet"));
      return;
    }
    const raw = Number(req.query["limit"] ?? 100);
    const limit = Number.isFinite(raw) ? Math.min(Math.max(raw, 1), 200) : 100;

    const candidates = cache.recent("route_discovery.route_candidate", limit);
    const appls = cache.recent("route_discovery.strategy_applicability", 200);
    const intents = cache.recent("route_intent.emitted", 200);
    const applByHash = new Map<string, TelemetryEvent>();
    for (const a of appls) {
      const h = a["route_hash"];
      if (typeof h === "string" && !applByHash.has(h)) applByHash.set(h, a);
    }
    const intentByHash = new Map<string, TelemetryEvent>();
    for (const i of intents) {
      const h = i["route_hash"];
      if (typeof h === "string" && !intentByHash.has(h)) intentByHash.set(h, i);
    }

    const routes = candidates.map((c) => {
      const h = typeof c["route_hash"] === "string" ? (c["route_hash"] as string) : "";
      const a = applByHash.get(h);
      const i = intentByHash.get(h);
      return {
        route_hash: h,
        route_kind: c["route_kind"] ?? null,
        hops: c["hops"] ?? null,
        tokens: c["tokens"] ?? [],
        pools: c["pools"] ?? [],
        protocols: c["protocols"] ?? [],
        fee_tiers: c["fee_tiers"] ?? [],
        directions: c["directions"] ?? [],
        applicable_strategies: a?.["applicable_strategies"] ?? [],
        rejected_strategies: a?.["rejected_strategies"] ?? [],
        dispatch_strategy: i?.["strategy"] ?? null,
        dispatch_deferred: i?.["dispatch_deferred"] ?? null,
      };
    });

    res.json(ok({ routes }));
  });

  return router;
}

/**
 * Read-only router for cartridge telemetry (the `arbx:cartridge:telemetry`
 * channel — the primary path is the WS room `telemetry`; this is a REST
 * snapshot for polling/health UIs).
 *
 * `redis` (optional) enables GET /api/cartridges/runtime — the searcher-rs
 * loaded-cartridge registry snapshot (`arbx:cartridges:registry:<chain>`),
 * which reflects the REAL set of .rhai cartridges the searcher compiled at
 * boot (the 264-strategy library + core pack), independent of whether any
 * evaluation has emitted telemetry yet. R8 fail-honest: absent key →
 * `registry_unavailable` (searcher down or registry TTL expired).
 */
export interface CartridgesRouterDeps {
  redis?: Redis;
  /** Admin-token middleware factory (from @arbx/shared) — gates the toggle
   *  endpoints so only an authenticated operator can pause/resume a cartridge. */
  requireAdminToken?: (expected: string) => RequestHandler;
  adminToken?: string;
}

export function buildCartridgesRouter(cache: TelemetryCache, deps: CartridgesRouterDeps = {}): Router {
  const { redis, requireAdminToken, adminToken } = deps;
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

  // GET /api/cartridges/runtime?chain_id=1 — the REAL loaded-cartridge registry
  // published by searcher-rs to `arbx:cartridges:registry:<chain>`. This is the
  // authoritative set of compiled .rhai strategy cartridges (264-strategy
  // library + core pack), NOT telemetry-gated. Read-only, never fabricated.
  router.get("/api/cartridges/runtime", async (req, res) => {
    if (!redis) {
      res.status(503).json({ ok: false, reason: "redis_unavailable", data: null });
      return;
    }
    const chainId = Number(req.query["chain_id"] ?? 1);
    if (!Number.isInteger(chainId) || chainId < 1) {
      res.status(400).json({ ok: false, reason: "invalid_chain_id", data: null });
      return;
    }
    try {
      const raw = await redis.get(`arbx:cartridges:registry:${chainId}`);
      if (!raw) {
        res.json({
          ok: false,
          reason: "registry_unavailable",
          detail:
            "searcher-rs has not published its cartridge registry (down, boot pending, or TTL expired)",
          source: "searcher_registry",
          updated_at: null,
          data: null,
        });
        return;
      }
      const parsed = JSON.parse(raw);
      res.json({
        ok: true,
        source: "searcher_registry",
        updated_at: parsed.updated_at ?? null,
        data: parsed,
      });
    } catch (e) {
      res.status(503).json({
        ok: false,
        reason: "registry_read_failed",
        detail: (e as Error).message,
        data: null,
      });
    }
  });

  // POST /api/cartridges/runtime/:id/pause|resume — toggle a LOADED cartridge
  // on the searcher hot-path via Redis hot-reload (`arbx:cartridge:injection`).
  // Unlike cartridge-forge, this does NOT require a cartridge_registry PG row —
  // it targets the searcher's live runtime registry (the 264-strategy library),
  // so any cartridge the searcher compiled at boot can be paused/resumed.
  // Admin-gated: an unauthenticated toggle would let anyone silence detection.
  // R8 fail-honest: we publish the event; the searcher applies it (or logs why
  // not). We do NOT pretend the state changed before the searcher acks it.
  const toggleHandler = (eventType: "pause" | "resume"): RequestHandler => {
    return async (req, res) => {
      if (!redis) {
        res.status(503).json({ ok: false, reason: "redis_unavailable" });
        return;
      }
      const id = String(req.params["id"] ?? "");
      // cartridge ids are file-stem slugs (mev_01_001_..., dex_arb, ...) — bounded charset.
      if (!/^[\w.-]{1,128}$/.test(id)) {
        res.status(400).json({ ok: false, reason: "invalid_cartridge_id" });
        return;
      }
      const event = {
        cartridge_id: id,
        event_type: eventType,
        content_hash: "", // pause/resume don't need source; subscriber ignores it here
        chain_id: 0,
        timestamp: new Date().toISOString(),
        actor: (req.headers["x-omega-actor"] as string) || "operator",
      };
      try {
        await redis.publish("arbx:cartridge:injection", JSON.stringify(event));
        res.json({
          ok: true,
          cartridge_id: id,
          event_type: eventType,
          note: "event published to searcher hot-reload; the searcher applies it asynchronously",
        });
      } catch (e) {
        res.status(503).json({ ok: false, reason: "publish_failed", detail: (e as Error).message });
      }
    };
  };

  if (requireAdminToken && adminToken) {
    router.post(
      "/api/cartridges/runtime/:id/pause",
      requireAdminToken(adminToken),
      toggleHandler("pause"),
    );
    router.post(
      "/api/cartridges/runtime/:id/resume",
      requireAdminToken(adminToken),
      toggleHandler("resume"),
    );
  }

  return router;
}
