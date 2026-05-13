"use client";

/**
 * BlockersPanel — flat readiness blockers list (P2).
 *
 * Consumes /api/readiness/blockers (proxied to /api/v1/readiness/blockers).
 * Renders each blocker with severity tone, required action, and the phases
 * it blocks (A.4, A.5, A.6, A.7, A.8, A.9, LIVE).
 *
 * R8 fail-honest contract:
 *   - On loading: skeleton row, no "all clear" claim.
 *   - On error: destructive alert with the verbatim edge error message.
 *   - On empty (zero blockers): displays "All blockers resolved" — this
 *     SHOULD never happen pre-A.9 because the doctrine list always has
 *     entries, but the UI handles it gracefully.
 *   - Env values are NEVER displayed (evidence.redacted_value is "present"
 *     or null; we never get the raw value here).
 *
 * Polling: 30s, much slower than SystemGuardBanner (15s) because blockers
 * change on operator-driven events (commits, env exports), not runtime drift.
 */

import * as React from "react";
import { AlertCircleIcon, ShieldOffIcon, RefreshCwIcon, CheckCircle2Icon } from "lucide-react";

import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Accordion, AccordionContent, AccordionItem, AccordionTrigger } from "@/components/ui/accordion";
import { cn } from "@/lib/utils";
import { getReadinessBlockers } from "@/lib/api-client";
import type { ReadinessBlocker, ReadinessBlockersResponse } from "@/lib/schemas";

type State =
  | { kind: "loading" }
  | { kind: "error"; detail: string }
  | { kind: "ok"; data: ReadinessBlockersResponse };

const POLL_MS = 30_000;

export function BlockersPanel() {
  const [state, setState] = React.useState<State>({ kind: "loading" });

  React.useEffect(() => {
    let alive = true;
    const tick = async () => {
      const r = await getReadinessBlockers();
      if (!alive) return;
      if (r.ok) setState({ kind: "ok", data: r.data });
      else setState({ kind: "error", detail: r.error.slice(0, 200) });
    };
    void tick();
    const id = setInterval(tick, POLL_MS);
    return () => {
      alive = false;
      clearInterval(id);
    };
  }, []);

  return (
    <Card data-slot="blockers-panel">
      <CardHeader>
        <div className="flex items-center justify-between">
          <div>
            <CardTitle className="flex items-center gap-2">
              <ShieldOffIcon className="size-4 text-destructive" aria-hidden="true" />
              Readiness blockers
            </CardTitle>
            <CardDescription>
              Concrete obstacles between the current paper state and any live
              flip. Sourced from <code className="font-mono text-[11px]">/api/readiness/blockers</code>.
              Env values are redacted to <code className="font-mono text-[11px]">"present"</code> or <code className="font-mono text-[11px]">null</code> — raw values never reach the UI.
            </CardDescription>
          </div>
          {state.kind === "ok" && <SummaryBadges summary={state.data.summary} />}
        </div>
      </CardHeader>
      <CardContent className="space-y-3">
        {state.kind === "loading" && <LoadingRows />}
        {state.kind === "error" && <ErrorAlert detail={state.detail} />}
        {state.kind === "ok" && <BlockersList blockers={state.data.blockers} />}
      </CardContent>
    </Card>
  );
}

function LoadingRows() {
  return (
    <div className="flex items-center gap-2 text-xs text-muted-foreground">
      <RefreshCwIcon className="size-3 animate-spin" aria-hidden="true" />
      Loading blockers from edge…
    </div>
  );
}

function ErrorAlert({ detail }: { detail: string }) {
  // CRITICAL R8: on error we DO NOT render "no blockers". We render the
  // error verbatim so the operator sees the failure surface.
  return (
    <Alert variant="destructive">
      <AlertCircleIcon />
      <AlertTitle>Cannot fetch blockers</AlertTitle>
      <AlertDescription className="font-mono text-xs">{detail}</AlertDescription>
    </Alert>
  );
}

