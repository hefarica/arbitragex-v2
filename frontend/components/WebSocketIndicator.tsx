"use client";

import { useEffect, useState } from "react";

import { useWsStatus, type WsStatus } from "@/lib/store/omni-store";
import { cn } from "@/lib/utils";

/**
 * WebSocketIndicator — surfaces the REAL live-opportunity stream status from the
 * Omni-Store SSOT (set by useOmniOpportunities via socket.io, with an HTTP
 * polling fallback). It never opens its own socket and never fabricates "LIVE":
 *
 *   LIVE        → socket.io connected, receiving mempool opportunities
 *   POLLING     → degraded; WS unavailable, HTTP fallback active
 *   CONNECTING  → handshake in progress
 *   STALE       → connection dropped / errored
 *   IDLE        → no stream consumer mounted (default outside /opportunities)
 *
 * R1: gated behind isMounted so the SSR paint (neutral default) matches the
 *   first client paint, then upgrades to the live store value — zero hydration
 *   risk.
 * R8 fail-honest: renders exactly what the store reports; shows "IDLE" when
 *   nothing is streaming rather than a misleading green.
 */

interface IndicatorMeta {
  label: string;
  className: string;
  pulse: boolean;
}

const STATUS_META: Record<WsStatus, IndicatorMeta> = {
  LIVE: { label: "LIVE", className: "bg-success/15 text-success border-success/40", pulse: true },
  POLLING: { label: "POLLING", className: "bg-warning/15 text-warning border-warning/40", pulse: true },
  CONNECTING: { label: "CONNECTING", className: "bg-info/15 text-info border-info/40", pulse: true },
  STALE: { label: "STALE", className: "bg-destructive/15 text-destructive border-destructive/40", pulse: false },
  DISCONNECTED: { label: "IDLE", className: "bg-muted/60 text-muted-foreground border-border", pulse: false },
};

export function WebSocketIndicator() {
  const [isMounted, setIsMounted] = useState(false);
  const status = useWsStatus();

  useEffect(() => {
    setIsMounted(true);
  }, []);

  // Pre-mount: render the neutral default so SSR and first CSR paint agree.
  const meta = isMounted ? STATUS_META[status] : STATUS_META.DISCONNECTED;

  return (
    <span
      title={`Live opportunity stream: ${meta.label}`}
      aria-label={`Live opportunity stream status: ${meta.label}`}
      className={cn(
        "hidden items-center gap-1.5 rounded-md border px-2 py-1 text-[11px] font-mono font-medium sm:inline-flex",
        meta.className,
      )}
    >
      <span
        className={cn("size-1.5 rounded-full bg-current", meta.pulse && "animate-pulse")}
        aria-hidden
      />
      {meta.label}
    </span>
  );
}
