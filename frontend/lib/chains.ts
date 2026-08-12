/**
 * Single source of truth for the chain catalog used across /strategies tabs.
 *
 * Audit 2026-05-10 fix (Array B in the post-Phase-4 audit): DexesTab and
 * PoolsTab each duplicated a `SUPPORTED_CHAINS` array. Two literal arrays
 * → drift potential when a new chain is enabled. The backend already
 * exposes `/api/chains` (sourced from the `chains` table) — this module
 * fetches that endpoint and exports a typed catalog, with a doctrinal
 * static fallback so the UI never blanks on a transient fetch failure.
 *
 * R8 fail-honest:
 *   - Successful fetch → catalog from backend (canonical).
 *   - Fetch error → static fallback (logs WARN; UI still operates).
 *   - Empty backend response → empty list (operator must seed `chains`).
 *
 * The static fallback list intentionally matches the EVM mainnets the
 * Rust `shared_rs::chains` module + database seed both already encode —
 * see migrations 044/045 and `backend/shared-rs/src/chains.rs` for the
 * cross-layer source of truth on chain identifiers.
 */

"use client";
import { getApiBaseUrl } from "@/lib/api-client";

import { useEffect, useState } from "react";

export interface ChainInfo {
  chain_id: number;
  name: string;
  /** ISO short label rendered in chain pickers (e.g. "ETH", "ARB"). */
  short: string;
  /** Backend-supplied native currency symbol when available. */
  native_currency?: string;
  /** Backend-supplied explorer URL when available. */
  explorer_url?: string;
  is_active?: boolean;
  /** Tailwind text color for the chain badge (set by getChainStyle). */
  color?: string;
  /** Tailwind background tint for the chain badge. */
  bg?: string;
  /** Tailwind border tint for the chain badge. */
  border?: string;
}

/**
 * Brand-aligned color palette for chain badges. Static, doctrinal (RULE 00
 * exception: chain identifiers are protocol-level constants, not productive
 * data). Picked to be visually distinct AND legible on the dark theme used
 * by the operator console.
 */
const CHAIN_STYLES: Record<number, { color: string; bg: string; border: string }> = {
  1:     { color: "text-blue-300",   bg: "bg-blue-500/15",   border: "border-blue-500/40"   },  // Ethereum
  42161: { color: "text-sky-300",    bg: "bg-sky-500/15",    border: "border-sky-500/40"    },  // Arbitrum
  10:    { color: "text-red-300",    bg: "bg-red-500/15",    border: "border-red-500/40"    },  // Optimism
  8453:  { color: "text-blue-200",   bg: "bg-blue-600/15",   border: "border-blue-600/40"   },  // Base
  137:   { color: "text-purple-300", bg: "bg-purple-500/15", border: "border-purple-500/40" },  // Polygon
  56:    { color: "text-yellow-300", bg: "bg-yellow-500/15", border: "border-yellow-500/40" },  // BSC
};

const DEFAULT_CHAIN_STYLE = {
  color: "text-slate-300",
  bg: "bg-slate-500/15",
  border: "border-slate-500/40",
} as const;

/**
 * Returns the typed styling for a chain badge. Falls back to slate when the
 * chain_id has no doctrinal style — never throws, never null.
 */
export function getChainStyle(chain_id: number): { color: string; bg: string; border: string } {
  return CHAIN_STYLES[chain_id] ?? DEFAULT_CHAIN_STYLE;
}

/**
 * Synchronous resolver: chain_id → ChainInfo. First checks the doctrinal
 * static list (covers the 6 EVM mainnets), then returns a minimal placeholder
 * for unknown chains so the UI never blanks. Use this in pure render
 * contexts where the async useChains() hook is overkill.
 */
export function chainInfoSync(chain_id: number): ChainInfo {
  const known = DOCTRINAL_CHAINS.find((c) => c.chain_id === chain_id);
  const style = getChainStyle(chain_id);
  if (known) {
    return { ...known, ...style };
  }
  return {
    chain_id,
    name: `Chain ${chain_id}`,
    short: `#${chain_id}`,
    is_active: true,
    ...style,
  };
}

