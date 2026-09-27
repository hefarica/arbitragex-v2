/**
 * =============================================================================
 * OMEGA OMNI-STORE — Single Source of Truth
 * =============================================================================
 *
 * Architecture: Zustand with Slices pattern.
 * - RegistrySlice: Static configuration (Chains, Dexes, Pools)
 * - OpportunitySlice: High-frequency mempool stream
 * - WalletSlice: Connection and balances
 *
 * Performance Rules:
 * 1. NEVER use `const state = useOmniStore()` — subscribes to everything
 * 2. ALWAYS use selectors: `useOmniStore((state) => state.opportunities)`
 * 3. Use `useShallow` for object/array selectors to prevent unnecessary re-renders
 *
 * R8 Fail-Honest: Status fields surface errors; never fabricate data.
 */

import { create, type StoreApi } from "zustand";
import { devtools } from "zustand/middleware";

// =============================================================================
// Type Imports (from existing canonical types)
// =============================================================================

import type { Chain, DEX, Pool } from "@/lib/registries/types";
import type { OmniOpportunity } from "./types";
import { routeGroupKeyOf } from "./route-key";
import type { WalletRow } from "@/lib/api/wallets";
import { getApiBaseUrl } from "@/lib/api-client";
import {
  createRuntimeAckSlice,
  createTelemetrySlice,
  createUniverseSlice,
  createVersionSlice,
  type RuntimeAckSlice,
  type RuntimeVersions,
  type TelemetrySlice,
  type UniverseSlice,
  type VersionSlice,
  type FetchStatus,
} from "./runtime-slices";
import {
  createRealtimeSlice,
  type RealtimeSlice,
} from "./realtime-slices";
import {
  createDetectorsSlice,
  createPairsSlice,
  createStrategiesSlice,
  type DetectorsSlice,
  type PairsSlice,
  type StrategiesSlice,
} from "./catalog-slices";
import {
  createQuoteAnchorSlice,
  type QuoteAnchorSlice,
} from "./quote-slices";

// Re-export the FE-MASTER runtime slice types for consumers.
export type {
  RuntimeAckSlice,
  RuntimeVersions,
  TelemetrySlice,
  UniverseSlice,
  VersionSlice,
  FetchStatus,
} from "./runtime-slices";
export type {
  DetectorsSlice,
  PairsSlice,
  StrategiesSlice,
} from "./catalog-slices";
export type { QuoteAnchorSlice } from "./quote-slices";

// Re-export OmniOpportunity as the canonical opportunity type
export type { OmniOpportunity } from "./types";

/** WebSocket connection status (extends socket-lifecycle.ts with POLLING fallback) */
export type WsStatus = "CONNECTING" | "LIVE" | "STALE" | "POLLING" | "DISCONNECTED";

/** Registry loading status */
export type RegistryStatus = "idle" | "loading" | "ready" | "error";

// =============================================================================
// Slice 1: Registry — Static configuration data
// =============================================================================

interface RegistrySlice {
  /** Chain metadata indexed by chain_id */
  chains: Map<number, Chain>;
  /** DEX configurations indexed by dex_id */
  dexes: Map<string, DEX>;
  /** Pool configurations indexed by address */
  pools: Map<string, Pool>;
  /** Loading status for registry data */
  registryStatus: RegistryStatus;
  /** Error message if registry fetch failed */
  registryError: string | null;
  /** Fetch all registry data from API (parallel calls) */
  fetchRegistry: (chainId?: number) => Promise<void>;
  /** Get a specific chain by ID */
  getChain: (chainId: number) => Chain | undefined;
  /** Get a specific DEX by ID */
  getDex: (dexId: string) => DEX | undefined;
  /** Get a specific pool by address */
  getPool: (address: string) => Pool | undefined;
}

// =============================================================================
// Slice 2: Opportunity — High-frequency mempool stream
// =============================================================================

