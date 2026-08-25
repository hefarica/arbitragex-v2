"use client";

/**
 * FE-MASTER · Route-discovery funnel card (FE-0038 — §46).
 *
 * The "Market Events → … → Reconciled" strip on /operations: the §46 chain
 * over three real wires — the provider-fed tick (upstream half, same wire
 * the §18 header reads; ArbxRealtimeProvider owns that cadence), the
 * outcomes sink summary (24h window, own poll — distinct datum from any
 * other surface's query), and the recon summary (Reconciled terminus, 30s
 * poll). Stages render as a tagged strip with per-stage window disclosure —
 * NEVER a proportional bar chart, because tick counters and windowed totals
 * do not share a scale (that would fabricate a conversion between them).
 *
 * R8: an absent tick field (knob OFF) or an unavailable downstream wire
 * renders the honest dash with the reason in the title; nothing is zeroed.
 * Shadow / read-only — no writes, no capital.
 */

// SSR-test support (repo pattern): classic JSX path needs the React namespace.
import * as React from "react";
import { NetworkIcon } from "lucide-react";

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { useRouteTick } from "@/lib/store/omni-store";
import { useRouteDiscoveryOutcomes } from "@/lib/hooks/useRouteDiscoveryOutcomes";
import { getReconSummary } from "@/lib/api-client";

import {
  buildRouteFunnelStages,
  FUNNEL_WINDOW_NOTE,
  type FunnelStage,
} from "./route-funnel";

const DASH = "—";
const RECON_POLL_MS = 30_000;

export function RouteDiscoveryFunnelCard() {
  const tick = useRouteTick();
  // Distinct window ⇒ distinct datum: this poll is not a duplicate of the
  // /route-outcomes page's (different surface, same read-only endpoint).
  const { totals: outcomeTotals } = useRouteDiscoveryOutcomes(24);

  const [recon, setRecon] = React.useState<{
    included: number | null;
    error: string | null;
  }>({ included: null, error: null });

  React.useEffect(() => {
    if (typeof window === "undefined") return;
    let alive = true;
    const poll = async () => {
      const res = await getReconSummary(24);
      if (!alive) return;
      if (res.ok) setRecon({ included: res.data.totals.included, error: null });
      else setRecon({ included: null, error: res.error });
    };
    void poll();
    const id = setInterval(() => void poll(), RECON_POLL_MS);
    return () => {
      alive = false;
      clearInterval(id);
    };
  }, []);

  const stages = buildRouteFunnelStages(
    tick,
    outcomeTotals,
    recon.included === null ? null : { included: recon.included },
  );

  return (
    <Card data-slot="route-discovery-funnel-card">
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-base">
          <NetworkIcon className="size-4 text-primary" />
          Route Discovery Funnel · Market Events → Reconciled (§46)
        </CardTitle>
        <CardDescription>
          {recon.error ? (
            <span className="font-mono text-xs text-warning">
              recon unavailable: {recon.error} — la etapa Reconciled queda en guion honesto
            </span>
          ) : (
            "Upstream = último tick (provider-fed); downstream = ventanas 24h del sink de outcomes y del recon."
          )}
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-2">
        <div
          className="flex flex-wrap items-center gap-1.5"
          data-testid="route-funnel-strip"
          role="list"
          aria-label="Route discovery funnel stages"
        >
          {stages.map((stage: FunnelStage) => (
            <span
              key={stage.id}
              role="listitem"
              title={`${stage.source} · ${stage.hint}`}
              className="inline-flex items-center gap-1.5 rounded-md border px-2 py-1 text-xs font-mono"
              data-stage={stage.id}
            >
              <span className="text-muted-foreground">{stage.label}</span>
              <span
                className={`font-semibold tabular-nums ${
                  stage.value === null ? "text-muted-foreground/60" : ""
                }`}
              >
                {stage.value === null ? DASH : stage.value.toLocaleString()}
              </span>
              <span className="rounded bg-muted px-1 text-[9px] uppercase tracking-wide text-muted-foreground">
                {stage.window}
              </span>
            </span>
          ))}
        </div>
        <p className="text-[10px] text-muted-foreground">{FUNNEL_WINDOW_NOTE}</p>
      </CardContent>
    </Card>
  );
}
