"use client";

/**
 * ProgressRealCard — workspace progress for the operator, split into two
 * SEMANTICALLY DISTINCT sections so a viewer can never confuse a hand-edited
 * doctrine number for a live measurement (the bug this refactor fixes):
 *
 *   ── SECTION 1: LIVE RUNTIME (auto-refreshed, measured) ──────────────────
 *      Derived on every mount from the backend and fail-honest (R8): never
 *      green by default, "Unavailable" when an endpoint is unreachable.
 *        - Readiness gates green = summary.green / summary.total from
 *          GET /api/readiness (17 live verifiers; the denominator is read
 *          from summary.total, NEVER hardcoded). This is the real progress
 *          meter — it moves as gates actually flip green.
 *        - GO live   = readiness.flip_blocked (true ⇒ NO-GO).
 *        - engine    = /api/strategies/runtime-status strategies[].engine_loaded.
 *
 *   ── SECTION 2: DOCTRINAL MILESTONES (manual, hand-edited) ───────────────
 *      PROJECT DOCTRINE values, NOT backend metrics. They move ONLY when a
 *      human bumps the constant on a milestone-completing commit, so they are
 *      labelled "manual" and stamped with MILESTONES_LAST_UPDATED. Honest by
 *      construction: the UI tells you they are manual and how stale they are.
 *      The P0/P1/A.4/A.5/Capital tiles are likewise manual doctrine — they are
 *      grouped here, NOT next to the live tiles, so they cannot masquerade as
 *      runtime status.
 *
 * Why not wire A.4/A.5/Capital to the backend? Their authoritative backend
 * source (/api/readiness/blockers doctrinalBlockers, /api/readiness/decision)
 * is itself a hardcoded milestone literal — fetching it would only disguise a
 * hardcode as live data (arbx-no-hardcode-doctrine). So they stay honest
 * manual doctrine until a real runtime verifier emits them.
 *
 * Hard rules (CLAUDE.md §24 + arbx-mev-ethics-gate):
 *   - "Live trading" tile is statically OFF; no tx-submitting code path exists.
 *   - "Capital exposure" is $0 (manual invariant; no signer funded).
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
// Updating these requires, IN THE SAME COMMIT:
//   1. A milestone-completing commit (e.g. "A.5 paper-shadow PASS").
//   2. Bump the relevant percentage HERE.
//   3. Bump MILESTONES_LAST_UPDATED to the commit date.
//
// This is the SINGLE PLACE in the codebase where milestone completion %
// is recorded. Do NOT scatter copies across pages.
//
// MILESTONES_LAST_UPDATED is the git date of the most recent edit to the
// constants below (git blame: 85/58/100 → 2026-05-13 c06bd04;
// 80 → 2026-06-14 413ad79). It is a COMMITTED constant, never Date.now() —
// a runtime timestamp would falsely imply these were re-verified on load.
// ─────────────────────────────────────────────────────────────────────────

const MILESTONES_LAST_UPDATED = "2026-06-14";

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
const FE_AUDIT_DETAIL =
  "Self-assessed (not measured coverage): routes, components, hooks, schemas, " +
  "edge contract — all mapped read-only in FASE 0.";

const FE_INTEGRATION_PCT = 80; // code-brechas: 4 endpoints wired + panels rendered + nav/home/sidebar/gate
const FE_INTEGRATION_DETAIL =
  "Code brechas closed: /api/metrics/paper-shadow + /api/sim-ctl/fork-status endpoints, with " +
  "ForkValidationPanel + PaperShadowPanel now rendered on /live-readiness; opportunity simulate " +
  "+ alertmanager-webhook wired; 4-group nav (credentials first); home tiles (live-readiness, " +
  "credentials, paper/history); dynamic paper-mode badge; PaperModeToggle readiness gate. " +
  "Remaining: live data needs operator RPC/sim injection (runbook); ServiceControlPanel (501 by " +
  "design); SimulationPipelineTimeline.";

type RuntimeProbe =
  | { kind: "loading" }
  | {
      kind: "ready";
      flipBlocked: boolean | null;
      engineLoaded: boolean | null;
      readinessGreen: number | null;
      readinessTotal: number | null;
      readinessGeneratedAt: string | null;
      readinessError: string | null;
      runtimeError: string | null;
    };

async function probe(): Promise<RuntimeProbe> {
  const [rd, rs] = await Promise.allSettled([getReadiness(), getRuntimeStatus(1)]);

  let flipBlocked: boolean | null = null;
  let readinessGreen: number | null = null;
  let readinessTotal: number | null = null;
  let readinessGeneratedAt: string | null = null;
  let readinessError: string | null = null;
  if (rd.status === "fulfilled" && rd.value.ok) {
    flipBlocked = rd.value.data.flip_blocked;
    readinessGreen = rd.value.data.summary.green;
    readinessTotal = rd.value.data.summary.total;
    readinessGeneratedAt = rd.value.data.generated_at;
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

  return {
    kind: "ready",
    flipBlocked,
    engineLoaded,
    readinessGreen,
    readinessTotal,
    readinessGeneratedAt,
    readinessError,
    runtimeError,
  };
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
            <CardTitle>Workspace progress</CardTitle>
            <CardDescription>
              Two sections. <strong>Live runtime</strong> is read live from the backend on
              each page load.{" "}
              <strong>Doctrinal milestones</strong> are manual values, hand-edited on
              milestone-completing commits — not live telemetry.
            </CardDescription>
          </div>
          <Badge variant="destructive" className="font-mono text-[10px] uppercase">
            <ShieldOffIcon className="mr-1 inline size-3" />
            Live trading: OFF
          </Badge>
        </div>
      </CardHeader>
      <CardContent className="space-y-6">
        {/* ── SECTION 1: LIVE RUNTIME (measured, auto-refreshed, fail-honest) ── */}
        <section data-slot="live-runtime-section" className="space-y-4">
          <div className="flex items-center gap-2">
            <h3 className="text-sm font-semibold">Live runtime</h3>
            <Badge variant="outline" className="font-mono text-[10px]">
              <ShieldCheckIcon className="mr-1 inline size-3 text-success" />
              live · on load
            </Badge>
          </div>

          <LiveReadinessRow state={state} />

          <div className="grid grid-cols-2 gap-2 sm:grid-cols-3">
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
        </section>

        {/* ── SECTION 2: DOCTRINAL MILESTONES (manual, hand-edited) ── */}
        <section data-slot="doctrinal-milestones-section" className="space-y-4 border-t pt-5">
          <div className="flex flex-wrap items-center gap-2">
            <h3 className="text-sm font-semibold">Doctrinal milestones</h3>
            <Badge variant="outline" className="font-mono text-[10px]">
              doctrinal · manual · workspace-verified
            </Badge>
          </div>
          <p className="text-xs text-muted-foreground">
            Manual values — hand-edited, they move only on milestone-completing commits (not
            live telemetry). Last updated {MILESTONES_LAST_UPDATED}.
          </p>

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
            detail={FE_AUDIT_DETAIL}
            tone="info"
          />
          <ProgressRow
            label="Frontend integration applied"
            pct={FE_INTEGRATION_PCT}
            detail={FE_INTEGRATION_DETAIL}
            tone="info"
          />

          <div className="grid grid-cols-2 gap-2 border-t pt-4 sm:grid-cols-3 lg:grid-cols-5">
            <RuntimeTile label="P0 banner" value="PASS" tone="success" />
            <RuntimeTile label="P1 progress + evidence" value="IN PROGRESS" tone="info" />
            <RuntimeTile label="A.4 fork" value="BLOCKED" tone="warning" />
            <RuntimeTile label="A.5 paper-shadow" value="NO-GO" tone="warning" />
            <RuntimeTile label="Capital exposure" value="$0" tone="success" />
          </div>
        </section>
      </CardContent>
    </Card>
  );
}