interface OpportunitySlice {
  /** Live opportunities from mempool (capped at MAX_OPPORTUNITIES) */
  opportunities: OmniOpportunity[];
  /**
   * AUDIT-CARDS-MINOR (§2) · WO-H4: distinct routes in the ≤5 min window of
   * the LAST /api/opportunities/live snapshot envelope (COUNT(*) OVER ()).
   * R8: null until a snapshot carrying the field arrives — absent is never
   * invented as 0; a real 0 (computed empty window) is stored as 0.
   */
  windowTotal: number | null;
  /** WebSocket connection status */
  wsStatus: WsStatus;
  /** ISO timestamp of last received opportunity */
  lastUpdate: string | null;
  /** Connect to WebSocket stream */
  connectStream: () => void;
  /** Disconnect from WebSocket stream */
  disconnectStream: () => void;
  /** Add a new opportunity (called by WebSocket handler) */
  addOpportunity: (opp: OmniOpportunity) => void;
  /** Replace the entire opportunity list in a single update (batch) */
  setOpportunities: (opps: OmniOpportunity[]) => void;
  /** Set window_total from a snapshot envelope (null = absent, R8) */
  setWindowTotal: (total: number | null) => void;
  /**
   * MEM-RENDER-01: drop opportunities detected more than maxAgeMs ago.
   * Vigency eviction — the live grid keeps only active/vigent cards instead
   * of retaining the last 200 unique detections for hours.
   */
  pruneStale: (maxAgeMs: number) => void;
  /** Clear all opportunities */
  clearOpportunities: () => void;
  /** Update WS status (called by socket lifecycle) */
  setWsStatus: (status: WsStatus) => void;
}

// =============================================================================
// Slice 3: Wallet — Connection and balances
// =============================================================================

interface WalletSlice {
  /** Connected wallet address (checksummed) */
  address: `0x${string}` | null;
  /** Native token balance in wei */
  balance: bigint | null;
  /** Whether wallet is connected */
  isConnected: boolean;
  /** Current chain ID */
  chainId: number | null;
  /** Set wallet connection */
  setWallet: (address: `0x${string}`, balance: bigint, chainId: number) => void;
  /** Update balance */
  setBalance: (balance: bigint) => void;
  /** Disconnect wallet */
  disconnect: () => void;
  /** Map of all known wallets indexed by address */
  wallets: Map<string, WalletRow>;
  /** Fetch all wallets from API */
  fetchWallets: () => Promise<void>;
}

// =============================================================================
// Composed Store Type
// =============================================================================

type OmniStoreState = RegistrySlice &
  OpportunitySlice &
  WalletSlice &
  UniverseSlice &
  TelemetrySlice &
  VersionSlice &
  RuntimeAckSlice &
  StrategiesSlice &
  DetectorsSlice &
  PairsSlice &
  QuoteAnchorSlice &
  RealtimeSlice;

// =============================================================================
// Constants
// =============================================================================

/** Maximum opportunities to keep in memory (prevents memory leak) */
const MAX_OPPORTUNITIES = 200;

// CARDS-DEDUP-HOPS (2026-09-20): ISO min/max helpers for rolling the vigency
// aggregates when a re-detection of the same route arrives. null-safe — a row
// without a parseable timestamp keeps null (R8: undated ≠ epoch).
function earlierIso(a: string | null, b: string | null): string | null {
  if (a == null) return b;
  if (b == null) return a;
  return Date.parse(b) < Date.parse(a) ? b : a;
}
function laterIso(a: string | null, b: string | null): string | null {
  if (a == null) return b;
  if (b == null) return a;
  return Date.parse(b) > Date.parse(a) ? b : a;
}

/**
 * CARDS-PRECEDENCE-01 — THE PRECEDENCE CONTRACT (operator order 2026-09-26).
 *
 * Two transports feed ONE card (R7):
 *   · SNAPSHOT — `GET /api/opportunities/live` (edge → api-server LIVE_QUERY).
 *     It is the ONLY producer of the ENRICHED shape: token metadata/validation,
 *     chain base token, leg symbols, live PriceBus prices, and the whole
 *     simulated_* ladder + target.
 *   · WS PUSH — `new_opportunity` = `row_to_json(opportunities)` via PG NOTIFY.
 *     Its exact key set is 26 raw columns and is DOCUMENTED by the producer
 *     itself (`backend/api-server/src/websocket.ts:506-514`): it carries the
 *     core economics but NOT the REST-only enrichments.
 *
 * CONTRACT: a value that WAS COMPUTED may never be replaced by a placeholder
 * produced by a transport that never computed it. Concretely:
 *   1. A grouped SNAPSHOT row (server aggregates present) is SSOT — verbatim.
 *   2. A RAW row may only ADD what it actually carries. For every field the
 *      raw transport does not carry, the previously computed value survives.
 *   3. `None ≠ Some(0)` (R8): absence is not zero, and an absent value never
 *      overwrites a present one.
 *
 * The lists below are the machine-readable form of the producer's own contract
 * at `websocket.ts:506-514`. Adding an enriched field to the snapshot means
 * adding it HERE, in one place — never a code branch per field.
 */

