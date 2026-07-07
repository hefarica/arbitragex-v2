"use client";

/**
 * ConfidenceScoringPanel — A.8 wire status surface.
 *
 * Consumes /api/scoring/status (proxied to /api/v1/scoring/status).
 *
 * Renders the component map (which primitives are wired) + the blocked
 * reasons (what stops the pipeline from being LIVE). Two states matter:
 *
 *   - scoring_status="enabled"   → all components wired, recent_scored_count
 *                                  may be > 0; pipeline emitting scores.
 *   - scoring_status="partial"   → primitives wired but pipeline integration
 *                                  pending. This is the current honest state.
 *   - scoring_status="blocked"   → critical blocker (e.g., compile failure).
 *   - scoring_status="not_available" → endpoint reachable but no components.
 *
 * R8 fail-honest contract:
 *   - Loading: spinner only, no "all wired" claim.
 *   - Error: verbatim edge detail.
 *   - ok: every component shows its wired/not-wired state and evidence_ref.
 *
 * Structural barriers (echo SystemGuardBanner):
 *   - live_trading: ALWAYS false (asserted by test)
 *   - capital_exposure_usd: ALWAYS 0
 *   - Panel has NO control to enable live or submit
 */

import * as React from "react";
import {
  CheckCircle2Icon,
  AlertCircleIcon,
  ShieldOffIcon,
  RefreshCwIcon,
  CircleIcon,
} from "lucide-react";

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Accordion, AccordionContent, AccordionItem, AccordionTrigger } from "@/components/ui/accordion";
import { Progress } from "@/components/ui/progress";
import { cn } from "@/lib/utils";
import { getScoringStatus } from "@/lib/api-client";
import type { ScoringStatusResponse } from "@/lib/schemas";

type State =
  | { kind: "loading" }
  | { kind: "error"; detail: string }
  | { kind: "ok"; data: ScoringStatusResponse };

const POLL_MS = 30_000;

