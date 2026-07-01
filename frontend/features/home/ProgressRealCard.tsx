"use client";

/**
 * ProgressRealCard — workspace progress for the operator, split into two
 * SEMANTICALLY DISTINCT sections so a hand-edited doctrine number can never be
 * confused for a live measurement:
 *
 *   ── SECTION 1: LIVE RUNTIME (auto-derived, measured, fail-honest) ───────
 *      A thin composer over THREE already-wired endpoints; a value only moves
 *      when a real backend field moves, and absence renders text — never a
 *      fabricated number (R8). Auto-derived rows:
 *        - Readiness gates green = summary.green / summary.total (denominator
 *          read from summary.total, never hardcoded) — GET /api/readiness.
 *        - Per-group readiness ratios (6 groups) — client groupBy on items[].
 *        - Scanner-heartbeat engines K/S + candidates_1h + rejections_1h +
 *          pg/redis source health — GET /api/strategies/runtime-status.
 *          NB: engine_loaded is INFERRED from a Redis scanner-heartbeat key,
 *          so it is labelled "scanner heartbeat", not "engine registered".
 *        - GO live = readiness.flip_blocked.
 *
 *   ── SECTION 2: DOCTRINAL MILESTONES (manual, hand-edited) ───────────────
 *      The 4 bars + P0/P1/A.4/A.5/Capital tiles have NO machine-readable
 *      source (re-verified: PACKAGE_MANIFEST, app.toml, feature_manifest,
 *      STATUS_REPORT — none map to them). They are PROJECT DOCTRINE that moves
 *      ONLY when a human bumps the constant on a milestone-completing commit,
 *      so they are labelled "manual" and stamped MILESTONES_LAST_UPDATED.
 *      Auto-deriving them would require a new per-bar verifier set or a typed
 *      milestones manifest carrying real evidence — until then they stay here,
 *      visually separate from the live section, never auto-bumped.
 *
 * Not derivable today (DESIGN-ONLY, never faked): CI / last-deploy state —
 * needs a new endpoint + a GitHub token (actions:read); and a workflow
 * "success" is not proof of a live deploy (the VPS ref can advance without a
 * rebuild), so no CI badge is rendered.
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
import { getReadiness, getRuntimeStatus, getReadinessDecision } from "@/lib/api-client";

// ─────────────────────────────────────────────────────────────────────────
// Auto-derive engine — pure derivation over already-fetched endpoints.
// Exported for direct unit testing (the engine's logic, not its markup).
// A number only moves when a real endpoint field moves; absence is rendered
// as text by the callers, never as a fabricated value (fail-honest).
// ─────────────────────────────────────────────────────────────────────────

export type ReadinessGroupRatio = { group: string; label: string; green: number; total: number };

const READINESS_GROUP_LABELS: Record<string, string> = {
  security_compliance: "Security & compliance",
  audit_trail: "Audit trail",
  risk_doctrines: "Risk doctrines",
  tokens_strategies: "Tokens & strategies",
  contracts: "Contracts",
  operations: "Operations",
};
const READINESS_GROUP_ORDER = Object.keys(READINESS_GROUP_LABELS);

export function groupReadiness(
  items: ReadonlyArray<{ group: string; status: string }>,
): ReadinessGroupRatio[] {
  const counts = new Map<string, { green: number; total: number }>();
  for (const it of items) {
    const e = counts.get(it.group) ?? { green: 0, total: 0 };
    e.total += 1;
    if (it.status === "green") e.green += 1;
    counts.set(it.group, e);
  }
  // Stable known order first; then any unknown group the backend may add later.
  const known = READINESS_GROUP_ORDER.filter((g) => counts.has(g));
  const unknown = [...counts.keys()].filter((g) => !READINESS_GROUP_ORDER.includes(g));
  return [...known, ...unknown].map((group) => {
    const c = counts.get(group)!;
    return { group, label: READINESS_GROUP_LABELS[group] ?? group, green: c.green, total: c.total };
  });
}

export type RuntimeSummary = {
  enginesLoaded: number;
  enginesTotal: number;
  candidates1h: number;
  rejections1h: number;
};

export function summarizeRuntime(
  strategies: ReadonlyArray<{ engine_loaded: boolean; candidates_1h: number; rejections_1h: number }>,
): RuntimeSummary {
  return {
    enginesLoaded: strategies.filter((s) => s.engine_loaded).length,
    enginesTotal: strategies.length,
    candidates1h: strategies.reduce((n, s) => n + s.candidates_1h, 0),
    rejections1h: strategies.reduce((n, s) => n + s.rejections_1h, 0),
  };
}

export type DecisionSummary = {
  paperMode: boolean | null;
  goA5: boolean | null;
  reasons: string[];
  nextAction: string | null;
};

// paper_mode + go_a5 are genuinely dynamic decision fields; reasons/next_action
// are the live "why blocked / what next". verdict is intentionally excluded —
// it is hardcoded NO_GO server-side, so it is not a live signal. Null input
// (endpoint unavailable) yields nulls + empty reasons — never a fabricated verdict.
export function summarizeDecision(
  decision:
    | { paper_mode: boolean; go_a5: boolean; reasons: string[]; next_action: string }
    | null,
): DecisionSummary {
  if (!decision) return { paperMode: null, goA5: null, reasons: [], nextAction: null };
  return {
    paperMode: decision.paper_mode,
    goA5: decision.go_a5,
    reasons: decision.reasons,
    nextAction: decision.next_action,
  };
}

// ─────────────────────────────────────────────────────────────────────────
// Doctrinal milestones (workspace verified, NOT backend-emitted)
//
// Updating these requires, IN THE SAME COMMIT:
//   1. A milestone-completing commit (e.g. "A.5 paper-shadow PASS").
//   2. Bump the relevant percentage HERE.
//   3. Bump MILESTONES_LAST_UPDATED to the commit date.
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
      readinessGreen: number | null;
      readinessTotal: number | null;
      readinessGroups: ReadinessGroupRatio[] | null;
      readinessGeneratedAt: string | null;
      readinessError: string | null;
      runtime: RuntimeSummary | null;
      sourcePostgres: string | null;
      sourceRedis: string | null;
      runtimeError: string | null;
      decision: DecisionSummary;
      decisionError: string | null;
    };

async function probe(): Promise<RuntimeProbe> {
  const [rd, rs, dc] = await Promise.allSettled([
    getReadiness(),
    getRuntimeStatus(1),
    getReadinessDecision(),
  ]);

  let flipBlocked: boolean | null = null;
  let readinessGreen: number | null = null;
  let readinessTotal: number | null = null;
  let readinessGroups: ReadinessGroupRatio[] | null = null;
  let readinessGeneratedAt: string | null = null;
  let readinessError: string | null = null;
  if (rd.status === "fulfilled" && rd.value.ok) {
    const d = rd.value.data;
    flipBlocked = d.flip_blocked;
    readinessGreen = d.summary.green;
    readinessTotal = d.summary.total;
    readinessGroups = groupReadiness(d.items);
    readinessGeneratedAt = d.generated_at;
  } else if (rd.status === "fulfilled" && !rd.value.ok) {
    readinessError = rd.value.error.slice(0, 80);
  } else if (rd.status === "rejected") {
    readinessError = (rd.reason as Error)?.message?.slice(0, 80) ?? "unknown";
  }

  let runtime: RuntimeSummary | null = null;
  let sourcePostgres: string | null = null;
  let sourceRedis: string | null = null;
  let runtimeError: string | null = null;
  if (rs.status === "fulfilled" && rs.value.ok) {
    const d = rs.value.data;
    runtime = summarizeRuntime(d.strategies);
    sourcePostgres = d.source.postgres;
    sourceRedis = d.source.redis;
  } else if (rs.status === "fulfilled" && !rs.value.ok) {
    runtimeError = rs.value.error.slice(0, 80);
  } else if (rs.status === "rejected") {
    runtimeError = (rs.reason as Error)?.message?.slice(0, 80) ?? "unknown";
  }

  let decision: DecisionSummary = summarizeDecision(null);
  let decisionError: string | null = null;
  if (dc.status === "fulfilled" && dc.value.ok) {
    decision = summarizeDecision(dc.value.data);
  } else if (dc.status === "fulfilled" && !dc.value.ok) {
    decisionError = dc.value.error.slice(0, 80);
  } else if (dc.status === "rejected") {
    decisionError = (dc.reason as Error)?.message?.slice(0, 80) ?? "unknown";
  }

  return {
    kind: "ready",
    flipBlocked,
    readinessGreen,
    readinessTotal,
    readinessGroups,
    readinessGeneratedAt,
    readinessError,
    runtime,
    sourcePostgres,
    sourceRedis,
    runtimeError,
    decision,
    decisionError,
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
              each page load (auto-derived, fail-honest).{" "}
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
        {/* ── SECTION 1: LIVE RUNTIME (auto-derived, fail-honest) ── */}
        <section data-slot="live-runtime-section" className="space-y-4">
          <div className="flex items-center gap-2">
            <h3 className="text-sm font-semibold">Live runtime</h3>
            <Badge variant="outline" className="font-mono text-[10px]">
              <ShieldCheckIcon className="mr-1 inline size-3 text-success" />
              live · on load
            </Badge>
          </div>

          <LiveReadinessRow state={state} />
          <ReadinessGroups state={state} />
          <RuntimeEvidence state={state} />
          <DecisionEvidence state={state} />

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
// Fail-honest: "Loading…" before the probe resolves and "Unavailable" (no
// progress fill) on error — never fabricates a count; denominator is
// summary.total (live), never a hardcoded 16/17.
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

