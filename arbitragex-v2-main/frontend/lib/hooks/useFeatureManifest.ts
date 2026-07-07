/**
 * =============================================================================
 * OMEGA USEFEATUREMANIFEST HOOK — Infrastructure Abstraction
 * =============================================================================
 *
 * Encapsulates fetch logic for feature manifest.
 * Used by omega-s5/core page.
 *
 * Pattern: Hook → api-client → Edge Worker
 */

"use client";

import { useEffect, useState } from "react";
import { getApiBaseUrl } from "@/lib/api-client";

// =============================================================================
// Types
// =============================================================================

export interface FeatureManifestEntry {
  feature_key: string;
  state_hash: string;
  enabled: boolean;
  version: string;
}

export interface UseFeatureManifestOptions {
  /** Filter by feature key (e.g., "omega.s5.core") */
  featureKey?: string;
}

export interface UseFeatureManifestResult {
  data: FeatureManifestEntry[];
  feature: FeatureManifestEntry | null;
  isLoading: boolean;
  error: string | null;
  refetch: () => Promise<void>;
}

// =============================================================================
// Hook Implementation
// =============================================================================

/**
 * Hook to fetch feature manifest from /api/system/feature_manifest.
 *
 * @example
 * ```tsx
 * function CorePage() {
 *   const { feature } = useFeatureManifest({ featureKey: "omega.s5.core" });
 * }
 * ```
 */
export function useFeatureManifest(
  options: UseFeatureManifestOptions = {}
): UseFeatureManifestResult {
  const { featureKey } = options;

  const [data, setData] = useState<FeatureManifestEntry[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchData = async () => {
    setIsLoading(true);
    setError(null);

    try {
      const res = await fetch(`${getApiBaseUrl()}/api/system/feature_manifest`, {
        headers: { accept: "application/json" },
        signal: AbortSignal.timeout(5000),
      });

      if (!res.ok) {
        setError(`HTTP ${res.status}`);
        return;
      }

      const json = await res.json();
      const features: FeatureManifestEntry[] = Array.isArray(json?.features)
        ? json.features
        : [];

      setData(features);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    fetchData();
  }, []);

  // Find specific feature if key provided
  const feature = featureKey
    ? data.find((f) => f.feature_key === featureKey) ?? null
    : null;

  return {
    data,
    feature,
    isLoading,
    error,
    refetch: fetchData,
  };
}
