/**
 * B1.b — ChainStatusBadge
 *
 * Renders Active/Disabled/Probing/Unknown badges for a chain row. The badge
 * derives state purely from props so it is server-renderable and SSR-safe.
 *
 * R8 fail-honest: `enabled === null` ⇒ "Unknown" (we surface absence rather
 * than fake an Active state). The badge never invents status.
 */
"use client";

import * as React from "react";
import { Badge } from "@/components/ui/badge";

interface Props {
  enabled: boolean | null;
  reason?: string | null;
}

export function ChainStatusBadge({ enabled, reason }: Props) {
  if (enabled === null) {
    return (
      <Badge variant="outline" title={reason ?? "Status unknown — chain row missing or fetch failed"}>
        Unknown
      </Badge>
    );
  }
  if (enabled) {
    return (
      <Badge variant="success" title={reason ?? "Chain enabled in chains_runtime"}>
        Active
      </Badge>
    );
  }
  return (
    <Badge variant="secondary" title={reason ?? "Soft-disabled — searcher-rs ignores this chain"}>
      Disabled
    </Badge>
  );
}
