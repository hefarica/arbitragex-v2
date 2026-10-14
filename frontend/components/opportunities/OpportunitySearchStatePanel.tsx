"use client";
/**
 * OpportunitySearchStatePanel — the §11 search-state block for the approved card.
 *
 * WHY IT IS A SEPARATE COMPONENT
 * ------------------------------
 * §11 opens with a hard constraint: the approved card is NOT to be redesigned
 * and its streaming is not to be replaced by page refreshes. This panel adds
 * information INSIDE the existing card using the existing visual vocabulary
 * (`rounded-lg border border-border bg-muted/20`, `text-[9px] uppercase
 * tracking-wide`, `font-mono text-[11px]`, the same `text-success` /
 * `text-destructive` tokens) — no new visual system, no re-layout of what the
 * operator already approved.
 *
 * It is a child component with its OWN store subscription on purpose. The card
 * is `React.memo`'d on `opp`; a per-strategy census (routes explored, sizes
 * quoted, replays rejected) is NOT a property of one row, so threading it
 * through the card's props would either re-render every card on every census
 * or silently freeze the counters. Subscribing here updates exactly this
 * subtree, and the card's memo comparator and its CARDS-PAINT-GATE
 * classification stay untouched.
 *
 * RENDER RULES (enforced by tests, not by hope)
 * --------------------------------------------
 *  · a COMPUTED cell prints the number AND the scope it was measured in
 *    (`cli` = counted by this client over its observed inventory, `srv` = the
 *    api-server's own session census). A client census is never dressed as a
 *    producer counter.
 *  · an ABSENT cell prints `n/c` and carries the machine reason in `title`
 *    (R8/R10). It NEVER prints `0` — a zero over a metric nobody measured is
 *    the false zero E2E-COMPUTE GUARD forbids.
 *  · the COLOUR of a number follows its SIGN only. `net` is tinted by its own
 *    sign; the GAP (a distance to the objective) is deliberately never tinted
 *    success — painting it green would promise a state the wire has not
 *    declared. The lifecycle chips below are the only place a stage is
 *    asserted, and each one names the field that asserted it.
 */
import React, { useMemo } from "react";
import { Activity, AlertTriangle, Gauge, Target } from "lucide-react";

import {
  useOmniStore,
  useSnapshotReceivedAt,
  useStrategyStreamEntry,
} from "@/lib/store/omni-store";
import { strategyKeyOf } from "@/lib/store/stream-contract";
import {
  deriveStrategySearchState,
  progressNumber,
  type PlanFacts,
  type ProgressCell,
  type SearchPhase,
  type StrategyProgress,
  type StrategySearchState,
} from "@/lib/store/search-state";
import type { OmniOpportunity } from "@/lib/store/types";

/** Compact USD, sign BEFORE the `$` (one value never renders two ways). */
function usd(v: number | null): string {
  if (v == null || !Number.isFinite(v)) return "—";
  const sign = v < 0 ? "-" : "";
  const a = Math.abs(v);
  if (a === 0) return `${sign}$0.0000`;
  if (a < 0.01) return `${sign}$${a.toPrecision(3)}`;
  if (a >= 1e9) return `${sign}$${a.toExponential(2).replace("e+", "e")}`;
  if (a >= 1e6) return `${sign}$${(a / 1e6).toFixed(2)}M`;
  if (a >= 1e3) return `${sign}$${(a / 1e3).toFixed(1)}k`;
  return `${sign}$${a.toFixed(a < 1 ? 4 : 2)}`;
}

const PHASE_LABEL: Record<SearchPhase, string> = {
  no_observations: "SIN OBSERVACIONES",
  searching: "BUSCANDO",
  target_met: "OBJETIVO ALCANZADO",
  search_stalled: "SIN OBSERVACIONES FRESCAS",
};

const SCOPE_MARK: Record<string, string> = {
  client_observed_inventory: "cli",
  server_session: "srv",
};

