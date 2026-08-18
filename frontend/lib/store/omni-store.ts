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
  /** Replace the entire opportunity list in a single update (batch) */
  setOpportunities: (opps: OmniOpportunity[]) => void;
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

type OmniStoreState = RegistrySlice & OpportunitySlice & WalletSlice;

// =============================================================================
// Constants
// =============================================================================

/** Maximum opportunities to keep in memory (prevents memory leak) */
const MAX_OPPORTUNITIES = 200;

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
            const next = state.opportunities.slice();
            next[idx] = opp;
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
      setOpportunities: (opps: OmniOpportunity[]) =>
        set({
          opportunities: opps.slice(0, MAX_OPPORTUNITIES),
          lastUpdate: new Date().toISOString(),
        }),

      clearOpportunities: () => set({ opportunities: [], lastUpdate: null }),

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
