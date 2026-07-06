"use client";

/**
 * AgentTeamsPanel — surfaces the 17 ArbitrageX Agent Teams (P2-continued).
 *
 * Consumes /api/agents/status (proxied to /api/v1/agents/status).
 *
 * The panel honestly distinguishes:
 *   - workspace_verified verdicts (from the most recent operator pass)
 *   - runtime overlays (where the backend has demoted PASS to BLOCKED on
 *     account of a failing prerequisite — e.g. verifyAll failure).
 *
 * R8 fail-honest:
 *   - loading → spinner row, never any "all green" claim
 *   - error → verbatim edge error, NEVER fabricated agent list
 *   - empty → "No agents reported" tile (should not happen — 17 are hardcoded)
 *   - ok → summary tiles + accordion of agents grouped by verdict
 *
 * Polling cadence: 30s. Agent verdicts move on commits, not on milliseconds.
 */

import * as React from "react";
import {
  AlertCircleIcon,
  CheckCircle2Icon,
  ShieldOffIcon,
  RefreshCwIcon,
  CircleIcon,
} from "lucide-react";

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Accordion, AccordionContent, AccordionItem, AccordionTrigger } from "@/components/ui/accordion";
import { cn } from "@/lib/utils";
import { getAgentTeamsStatus } from "@/lib/api-client";
import type { AgentStatusRow, AgentsStatusResponse } from "@/lib/schemas";

type State =
  | { kind: "loading" }
  | { kind: "error"; detail: string }
  | { kind: "ok"; data: AgentsStatusResponse };

const POLL_MS = 30_000;