/** One signed number: colour from the SIGN, never from an expected state. */
function SignedValue({ value }: { value: number | null }) {
  if (value == null) {
    return <span className="text-muted-foreground/60">—</span>;
  }
  const tone =
    value > 0 ? "text-success" : value < 0 ? "text-destructive" : "text-muted-foreground";
  return <span className={tone}>{usd(value)}</span>;
}

/**
 * A value that is NOT a result, so its colour may not imply one:
 *   · the OBJECTIVE is a configured floor, not an achieved outcome — painting
 *     it green reads as "reached", which is the state the lifecycle chip owns;
 *   · the GAP is a DISTANCE to that floor — its magnitude is positive by
 *     construction, and §11 forbids a colour that promises a later state.
 * Both render muted; the sign tint belongs to the RESULT cells only (which is
 * where the card's own headline net already applies it).
 */
function NeutralValue({ value }: { value: number | null }) {
  if (value == null) {
    return <span className="text-muted-foreground/60">—</span>;
  }
  return <span className="text-muted-foreground">{usd(value)}</span>;
}

/**
 * One progress cell. COMPUTED → number + scope mark; ABSENT → `n/c` with the
 * reason in the tooltip. The ABSENT branch is the one that keeps this panel
 * honest: it is the ONLY thing rendered when a producer does not exist.
 */
function ProgressChip({
  label,
  cell,
  testId,
}: {
  label: string;
  cell: ProgressCell;
  testId: string;
}) {
  if (cell.state === "ABSENT") {
    return (
      <span
        data-testid={testId}
        data-state="absent"
        title={`${label}: AUSENTE — ${cell.reason} (no computado; nunca se pinta un 0 falso)`}
        className="text-muted-foreground/60"
      >
        {label} <span className="font-bold">n/c</span>
      </span>
    );
  }
  return (
    <span
      data-testid={testId}
      data-state="computed"
      title={`${label}: ${cell.value} · medido por ${SCOPE_MARK[cell.scope] ?? cell.scope}${
        cell.note ? ` (${cell.note})` : ""
      }`}
      className="text-foreground/90"
    >
      {label} <span className="font-bold tabular-nums">{cell.value}</span>
      <span className="text-[8px] text-muted-foreground/60 ml-0.5">
        {SCOPE_MARK[cell.scope] ?? cell.scope}
      </span>
    </span>
  );
}

function PlanLine({ plan }: { plan: PlanFacts }) {
  const net = progressNumber(plan.net);
  const gap = progressNumber(plan.gap);
  const route = `${plan.opportunity_id.slice(0, 10)} · ${plan.hops || "?"} hops`;
  return (
    <div className="flex flex-wrap items-center gap-x-2 gap-y-0.5" data-testid="search-state-plan">
      <span className="text-muted-foreground/80">{route}</span>
      <span className="text-muted-foreground/60">·</span>
      <span title={plan.basis_reason ?? undefined}>
        net <SignedValue value={net} />
        <span className="text-[8px] text-muted-foreground/60 ml-0.5">{plan.basis}</span>
      </span>
      {gap != null && (
        <>
          <span className="text-muted-foreground/60">·</span>
          {/* A DISTANCE, not a sign: never tinted success (§11 colour rule). */}
          <span
            data-testid="search-state-gap"
            title="brecha = max(0, objetivo − net): distancia al objetivo, no un estado alcanzado"
            className="text-muted-foreground"
          >
            brecha <span className="font-bold tabular-nums">{usd(gap)}</span>
          </span>
        </>
      )}
      {plan.meets_target === true && (
        <span className="px-1 rounded bg-success/15 text-success border border-success/40 font-bold">
          OBJETIVO
        </span>
      )}
    </div>
  );
}

export interface OpportunitySearchStatePanelProps {
  opp: OmniOpportunity;
  /** SSR/CSR gate: time-dependent text renders client-side only (R1). */
  isMounted: boolean;
  /** Parent clock (ms epoch). */
  now: number;
}

