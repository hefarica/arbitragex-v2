"use client";

/**
 * RiskCircuitPanel — A.6 comprehensive circuit breakers surface.
 *
 * Consumes /api/risk/circuit-breakers/status (proxied to /api/v1/...).
 *
 * Renders 10 breakers grouped by state priority (KILLED > PAUSED > BLOCKED >
 * WARN > UNKNOWN > NOT_AVAILABLE > PASS). Each breaker shows its current
 * value, threshold, unit, evidence source, action, and what it blocks.
 *
 * R8 fail-honest contract:
 *   - loading → spinner
 *   - error   → verbatim edge detail
 *   - ok      → real breaker rows; NEVER "all green" if any state ≠ PASS
 *   - NOT_AVAILABLE rows are clearly badged so the operator knows the
 *     breaker is structurally inert until a data source exists
 *   - BLOCKED rows include the prerequisite (env, A.4, A.5)
 *
 * Immutable safety tiles (echo SystemGuardBanner):
 *   - Live trading: OFF (always)
 *   - Capital: $0 (always)
 *
 * Polling: 15s — kill_switch can change at any moment.
 */

import * as React from "react";
import {
  CheckCircle2Icon,
  AlertCircleIcon,
  ShieldOffIcon,
  RefreshCwIcon,
  CircleIcon,
  ZapIcon,
} from "lucide-react";

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Accordion, AccordionContent, AccordionItem, AccordionTrigger } from "@/components/ui/accordion";
import { cn } from "@/lib/utils";
import { getCircuitBreakersStatus } from "@/lib/api-client";
import type { CircuitBreaker, CircuitBreakersStatusResponse } from "@/lib/schemas";

type State =
  | { kind: "loading" }
  | { kind: "error"; detail: string }
  | { kind: "ok"; data: CircuitBreakersStatusResponse };

const POLL_MS = 15_000;

