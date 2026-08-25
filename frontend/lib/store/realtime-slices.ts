/**
 * =============================================================================
 * FE-MASTER · Realtime slice for the Omni-Store (FE-0008, §33)
 * =============================================================================
 *
 * The per-channel connection policy state OWNED by ArbxRealtimeProvider —
 * the single mount that owns every realtime surface's cadence (WS rooms
 * where they exist, REST snapshot polling where they do not). FE-0009's
 * global posture bar renders EXACTLY this slice; nothing else writes it.
 *
 * RULE 00 — the channel set is the set of surfaces that have BOTH a wire
 * today AND a store setter waiting for it (all verified against source):
 *
 *   routes       WS room `route_discovery` (subscribe:route_discovery →
 *                `route_discovery_telemetry`; the worker publishes the SAME
 *                enriched tick_summary to the channel and the durable
 *                snapshot, route_discovery_worker.rs:1350-1355) + REST
 *                fallback GET /api/route-discovery/tick → TelemetrySlice.
 *   runtime_ack  WS room `runtime_ack` (admin capability re-checked
 *                server-side, websocket.ts:313-324) → RuntimeAckSlice.
 *   pairs        REST only (EMIT-06 — no WS room) → PairsSlice.
 *   quote_anchor REST only (EMIT-02) → QuoteAnchorSlice.
 *
 * Explicitly DEFERRED (no store surface yet, never fabricated states):
 *   health/drift leaves (FE-0006/0007), cartridge `telemetry` room,
 *   `metrics`/`convergence`/`prices` rooms, and the opportunities feed —
 * the opportunities page keeps its own tested page-local lifecycle
 * (useOpportunitiesStream + WS-POLL-1); consolidating it under the
 * provider is a follow-up once FE-0009 consumers exist.
 *
 * R8: `lastMessageAt = null` means "never accepted a payload" — status
 * stays honest (`connecting`/`disconnected`), never a fake `live`.
 */

import type { StoreApi } from "zustand";

import { RouteDiscoveryTickSummarySchema, type RouteDiscoveryTickSummary } from "@/lib/apex/schemas";
import { RuntimeAckBroadcastSchema, type RuntimeAckBroadcast } from "@/lib/statemachine/useRuntimeAckSocket";

// ─── Channel model ──────────────────────────────────────────────────────────

export const REALTIME_CHANNELS = [
  "routes",
  "runtime_ack",
  "pairs",
  "quote_anchor",
] as const;

export type RealtimeChannelId = (typeof REALTIME_CHANNELS)[number];

/** How the channel is being fed RIGHT NOW. */
export type ChannelTransport = "ws" | "rest";

/**
 * FE-0009's badge vocabulary, as the raw truth the provider can certify:
 *   connecting  — mount state, nothing accepted yet
 *   live        — transport delivering accepted payloads
 *   polling     — WS down, REST fallback loop running
 *   stale       — transport nominally up but no accepted payload within
 *                 3× the channel's cadence budget (honest gap marker)
 *   disconnected— no transport at all (pre-connect or after teardown)
 */
export type ChannelStatus =
  | "connecting"
  | "live"
  | "polling"
  | "stale"
  | "disconnected";

export interface RealtimeChannelState {
  transport: ChannelTransport;
  status: ChannelStatus;
  /** ISO timestamp of the last ACCEPTED payload (WS push or REST fetch). */
  lastMessageAt: string | null;
  /** Last honest failure string (schema reject, fetch error). Never cleared silently. */
  lastError: string | null;
}

export interface RealtimeSlice {
  channels: Record<RealtimeChannelId, RealtimeChannelState>;
  /** Global socket.io liveness (one connection for all WS channels). */
  wsConnected: boolean;
  setChannel: (id: RealtimeChannelId, patch: Partial<RealtimeChannelState>) => void;
  setWsConnected: (connected: boolean) => void;
}

