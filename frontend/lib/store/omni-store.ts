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

// Re-export OmniOpportunity as the canonical opportunity type
export type { OmniOpportunity } from "./types";

/** WebSocket connection status (extends socket-lifecycle.ts with POLLING fallback) */
export type WsStatus = "CONNECTING" | "LIVE" | "STALE" | "POLLING";

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
  /** Fetch all registry data from API (single call) */
  fetchRegistry: () => Promise<void>;
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

      fetchRegistry: async () => {
        const currentStatus = get().registryStatus;
        if (currentStatus === "loading") return; // Prevent duplicate calls

        set({ registryStatus: "loading", registryError: null });

        try {
          // TODO: Replace with actual API call
          // const response = await fetch("/api/dexes");
          // const data = await response.json();
          // Transform into Maps...

          // Placeholder: Mark as ready
          set({ registryStatus: "ready" });
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
        // TODO: WebSocket connection logic will be migrated from useOpportunitiesStream
      },

      disconnectStream: () => {
        set({ wsStatus: "STALE" });
        // TODO: Cleanup WebSocket
      },

      addOpportunity: (opp: OmniOpportunity) =>
        set((state) => ({
          opportunities: [opp, ...state.opportunities].slice(0, MAX_OPPORTUNITIES),
          lastUpdate: new Date().toISOString(),
        })),

      clearOpportunities: () => set({ opportunities: [], lastUpdate: null }),

      setWsStatus: (status: WsStatus) => set({ wsStatus: status }),

      // =========================================================================
      // Wallet Slice
      // =========================================================================
      address: null,
      balance: null,
      isConnected: false,
      chainId: null,

      setWallet: (address, balance, chainId) =>
        set({
          address,
          balance,
          chainId,
          isConnected: true,
        }),

      setBalance: (balance) => set({ balance }),

      disconnect: () =>
        set({
          address: null,
          balance: null,
          chainId: null,
          isConnected: false,
        }),
    }),
    { name: "arbx-omni-store" }
  )
);

// =============================================================================
// Selector Hooks (Performance-optimized)
// =============================================================================

/** Hook to get only opportunities (prevents re-renders from other state changes) */
export const useOpportunities = () => useOmniStore((state) => state.opportunities);

/** Hook to get only WS status */
export const useWsStatus = () => useOmniStore((state) => state.wsStatus);

/** Hook to get only registry status */
export const useRegistryStatus = () => useOmniStore((state) => state.registryStatus);

/** Hook to get only wallet state */
export const useWallet = () =>
  useOmniStore((state) => ({
    address: state.address,
    balance: state.balance,
    isConnected: state.isConnected,
    chainId: state.chainId,
  }));