/**
 * Doctrinal fallback. EVM mainnet chain IDs are protocol-level constants
 * (not operator-tunable productive data), so a static fallback here is
 * NOT a no-hardcode violation — it matches the seed in migration 044
 * and the `shared_rs::chains::routers_for_chain` enum.
 */
export const DOCTRINAL_CHAINS: readonly ChainInfo[] = [
  { chain_id: 1,     name: "Ethereum", short: "ETH",   native_currency: "ETH",   is_active: true },
  { chain_id: 42161, name: "Arbitrum", short: "ARB",   native_currency: "ETH",   is_active: true },
  { chain_id: 10,    name: "Optimism", short: "OP",    native_currency: "ETH",   is_active: true },
  { chain_id: 8453,  name: "Base",     short: "BASE",  native_currency: "ETH",   is_active: true },
  { chain_id: 137,   name: "Polygon",  short: "MATIC", native_currency: "MATIC", is_active: true },
  { chain_id: 56,    name: "BSC",      short: "BSC",   native_currency: "BNB",   is_active: true },
] as const;

/**
 * Derives a 3-4 letter short label from the backend's lowercase chain name.
 * Backend stores `name` as `"ethereum" | "arbitrum" | ...` — we map known
 * names to their canonical short labels and capitalise the first 3-4
 * letters as a fallback.
 */
const NAME_TO_SHORT: Record<string, string> = {
  ethereum: "ETH",
  arbitrum: "ARB",
  optimism: "OP",
  base: "BASE",
  polygon: "MATIC",
  bsc: "BSC",
};

function chainShortFromName(name: string): string {
  const lower = name.toLowerCase();
  const short = (NAME_TO_SHORT as Record<string, string>)[lower];
  if (short) return short;
  // Fallback: first 4 chars uppercased.
  return name.slice(0, 4).toUpperCase();
}

interface BackendChainRow {
  chain_id: number;
  name: string;
  native_currency?: string;
  explorer_url?: string;
  is_active?: boolean;
}

/**
 * React hook that returns the current chain catalog. Starts with the
 * doctrinal static list so SSR + first paint render the operator's
 * familiar 6 mainnets, then refreshes from `/api/chains` once the
 * fetch resolves.
 */
export function useChains(): { chains: readonly ChainInfo[]; error: string | null } {
  const [chains, setChains] = useState<readonly ChainInfo[]>(DOCTRINAL_CHAINS);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const edgeUrl = getApiBaseUrl();
    const ctrl = new AbortController();
    fetch(`${edgeUrl}/api/chains`, {
      signal: ctrl.signal,
      credentials: "include",
      headers: { accept: "application/json" },
    })
      .then((r) => {
        if (!r.ok) throw new Error(`HTTP ${r.status}`);
        return r.json() as Promise<{ success: boolean; data?: BackendChainRow[] }>;
      })
      .then((j) => {
        if (j.success && Array.isArray(j.data) && j.data.length > 0) {
          const rows: ChainInfo[] = j.data
            .filter((r): r is BackendChainRow => Number.isFinite(r.chain_id))
            .map((r) => ({
              chain_id: r.chain_id,
              name: r.name.charAt(0).toUpperCase() + r.name.slice(1),
              short: chainShortFromName(r.name),
              native_currency: r.native_currency,
              explorer_url: r.explorer_url,
              is_active: r.is_active ?? true,
            }))
            // Operator only cares about active chains in selectors.
            .filter((c) => c.is_active !== false);
          setChains(rows);
          setError(null);
        }
      })
      .catch((e: Error) => {
        if (e.name === "AbortError") return;
        // Static fallback already populated; surface the error inline so
        // the operator knows the catalog may be stale.
        setError(e.message);
      });
    return () => ctrl.abort();
  }, []);

  return { chains, error };
}