// ── Per-group readiness ratios (auto-derived, client groupBy on items[]) ──
// Fail-honest: renders nothing until live items arrive — the global row above
// already shows Loading/Unavailable; we never fabricate per-group ratios.
function ReadinessGroups({ state }: { state: RuntimeProbe }) {
  if (state.kind === "loading" || state.readinessGroups === null || state.readinessGroups.length === 0) {
    return null;
  }
  return (
    <div data-slot="readiness-groups" className="grid grid-cols-2 gap-2 sm:grid-cols-3">
      {state.readinessGroups.map((g) => {
        const allGreen = g.green === g.total;
        return (
          <div key={g.group} className="rounded-md border bg-muted/20 px-2.5 py-1.5">
            <div className="text-[11px] text-muted-foreground">{g.label}</div>
            <div
              className={`font-mono text-sm font-semibold tabular-nums ${
                allGreen ? "text-success" : "text-foreground"
              }`}
            >
              {g.green}/{g.total} green
            </div>
          </div>
        );
      })}
    </div>
  );
}

// ── Runtime evidence (auto-derived from /api/strategies/runtime-status) ──
// engine_loaded is INFERRED from a Redis scanner-heartbeat key → labelled
// "scanner heartbeat", not "engine registered". Fail-honest on error.
function RuntimeEvidence({ state }: { state: RuntimeProbe }) {
  if (state.kind === "loading") {
    return (
      <p className="text-xs text-muted-foreground">Querying /api/strategies/runtime-status…</p>
    );
  }
  if (state.runtimeError !== null || state.runtime === null) {
    return (
      <p
        className="inline-flex items-center gap-1 text-xs text-destructive"
        title={state.runtimeError ?? undefined}
      >
        <AlertTriangleIcon className="size-3" />
        runtime-status unavailable
      </p>
    );
  }
  const r = state.runtime;
  return (
    <div
      data-slot="runtime-evidence"
      className="flex flex-wrap items-center gap-x-3 gap-y-1 font-mono text-xs"
    >
      <span>
        Scanner heartbeat: {r.enginesLoaded}/{r.enginesTotal} engines
      </span>
      <span className="text-muted-foreground/60">·</span>
      <span>Candidates 1h: {r.candidates1h}</span>
      <span className="text-muted-foreground/60">·</span>
      <span>Rejections 1h: {r.rejections1h}</span>
      <SourcePill label="pg" value={state.sourcePostgres} />
      <SourcePill label="redis" value={state.sourceRedis} />
    </div>
  );
}