/**
 * Fields the raw WS transport does not emit at all; their incoming mapper
 * value is `null`/`undefined`. Preserve the previous computed value.
 */
const RAW_ABSENT_NULL_FIELDS: ReadonlyArray<keyof OmniOpportunity> = [
  // REST-only token enrichment (token_in_info/token_out_info/leg_symbols/
  // chain_base_token_symbol are named verbatim in websocket.ts:509-510).
  "token_in_info",
  "token_out_info",
  "chain_base_token_symbol",
  "leg_symbols",
  // Live PriceBus prices, enriched per request by the LIVE_QUERY.
  "token_prices_usd",
  // The whole forward-sim ladder + inverse-sizing target.
  "raw_simulated_net_profit_usd",
  "simulated_net_profit_usd",
  "simulated_amount_in_usd",
  "simulated_roi_pct",
  "simulated_cost_breakdown",
  // CARDS-NOTIONAL-01: the SIM ladder's own gross + Σcosts. REST-only, like the
  // rest of the ladder — a raw WS row must not blank the closed triple and drop
  // the card onto a borrowed (canonical) gross.
  "simulated_gross_usd",
  "simulated_costs_total_usd",
  "simulated_target",
  "simulated_at",
  "simulated_notes",
  // REST-only lifecycle derivation (websocket.ts:510).
  "paper_status",
  // ECON-DECLARE-01 (2026-09-27): the persisted route topology is REST-only —
  // `publisher::publish` serializes the `Opportunity` row and `route_metadata`
  // lives in its OWN JSONB column, so a raw WS row's mapper value is `null`.
  // Spreading it over the snapshot therefore WIPED the topology on the first live
  // update, taking with it `hop_count` and the per-hop ledger. That wipe is also
  // what would have re-opened the operator's declaration gap at runtime: the
  // notional-basis declaration rides the same JSONB
  // (`economics_amount_in_wei` / `economics_basis`), so losing the column turns a
  // fully declared row back into `@undeclared` mid-session — a value that shows
  // and then vanishes, which is worse than one that never showed.
  //
  // Safe to preserve: `routeGroupKeyOf` (chain, chain_out, strategy, token_in,
  // token_out, dex_a, dex_b — the LIVE_QUERY GROUP BY twin) does NOT include
  // `route_metadata`, so a different topology is a different route group and can
  // never be hidden by this rule. Same reasoning as the SIM ladder above.
  "route_metadata",
];

/**
 * Fields the raw transport does not emit AND whose mapper default is an EMPTY
 * ARRAY rather than null (`types.ts:486-491`). `[]` here is a mapper artifact,
 * never a computed "no chains/dexes on a real route" — so it counts as absent
 * for precedence. NOTE: `simulated_notes` is deliberately NOT in this list —
 * for that field an empty array IS a computed "nothing to report"
 * (OpportunityDetailTabs.tsx:615-617), so emptiness must survive as-is.
 */
const RAW_ABSENT_EMPTY_ARRAY_FIELDS: ReadonlyArray<keyof OmniOpportunity> = [
  "chains_used",
  "dexes_used",
];

/** True when a raw-transport value carries no information for this field. */
function isAbsentForPrecedence(value: unknown): boolean {
  return value == null || (Array.isArray(value) && value.length === 0);
}

/**
 * CARDS-DEDUP-HOPS: merge a re-detection of an EXISTING route card
 * (same routeGroupKeyOf, different row id — the WS path delivers raw
 * re-detections). The incoming economics REPLACE the card's values in place
 * (streaming-snapshot refresh, operator order 2026-09-20 — trade values
 * change over time; the card must update without remounting or duplicating),
 * while the vigency aggregates roll forward: one more observed detection,
 * earliest first_seen, latest last_seen.
 *
 * A row that CARRIES server-side aggregates (grouped snapshot from WO-3's
 * LIVE_QUERY) is authoritative as-is — the server's GROUP BY is the single
 * source of truth and overwrites any locally-rolled counters.
 *
 * NOTE (adversarial review 2026-09-20): the locally-rolled count is only
 * reconciled with the server's COUNT(*) when a snapshot actually runs. The
 * 5 s LIVE reconcile (`useOmniOpportunities.ts:43,205-208`) does run it, so
 * the local count is corrected every SNAPSHOT_INTERVAL_MS.
 */
