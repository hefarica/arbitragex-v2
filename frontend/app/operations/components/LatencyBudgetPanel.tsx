"use client";

/**
 * ARBX-QB-07-008 — thin client wrapper for LatencyBudgetCard.
 *
 * R1 mounted-snapshot: the Server Component fetches the tick once (boot
 * snapshot → props); this wrapper overlays the LIVE tick from the omni-store
 * (ArbxRealtimeProvider owns the cadence — no own poll, no own socket).
 * Overlay rule (RULE 00): the live tick WINS only when it actually carries
 * the lat.* block; a live tick WITHOUT the key (knob off / not emitted yet)
 * keeps serving the boot snapshot — real data from a real GET, never a
 * fabricated default. The card caption discloses the source rule.
 */

// SSR-test support (repo pattern): classic JSX path needs the React namespace.
import * as React from "react";

import { useRouteTick } from "@/lib/store/omni-store";
import type { LatencyStageRow } from "@/lib/apex/schemas";

import { LatencyBudgetCard } from "./LatencyBudgetCard";

interface Props {
  /** Boot snapshot from the /operations Server Component (GET tick). */
  initialStages: LatencyStageRow[] | null;
  initialPassP95: boolean | null;
  initialCycles: number;
  /** Boot-fetch error, rendered verbatim while no stages exist (R8). */
  initialError: string | null;
}

export function LatencyBudgetPanel({
  initialStages,
  initialPassP95,
  initialCycles,
  initialError,
}: Props) {
  const live = useRouteTick();
  const liveHasLat = Array.isArray(live?.lat_stages);

  return (
    <LatencyBudgetCard
      stages={liveHasLat ? (live.lat_stages as LatencyStageRow[]) : initialStages}
      passP95={liveHasLat ? live.lat_pass_p95 ?? null : initialPassP95}
      cycles={liveHasLat ? live.lat_cycles ?? 0 : initialCycles}
      error={initialError}
    />
  );
}
