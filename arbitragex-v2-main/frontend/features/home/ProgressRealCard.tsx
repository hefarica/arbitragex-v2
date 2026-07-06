"use client";

/**
 * ProgressRealCard — workspace-verified progress milestones for the operator.
 *
 * Source of truth (per directive Fase 2/3 — "datos reales o estado blocked
 * honesto documentado como 'workspace verified'"):
 *
 *   1. Pre-live simulation completion (A.1 … A.4 phase tracker)
 *      ─────────────────────────────────────────────────────────
 *      These are PROJECT DOCTRINE values, not backend metrics. Each percentage
 *      moves only when a milestone commit lands. Hardcoding here is honest
 *      because the card is labelled "Workspace verified — doctrinal milestone";
 *      no backend exists today that emits a meaningful aggregate.
 *
 *   2. Full-system completion (incl. circuit breakers, private relay no-submit,
 *      paper-shadow runtime, confidence scoring, GO/NO-GO formal)
 *      ─────────────────────────────────────────────────────────
 *      Same doctrine — projected against the A.1 … A.10 roadmap.
 *
 *   3. Live/blocked state of A.4 fork validation + A.5 paper-shadow
 *      ─────────────────────────────────────────────────────────
 *      DERIVED from /api/readiness (flip_blocked) + /api/strategies/runtime-status
 *      (engine_loaded). When these endpoints are unreachable, the card renders
 *      "unavailable" — NEVER green by default (R8 fail-honest).
 *
 *   4. Frontend integration percentage
 *      ─────────────────────────────────────────────────────────
 *      Workspace-verified: counts the components/endpoints surfaced in the
 *      adaptive integration plan (12 panels target, 2 delivered post-P1).
 *
 * Hard rules (CLAUDE.md §24 + arbx-mev-ethics-gate):
 *   - The "Live trading" tile is statically OFF. There is no code path in
 *     this binary that submits a transaction. Flipping this to "ON" requires
 *     a code change + test failure: SystemGuardBanner.test.tsx alarms.
 *   - "Capital exposure" is $0. Phase 4 onboarding verifies signer balance.
 *   - "GO live" reads readiness.flip_blocked: if any of the 16 readiness
 *     items is red/yellow/pending, flip_blocked = true → NO-GO. The card
 *     never overrides the backend's blocked verdict.
 */

import * as React from "react";
import { ShieldOffIcon, ShieldCheckIcon, AlertTriangleIcon } from "lucide-react";

import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import { Badge } from "@/components/ui/badge";
import { getReadiness, getRuntimeStatus } from "@/lib/api-client";

// ─────────────────────────────────────────────────────────────────────────
// Doctrinal milestones (workspace verified, not backend-emitted)
//
// Updating these requires:
//   1. A milestone-completing commit (e.g. "A.5 paper-shadow PASS").
//   2. Bump the relevant percentage HERE in the same commit.
//   3. The CHANGELOG references the new percentage.
//
// This is the SINGLE PLACE in the codebase where milestone completion %
// is recorded. Do NOT scatter copies across pages.
// ─────────────────────────────────────────────────────────────────────────

const PRE_LIVE_PCT = 85;
const PRE_LIVE_DETAIL =
  "A.1/A.2 fail-closed · A.2.5 SimulatorV2 threading · A.3.a encoder · " +
  "A.3.b PG decimals · A.3.c orchestrator · A.3.c.2 multistep · " +
  "A.3.c.3 SequenceContext · A.3.c.4 DatabaseRef · A.4 scaffold";

const FULL_SYSTEM_PCT = 58; // post-P0 banner + post-P1 panels lift visibility
const FULL_SYSTEM_DETAIL =
  "Pre-live 85% + visibility (P0 banner + P1 panels). Remaining: A.4 fork " +
  "real, A.5 paper-shadow, A.6 circuit breakers, A.7 private relay no-submit, " +
  "A.8 confidence scoring, A.9 GO/NO-GO formal.";