function mergeRedetection(
  prev: OmniOpportunity,
  incoming: OmniOpportunity,
  hits: number,
): OmniOpportunity {
  if (incoming.first_seen_at != null || incoming.confirmations != null) {
    return incoming; // grouped snapshot row — server aggregates win (rule 1)
  }
  const merged: OmniOpportunity = {
    ...incoming,
    first_seen_at: prev.first_seen_at ?? earlierIso(prev.detected_at, incoming.detected_at),
    last_seen_at: laterIso(prev.last_seen_at ?? prev.detected_at, incoming.detected_at),
    confirmations: (prev.confirmations ?? 0) + hits,
  };
  // ENRICH-PRESERVE-01 → CARDS-PRECEDENCE-01 (2026-09-26, operator report):
  // WS redetections are raw PG rows — they carry NONE of the REST-snapshot
  // enrichment. Spreading the raw row over the snapshot wiped every enriched
  // field on the first live update (operator symptom: token symbol letters
  // vanish after the first refresh, "Applied Strategy Config" goes blank).
  // The earlier fix hardcoded a PARTIAL list and missed `paper_status`,
  // `chains_used` and `dexes_used`, which the producer's own contract names as
  // WS-absent (websocket.ts:510) — PROVEN on live data (route group
  // 1||dex_arb|…|PancakeSwap V3|PancakeSwap V3):
  //   paper_status "paper_rejected" → null · chains_used [1] → [] ·
  //   dexes_used ["pancakeswap v3"] → [].
  // Now driven by the contract lists above: one place, no per-field branch.
  const mutable = merged as unknown as Record<string, unknown>;
  for (const field of RAW_ABSENT_NULL_FIELDS) {
    if (incoming[field] == null) {
      mutable[field as string] = prev[field] ?? null;
    }
  }
  for (const field of RAW_ABSENT_EMPTY_ARRAY_FIELDS) {
    if (isAbsentForPrecedence(incoming[field])) {
      mutable[field as string] = isAbsentForPrecedence(prev[field]) ? incoming[field] : prev[field];
    }
  }
  return merged;
}

// =============================================================================
// Omni-Store Implementation
// =============================================================================

export const useOmniStore = create<OmniStoreState>()(
  process.env.NODE_ENV === "development"
    ? devtools(
        (set, get) => storeFactory(set, get),
        { name: "OmniStore", maxAge: 50 },
      )
    : storeFactory,
);

