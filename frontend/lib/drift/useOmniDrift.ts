/**
 * useOmniDrift — React hook surfacing PG↔Redis↔searcher-rs↔frontend drift.
 *
 * Polls `/api/system/drift` every 5s and exposes:
 *   - unresolved observations
 *   - per-resource latest hash drift
 *   - reload trigger that re-publishes the canonical config_hash
 *
 * Reads ONLY (no writes); UI must call POST /:resource/:id/reload to heal.
 */
"use client";

import { useEffect, useState } from "react";
import type { DriftObservation } from "@/lib/registries/types-omni";

export interface DriftSnapshot {
  observations: DriftObservation[];
  byResource: Record<string, DriftObservation[]>;
  worstSeverity: "info" | "warn" | "error" | "critical" | "none";
  refreshedAt: string;
}

const SEVERITY_RANK: Record<DriftObservation["severity"], number> = {
  info: 1, warn: 2, error: 3, critical: 4,
};

export function useOmniDrift(pollMs = 5000): DriftSnapshot & { loading: boolean; error: string | null } {
  const [state, setState] = useState<DriftSnapshot>({
    observations: [],
    byResource: {},
    worstSeverity: "none",
    refreshedAt: new Date(0).toISOString(),
  });
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    const tick = async () => {
      try {
        const r = await fetch("/api/system/drift", { cache: "no-store" });
        if (!r.ok) throw new Error(`HTTP ${r.status}`);
        const body = (await r.json()) as { drift: DriftObservation[] };
        if (cancelled) return;
        const byResource: Record<string, DriftObservation[]> = {};
        let worst: DriftSnapshot["worstSeverity"] = "none";
        for (const o of body.drift) {
          (byResource[o.resource] ??= []).push(o);
          if (worst === "none" || SEVERITY_RANK[o.severity] > SEVERITY_RANK[worst as DriftObservation["severity"]]) {
            worst = o.severity;
          }
        }
        setState({
          observations: body.drift,
          byResource,
          worstSeverity: worst,
          refreshedAt: new Date().toISOString(),
        });
        setError(null);
      } catch (e) {
        if (!cancelled) setError((e as Error).message);
      } finally {
        if (!cancelled) setLoading(false);
      }
    };
    tick();
    const id = setInterval(tick, pollMs);
    return () => { cancelled = true; clearInterval(id); };
  }, [pollMs]);

  return { ...state, loading, error };
}