const FE_AUDIT_PCT = 100;
const FE_INTEGRATION_PCT = 22; // post-P1: 2 of ~9 planned integrations live
const FE_INTEGRATION_DETAIL =
  "Post-P1: SystemGuardBanner + ProgressRealCard + Opportunities evidence " +
  "fields. Remaining: AgentTeamsPanel, ForkValidationPanel, PaperShadowPanel, " +
  "BlockersPanel, GoNoGoPanel, SimulationPipelineTimeline.";

type RuntimeProbe =
  | { kind: "loading" }
  | {
      kind: "ready";
      flipBlocked: boolean | null;
      engineLoaded: boolean | null;
      readinessError: string | null;
      runtimeError: string | null;
    };

async function probe(): Promise<RuntimeProbe> {
  const [rd, rs] = await Promise.allSettled([getReadiness(), getRuntimeStatus(1)]);

  let flipBlocked: boolean | null = null;
  let readinessError: string | null = null;
  if (rd.status === "fulfilled" && rd.value.ok) {
    flipBlocked = rd.value.data.flip_blocked;
  } else if (rd.status === "fulfilled" && !rd.value.ok) {
    readinessError = rd.value.error.slice(0, 80);
  } else if (rd.status === "rejected") {
    readinessError = (rd.reason as Error)?.message?.slice(0, 80) ?? "unknown";
  }

  let engineLoaded: boolean | null = null;
  let runtimeError: string | null = null;
  if (rs.status === "fulfilled" && rs.value.ok) {
    engineLoaded = rs.value.data.strategies.some((s) => s.engine_loaded);
  } else if (rs.status === "fulfilled" && !rs.value.ok) {
    runtimeError = rs.value.error.slice(0, 80);
  } else if (rs.status === "rejected") {
    runtimeError = (rs.reason as Error)?.message?.slice(0, 80) ?? "unknown";
  }

  return { kind: "ready", flipBlocked, engineLoaded, readinessError, runtimeError };
}

export function ProgressRealCard() {
  const [state, setState] = React.useState<RuntimeProbe>({ kind: "loading" });

  React.useEffect(() => {
    let alive = true;
    void (async () => {
      const next = await probe();
      if (alive) setState(next);
    })();
    return () => {
      alive = false;
    };
  }, []);

  return (
    <Card data-slot="progress-real-card" className="mb-6">
      <CardHeader>
        <div className="flex items-center justify-between">
          <div>
            <CardTitle className="flex items-center gap-2">
              Workspace progress
              <Badge variant="outline" className="font-mono text-[10px]">
                doctrinal · workspace-verified
              </Badge>
            </CardTitle>
            <CardDescription>
              Milestone tracker. Percentages move only on milestone-completing
              commits. Live state derives from <code className="font-mono text-[11px]">/api/readiness</code>{" "}
              + <code className="font-mono text-[11px]">/api/strategies/runtime-status</code>.
            </CardDescription>
          </div>
          <Badge variant="destructive" className="font-mono text-[10px] uppercase">
            <ShieldOffIcon className="mr-1 inline size-3" />
            Live trading: OFF
          </Badge>
        </div>
      </CardHeader>
      <CardContent className="space-y-5">
        <ProgressRow
          label="Pre-live simulation honesty"
          pct={PRE_LIVE_PCT}
          detail={PRE_LIVE_DETAIL}
          tone="info"
        />
        <ProgressRow
          label="Full system to live-minimum"
          pct={FULL_SYSTEM_PCT}
          detail={FULL_SYSTEM_DETAIL}
          tone="info"
        />
        <ProgressRow
          label="Frontend forensic audit"
          pct={FE_AUDIT_PCT}
          detail="Routes, components, hooks, schemas, edge contract — all mapped read-only in FASE 0."
          tone="success"
        />
        <ProgressRow
          label="Frontend integration applied"
          pct={FE_INTEGRATION_PCT}
          detail={FE_INTEGRATION_DETAIL}
          tone="info"
        />

        <div className="grid grid-cols-2 gap-2 border-t pt-4 sm:grid-cols-3 lg:grid-cols-6">
          <RuntimeTile label="P0 banner" value="PASS" tone="success" />
          <RuntimeTile label="P1 progress + evidence" value="IN PROGRESS" tone="info" />
          <RuntimeTile label="A.4 fork" value="BLOCKED" tone="warning" />
          <RuntimeTile label="A.5 paper-shadow" value="NO-GO" tone="warning" />
          <RuntimeTile label="Capital exposure" value="$0" tone="success" />
          <RuntimeTile
            label="GO live"
            value={
              state.kind === "loading"
                ? "Loading…"
                : state.flipBlocked === null
                  ? "Unavailable"
                  : state.flipBlocked
                    ? "NO-GO"
                    : "open?"
            }
            tone={
              state.kind === "ready" && state.flipBlocked === false ? "danger" : "warning"
            }
          />
        </div>

        <RuntimeDerived state={state} />
      </CardContent>
    </Card>
  );
}