function storeFactory(
  set: StoreApi<OmniStoreState>["setState"],
  get: () => OmniStoreState,
): OmniStoreState {
  return {
    // FE-MASTER runtime slices (FE-0004) — see runtime-slices.ts.
    ...createUniverseSlice(set),
    ...createTelemetrySlice(set, () => get()),
    ...createVersionSlice(set),
    ...createRuntimeAckSlice(set),

    // FE-MASTER catalog slices (FE-0004 tramo 2) — see catalog-slices.ts.
    ...createStrategiesSlice(set, () => get()),
    ...createDetectorsSlice(set, () => get()),
    ...createPairsSlice(set, () => get()),

    // FE-MASTER quote anchor slice (FE-0013..0015 · EMIT-02) — see
    // quote-slices.ts: live snapshot, PairsSlice pattern.
    ...createQuoteAnchorSlice(set, () => get()),

    // FE-MASTER realtime slice (FE-0008 · §33) — see realtime-slices.ts:
    // per-channel connection policy, written ONLY by ArbxRealtimeProvider,
    // rendered by FE-0009.
    ...createRealtimeSlice(set),

    // =========================================================================
    // Registry Slice
    // =========================================================================
    chains: new Map(),
    dexes: new Map(),
    pools: new Map(),
      registryStatus: "idle",
      registryError: null,

      fetchRegistry: async (chainId = 1) => {
        const currentStatus = get().registryStatus;
        if (currentStatus === "loading") return;

        set({ registryStatus: "loading", registryError: null });

        try {
          const baseUrl = getApiBaseUrl();
          
          // Fetch chains and dexes in parallel
          const [chainsRes, dexesRes] = await Promise.all([
            fetch(`${baseUrl}/api/chains`, { credentials: "include" }),
            fetch(`${baseUrl}/api/dexes?chain_id=${chainId}`, { credentials: "include" })
          ]);

          if (!chainsRes.ok) throw new Error(`Chains fetch failed: ${chainsRes.status}`);
          if (!dexesRes.ok) throw new Error(`DEXes fetch failed: ${dexesRes.status}`);

          const chainsData = await chainsRes.json();
          const dexesData = await dexesRes.json();

          // The defi endpoints use inconsistent envelopes: /api/chains + /api/rpcs
          // return {success, data}, while /api/dexes returns {count, items}.
          // Normalise to an array, never throwing on a non-array (the old
          // `x.items || x` fell through to the {success,data} OBJECT and crashed
          // .forEach → registryError → the registry showed a false error).
          // FAIL-HONEST: Log unexpected formats for debugging instead of silent empty.
          const toArray = (d: unknown, endpoint: string): unknown[] => {
            const o = d as { data?: unknown; items?: unknown } | null;
            if (Array.isArray(o?.data)) return o.data as unknown[];
            if (Array.isArray(o?.items)) return o.items as unknown[];
            if (Array.isArray(d)) return d as unknown[];
            // Log unexpected format for debugging - don't silently return empty
            console.error(`[OmniStore] Unexpected response format from ${endpoint}:`, d);
            // Return empty but registry stays in "ready" state - upstream should validate
            return [];
          };

          const chainsMap = new Map<number, Chain>();
          toArray(chainsData, '/api/chains').forEach((c: any) => {
            const id = c.id || c.chain_id;
            chainsMap.set(id, c);
          });

          const dexesMap = new Map<string, DEX>();
          toArray(dexesData, '/api/dexes').forEach((d: any) => {
            // /api/dexes returns chain_id (singular); the dex-registry view expects
            // chain_ids (an array) for its chain badges + chain filter. Normalise
            // so the render never crashes on undefined.chain_ids (was throwing a
            // page-level TypeError once the data finally loaded).
            const chain_ids = Array.isArray(d.chain_ids)
              ? d.chain_ids
              : d.chain_id != null
                ? [d.chain_id]
                : [];
            dexesMap.set(d.id, { ...d, chain_ids });
          });

          set({ 
            chains: chainsMap, 
            dexes: dexesMap, 
            registryStatus: "ready" 
          });
        } catch (error) {
          const message = error instanceof Error ? error.message : "Unknown error";
          set({ registryStatus: "error", registryError: message });
        }
      },

      getChain: (chainId: number) => get().chains.get(chainId),
      getDex: (dexId: string) => get().dexes.get(dexId),
      getPool: (address: string) => get().pools.get(address),

      // =========================================================================
      // Opportunity Slice
      // =========================================================================
      opportunities: [],
      windowTotal: null, // AUDIT-CARDS-MINOR (§2) — absent until first snapshot (R8)
      wsStatus: "DISCONNECTED",
      lastUpdate: null,

      connectStream: () => {
        set({ wsStatus: "CONNECTING" });
        // WebSocket connection logic is handled by useOpportunitiesStream and calls addOpportunity/setWsStatus
      },

      disconnectStream: () => {
        set({ wsStatus: "DISCONNECTED" });
      },

      addOpportunity: (opp: OmniOpportunity) =>
        set((state) => {
          // UPSERT by id — Binance-style streaming (operator directive
          // 2026-08-18). The same id arrives from THREE directions now:
          //   - WS INSERT pushes  (new detection → prepend, card enters top)
          //   - WS UPDATE pushes  (economics computed / status transition /
          //                        paper-execution values — migration 107:
          //                        only rows that actually CHANGED notify)
          //   - reconnect replays / overlapping poll ticks
          // Existing id → REPLACE IN PLACE (position preserved — the card
          // updates where it is; React.memo + the card's business-equality
          // comparator skip the re-render when nothing visual changed).
          // Unknown id → prepend like before. This replaces the old
          // skip-duplicates guard, which silently DISCARDED row updates and
          // kept emitted cards frozen until the next full poll.
          const idx = state.opportunities.findIndex((o) => o.id === opp.id);
          if (idx !== -1) {
            if (state.opportunities[idx] === opp) return state;
            const prev = state.opportunities[idx]!;
            // CARDS-DEDUP-HOPS: a same-id row UPDATE is not a new detection —
            // keep the card's rolled vigency aggregates when the incoming row
            // doesn't carry them (single WS rows never do, R8).
            const next = state.opportunities.slice();
            // CARDS-DEDUP-HOPS: a same-id row UPDATE is not a new detection —
            // keep the card's rolled vigency aggregates when the incoming row
            // doesn't carry them (single WS rows never do, R8).
            // ENRICH-PRESERVE-01 (adversarial-review fix): route it through the
            // SAME preserve path as re-detections — a same-id raw WS update must
            // not wipe token metadata, live prices or the SIM ladder either
            // (grouped snapshot rows still win verbatim inside mergeRedetection).
            next[idx] = mergeRedetection(prev, opp, 0);
            return {
              opportunities: next,
              lastUpdate: new Date().toISOString(),
            };
          }
          // CARDS-DEDUP-HOPS (operator order 2026-09-20): a NEW id for an
          // EXISTING route group is a RE-DETECTION, not a new card — the
          // dashboard used to duplicate the card on every re-detection. The
          // card stays where it is (position preserved, stable React key =
          // routeGroupKeyOf → no remount/flicker) and only its economics
          // refresh in place while the aggregates roll forward.
          const key = routeGroupKeyOf(opp);
          const gIdx = state.opportunities.findIndex(
            (o) => o.id !== opp.id && routeGroupKeyOf(o) === key,
          );
          if (gIdx !== -1) {
            const prev = state.opportunities[gIdx]!;
            const next = state.opportunities.slice();
            next[gIdx] = mergeRedetection(prev, opp, 1);
            return {
              opportunities: next,
              lastUpdate: new Date().toISOString(),
            };
          }
          return {
            opportunities: [opp, ...state.opportunities].slice(0, MAX_OPPORTUNITIES),
            lastUpdate: new Date().toISOString(),
          };
        }),

      // PERF (2026-08-10): batch replacement of the whole list in ONE store
      // update. Polling and initial hydration used to call clearOpportunities()
      // then addOpportunity() 50 times — 51 Zustand updates + 51 devtools
      // serializations every 4-5 seconds, which was the dominant source of
      // memory churn and retained snapshots. setOpportunities does it in one.
      //
      // HARDENING (2026-08-21): MERGE by id instead of replace-all. The old
      // replace-all discarded the entire array and rebuilt from scratch every
      // poll — cards that hadn't changed got re-rendered anyway because their
      // array reference changed. Merge upserts: existing ids update in place
      // (React.memo + business-equality comparator skip re-render when nothing
      // visual changed), new ids prepend. This is the streaming snapshot+push
      // pattern: poll is the snapshot, WS is the push, merge keeps both
      // efficient by only touching what changed.
      setOpportunities: (opps: OmniOpportunity[]) =>
        set((state) => {
          // CARDS-DEDUP-HOPS (operator order 2026-09-20): merge by ROUTE GROUP
          // KEY, not by id. Identity on the wire is the ROUTE — the id is one
          // detection of it. Snapshot rows (grouped LIVE_QUERY) carry server
          // aggregates and are authoritative (SSOT, mergeRedetection returns
          // them verbatim); plain rows roll the local confirmation count
          // forward, reconciled by the next snapshot when one runs (LIVE mode
          // snapshots on mount/manual refresh only — the local count stands
          // in between, bounded by pruneStale's TTL). Batch order is NOT an
          // invariant: the dominant WS caller (ws-ingest-buffer flush)
          // delivers ARRIVAL order (oldest-first), so a group's economics are
          // picked by LATEST detected_at, never by array position (WARN-1,
          // adversarial review 2026-09-20).
          const batchBy = new Map<string, OmniOpportunity[]>();
          for (const opp of opps) {
            const key = routeGroupKeyOf(opp);
            const rows = batchBy.get(key);
            if (rows == null) batchBy.set(key, [opp]);
            else rows.push(opp);
          }
          // Index existing state rows by FIRST occurrence of their group key —
          // state rows are NEVER dropped here (pruneStale owns eviction) and
          // legacy duplicate cards in state collapse to their first entry.
          const stateIdx = new Map<string, number>();
          for (let i = 0; i < state.opportunities.length; i++) {
            const k = routeGroupKeyOf(state.opportunities[i]!);
            if (!stateIdx.has(k)) stateIdx.set(k, i);
          }
          const result: OmniOpportunity[] = [];
          const emitted = new Set<string>();
          // Batch groups first (prepend, newest at top, first-appearance order).
          for (const [key, rows] of batchBy) {
            // Economics row: a row carrying server aggregates is SSOT; among
            // plain rows the LATEST detected_at wins (tie → last in array) —
            // order-independent (the WS caller flushes oldest-first).
            let econ: OmniOpportunity | null = null;
            for (const r of rows) {
              if (r.first_seen_at != null || r.confirmations != null) {
                econ = r;
                break;
              }
            }
            if (econ == null) {
              econ = rows[0]!;
              let best = Date.parse(econ.detected_at ?? "");
              for (let i = 1; i < rows.length; i++) {
                const r = rows[i]!;
                const ts = Date.parse(r.detected_at ?? "");
                // Unparseable rows never win (R8); a parseable row only needs
                // >= so same-timestamp bursts resolve to the LAST one seen.
                if (Number.isNaN(best) || (!Number.isNaN(ts) && ts >= best)) {
                  econ = r;
                  best = ts;
                }
              }
            }
            const idx = stateIdx.get(key);
            if (idx != null) {
              result.push(mergeRedetection(state.opportunities[idx]!, econ, rows.length));
            } else if (
              econ.first_seen_at != null ||
              econ.confirmations != null ||
              rows.length === 1
            ) {
              result.push(econ);
            } else {
              // Pure-WS batch group with re-detections: roll the aggregates
              // locally — honest counting, next snapshot reconciles.
              let first: string | null = null;
              let last: string | null = null;
              for (const r of rows) {
                first = earlierIso(first, r.detected_at);
                last = laterIso(last, r.detected_at);
              }
              result.push({
                ...econ,
                first_seen_at: first,
                last_seen_at: last,
                confirmations: rows.length,
              });
            }
            emitted.add(key);
          }
          // Remaining state rows keep their relative order at the tail.
          for (const opp of state.opportunities) {
            const k = routeGroupKeyOf(opp);
            if (!emitted.has(k)) {
              result.push(opp);
              emitted.add(k);
            }
          }
          return {
            opportunities: result.slice(0, MAX_OPPORTUNITIES),
            lastUpdate: new Date().toISOString(),
          };
        }),

      clearOpportunities: () => set({ opportunities: [], lastUpdate: null }),

      // AUDIT-CARDS-MINOR (§2): WO-H4 envelope counter — written ONLY by the
      // snapshot paths (useOmniOpportunities reconcile/poll + the page's
      // manual refresh), never by WS pushes (single rows carry no envelope).
      setWindowTotal: (total) => set({ windowTotal: total }),

      // MEM-RENDER-01: vigency eviction. In LIVE mode nothing removes dead
      // cards (the periodic snapshot only runs in degraded POLLING mode), so
      // stale routes lingered until displaced by 200 newer events — hours at
      // real feed rates. Called on every batched WS flush and every poll.
      // R8 fail-honest: an unparseable/missing detected_at keeps the card —
      // we never silently drop data we cannot date. (FE-0029: detected_at is
      // now honestly null on malformed payloads instead of a fabricated now()
      // that made such cards immortal with age 0.)
      // CARDS-DEDUP-HOPS: the card's vigency clock is its LAST RATIFICATION
      // (last_seen_at) — a route re-detected seconds ago is alive even if its
      // first detection is hours old. last_seen_at ?? detected_at.
      pruneStale: (maxAgeMs: number) =>
        set((state) => {
          const cutoff = Date.now() - maxAgeMs;
          const next = state.opportunities.filter((o) => {
            const ts = o.last_seen_at ?? o.detected_at;
            const t = ts == null ? NaN : Date.parse(ts);
            return Number.isNaN(t) || t >= cutoff;
          });
          if (next.length === state.opportunities.length) return state;
          return { opportunities: next };
        }),

      setWsStatus: (status: WsStatus) => set({ wsStatus: status }),

      // =========================================================================
      // Wallet Slice
      // =========================================================================
      address: null,
      balance: null,
      isConnected: false,
      chainId: null,
      wallets: new Map(),

      setWallet: (address, balance, chainId) =>
        set({
          address,
          balance,
          chainId,
          isConnected: true,
        }),

      setBalance: (balance) => set({ balance }),

      disconnect:
        () =>
          set({
            address: null,
            balance: null,
            chainId: null,
            isConnected: false,
          }),
      fetchWallets: async () => {
        try {
          const baseUrl = getApiBaseUrl();
          const res = await fetch(`${baseUrl}/api/v1/wallets`, { credentials: "include" });
          if (!res.ok) throw new Error(`Wallets fetch failed: ${res.status}`);
          const data = await res.json();
          const walletsMap = new Map<string, WalletRow>();
          (data.wallets || []).forEach((w: WalletRow) => {
            walletsMap.set(w.address, w);
          });
          set({ wallets: walletsMap });
        } catch (error) {
          console.error("Failed to fetch wallets:", error);
        }
      },
    };
}