export function ConfidenceScoringPanel() {
  const [state, setState] = React.useState<State>({ kind: "loading" });

  React.useEffect(() => {
    let alive = true;
    const tick = async () => {
      const r = await getScoringStatus();
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
    <Card data-slot="confidence-scoring-panel">
      <CardHeader>
        <div className="flex items-center justify-between">
          <div>
            <CardTitle className="flex items-center gap-2">
              Confidence scoring (A.8)
              <Badge variant="destructive" className="font-mono text-[10px] uppercase">
                <ShieldOffIcon className="mr-1 inline size-3" />
                paper-only · live OFF
              </Badge>
            </CardTitle>
            <CardDescription>
              Honest wire status of Bayesian + Kelly + VPIN primitives in the
              paper pipeline. Schema: <code className="font-mono text-[11px]">/api/scoring/status</code>.
              Scoring NEVER enables live trading.
            </CardDescription>
          </div>
          {state.kind === "ok" && <ScoringStatusBadge status={state.data.scoring_status} />}
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        {state.kind === "loading" && (
          <div className="flex items-center gap-2 text-xs text-muted-foreground">
            <RefreshCwIcon className="size-3 animate-spin" aria-hidden="true" />
            Loading scoring wire status…
          </div>
        )}
        {state.kind === "error" && (
          <Alert variant="destructive">
            <AlertCircleIcon />
            <AlertTitle>Cannot fetch scoring status</AlertTitle>
            <AlertDescription className="font-mono text-xs">{state.detail}</AlertDescription>
          </Alert>
        )}
        {state.kind === "ok" && <ScoringBody data={state.data} />}
      </CardContent>
    </Card>
  );
}

function ScoringStatusBadge({ status }: { status: string }) {
  if (status === "enabled") {
    return <Badge variant="info" className="font-mono text-[10px] uppercase">enabled</Badge>;
  }
  if (status === "partial") {
    return (
      <Badge variant="outline" className="border-amber-500/40 bg-amber-500/10 font-mono text-[10px] uppercase text-amber-700 dark:text-amber-300">
        partial
      </Badge>
    );
  }
  return <Badge variant="destructive" className="font-mono text-[10px] uppercase">{status}</Badge>;
}

function ScoringBody({ data }: { data: ScoringStatusResponse }) {
  const totalWired = data.components.filter((c) => c.wired).length;
  const total = data.components.length;
  const pctBps = total > 0 ? Math.round((totalWired / total) * 100) : 0;
  return (
    <div className="space-y-4">
      <div className="space-y-1.5">
        <div className="flex items-baseline justify-between text-sm">
          <span className="font-medium">Wire coverage</span>
          <span className="font-mono tabular-nums text-muted-foreground">
            <span className={totalWired === total ? "text-success" : "text-foreground"}>
              {totalWired} / {total}
            </span>
            <span className="mx-1">·</span>
            <span>{pctBps}%</span>
          </span>
        </div>
        <Progress value={pctBps} aria-label={`Wire coverage ${pctBps}%`} />
      </div>

      <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
        <FlagTile label="Mode" value={data.mode} tone="safe" />
        <FlagTile label="Live trading" value={data.live_trading ? "ON" : "OFF"} tone={data.live_trading ? "danger" : "safe"} />
        <FlagTile label="Submit" value={data.submit_enabled ? "ON" : "OFF"} tone={data.submit_enabled ? "danger" : "safe"} />
        <FlagTile label="Capital" value={`$${data.capital_exposure_usd}`} tone="safe" />
      </div>

      <div className="grid grid-cols-2 gap-2 sm:grid-cols-3">
        <SignalTile label="Bayesian filter" wired={data.bayesian_filter_wired} />
        <SignalTile label="Kelly sizing" wired={data.kelly_sizing_wired} />
        <SignalTile label="VPIN toxicity" wired={data.vpin_wired} />
        <SignalTile label="Pipeline integrated" wired={data.scoring_pipeline_wired} />
        <SignalTile label="Recent scored" wired={data.recent_scored_count > 0} valueLabel={String(data.recent_scored_count)} />
        <SignalTile label="Threshold (bps)" wired={data.confidence_threshold_bps > 0} valueLabel={String(data.confidence_threshold_bps)} />
      </div>

      <Accordion type="multiple" className="w-full">
        <AccordionItem value="components">
          <AccordionTrigger>
            <span className="text-sm font-medium">Component wire map ({total} items)</span>
          </AccordionTrigger>
          <AccordionContent>
            <ul className="space-y-2 text-xs">
              {data.components.map((c) => (
                <li
                  key={c.name}
                  data-wired={c.wired}
                  className="rounded-md border bg-muted/30 p-2"
                >
                  <div className="flex items-center gap-2">
                    {c.wired ? (
                      <CheckCircle2Icon className="size-4 text-emerald-700 dark:text-emerald-300" />
                    ) : (
                      <CircleIcon className="size-4 text-muted-foreground" />
                    )}
                    <span className="font-medium">{c.name}</span>
                    <Badge
                      variant={c.wired ? "info" : "outline"}
                      className="ml-auto font-mono text-[9px]"
                    >
                      {c.wired ? "WIRED" : "PENDING"}
                    </Badge>
                  </div>
                  <p className="mt-1 text-muted-foreground">{c.description}</p>
                  <code className="mt-1 block break-all font-mono text-[10px] text-muted-foreground">
                    {c.evidence_ref}
                  </code>
                </li>
              ))}
            </ul>
          </AccordionContent>
        </AccordionItem>
        <AccordionItem value="blockers">
          <AccordionTrigger>
            <span className="text-sm font-medium">
              Blocked reasons ({data.blocked_reasons.length})
            </span>
          </AccordionTrigger>
          <AccordionContent>
            <ul className="space-y-2 text-xs">
              {data.blocked_reasons.length === 0 ? (
                <li className="italic text-muted-foreground">No blockers reported.</li>
              ) : (
                data.blocked_reasons.map((b) => (
                  <li key={b.id} data-severity={b.severity} className="rounded-md border bg-muted/30 p-2">
                    <div className="flex items-center gap-2">
                      <SeverityDot severity={b.severity} />
                      <span className="font-mono text-[10px] uppercase">{b.severity}</span>
                      <span className="font-medium">{b.id}</span>
                    </div>
                    <p className="mt-1">{b.description}</p>
                    <code className="mt-1 block break-all font-mono text-[10px] text-muted-foreground">
                      {b.evidence_ref}
                    </code>
                    <p className="mt-1 text-muted-foreground">
                      <span className="font-mono text-[10px] uppercase">Required action: </span>
                      {b.required_action}
                    </p>
                  </li>
                ))
              )}
            </ul>
          </AccordionContent>
        </AccordionItem>
      </Accordion>

      <div className="rounded-md border bg-muted/30 p-3 text-xs">
        <div className="text-[10px] uppercase tracking-wider text-muted-foreground">Next action</div>
        <p className="mt-0.5">{data.next_action}</p>
      </div>

      <div className="flex flex-wrap items-center gap-3 border-t pt-3 text-[11px] text-muted-foreground">
        <span>
          version <code className="font-mono">{data.scoring_version}</code>
        </span>
        <span>·</span>
        <span>
          last scored {data.last_scored_at ?? <em className="text-muted-foreground/60">never</em>}
        </span>
        <span>·</span>
        <span>
          decisions: {data.available_decisions.length}
        </span>
      </div>
    </div>
  );
}

function FlagTile({ label, value, tone }: { label: string; value: string; tone: "safe" | "danger" }) {
  const accent =
    tone === "safe"
      ? "border-emerald-500/40 bg-emerald-500/5 text-emerald-700 dark:text-emerald-300"
      : "border-destructive/40 bg-destructive/5 text-destructive";
  return (
    <div className={`rounded-md border px-2.5 py-1.5 text-[11px] uppercase tracking-wider ${accent}`}>
      <div className="text-foreground/60">{label}</div>
      <div className="mt-0.5 font-mono text-sm font-semibold normal-case tracking-tight">{value}</div>
    </div>
  );
}

function SignalTile({
  label,
  wired,
  valueLabel,
}: {
  label: string;
  wired: boolean;
  valueLabel?: string;
}) {
  const accent = wired
    ? "border-emerald-500/40 bg-emerald-500/5 text-emerald-700 dark:text-emerald-300"
    : "border-amber-500/40 bg-amber-500/5 text-amber-700 dark:text-amber-300";
  return (
    <div
      data-slot="signal-tile"
      data-wired={wired}
      className={cn("rounded-md border px-2.5 py-1.5 text-[11px] uppercase tracking-wider", accent)}
    >
      <div className="text-foreground/60">{label}</div>
      <div className="mt-0.5 font-mono text-sm font-semibold normal-case tracking-tight">
        {valueLabel ?? (wired ? "WIRED" : "PENDING")}
      </div>
    </div>
  );
}

function SeverityDot({ severity }: { severity: string }) {
  const cls =
    severity === "critical" ? "bg-destructive"
    : severity === "high" ? "bg-amber-500"
    : severity === "medium" ? "bg-yellow-400"
    : "bg-muted-foreground";
  return <span className={cn("inline-block size-2 rounded-full", cls)} />;
}
