/**
 * =============================================================================
 * OMEGA USECAPITALGATES HOOK — Infrastructure Abstraction
 * =============================================================================
 *
 * Encapsulates fetch logic for capital gates.
 * Eliminates API-leaks from omega-s5/operator page.
 *
 * Pattern: Hook → api-client → Edge Worker
 */

"use client";

import { useEffect, useState } from "react";
import { getApiBaseUrl } from "@/lib/api-client";
import type { CapitalGateEntity } from "@/lib/registries/types-omni";

// =============================================================================
// Types
// =============================================================================

export interface UseCapitalGatesResult {
  data: CapitalGateEntity[];
  isLoading: boolean;
  error: string | null;
  /** Refetch manually */
  refetch: () => Promise<void>;
}

// =============================================================================
// Hook Implementation
// =============================================================================

/**
 * Hook to fetch capital gates from /api/capital-gates.
 *
 * @example
 * ```tsx
 * function OperatorPage() {
 *   const { data, isLoading, error } = useCapitalGates();
 *   const global = data.find(g => g.scope === "global");
 * }
 * ```
 */
export function useCapitalGates(): UseCapitalGatesResult {
  const [data, setData] = useState<CapitalGateEntity[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchData = async () => {
    setIsLoading(true);
    setError(null);

    try {
      const res = await fetch(`${getApiBaseUrl()}/api/capital-gates`, {
        headers: { accept: "application/json" },
        signal: AbortSignal.timeout(5000),
      });

      if (!res.ok) {
        setError(`HTTP ${res.status}`);
        return;
      }

      const json = await res.json();
      const rows: CapitalGateEntity[] = Array.isArray(json?.rows)
        ? json.rows
        : [];

      setData(rows);
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