// ── Live readiness aggregate: N of M green from /api/readiness summary ──
// Fail-honest: shows "Loading…" before the probe resolves and "Unavailable"
// (with no progress fill) when the endpoint errors — it NEVER fabricates a
// count. The denominator is summary.total (live), never a hardcoded 16/17.
function LiveReadinessRow({ state }: { state: RuntimeProbe }) {
  if (state.kind === "loading") {
    return <ReadinessRow value="Loading…" detail="Querying /api/readiness…" />;
  }
  if (
    state.readinessError !== null ||
    state.readinessGreen === null ||
    state.readinessTotal === null ||
    state.readinessTotal === 0
  ) {
    return (
      <ReadinessRow
        value="Unavailable"
        tone="warning"
        detail={
          state.readinessError
            ? `readiness endpoint error: ${state.readinessError}`
            : "readiness endpoint returned no summary"
        }
      />
    );
  }
  const pct = Math.round((100 * state.readinessGreen) / state.readinessTotal);
  return (
    <ReadinessRow
      value={`${state.readinessGreen} of ${state.readinessTotal} green`}
      pct={pct}
      detail={`Live — verified by the readiness endpoint${
        state.readinessGeneratedAt ? ` · as of ${state.readinessGeneratedAt}` : ""
      }`}
    />
  );
}

function ReadinessRow({
  value,
  detail,
  pct = null,
  tone = "live",
}: {
  value: string;
  detail: string;
  pct?: number | null;
  tone?: "live" | "warning";
}) {
  return (
    <div data-slot="live-readiness-row" className="space-y-1.5">
      <div className="flex items-baseline justify-between text-sm">
        <span className="font-medium">Readiness gates green</span>
        <span
          className={`font-mono tabular-nums ${
            tone === "warning" ? "text-amber-600 dark:text-amber-400" : "text-foreground"
          }`}
        >
          {value}
          {pct !== null && <span className="ml-1 text-muted-foreground">· {pct}%</span>}
        </span>
      </div>
      {pct !== null ? (
        <Progress value={pct} aria-label={`Readiness gates: ${pct}% green`} />
      ) : (
        <div
          className="h-2 w-full rounded-full border border-dashed border-muted-foreground/30"
          aria-hidden
        />
      )}
      <p className="text-xs leading-relaxed text-muted-foreground">{detail}</p>
    </div>
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
