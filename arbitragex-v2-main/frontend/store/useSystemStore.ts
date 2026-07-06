"use client";

/**
 * useSystemStore — Single Source of Truth for the Readiness Pipeline.
 *
 * Persisted to localStorage so state survives page reloads.
 * SSR-safe: uses the `skipHydration` pattern for Next.js 16 (App Router).
 *
 * Flow:
 *   Topology Vault (Step 1) → writes activeChains / topology
 *   Credentials   (Step 2) → reads activeChains, writes credentialStatus
 *   Chains & DEXes (Step 3) → reads activeChains
 *   Engines        (Step 4) → reads all three, unlocks strategies
 */

import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";

// ─── Domain types ────────────────────────────────────────────────────────────

export interface ActiveChain {
  chainId: number;
  name: string;
  /** Masked host of the first HTTP provider, e.g. "eth-mainnet.g.alchemy.com" */
  rpcHttpHost: string;
  /** Masked host of the first WS provider */
  rpcWsHost: string;
  /** Topology Vault version that produced this entry */
  versionId: number;
  validatedAt: string; // ISO-8601
}

export type CredentialStatus = "untested" | "valid" | "invalid" | "expired";

export interface CredentialEntry {
  provider: string;
  scope: string;
  status: CredentialStatus;
  validatedAt: string | null;
}

// ─── State shape ─────────────────────────────────────────────────────────────

export interface SystemStoreState {
  // ── Topology (Step 1) ──────────────────────────────────────────────────────
  topology: {
    isReady: boolean;
    activeChains: ActiveChain[];
    /** Topology Vault version_id of the last successful mutation */
    versionId: number | null;
    updatedAt: string | null;
  };

  // ── Credentials (Step 2) ───────────────────────────────────────────────────
  credentials: {
    /** Keyed by "provider:scope" */
    entries: Record<string, CredentialEntry>;
    /** True when all required credentials for active chains are valid */
    isReady: boolean;
  };

  // ── Markets (Step 3) ───────────────────────────────────────────────────────
  markets: {
    isReady: boolean;
    validatedAt: string | null;
  };

  // ── Engines (Step 4) ───────────────────────────────────────────────────────
  engines: {
    isReady: boolean;
    enabledStrategies: string[];
  };
}

// ─── Actions ─────────────────────────────────────────────────────────────────

export interface SystemStoreActions {
  /** Called by TopologyVaultClient after a successful mutation */
  setTopologyReady: (chains: ActiveChain[], versionId: number, updatedAt: string) => void;
  /** Called when topology snapshot returns empty_vault */
  clearTopology: () => void;

  /** Called by CredentialsClient after a successful test/save */
  setCredentialStatus: (provider: string, scope: string, status: CredentialStatus, validatedAt: string | null) => void;
  /** Recompute isReady based on active chains */
  recomputeCredentialsReady: () => void;

  setMarketsReady: (validatedAt: string) => void;
  setEnginesReady: (strategies: string[]) => void;

  /** Full reset (e.g. on logout) */
  reset: () => void;
}

// ─── Initial state ───────────────────────────────────────────────────────────

const INITIAL: SystemStoreState = {
  topology: { isReady: false, activeChains: [], versionId: null, updatedAt: null },
  credentials: { entries: {}, isReady: false },
  markets: { isReady: false, validatedAt: null },
  engines: { isReady: false, enabledStrategies: [] },
};

// ─── Required credential providers per active chain ──────────────────────────
// A chain is "credentials-ready" when both rpc_http and rpc_ws entries for
// that chainId are valid. CEX/MEV/oracle credentials are global and optional
// for the base readiness gate.
function computeCredentialsReady(
  activeChains: ActiveChain[],
  entries: Record<string, CredentialEntry>
): boolean {
  if (activeChains.length === 0) return false;
  return activeChains.every((c) => {
    const httpKey = `rpc_http:chain:${c.chainId}`;
    const wsKey   = `rpc_ws:chain:${c.chainId}`;
    return (
      entries[httpKey]?.status === "valid" &&
      entries[wsKey]?.status  === "valid"
    );
  });
}

// ─── Store ───────────────────────────────────────────────────────────────────

export const useSystemStore = create<SystemStoreState & SystemStoreActions>()(
  persist(
    (set, get) => ({
      ...INITIAL,

      setTopologyReady(chains, versionId, updatedAt) {
        set((s) => ({
          topology: { isReady: chains.length > 0, activeChains: chains, versionId, updatedAt },
          // Recompute credentials readiness with new chains
          credentials: {
            ...s.credentials,
            isReady: computeCredentialsReady(chains, s.credentials.entries),
          },
        }));
      },

      clearTopology() {
        set({ topology: INITIAL.topology, credentials: INITIAL.credentials });
      },

      setCredentialStatus(provider, scope, status, validatedAt) {
        set((s) => {
          const key = `${provider}:${scope}`;
          const entries = { ...s.credentials.entries, [key]: { provider, scope, status, validatedAt } };
          return {
            credentials: {
              entries,
              isReady: computeCredentialsReady(s.topology.activeChains, entries),
            },
          };
        });
      },

      recomputeCredentialsReady() {
        const { topology, credentials } = get();
        set({
          credentials: {
            ...credentials,
            isReady: computeCredentialsReady(topology.activeChains, credentials.entries),
          },
        });
      },

      setMarketsReady(validatedAt) {
        set({ markets: { isReady: true, validatedAt } });
      },

      setEnginesReady(strategies) {
        set({ engines: { isReady: strategies.length > 0, enabledStrategies: strategies } });
      },

      reset() {
        set(INITIAL);
      },
    }),
    {
      name: "arbx-system-store",
      storage: createJSONStorage(() => {
        // SSR guard: localStorage is not available on the server
        if (typeof window === "undefined") {
          return {
            getItem: () => null,
            setItem: () => {},
            removeItem: () => {},
          };
        }
        return window.localStorage;
      }),
      // Only persist topology + credentials; engines/markets are re-derived
      partialize: (s) => ({
        topology: s.topology,
        credentials: s.credentials,
      }),
      // skipHydration: true prevents SSR/client mismatch in Next.js App Router.
      // Components call useSystemStore.persist.rehydrate() inside useEffect.
      skipHydration: true,
    }
  )
);

// ─── Hydration helper ────────────────────────────────────────────────────────
// Call this once in a top-level client layout or in each component's useEffect.
let _rehydrated = false;
export function rehydrateSystemStore() {
  if (_rehydrated || typeof window === "undefined") return;
  _rehydrated = true;
  void useSystemStore.persist.rehydrate();
}