function SourcePill({ label, value }: { label: string; value: string | null }) {
  const ok = value === "ok";
  return (
    <span
      className={`rounded border px-1.5 py-0.5 text-[10px] ${
        ok
          ? "border-emerald-500/40 text-emerald-700 dark:text-emerald-300"
          : "border-amber-500/40 text-amber-700 dark:text-amber-300"
      }`}
    >
      {label}:{value ?? "unavailable"}
    </span>
  );
}

// ── Decision evidence (auto-derived from /api/readiness/decision) ──
// Trade mode (paper_mode) + A.5 (go_a5) are dynamic; reasons/next_action are
// the live "why blocked / what next". verdict is NOT shown (hardcoded NO_GO
// server-side → not a live signal). Fail-honest: text on loading/error.
function DecisionEvidence({ state }: { state: RuntimeProbe }) {
  if (state.kind === "loading") {
    return <p className="text-xs text-muted-foreground">Querying /api/readiness/decision…</p>;
  }
  const { decision, decisionError } = state;
  const { paperMode, goA5, reasons, nextAction } = decision;
  if (decisionError !== null || paperMode === null) {
    return (
      <p
        className="inline-flex items-center gap-1 text-xs text-destructive"
        title={decisionError ?? undefined}
      >
        <AlertTriangleIcon className="size-3" />
        decision unavailable
      </p>
    );
  }
  return (
    <div data-slot="decision-evidence" className="space-y-1.5">
      <div className="flex flex-wrap items-center gap-2 text-xs">
        <DecisionBadge
          label="Trade mode"
          value={paperMode ? "PAPER" : "LIVE"}
          tone={paperMode ? "ok" : "danger"}
        />
        <DecisionBadge
          label="A.5"
          value={goA5 === null ? "—" : goA5 ? "GO" : "blocked"}
          tone={goA5 ? "ok" : "warn"}
        />
      </div>
      {(reasons.length > 0 || nextAction) && (
        <div className="text-xs text-muted-foreground">
          {reasons.length > 0 && (
            <ul className="list-inside list-disc space-y-0.5">
              {reasons.slice(0, 5).map((r) => (
                <li key={r}>{r}</li>
              ))}
            </ul>
          )}
          {nextAction && <p className="mt-1">Next: {nextAction}</p>}
        </div>
      )}
    </div>
  );
}

function DecisionBadge({
  label,
  value,
  tone,
}: {
  label: string;
  value: string;
  tone: "ok" | "warn" | "danger";
}) {
  const accent =
    tone === "ok" ? "border-emerald-500/40 text-emerald-700 dark:text-emerald-300"
    : tone === "danger" ? "border-destructive/40 text-destructive"
    : "border-amber-500/40 text-amber-700 dark:text-amber-300";
  return (
    <span className={`rounded border px-1.5 py-0.5 font-mono text-[10px] ${accent}`}>
      {label}: {value}
    </span>
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
  return (
    <div className="flex flex-wrap items-center gap-x-4 gap-y-1 border-t pt-3 text-xs text-muted-foreground">
      {state.readinessError ? (
        <span title={state.readinessError} className="inline-flex items-center gap-1 text-destructive">
          <AlertTriangleIcon className="size-3" />
          readiness unavailable
        </span>
      ) : state.flipBlocked === null ? (
        <span>readiness: no flip_blocked field</span>
      ) : (
        <span className="inline-flex items-center gap-1">
          <ShieldCheckIcon className="size-3 text-success" />
          readiness loaded · flip_blocked = {String(state.flipBlocked)}
        </span>
      )}
    </div>
  );
}