// =============================================================================
// Selector Hooks (Performance-optimized)
// =============================================================================

export const useOpportunities = () => useOmniStore((state) => state.opportunities);
export const useWsStatus = () => useOmniStore((state) => state.wsStatus);
export const useRegistryStatus = () => useOmniStore((state) => state.registryStatus);
export const useChainsMap = () => useOmniStore((state) => state.chains);
export const useDexesMap = () => useOmniStore((state) => state.dexes);

export const useWallet = () =>
  useOmniStore((state) => ({
    address: state.address,
    balance: state.balance,
    isConnected: state.isConnected,
    chainId: state.chainId,
  }));

// =============================================================================
// FE-MASTER Runtime Selectors (FE-0004) — selector-only access, per the
// store's performance rules. Object/array selectors must use useShallow at
// the call site when consumers destructure multiple fields.
// =============================================================================

export const useUniverseKpis = () => useOmniStore((state) => state.universe);
export const useLastResolve = () => useOmniStore((state) => state.lastResolve);
export const useRouteTick = () => useOmniStore((state) => state.tick);
export const useTickStatus = () => useOmniStore((state) => state.tickStatus);
export const useTickError = () => useOmniStore((state) => state.tickError);
export const useRuntimeVersions = () => useOmniStore((state) => state.versions);
export const useLastRuntimeAck = () => useOmniStore((state) => state.lastAck);
export const useRuntimeAckLog = () => useOmniStore((state) => state.ackLog);

