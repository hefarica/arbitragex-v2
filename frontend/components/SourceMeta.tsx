"use client";

/**
 * SourceMeta — discreet operational provenance for a panel: which upstream the
 * data came from and how fresh it is, e.g. "source: edge · updated 3s ago".
 *
 * Operator-console doctrine: every live screen should make its data source and
 * freshness legible, so a stalled poll never masquerades as a healthy one. It
 * reuses LastUpdated for the freshness/staleness tick (turns amber when the
 * poll stalls). Client-only by construction (LastUpdated reads Date.now in an
 * effect), so it renders the source label deterministically and the freshness
 * only after mount — no hydration mismatch.
 */
import * as React from "react";

import { LastUpdated } from "@/components/last-updated";

export function SourceMeta({
  source,
  at = null,
  pollMs,
  className = "",
}: {
  /** the real upstream: "edge", "api-server", "websocket", … (never invented). */
  source: string;
  /** ms epoch of the last successful fetch; null → freshness hidden. */
  at?: number | null;
  /** poll interval (ms); when set, enables the "updated Ns ago / stale" tick. */
  pollMs?: number;
  className?: string;
}): React.ReactElement {
  return (
    <span
      data-testid="source-meta"
      className={`inline-flex items-center gap-1.5 font-mono text-[10px] uppercase tracking-wider text-muted-foreground/60 ${className}`}
    >
      <span>source: {source}</span>
      {pollMs != null && at != null && (
        <>
          <span aria-hidden>·</span>
          <LastUpdated at={at} pollMs={pollMs} />
        </>
      )}
    </span>
  );
}

export default SourceMeta;