export function AgentTeamsPanel() {
  const [state, setState] = React.useState<State>({ kind: "loading" });

  React.useEffect(() => {
    let alive = true;
    const tick = async () => {
      const r = await getAgentTeamsStatus();
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
    <Card data-slot="agent-teams-panel">
      <CardHeader>
        <div className="flex items-center justify-between">
          <div>
            <CardTitle className="flex items-center gap-2">
              Agent Teams
              <Badge variant="outline" className="font-mono text-[10px]">
                workspace-verified · 17 agents
              </Badge>
            </CardTitle>
            <CardDescription>
              Verdicts of the build/audit/deploy agent cycle. Source <code className="font-mono text-[11px]">workspace_verified</code> reflects the most recent operator pass; <code className="font-mono text-[11px]">runtime</code> appears when the backend demotes a verdict on a failing prerequisite.
            </CardDescription>
          </div>
          {state.kind === "ok" && <SummaryRow summary={state.data.summary} />}
        </div>
      </CardHeader>
      <CardContent className="space-y-3">
        {state.kind === "loading" && (
          <div className="flex items-center gap-2 text-xs text-muted-foreground">
            <RefreshCwIcon className="size-3 animate-spin" aria-hidden="true" />
            Loading agent teams status from edge…
          </div>
        )}
        {state.kind === "error" && (
          <Alert variant="destructive">
            <AlertCircleIcon />
            <AlertTitle>Cannot fetch agent teams status</AlertTitle>
            <AlertDescription className="font-mono text-xs">{state.detail}</AlertDescription>
          </Alert>
        )}
        {state.kind === "ok" && <AgentsList agents={state.data.agents} />}
      </CardContent>
    </Card>
  );
}

function SummaryRow({ summary }: { summary: AgentsStatusResponse["summary"] }) {
  return (
    <div className="flex flex-wrap gap-1">
      {summary.pass > 0 && (
        <Badge variant="outline" className="border-emerald-500/40 bg-emerald-500/10 font-mono text-[10px] text-emerald-700 dark:text-emerald-300">
          {summary.pass} PASS
        </Badge>
      )}
      {summary.partial > 0 && (
        <Badge variant="outline" className="border-amber-500/40 bg-amber-500/10 font-mono text-[10px] text-amber-700 dark:text-amber-300">
          {summary.partial} PARTIAL
        </Badge>
      )}
      {summary.blocked > 0 && (
        <Badge variant="destructive" className="font-mono text-[10px]">
          {summary.blocked} BLOCKED
        </Badge>
      )}
      {summary.no_go > 0 && (
        <Badge variant="destructive" className="font-mono text-[10px]">
          {summary.no_go} NO-GO
        </Badge>
      )}
      {summary.not_run > 0 && (
        <Badge variant="outline" className="font-mono text-[10px]">
          {summary.not_run} NOT RUN
        </Badge>
      )}
      {summary.unknown > 0 && (
        <Badge variant="outline" className="font-mono text-[10px]">
          {summary.unknown} UNKNOWN
        </Badge>
      )}
    </div>
  );
}

function AgentsList({ agents }: { agents: AgentStatusRow[] }) {
  if (agents.length === 0) {
    return (
      <div className="rounded-md border bg-muted/30 p-3 text-xs italic text-muted-foreground">
        No agents reported. This is unexpected — check api-server logs.
      </div>
    );
  }
  // Verdict priority: NO_GO > BLOCKED > PARTIAL > UNKNOWN > NOT_RUN > PASS.
  // Critical visibility first.
  const order: Record<string, number> = {
    NO_GO: 0, BLOCKED: 1, PARTIAL: 2, UNKNOWN: 3, NOT_RUN: 4, PASS: 5,
  };
  const sorted = [...agents].sort((a, b) => (order[a.verdict] ?? 99) - (order[b.verdict] ?? 99));

  return (
    <Accordion type="multiple" className="w-full">
      {sorted.map((a) => (
        <AccordionItem key={a.id} value={a.id} data-verdict={a.verdict}>
          <AccordionTrigger>
            <div className="flex flex-1 items-center justify-between pr-2">
              <div className="flex items-center gap-2 text-left">
                <VerdictIcon verdict={a.verdict} />
                <div>
                  <div className="text-sm font-medium">{a.name}</div>
                  <div className="font-mono text-[10px] text-muted-foreground">{a.id}</div>
                </div>
              </div>
              <div className="flex flex-wrap items-center gap-1">
                <VerdictBadge verdict={a.verdict} />
                <SourceBadge source={a.source} />
                {a.blocks.map((p) => (
                  <Badge key={p} variant="outline" className="font-mono text-[9px]">
                    {p}
                  </Badge>
                ))}
              </div>
            </div>
          </AccordionTrigger>
          <AccordionContent>
            <div className="space-y-2 text-xs">
              <div>
                <div className="text-[10px] uppercase tracking-wider text-muted-foreground">
                  Evidence
                </div>
                <ul className="mt-0.5 list-inside list-disc space-y-0.5">
                  {a.evidence.length === 0 ? (
                    <li className="italic text-muted-foreground">No evidence recorded.</li>
                  ) : (
                    a.evidence.map((line, i) => <li key={i}>{line}</li>)
                  )}
                </ul>
              </div>
              {a.next_action && (
                <div className="rounded-md border bg-muted/30 p-2">
                  <div className="text-[10px] uppercase tracking-wider text-muted-foreground">
                    Next action
                  </div>
                  <p className="mt-0.5">{a.next_action}</p>
                </div>
              )}
              <MetaLine agent={a} />
            </div>
          </AccordionContent>
        </AccordionItem>
      ))}
    </Accordion>
  );
}

function VerdictIcon({ verdict }: { verdict: string }) {
  if (verdict === "PASS") return <CheckCircle2Icon className="size-4 text-emerald-700 dark:text-emerald-300" aria-hidden="true" />;
  if (verdict === "NO_GO" || verdict === "BLOCKED") return <ShieldOffIcon className="size-4 text-destructive" aria-hidden="true" />;
  if (verdict === "PARTIAL") return <CircleIcon className="size-4 text-amber-700 dark:text-amber-300" aria-hidden="true" />;
  return <CircleIcon className="size-4 text-muted-foreground" aria-hidden="true" />;
}

function VerdictBadge({ verdict }: { verdict: string }) {
  if (verdict === "PASS") {
    return <Badge variant="info" className="font-mono text-[10px]">PASS</Badge>;
  }
  if (verdict === "PARTIAL") {
    return <Badge variant="outline" className="border-amber-500/40 bg-amber-500/10 font-mono text-[10px] text-amber-700 dark:text-amber-300">PARTIAL</Badge>;
  }
  if (verdict === "BLOCKED" || verdict === "NO_GO") {
    return <Badge variant="destructive" className="font-mono text-[10px]">{verdict}</Badge>;
  }
  return <Badge variant="outline" className="font-mono text-[10px]">{verdict}</Badge>;
}

function SourceBadge({ source }: { source: string }) {
  const accent =
    source === "runtime"
      ? "border-info/40 bg-info/10"
      : source === "workspace_verified"
        ? "border-border"
        : "border-muted-foreground/40";
  return (
    <Badge variant="outline" className={cn("font-mono text-[9px] uppercase", accent)}>
      {source === "workspace_verified" ? "ws-verified" : source}
    </Badge>
  );
}

function MetaLine({ agent }: { agent: AgentStatusRow }) {
  const parts: string[] = [
    `category=${agent.category}`,
    `risk=${agent.risk}`,
    `status=${agent.status}`,
    `operator_required=${String(agent.operator_required)}`,
  ];
  if (agent.last_run_at) parts.push(`last_run_at=${agent.last_run_at}`);
  return (
    <code className="block break-all font-mono text-[10px] text-muted-foreground">
      {parts.join(" · ")}
    </code>
  );
}