// FE-MASTER catalog selectors (FE-0004 tramo 2 — P5/P6/P7).
export const useStrategyCatalog = () => useOmniStore((state) => state.strategyCatalog);
export const useStrategyByMevId = () => useOmniStore((state) => state.strategyByMevId);
export const useStrategyCatalogStatus = () => useOmniStore((state) => state.strategyCatalogStatus);
export const useDetectorCatalog = () => useOmniStore((state) => state.detectorCatalog);
export const useDetectorById = () => useOmniStore((state) => state.detectorById);
export const useDetectorCatalogStatus = () => useOmniStore((state) => state.detectorCatalogStatus);
export const usePairs = () => useOmniStore((state) => state.pairs);
export const usePairsStatus = () => useOmniStore((state) => state.pairsStatus);
export const usePairsError = () => useOmniStore((state) => state.pairsError);
export const usePairsUpdatedAt = () => useOmniStore((state) => state.pairsUpdatedAt);

// FE-MASTER quote anchor selectors (FE-0013..0015 · EMIT-02).
export const useQuoteAnchor = () => useOmniStore((state) => state.quoteAnchor);
export const useQuoteAnchorStatus = () => useOmniStore((state) => state.quoteAnchorStatus);
export const useQuoteAnchorError = () => useOmniStore((state) => state.quoteAnchorError);
export const useQuoteAnchorUpdatedAt = () => useOmniStore((state) => state.quoteAnchorUpdatedAt);

// FE-MASTER realtime selectors (FE-0008 · §33) — FE-0009's posture bar.
export const useRealtimeChannels = () => useOmniStore((state) => state.channels);
export const useWsConnected = () => useOmniStore((state) => state.wsConnected);
