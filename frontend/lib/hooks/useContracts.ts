/**
 * =============================================================================
 * OMEGA USECONTRACTS HOOK — Infrastructure Abstraction
 * =============================================================================
 *
 * Encapsulates fetch logic for contract entities.
 * Eliminates API-leaks from omega-s5 pages.
 *
 * Pattern: Hook → api-client → Edge Worker
 */

"use client";

import { useEffect, useState } from "react";
import { getApiBaseUrl } from "@/lib/api-client";
import type { ContractEntity } from "@/lib/registries/types-omni";

// =============================================================================
// Types
// =============================================================================

export interface UseContractsOptions {
  /** Filter by contract kind (e.g., "factory", "resolution_core") */
  contractKind?: string;
  /** Filter by multiple contract kinds */
  contractKinds?: Array<ContractEntity["contract_kind"]>;
}

export interface UseContractsResult {
  data: ContractEntity[];
  isLoading: boolean;
  error: string | null;
  /** Refetch manually */
  refetch: () => Promise<void>;
}

// =============================================================================
// Hook Implementation
// =============================================================================

/**
 * Hook to fetch contract entities from /api/contracts.
 *
 * @example
 * ```tsx
 * function WalletsPage() {
 *   const { data, isLoading, error } = useContracts({
 *     contractKinds: ["wallet_topology", "gas_sponsor"]
 *   });
 * }
 * ```
 */
export function useContracts(options: UseContractsOptions = {}): UseContractsResult {
  const { contractKind, contractKinds } = options;

  const [data, setData] = useState<ContractEntity[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchData = async () => {
    setIsLoading(true);
    setError(null);

    try {
      // Build URL with optional filter
      let url = `${getApiBaseUrl()}/api/contracts`;
      if (contractKind) {
        url += `?contract_kind=${encodeURIComponent(contractKind)}`;
      }

      const res = await fetch(url, {
        headers: { accept: "application/json" },
        signal: AbortSignal.timeout(5000),
      });

      if (!res.ok) {
        setError(`HTTP ${res.status}`);
        return;
      }

      const json = await res.json();
      let rows: ContractEntity[] = Array.isArray(json?.rows) ? json.rows : [];

      // Client-side filter if multiple kinds specified
      if (contractKinds && contractKinds.length > 0) {
        rows = rows.filter((c) => contractKinds.includes(c.contract_kind));
      }

      setData(rows);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    fetchData();
  }, [contractKind]); // Re-fetch if contractKind changes

  return {
    data,
    isLoading,
    error,
    refetch: fetchData,
  };
}
