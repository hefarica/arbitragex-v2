/**
 * =============================================================================
 * OMEGA USECRUCIBLESTATUS HOOK — Infrastructure Abstraction
 * =============================================================================
 *
 * Encapsulates fetch logic for crucible survival status.
 * Eliminates API-leaks from omega-s5/crucible page.
 *
 * Pattern: Hook → api-client → Edge Worker
 */

"use client";

import { useEffect, useState } from "react";
import { getApiBaseUrl } from "@/lib/api-client";

// =============================================================================
// Types
// =============================================================================

export interface CrucibleNetworkStatus {
  chain_id: number;
  network: string;
  resolutions_total: number;
  resolutions_success: number;
  success_rate_pct: number;
  runtime_hours: number;
  doctrinal_reverts: number;
  capital_cap_usd: number;
}

export interface UseCrucibleStatusResult {
  data: CrucibleNetworkStatus[];
  isLoading: boolean;
  error: string | null;
  /** Refetch manually */
  refetch: () => Promise<void>;
}

// =============================================================================
// Hook Implementation
// =============================================================================

/**
 * Hook to fetch crucible survival status from /api/crucible/status.
 *
 * @example
 * ```tsx
 * function CruciblePage() {
 *   const { data, isLoading, error } = useCrucibleStatus();
 *   const allPassed = data.every(r => r.success_rate_pct >= 95);
 * }
 * ```
 */
export function useCrucibleStatus(): UseCrucibleStatusResult {
  const [data, setData] = useState<CrucibleNetworkStatus[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchData = async () => {
    setIsLoading(true);
    setError(null);

    try {
      const res = await fetch(`${getApiBaseUrl()}/api/crucible/status`, {
        headers: { accept: "application/json" },
        signal: AbortSignal.timeout(5000),
      });

      if (!res.ok) {
        setError(`HTTP ${res.status}`);
        return;
      }

      const json = await res.json();
      const networks: CrucibleNetworkStatus[] = Array.isArray(json?.networks)
        ? json.networks
        : [];

      setData(networks);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    fetchData();
  }, []);

  return {
    data,
    isLoading,
    error,
    refetch: fetchData,
  };
}
