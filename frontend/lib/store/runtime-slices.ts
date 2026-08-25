/**
 * =============================================================================
 * FE-MASTER · Runtime slices for the Omni-Store (FE-0004, tramo 1)
 * =============================================================================
 *
 * Extends the Omni-Store (§32) with the runtime instrumentation state whose
 * wire contracts ALREADY EXIST end-to-end:
 *
 *   - UniverseSlice    token-universe KPIs + last resolve preview
 *                      (ARBX-FE-EMIT-01 · apex/schemas/tokens.ts)
 *   - TelemetrySlice   route-discovery tick funnel + lat.* stages
 *                      (ARBX-FE-EMIT-05 · apex/schemas/telemetry.ts)
 *   - VersionSlice     runtime versions (quote/graph/universe/config) — all
 *                      nullable: filled as EMIT-02/04 land (R8: null = not
 *                      served, never 0)
 *   - RuntimeAckSlice  last Runtime ACK broadcast + capped log, recorded by
 *                      the ACK consumers (FE-0005/0008) for global posture
 *
 * Tramo 2 landed in catalog-slices.ts (StrategiesSlice/DetectorsSlice/
 * PairsSlice over the P5–P7 shape contracts + EMIT-06/07/08 payloads).
 *
 * Performance rules of omni-store.ts apply unchanged: selectors only,
 * never `useOmniStore()` whole-store subscription.
 */

import type { StoreApi } from "zustand";
import type {
  RouteDiscoveryTickSummary,
  TokenResolveResponse,
  TokenUniverseKpi,
} from "@/lib/apex/schemas";
import { getRouteDiscoveryTick } from "@/lib/api-client";
import type { RuntimeAckBroadcast } from "@/lib/statemachine/useRuntimeAckSocket";

/** Shared fetch lifecycle for slow-poll runtime surfaces. */
export type FetchStatus = "idle" | "loading" | "ready" | "error";

// =============================================================================
// Universe slice — token-universe consequence KPIs (§6) + resolve preview (§5)
// =============================================================================

export interface UniverseSlice {
  /** Last universe KPIs served by the backend (null = never served). */
  universe: TokenUniverseKpi | null;
  /** Last resolve preview (transient — the tab owns the working copy). */
  lastResolve: TokenResolveResponse | null;
  /** Record KPIs (called after a successful resolve; standalone GET = EMIT-04). */
  setUniverse: (kpis: TokenUniverseKpi) => void;
  /** Record a full resolve response (results stay tab-local, KPIs land here). */
  setResolveResult: (res: TokenResolveResponse) => void;
}

export function createUniverseSlice(
  set: StoreApi<UniverseSlice>["setState"],
): UniverseSlice {
  return {
    universe: null,
    lastResolve: null,
    setUniverse: (kpis) => set({ universe: kpis }),
    setResolveResult: (res) => set({ universe: res.universe, lastResolve: res }),
  };
}

// =============================================================================
// Telemetry slice — route-discovery tick (funnel §18 + latency §43-44)
// =============================================================================

export interface TelemetrySlice {
  /** Latest tick summary snapshot (null = never fetched / not available). */
  tick: RouteDiscoveryTickSummary | null;
  tickStatus: FetchStatus;
  /** Honest error string (404 universe/tick absent, 503, parse drift…). */
  tickError: string | null;
  tickUpdatedAt: string | null;
  /** Fetch once — cadence/retry policy belongs to the realtime provider (FE-0008). */
  fetchTick: (chainId?: number) => Promise<void>;
  /** Direct setter for the realtime provider's WS push path (when it exists). */
  setTick: (tick: RouteDiscoveryTickSummary) => void;
}

export function createTelemetrySlice(
  set: StoreApi<TelemetrySlice>["setState"],
  get: () => TelemetrySlice,
): TelemetrySlice {
  return {
    tick: null,
    tickStatus: "idle",
    tickError: null,
    tickUpdatedAt: null,
    fetchTick: async (chainId = 1) => {
      if (get().tickStatus === "loading") return;
      set({ tickStatus: "loading", tickError: null });
      const res = await getRouteDiscoveryTick(chainId);
      if (res.ok) {
        set({
          tick: res.data,
          tickStatus: "ready",
          tickUpdatedAt: new Date().toISOString(),
        });
      } else {
        // 404 = no snapshot yet (searcher down / restarted): honest absence,
        // surfaced as error text — NEVER a zeroed funnel (RULE 00).
        set({ tickStatus: "error", tickError: res.error });
      }
    },
    setTick: (tick) =>
      set({ tick, tickStatus: "ready", tickUpdatedAt: new Date().toISOString() }),
  };
}

// =============================================================================
// Version slice — runtime versions for configured-vs-effective surfaces (§11)
// =============================================================================

export interface RuntimeVersions {
  quote_version: number | null;
  graph_version: number | null;
  universe_version: number | null;
  config_version: number | null;
}

export interface VersionSlice {
  versions: RuntimeVersions;
  /** Partial merge — any EMIT payload that carries a version applies it. */
  applyVersions: (partial: Partial<RuntimeVersions>) => void;
}

export function createVersionSlice(
  set: StoreApi<VersionSlice>["setState"],
): VersionSlice {
  return {
    versions: { quote_version: null, graph_version: null, universe_version: null, config_version: null },
    applyVersions: (partial) =>
      set((state) => ({ versions: { ...state.versions, ...partial } })),
  };
}

// =============================================================================
// Runtime ACK slice — global visibility of the last ACK broadcast (§3/§35)
// =============================================================================

const MAX_ACK_LOG = 50;

export interface RuntimeAckSlice {
  lastAck: RuntimeAckBroadcast | null;
  /** Newest-first, capped — global audit trail for the posture bar. */
  ackLog: RuntimeAckBroadcast[];
  /** Record a validated ACK (callers pass only schema-parsed payloads). */
  recordAck: (ack: RuntimeAckBroadcast) => void;
}

export function createRuntimeAckSlice(
  set: StoreApi<RuntimeAckSlice>["setState"],
): RuntimeAckSlice {
  return {
    lastAck: null,
    ackLog: [],
    recordAck: (ack) =>
      set((state) => ({
        lastAck: ack,
        ackLog: [ack, ...state.ackLog].slice(0, MAX_ACK_LOG),
      })),
  };
}