export function createRealtimeSlice(
  set: StoreApi<RealtimeSlice>["setState"],
): RealtimeSlice {
  const blank = (): RealtimeChannelState => ({
    transport: "rest",
    status: "connecting",
    lastMessageAt: null,
    lastError: null,
  });
  const channels = {} as Record<RealtimeChannelId, RealtimeChannelState>;
  for (const id of REALTIME_CHANNELS) channels[id] = blank();
  return {
    channels,
    wsConnected: false,
    setChannel: (id, patch) =>
      set((state) => ({
        channels: { ...state.channels, [id]: { ...state.channels[id], ...patch } },
      })),
    setWsConnected: (connected) => set({ wsConnected: connected }),
  };
}

// ─── Pure policy helpers (unit-tested directly) ────────────────────────────

/**
 * Acceptance gate for `route_discovery_telemetry` payloads. The room carries
 * FIVE event types sharing only the `event` discriminator; only the tick
 * event carries the summary shape. Non-tick events are a legitimate
 * `ignored` (not an error); a tick-shaped payload that fails the schema is
 * a hard reject — the caller must NOT setTick (RG-1 fail-closed).
 */
export type TickAcceptance =
  | { ok: true; tick: RouteDiscoveryTickSummary }
  | { ok: false; kind: "ignored"; reason: "not_tick_event" }
  | { ok: false; kind: "rejected"; reason: string };

export function acceptTickPayload(raw: unknown): TickAcceptance {
  const ev = (raw as { event?: unknown } | null | undefined)?.event;
  if (typeof ev !== "string" || ev !== "route_discovery.tick") {
    return { ok: false, kind: "ignored", reason: "not_tick_event" };
  }
  const parsed = RouteDiscoveryTickSummarySchema.safeParse(raw);
  if (!parsed.success) {
    const first = parsed.error.issues[0];
    return {
      ok: false,
      kind: "rejected",
      reason: `schema_reject: ${first ? `${first.path.join(".") || "(root)"} ${first.message}` : "unknown"}`,
    };
  }
  return { ok: true, tick: parsed.data };
}

/**
 * Acceptance gate for `runtime_ack` broadcasts — the same 1:1 Zod the page
 * hook uses (RG-1/RG-2): malformed never reaches recordAck.
 */
export type AckAcceptance =
  | { ok: true; ack: RuntimeAckBroadcast }
  | { ok: false; reason: string };

export function acceptRuntimeAck(raw: unknown): AckAcceptance {
  const parsed = RuntimeAckBroadcastSchema.safeParse(raw);
  if (!parsed.success) {
    const first = parsed.error.issues[0];
    return {
      ok: false,
      reason: `schema_reject: ${first ? `${first.path.join(".") || "(root)"} ${first.message}` : "unknown"}`,
    };
  }
  return { ok: true, ack: parsed.data };
}

/**
 * Staleness budgets (ms) — 3× each surface's real cadence contract:
 * routes tick ~30s per loop; pairs/anchor snapshots TTL 35s (SET .. EX).
 * runtime_ack has NO budget: it is event-driven (a POST-driven broadcast),
 * and inventing a cadence for it would fabricate a gap that is not one.
 */
export const STALENESS_BUDGET_MS: Record<RealtimeChannelId, number> = {
  routes: 90_000,
  runtime_ack: Number.POSITIVE_INFINITY,
  pairs: 105_000,
  quote_anchor: 105_000,
};

/**
 * Pure sweep: which channels have an accepted payload older than their
 * budget. Never flips a channel that has not accepted anything yet
 * (`lastMessageAt === null` is "connecting", not "stale" — R8).
 */
export function staleChannels(
  channels: Record<RealtimeChannelId, RealtimeChannelState>,
  nowMs: number,
): RealtimeChannelId[] {
  const stale: RealtimeChannelId[] = [];
  for (const id of REALTIME_CHANNELS) {
    const ch = channels[id];
    if (!ch.lastMessageAt) continue;
    const age = nowMs - Date.parse(ch.lastMessageAt);
    if (Number.isFinite(age) && age > STALENESS_BUDGET_MS[id]) stale.push(id);
  }
  return stale;
}