function SummaryBadges({
  summary,
}: {
  summary: ReadinessBlockersResponse["summary"];
}) {
  return (
    <div className="flex flex-wrap gap-1">
      {summary.critical > 0 && (
        <Badge variant="destructive" className="font-mono text-[10px]">
          {summary.critical} critical
        </Badge>
      )}
      {summary.high > 0 && (
        <Badge variant="outline" className="border-amber-500/40 bg-amber-500/10 font-mono text-[10px] text-amber-700 dark:text-amber-300">
          {summary.high} high
        </Badge>
      )}
      {summary.medium > 0 && (
        <Badge variant="outline" className="font-mono text-[10px]">
          {summary.medium} medium
        </Badge>
      )}
      {summary.low > 0 && (
        <Badge variant="outline" className="font-mono text-[10px]">
          {summary.low} low
        </Badge>
      )}
      {summary.blocked_phases.length > 0 && (
        <Badge variant="outline" className="font-mono text-[10px]">
          blocks: {summary.blocked_phases.join(", ")}
        </Badge>
      )}
    </div>
  );
}

function BlockersList({ blockers }: { blockers: ReadinessBlocker[] }) {
  if (blockers.length === 0) {
    return (
      <div className="flex items-center gap-2 rounded-md border border-emerald-500/40 bg-emerald-500/5 p-3 text-sm">
        <CheckCircle2Icon className="size-4 text-emerald-700 dark:text-emerald-300" />
        <span>All readiness blockers resolved (verify A.9 formal sign-off before any flip).</span>
      </div>
    );
  }
  // Group by severity for visual ordering (critical first).
  const order: Record<string, number> = { critical: 0, high: 1, medium: 2, low: 3 };
  const sorted = [...blockers].sort((a, b) => (order[a.severity] ?? 99) - (order[b.severity] ?? 99));

  return (
    <Accordion type="multiple" className="w-full">
      {sorted.map((b) => (
        <AccordionItem key={b.id} value={b.id} data-severity={b.severity}>
          <AccordionTrigger>
            <div className="flex flex-1 items-center justify-between pr-2">
              <div className="flex items-center gap-2">
                <SeverityDot severity={b.severity} />
                <span className="text-sm font-medium">{b.title}</span>
              </div>
              <div className="flex flex-wrap gap-1">
                <Badge variant="outline" className="font-mono text-[9px] uppercase">
                  {b.status}
                </Badge>
                {b.blocks.map((p) => (
                  <Badge key={p} variant="outline" className="font-mono text-[9px]">
                    {p}
                  </Badge>
                ))}
              </div>
            </div>
          </AccordionTrigger>
          <AccordionContent>
            <div className="space-y-2 text-xs">
              <p className="text-muted-foreground">{b.description}</p>
              <div className="rounded-md border bg-muted/30 p-2">
                <div className="text-[10px] uppercase tracking-wider text-muted-foreground">Required action</div>
                <p className="mt-0.5 text-foreground">{b.required_action}</p>
              </div>
              <EvidenceLine blocker={b} />
            </div>
          </AccordionContent>
        </AccordionItem>
      ))}
    </Accordion>
  );
}

function SeverityDot({ severity }: { severity: string }) {
  const cls =
    severity === "critical" ? "bg-destructive"
    : severity === "high" ? "bg-amber-500"
    : severity === "medium" ? "bg-yellow-400"
    : "bg-muted-foreground";
  return (
    <span
      aria-label={severity}
      className={cn("inline-block size-2 rounded-full", cls)}
    />
  );
}

function EvidenceLine({ blocker }: { blocker: ReadinessBlocker }) {
  const ev = blocker.evidence;
  const parts: string[] = [`source=${ev.source}`];
  if (ev.source === "env") {
    parts.push(`env_present=${String(ev.env_present)}`);
    if (ev.value_length !== null) parts.push(`length=${ev.value_length}`);
  }
  if (ev.readiness_id) parts.push(`readiness_id=${ev.readiness_id}`);
  if (ev.readiness_status) parts.push(`status=${ev.readiness_status}`);
  parts.push(`operator_required=${String(blocker.operator_required)}`);
  parts.push(`category=${blocker.category}`);
  return (
    <code className="block break-all font-mono text-[10px] text-muted-foreground">
      {parts.join(" · ")}
    </code>
  );
}