/**
 * PURE VIEW — every render rule lives here and nothing else. It takes the
 * already-derived state, so the §14 UI assertions (a stale metric prints `n/c`
 * and never `0`; the gap is not tinted success; the negative result stays
 * visible) are testable with `renderToStaticMarkup` in the node environment,
 * without a DOM, without a store, and without mocking a subscriber.
 */
export function SearchStateView({
  state,
  strategyKind,
}: {
  state: StrategySearchState;
  strategyKind: string | null;
}) {
  const p: StrategyProgress = state.progress;
  const current = state.current;
  const bestNegative = state.best.best_negative;

  const phaseTone =
    state.phase === "target_met"
      ? "bg-success/10 text-success border-success/40"
      : state.phase === "searching"
        ? "bg-info/10 text-info border-info/40"
        : state.phase === "search_stalled"
          ? "bg-warning/10 text-warning border-warning/40"
          : "bg-muted/60 text-muted-foreground border-border";

  return (
    <div
      data-testid="search-state-panel"
      className="rounded-lg border border-border bg-muted/20 mb-3 overflow-hidden font-mono text-[10px]"
    >
      <div className="px-2.5 py-1.5 border-b border-border/60 flex items-center justify-between gap-2">
        <span className="text-[9px] uppercase tracking-wide text-muted-foreground font-semibold flex items-center gap-1">
          <Activity size={10} /> Estado de búsqueda
          <span className="normal-case tracking-normal text-muted-foreground/60">
            · {strategyKind ?? "—"}
          </span>
        </span>
        <span
          data-testid="search-state-phase"
          title={state.phase_reason}
          className={`px-1.5 py-0.5 rounded border font-bold uppercase tracking-wide ${phaseTone}`}
        >
          {PHASE_LABEL[state.phase]}
        </span>
      </div>

      <div className="p-2 space-y-1">
        {/* ── Current route + best plan in force ─────────────────────────── */}
        {current != null ? (
          <PlanLine plan={current} />
        ) : (
          <div data-testid="search-state-no-plan" className="text-muted-foreground/70">
            Ruta actual: n/c — sin observaciones de esta estrategia todavía
          </div>
        )}
        {state.best.best_positive != null &&
          state.best.best_positive.plan_key !== current?.plan_key && (
            <div className="flex items-center gap-1 text-muted-foreground/80">
              <span>mejor vigente:</span>
              <PlanLine plan={state.best.best_positive} />
            </div>
          )}

        {/* ── §11.5: the best NEGATIVE stays visible while alternatives are
               searched, and stays visible when there is no positive at all.
               The diagnostics below are not conditional on profitability. ── */}
        {bestNegative != null && (
          <div className="flex items-center gap-1" data-testid="search-state-best-negative">
            <span className="text-muted-foreground/80">mejor negativo conservado:</span>
            <PlanLine plan={bestNegative} />
          </div>
        )}
        {state.best.best_zero != null && (
          <div className="text-muted-foreground/80" data-testid="search-state-best-zero">
            mejor cero real: {usd(0)} (computado y exactamente cero — no es una ausencia)
          </div>
        )}

        {/* ── Money group: size · financing · gross · Σcostes · net · target ── */}
        <div className="grid grid-cols-2 gap-x-3 gap-y-0.5" data-testid="search-state-economics">
          <span className="text-muted-foreground/80 truncate" title={current?.declared_size_wei ?? undefined}>
            tamaño{" "}
            {current?.declared_notional_usd != null ? (
              <span className="text-foreground/90 tabular-nums">
                {usd(current.declared_notional_usd)}
              </span>
            ) : current?.declared_size_wei != null ? (
              <span className="text-foreground/90">{current.declared_size_wei} wei</span>
            ) : (
              <span className="text-muted-foreground/60" title="sin notional declarado por el productor">
                n/c
              </span>
            )}
          </span>
          <span className="text-muted-foreground/80" title="fee de financiación (TLS/flash) del plan vigente">
            financiación{" "}
            {current != null && current.financing_fee.state === "COMPUTED" ? (
              <span className="text-foreground/90 tabular-nums">
                {usd(current.financing_fee.value)}
              </span>
            ) : (
              <span
                className="text-muted-foreground/60"
                title={`AUSENTE — ${
                  current?.financing_fee.state === "ABSENT"
                    ? current.financing_fee.reason
                    : "no_observations_observed"
                }`}
              >
                n/c
              </span>
            )}
          </span>
          <span className="text-muted-foreground/80">
            gross{" "}
            <SignedValue value={current ? progressNumber(current.gross) : null} />
          </span>
          {/* An OUTFLOW: painted with the card's own ladder convention (cost rows
              are `down` → destructive), not tinted success for being positive. */}
          <span className="text-muted-foreground/80">
            Σcostes{" "}
            {current && progressNumber(current.total_cost) != null ? (
              <span className="text-destructive">
                {usd(progressNumber(current.total_cost))}
              </span>
            ) : (
              <span className="text-muted-foreground/60">—</span>
            )}
          </span>
          <span className="text-muted-foreground/80">
            neto <SignedValue value={current ? progressNumber(current.net) : null} />
          </span>
          <span className="text-muted-foreground/80 flex items-center gap-1">
            <Target size={9} />
            objetivo <NeutralValue value={current ? progressNumber(current.target) : null} />
          </span>
          <span className="text-muted-foreground/80" data-testid="search-state-gap-cell">
            brecha <NeutralValue value={current ? progressNumber(current.gap) : null} />
          </span>
          <span
            className="text-muted-foreground/80"
            title={current?.computation_status === "error" ? current.computation_error_reason ?? undefined : undefined}
          >
            quote{" "}
            {current?.computation_status != null ? (
              <span className="text-foreground/90">
                {current.computation_status === "computed"
                  ? "exacta"
                  : current.computation_status === "partial"
                    ? "incompleta"
                    : "sin quote"}
              </span>
            ) : (
              <span className="text-muted-foreground/60" title="economics_object_not_emitted">
                n/c
              </span>
            )}
          </span>
        </div>

        {/* ── Itemised costs (each row states its own absence reason) ─────── */}
        {current != null && current.costs.length > 0 && (
          <div className="flex flex-wrap gap-x-2 gap-y-0.5" data-testid="search-state-costs">
            {current.costs.map((c) => (
              <span
                key={c.label}
                className="text-muted-foreground/70"
                title={c.reason ?? undefined}
              >
                {c.label}{" "}
                {c.value != null ? (
                  <span className="text-foreground/80 tabular-nums">{usd(c.value)}</span>
                ) : (
                  <span className="text-muted-foreground/50">n/c</span>
                )}
              </span>
            ))}
          </div>
        )}

        {/* ── §11.3 real progress — every cell COMPUTED (with scope) or ABSENT ── */}
        <div
          className="flex flex-wrap gap-x-2.5 gap-y-0.5 border-t border-border/50 pt-1"
          data-testid="search-state-progress"
        >
          <ProgressChip label="rutas" cell={p.routes_explored} testId="progress-routes" />
          <ProgressChip label="tamaños" cell={p.sizes_quoted} testId="progress-sizes" />
          <ProgressChip label="exactas" cell={p.quotes_exact} testId="progress-exact" />
          <ProgressChip label="incompletas" cell={p.quotes_incomplete} testId="progress-incomplete" />
          <ProgressChip label="sin quote" cell={p.quotes_error} testId="progress-error" />
          <ProgressChip label="presupuesto" cell={p.budget} testId="progress-budget" />
          <ProgressChip label="última mejora" cell={p.last_improvement} testId="progress-improvement" />
          <ProgressChip label="snapshot" cell={p.snapshot_age_ms} testId="progress-snapshot" />
        </div>

        {/* ── §11.4 sequencing diagnostics — idempotency is OBSERVABLE ────── */}
        <div
          className="flex flex-wrap gap-x-2.5 gap-y-0.5 text-muted-foreground/70"
          data-testid="search-state-stream"
        >
          <span title="eventos aplicados (aceptados) para esta estrategia">
            aplicados <span className="font-bold tabular-nums">{state.counters.accepted}</span>
          </span>
          <span title="replays idempotentes descartados (mismo event_id / mismo digest)">
            replays <span className="font-bold tabular-nums">{state.counters.duplicates}</span>
          </span>
          <span
            title="eventos RECHAZADOS por ser más antiguos que la observación vigente (nunca sobrescriben)"
            className={state.counters.stale_rejected > 0 ? "text-warning" : undefined}
          >
            antiguos rechazados{" "}
            <span className="font-bold tabular-nums">{state.counters.stale_rejected}</span>
          </span>
          {state.counters.incomparable > 0 && (
            <span title="aceptados sin autoridad de orden: la fila no trae fecha (R8: no se descarta lo que no se puede datar)">
              sin fecha <span className="font-bold tabular-nums">{state.counters.incomparable}</span>
            </span>
          )}
        </div>

        {/* ── §11.5 diagnostics: visible even with no profitable candidate ── */}
        {state.diagnostics.length > 0 && (
          <div
            className="flex flex-wrap gap-x-2 gap-y-0.5 text-muted-foreground/70"
            data-testid="search-state-diagnostics"
          >
            <AlertTriangle size={9} className="text-warning shrink-0 mt-0.5" />
            {state.diagnostics.slice(0, 3).map((d) => (
              <span key={d.plan_key} title={d.plan_key}>
                {d.reason}
              </span>
            ))}
            {state.diagnostics.length > 3 && (
              <span title="diagnósticos adicionales de esta estrategia">
                +{state.diagnostics.length - 3} más
              </span>
            )}
          </div>
        )}

        {/* ── Lifecycle: each stage REACHED only when the wire says so, and
               each chip names the field that said it. A positive number is not
               a target, and an approved simulation is not a submission. ── */}
        <div className="flex flex-wrap gap-1 pt-0.5" data-testid="search-state-lifecycle">
          <Gauge size={9} className="text-muted-foreground/60 shrink-0 mt-0.5" />
          {state.lifecycle.map((s) => (
            <span
              key={s.key}
              title={`${s.label} ← ${s.source}`}
              data-reached={s.reached ? "true" : "false"}
              data-testid={`lifecycle-${s.key}`}
              className={`px-1 rounded border ${
                s.reached
                  ? "bg-success/10 text-success border-success/30"
                  : "bg-muted/50 text-muted-foreground/50 border-border/60"
              }`}
            >
              {s.label}
            </span>
          ))}
        </div>

        <div className="text-[8px] text-muted-foreground/50 leading-tight">
          n/c = no computado (motivo en el tooltip) · cli = censo de este cliente ·
          srv = censo de la sesión del api-server
        </div>
      </div>
    </div>
  );
}

/**
 * CONNECTED WRAPPER — the only part that touches the store. It resolves the
 * strategy's observed inventory, folds it with the sequencing ledger, and hands
 * the result to the pure view above.
 */
export function OpportunitySearchStatePanel({
  opp,
  isMounted,
  now,
}: OpportunitySearchStatePanelProps) {
  const strategyKey = strategyKeyOf(opp);
  // Raw array reference from the store (stable between writes) — the filter
  // runs in render, so the selector itself stays referentially stable.
  const allRows = useOmniStore((s) => s.opportunities);
  const snapshotReceivedAt = useSnapshotReceivedAt();
  const entry = useStrategyStreamEntry(strategyKey);

  const state = useMemo(() => {
    const rows = allRows.filter((r) => strategyKeyOf(r) === strategyKey);
    return deriveStrategySearchState(strategyKey, rows, {
      publisher: entry.publisher,
      counters: entry.counters,
      lastImprovementMs: entry.improvement.lastImprovementMs,
      snapshotReceivedAtMs: snapshotReceivedAt,
      nowMs: isMounted ? now : null,
    });
  }, [allRows, strategyKey, entry, snapshotReceivedAt, isMounted, now]);

  return <SearchStateView state={state} strategyKind={opp.strategy_kind} />;
}