export function RiskCircuitPanel() {
  const [state, setState] = React.useState<State>({ kind: "loading" });

  React.useEffect(() => {
    let alive = true;
    const tick = async () => {
      const r = await getCircuitBreakersStatus();
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
    <Card data-slot="risk-circuit-panel">
      <CardHeader>
        <div className="flex items-center justify-between">
          <div>
            <CardTitle className="flex items-center gap-2">
              <ZapIcon className="size-4 text-destructive" aria-hidden="true" />
              Risk circuit breakers (A.6)
              <Badge variant="destructive" className="font-mono text-[10px] uppercase">
                <ShieldOffIcon className="mr-1 inline size-3" />
                live OFF
              </Badge>
            </CardTitle>
            <CardDescription>
              10 comprehensive breakers. Source: <code className="font-mono text-[11px]">/api/risk/circuit-breakers/status</code>.
              Missing data sources render <code className="font-mono text-[11px]">NOT_AVAILABLE</code> — never fabricated PASS.
            </CardDescription>
          </div>
          {state.kind === "ok" && <OverallStateBadge state={state.data.overall_state} />}
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        {state.kind === "loading" && (
          <div className="flex items-center gap-2 text-xs text-muted-foreground">
            <RefreshCwIcon className="size-3 animate-spin" aria-hidden="true" />
            Loading circuit breaker status…
          </div>
        )}
        {state.kind === "error" && (
          <Alert variant="destructive">
            <AlertCircleIcon />
            <AlertTitle>Cannot fetch circuit breakers</AlertTitle>
            <AlertDescription className="font-mono text-xs">{state.detail}</AlertDescription>
          </Alert>
        )}
        {state.kind === "ok" && <Body data={state.data} />}
      </CardContent>
    </Card>
  );
}

function OverallStateBadge({ state }: { state: string }) {
  const variant: "info" | "destructive" | "outline" =
    state === "PASS" ? "info"
    : state === "KILLED" || state === "PAUSED" || state === "BLOCKED" ? "destructive"
    : "outline";
  return (
    <Badge variant={variant} className="font-mono text-[10px] uppercase">
      Overall: {state}
    </Badge>
  );
}

function Body({ data }: { data: CircuitBreakersStatusResponse }) {
  const s = data.summary;
  return (
    <div className="space-y-4">
      <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
        <SummaryTile label="PASS" value={s.pass} tone="safe" />
        <SummaryTile label="WARN" value={s.warn} tone="warning" />
        <SummaryTile label="PAUSED" value={s.paused} tone="danger" />
        <SummaryTile label="KILLED" value={s.killed} tone="danger" />
        <SummaryTile label="BLOCKED" value={s.blocked} tone="danger" />
        <SummaryTile label="NOT_AVAILABLE" value={s.not_available} tone="muted" />
        <SummaryTile label="UNKNOWN" value={s.unknown} tone="muted" />
        <SummaryTile label="TOTAL" value={s.total} tone="muted" />
      </div>

      <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
        <FlagTile label="Mode" value={data.mode} tone="safe" />
        <FlagTile label="Live trading" value={data.live_trading ? "ON" : "OFF"} tone={data.live_trading ? "danger" : "safe"} />
        <FlagTile label="Submit" value={data.submit_enabled ? "ON" : "OFF"} tone={data.submit_enabled ? "danger" : "safe"} />
        <FlagTile label="Capital" value={`$${data.capital_exposure_usd}`} tone="safe" />
      </div>

      <BreakerList breakers={data.breakers} />

      <div className="rounded-md border bg-muted/30 p-3 text-xs">
        <div className="text-[10px] uppercase tracking-wider text-muted-foreground">Next action</div>
        <p className="mt-0.5">{data.next_action}</p>
      </div>

      <div className="flex flex-wrap items-center gap-3 border-t pt-3 text-[11px] text-muted-foreground">
        <span>version <code className="font-mono">{data.version}</code></span>
        <span>·</span>
        <span>generated <code className="font-mono">{data.generated_at}</code></span>
      </div>
    </div>
  );
}

function BreakerList({ breakers }: { breakers: CircuitBreaker[] }) {
  // Priority order: critical states first.
  const order: Record<string, number> = {
    KILLED: 0, PAUSED: 1, BLOCKED: 2, WARN: 3, UNKNOWN: 4, NOT_AVAILABLE: 5, PASS: 6,
  };
  const sorted = [...breakers].sort((a, b) => (order[a.state] ?? 99) - (order[b.state] ?? 99));

  return (
    <Accordion type="multiple" className="w-full">
      {sorted.map((b) => (
        <AccordionItem key={b.id} value={b.id} data-state-val={b.state}>
          <AccordionTrigger>
            <div className="flex flex-1 items-center justify-between pr-2">
              <div className="flex items-center gap-2 text-left">
                <StateIcon state={b.state} />
                <div>
                  <div className="text-sm font-medium">{b.name}</div>
                  <div className="font-mono text-[10px] text-muted-foreground">
                    {b.id} · {b.category}
                  </div>
                </div>
              </div>
              <div className="flex flex-wrap items-center gap-1">
                <StateBadge state={b.state} />
                {b.action !== "none" && (
                  <Badge variant="outline" className="font-mono text-[9px] uppercase">
                    action: {b.action}
                  </Badge>
                )}
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

              <div className="grid grid-cols-2 gap-2 sm:grid-cols-3">
                <KvTile k="state" v={b.state} />
                <KvTile k="severity" v={b.severity} />
                <KvTile k="current" v={fmtValue(b.evidence.current_value, b.evidence.unit)} />
                <KvTile k="threshold" v={fmtValue(b.evidence.threshold, b.evidence.unit)} />
                <KvTile k="source" v={b.evidence.source} />
                <KvTile k="operator_required" v={String(b.operator_required)} />
              </div>

              <div className="rounded-md border bg-muted/30 p-2">
                <div className="text-[10px] uppercase tracking-wider text-muted-foreground">Evidence</div>
                <p className="mt-0.5">{b.evidence.detail}</p>
                {b.evidence.ref && (
                  <code className="mt-1 block break-all font-mono text-[10px] text-muted-foreground">
                    ref: {b.evidence.ref}
                  </code>
                )}
              </div>

              {b.required_action && (
                <div className="rounded-md border bg-muted/30 p-2">
                  <div className="text-[10px] uppercase tracking-wider text-muted-foreground">
                    Required action
                  </div>
                  <p className="mt-0.5">{b.required_action}</p>
                </div>
              )}
            </div>
          </AccordionContent>
        </AccordionItem>
      ))}
    </Accordion>
  );
}

function fmtValue(v: string | number | null, unit: string | null): string {
  if (v === null || v === undefined) return "—";
  const base = typeof v === "number" ? String(v) : v;
  return unit ? `${base} ${unit}` : base;
}

function StateIcon({ state }: { state: string }) {
  if (state === "PASS") return <CheckCircle2Icon className="size-4 text-emerald-700 dark:text-emerald-300" aria-hidden="true" />;
  if (state === "KILLED" || state === "PAUSED" || state === "BLOCKED")
    return <ShieldOffIcon className="size-4 text-destructive" aria-hidden="true" />;
  if (state === "WARN") return <CircleIcon className="size-4 text-amber-700 dark:text-amber-300" aria-hidden="true" />;
  return <CircleIcon className="size-4 text-muted-foreground" aria-hidden="true" />;
}

function StateBadge({ state }: { state: string }) {
  if (state === "PASS") {
    return <Badge variant="info" className="font-mono text-[10px]">PASS</Badge>;
  }
  if (state === "WARN") {
    return <Badge variant="outline" className="border-amber-500/40 bg-amber-500/10 font-mono text-[10px] text-amber-700 dark:text-amber-300">WARN</Badge>;
  }
  if (state === "KILLED" || state === "PAUSED" || state === "BLOCKED") {
    return <Badge variant="destructive" className="font-mono text-[10px]">{state}</Badge>;
  }
  return <Badge variant="outline" className="font-mono text-[10px]">{state}</Badge>;
}

function SummaryTile({
  label,
  value,
  tone,
}: {
  label: string;
  value: number;
  tone: "safe" | "warning" | "danger" | "muted";
}) {
  const accent =
    tone === "safe" ? "border-emerald-500/40 bg-emerald-500/5 text-emerald-700 dark:text-emerald-300"
    : tone === "warning" ? "border-amber-500/40 bg-amber-500/5 text-amber-700 dark:text-amber-300"
    : tone === "danger" ? "border-destructive/40 bg-destructive/5 text-destructive"
    : "border-border bg-muted/30 text-foreground";
  return (
    <div className={cn("rounded-md border px-2.5 py-1.5 text-[11px] uppercase tracking-wider", accent)}>
      <div className="text-foreground/60">{label}</div>
      <div className="mt-0.5 font-mono text-lg font-semibold normal-case tracking-tight tabular-nums">{value}</div>
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

function KvTile({ k, v }: { k: string; v: string }) {
  return (
    <div className="rounded border bg-muted/20 p-1.5">
      <div className="text-[9px] uppercase text-muted-foreground">{k}</div>
      <div className="font-mono text-[11px]">{v}</div>
    </div>
  );
}