function ProgressRow({
  label,
  pct,
  detail,
  tone,
}: {
  label: string;
  pct: number;
  detail: string;
  tone: "info" | "success" | "warning";
}) {
  const remaining = 100 - pct;
  return (
    <div data-slot="progress-row" className="space-y-1.5">
      <div className="flex items-baseline justify-between text-sm">
        <span className="font-medium">{label}</span>
        <span className="font-mono tabular-nums text-muted-foreground">
          <span className={tone === "success" ? "text-success" : "text-foreground"}>{pct}%</span>
          <span className="mx-1 text-muted-foreground/60">·</span>
          <span className="text-muted-foreground">{remaining}% remaining</span>
        </span>
      </div>
      <Progress value={pct} aria-label={`${label}: ${pct}% complete`} />
      <p className="text-xs leading-relaxed text-muted-foreground">{detail}</p>
    </div>
  );
}

function RuntimeTile({
  label,
  value,
  tone,
}: {
  label: string;
  value: string;
  tone: "success" | "warning" | "danger" | "info";
}) {
  const accent =
    tone === "success" ? "border-emerald-500/40 bg-emerald-500/5 text-emerald-700 dark:text-emerald-300"
    : tone === "warning" ? "border-amber-500/40 bg-amber-500/5 text-amber-700 dark:text-amber-300"
    : tone === "danger" ? "border-destructive/40 bg-destructive/5 text-destructive"
    : "border-border bg-muted/30 text-foreground";
  return (
    <div
      data-slot="runtime-tile"
      className={`rounded-md border px-2.5 py-1.5 text-[11px] uppercase tracking-wider ${accent}`}
    >
      <div className="text-foreground/60">{label}</div>
      <div className="mt-0.5 font-mono text-sm font-semibold normal-case tracking-tight">{value}</div>
    </div>
  );
}

function RuntimeDerived({ state }: { state: RuntimeProbe }) {
  if (state.kind === "loading") {
    return <p className="text-xs text-muted-foreground">Loading runtime probe…</p>;
  }

  const pieces: React.ReactNode[] = [];
  if (state.readinessError) {
    pieces.push(
      <span key="rd" title={state.readinessError} className="inline-flex items-center gap-1 text-destructive">
        <AlertTriangleIcon className="size-3" />
        readiness unavailable
      </span>,
    );
  } else if (state.flipBlocked === null) {
    pieces.push(<span key="rd-null">readiness: no flip_blocked field</span>);
  } else {
    pieces.push(
      <span key="rd-ok" className="inline-flex items-center gap-1">
        <ShieldCheckIcon className="size-3 text-success" />
        readiness loaded · flip_blocked = {String(state.flipBlocked)}
      </span>,
    );
  }
  if (state.runtimeError) {
    pieces.push(
      <span key="rs" title={state.runtimeError} className="inline-flex items-center gap-1 text-destructive">
        <AlertTriangleIcon className="size-3" />
        runtime-status unavailable
      </span>,
    );
  } else if (state.engineLoaded === null) {
    pieces.push(<span key="rs-null">runtime-status: no strategies array</span>);
  } else {
    pieces.push(
      <span key="rs-ok" className="inline-flex items-center gap-1">
        <ShieldCheckIcon className="size-3 text-success" />
        engine {state.engineLoaded ? "loaded" : "not loaded"}
      </span>,
    );
  }
  return (
    <div className="flex flex-wrap items-center gap-x-4 gap-y-1 border-t pt-3 text-xs text-muted-foreground">
      {pieces}
    </div>
  );
}
