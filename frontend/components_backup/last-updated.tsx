"use client";

/**
 * LastUpdated — visible data-freshness indicator for polling panels.
 *
 * The /live-readiness panels all poll (15-30s) but showed NO freshness signal,
 * so a stalled poll was indistinguishable from a live one. This renders
 * "updated Ns ago · polls every Ms" and re-ticks every second, turning amber
 * when the data is older than 2 poll intervals (poll stalled / tab throttled).
 *
 * Client-only by construction: `at` is set from useEffect fetches, so it is
 * always null during SSR/hydration (renders nothing) — no React #418 risk.
 */

import { useEffect, useState } from "react";

export function LastUpdated({ at, pollMs }: { at: number | null; pollMs: number }) {
  const [, force] = useState(0);
  useEffect(() => {
    const id = setInterval(() => force((n) => n + 1), 1000);
    return () => clearInterval(id);
  }, []);

  if (at == null) return null;
  const ageS = Math.max(0, Math.round((Date.now() - at) / 1000));
  const stalled = ageS * 1000 > pollMs * 2;
  return (
    <span
      className={`font-mono text-[10px] uppercase tracking-wider tabular-nums ${
        stalled ? "text-warning" : "text-muted-foreground/60"
      }`}
      title={stalled ? "poll appears stalled (no fresh data for >2 intervals)" : `polls every ${Math.round(pollMs / 1000)}s`}
    >
      {stalled ? "stale · " : ""}updated {ageS}s ago
    </span>
  );
}
