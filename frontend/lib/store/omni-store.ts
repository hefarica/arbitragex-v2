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

import { create } from "zustand";
import { devtools } from "zustand/middleware";

// =============================================================================
// Type Imports (from existing canonical types)
// =============================================================================

import type { Chain, DEX, Pool } from "@/lib/registries/types";
import type { OmniOpportunity } from "./types";
import type { WalletRow } from "@/lib/api/wallets";
import { getApiBaseUrl } from "@/lib/api-client";

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
  /** Clear all opportunities */
  clearOpportunities: () => void;
  /**
   * Merge a fresh snapshot into the store WITHOUT wiping existing entries.
   * Dedups by stable route key (routeKey); preserves the original detected_at
   * + id (age continuity + stable React key ⇒ no flash on re-detection);
   * refreshes metrics from the latest snapshot; prepends genuinely-new routes.
   * This is the poll/snapshot counterpart to addOpportunity (which serves the
   * WebSocket single-event stream).
   */
  mergeSnapshot: (items: OmniOpportunity[]) => void;
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

type OmniStoreState = RegistrySlice & OpportunitySlice & WalletSlice;

// =============================================================================
// Constants
// =============================================================================

/** Maximum opportunities to keep in memory (prevents memory leak) */
const MAX_OPPORTUNITIES = 200;

/**
 * Stable route identity for dedup across snapshots. The API may emit a fresh
 * detection id each cycle for the very same route; this key collapses them so
 * mergeOpportunitySnapshots updates the row in place instead of re-adding it
 * (the root cause of the "same trades flash as new every 5s" UX bug — see
 * systematic-debugging Phase 1: the poll path wiped+repopulated the store, so
 * React saw every row as new each cycle and re-ran the enter animation).
 * Lowercased + strategy-scoped so a dex_arb and a flashloan_arb on the same
 * pair stay distinct rows. Reused as the React list key in OpportunitiesClient
 * (stable key ⇒ no remount ⇒ no flash).
 */
export function routeKey(o: OmniOpportunity): string {
  const tin = (o.token_in || "").toLowerCase();
  const tout = (o.token_out || "").toLowerCase();
  const da = (o.dex_a || "").toLowerCase();
  const db = (o.dex_b ?? "").toLowerCase();
  return `${o.chain_id}:${o.strategy_kind}:${tin}:${tout}:${da}:${db}`;
}

/**
 * Strategy "must be profitable" rule (operator decision 2026-07-05). A viable
 * opportunity must be profitable — "negativo desaparece" / "mostrar las mayores
 * a 0". An opp that the backend rejected as non-positive (ANY of the ~8
 * non_positive_* tags across size_optimizer / sim_multistep / revm_backend /
 * scanner / sim_encoder — all caught by the substring below) OR whose computed
 * net is < 0 NEVER renders on the panel. This is a HARD, toggle-independent
 * filter: the "Show all / Viable only" toggle (server-side viable_only param)
 * controls only opps rejected for OTHER reasons (no price, no metadata,
 * below-floor…); strategy-non-positive opps are dropped at the store whatever
 * the toggle. net == null (cold-start, net not yet computed) is PRESERVED
 * (R8: null ≠ zero, never dropped). net <= 0 (negative OR break-even) is
 * DROPPED — the rule is strictly "show ABOVE 0" (operator decision 2026-07-05:
 * "mostrar las por encima de 0").
 */
const NON_POSITIVE_REJECT = /non_positive/;

function isStrategyNonPositive(o: OmniOpportunity): boolean {
  // (1) Backend authority: any non_positive_* rejection tag (covers all ~8
  //     variants — size_optimizer, sim_multistep, revm_backend, scanner,
  //     sim_encoder — without enumerating them).
  if (o.status === "rejected" && o.rejection_reason && NON_POSITIVE_REJECT.test(o.rejection_reason)) {
    return true;
  }
  // (2) Defensive belt: live net <= 0 even before the status write flips. Priority
  //     matches the card's net display: canonical spine → TS forward-sim.
  //     net == null is PRESERVED (R8 cold-start) — the guard is `net != null`.
  const net = o.net_expected_profit_usd ?? o.simulated_net_profit_usd;
  return net != null && net <= 0;
}

/**
 * Pure merge of a fresh snapshot into the existing opportunity list (testable
 * without Zustand). Dedups by routeKey; on a match PRESERVES the original
 * detected_at + id (age stays continuous, React key stays stable ⇒ no flash);
 * refreshes every other field from the fresh detection; appends genuinely-new
 * routes; KEEPS routes no longer in the snapshot (they age out visually + sink
 * below fresh ones after the newest-first sort); DROPS strategy-non-positive
 * opps (isStrategyNonPositive — HARD filter, toggle-independent, drop on
 * receive); caps at `cap`. Empty incoming ⇒ existing returned unchanged (a
 * momentary empty fetch must NOT wipe the feed).
 */
export function mergeOpportunitySnapshots(
  existing: OmniOpportunity[],
  incoming: OmniOpportunity[],
  cap: number = MAX_OPPORTUNITIES,
): OmniOpportunity[] {
  if (incoming.length === 0) return existing;
  const existingByKey = new Map<string, OmniOpportunity>();
  for (const o of existing) existingByKey.set(routeKey(o), o);

  const seenKeys = new Set<string>();
  const merged: OmniOpportunity[] = [];
  for (const item of incoming) {
    const k = routeKey(item);
    // Mark the route handled EVEN when we drop it as strategy-non-positive, so
    // the existing-passthrough below skips the stale positive version of a
    // route that just flipped negative (the flip case: it must DISAPPEAR).
    seenKeys.add(k);
    if (isStrategyNonPositive(item)) continue; // HARD filter — never renders
    const prev = existingByKey.get(k);
    merged.push(prev ? { ...item, detected_at: prev.detected_at, id: prev.id } : item);
  }
  for (const o of existing) {
    if (seenKeys.has(routeKey(o))) continue;   // updated above, or dropped as non-positive
    if (isStrategyNonPositive(o)) continue;     // belt-and-braces: never render non-positive
    merged.push(o);
  }
  merged.sort(
    (a, b) =>
      (new Date(b.detected_at).getTime() || 0) - (new Date(a.detected_at).getTime() || 0),
  );
  return merged.slice(0, cap);
}

// =============================================================================
// Omni-Store Implementation
// =============================================================================

export const useOmniStore = create<OmniStoreState>()(
  devtools(
    (set, get) => ({
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
        set((state) => ({
          opportunities: [opp, ...state.opportunities].slice(0, MAX_OPPORTUNITIES),
          lastUpdate: new Date().toISOString(),
        })),

      clearOpportunities: () => set({ opportunities: [], lastUpdate: null }),

      mergeSnapshot: (items) =>
        set((state) => ({
          opportunities: mergeOpportunitySnapshots(state.opportunities, items),
          lastUpdate: new Date().toISOString(),
        })),

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
    }),
    { name: "arbx-omni-store" }
  )
);

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
