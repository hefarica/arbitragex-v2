"use client";

/**
 * =============================================================================
 * LatencyCandidatesPanel — FE-0037 (§45, wire ARBX-FE-EMIT-09)
 * =============================================================================
 *
 * The per-candidate latency waterfall: each route the worker kept this tick,
 * with its traversed-stage timings. This is the granularity the aggregate
 * `lat_stages` (FE-0036) cannot express — FE-0037 was BLOCKED until the
 * per-candidate wire existed; a "degraded" panel over aggregates dressed as
 * per-candidate was rejected as misleading (registry decision 2026-08-24).
 *
 * PURE component (repo pattern): props carry the tick's `lat_candidates` /
 * `lat_candidates_meta`; the dialog container is the only selector site.
 *
 * THREE honest states (semantics agreed with d9, cont.72/73):
 *   1. rows === undefined  — key absent from the tick: backend pre-EMIT-09
 *      (deploy-drift window). The whole GROUP is "no emitido" — same category
 *      as any incondicional-y-partial key (lat_stages).
 *   2. rows === [] + sampled 0 — an HONEST EMPTY TICK: the finder found 0
 *      routes. A real state, not an error, not "no computado".
 *   3. `stages.reprice_us` ABSENT in one row — that route did not traverse
 *      the adapter this tick (scoped-out / F_e-dropped / malformed for
 *      triangular; by construction for v2v2/v2v3/v3v2/v3v3). Absence IS the
 *      state (R8): rendered per-key, never as 0.
 *
 * Frame honesty (the dialog is per-OPPORTUNITY, the wire is per-ROUTE):
 * the opportunities wire carries NO route_hash (nivel-(b), FE-0028 gap) —
 * there is no join key. The panel declares it renders the TICK's top-K, and
 * never claims a row belongs to this opportunity. Matching rows by
 * hops/kind heuristics would fabricate identity — not done.
 *
 * §79: every value here is wire-owned. total_us == Σ stages is guaranteed by
 * the schema's superRefine (mirror) — the FE renders the parts and the sum,
 * it never re-derives a verdict. `attribution` literals render VERBATIM.
 */

// SSR-test support (repo pattern): classic JSX path needs the React namespace.
import * as React from "react";

import type {
  LatCandidateRow,
  LatCandidatesTelemetry,
} from "@/lib/apex/schemas";

/** µs → ms display. Twin of HomeStoreAggregation.usToMs (FE-0042) — kept
 * local to avoid coupling the opportunities dialog to the home component. */
export function fmtMs(us: number): string {
  return `${(us / 1000).toFixed(2)} ms`;
}

/** 8 + … + 6 elision; the full hash rides the node's title. */
function shortHash(h: string): string {
  return h.length <= 16 ? h : `${h.slice(0, 8)}…${h.slice(-6)}`;
}

type LatencyMeta = LatCandidatesTelemetry["lat_candidates_meta"];

/** A single stage line of the waterfall: label + value + proportional bar. */
function StageBar({
  label,
  us,
  total,
  note,
  testId,
}: {
  label: string;
  us: number | null;
  /** Denominator for the proportional bar (the row's total_us). */
  total: number;
  note?: string;
  testId: string;
}) {
  // us === null encodes ONLY "key absent" (state 3) — the wire never sends
  // null reprice, it omits the key (mirror: .optional()).
  const pct = us != null && total > 0 ? Math.round((us / total) * 100) : 0;
  return (
    <div data-testid={testId} className="flex items-baseline gap-2 py-0.5 font-mono text-xs">
      <span className="w-20 shrink-0 text-muted-foreground">{label}</span>
      <span className="w-20 shrink-0 text-right">
        {us != null ? fmtMs(us) : "—"}
      </span>
      <span className="h-2 flex-1 overflow-hidden rounded-sm bg-muted/30">
        {us != null && (
          <span
            className="block h-full rounded-sm bg-primary/60"
            style={{ width: `${pct}%` }}
          />
        )}
      </span>
      {note && (
        <span className="w-full text-[10px] italic text-muted-foreground/70">{note}</span>
      )}
    </div>
  );
}

export function LatencyCandidatesPanel({
  rows,
  meta,
}: {
  rows?: LatCandidateRow[];
  meta?: LatencyMeta;
}): JSX.Element {
  // State 1 — the whole group is absent: pre-EMIT-09 backend (deploy drift).
  if (rows === undefined) {
    return (
      <div data-testid="latency-candidates-panel">
        <p className="mt-1 text-xs italic text-muted-foreground/70" data-testid="lat-candidates-absent">
          no emitido — lat_candidates ausente en este tick: backend pre
          ARBX-FE-EMIT-09 o grupo no emitido (nivel-(b), ventana de deploy).
        </p>
      </div>
    );
  }

  return (
    <div data-testid="latency-candidates-panel">
      {/* Frame honesty: per-route wire inside a per-opportunity dialog. */}
      <p className="mb-2 text-[11px] leading-snug text-muted-foreground/80">
        Waterfall §45 — top-K rutas del último tick (lat_candidates, EMIT-09).
        El wire de opportunities no lleva route_hash (nivel-(b)): NO hay join
        per-oportunidad — las filas son el top-K del tick, marco declarado,
        nunca un reclamo de pertenencia a esta oportunidad.
      </p>

      {/* Meta: attribution VERBATIM + the cut counters (a recorte is never
          silent). Counters render only if the meta block came. */}
      {meta != null && (
        <div
          data-testid="lat-candidates-meta"
          className="mb-3 rounded-lg border border-border/60 bg-muted/20 p-2 font-mono text-[11px] leading-relaxed"
        >
          <div>
            attribution: gates=<span className="text-foreground">{meta.attribution.gates}</span> ·
            reprice=<span className="text-foreground">{meta.attribution.reprice}</span>{" "}
            <span className="text-muted-foreground/70">(verbatim del productor)</span>
          </div>
          <div data-testid="lat-candidates-counters">
            sampled {meta.sampled} · kept {Math.min(meta.sampled, meta.cap)} · dropped{" "}
            {meta.dropped} · cap {meta.cap}
          </div>
          {meta.truncated && (
            <div data-testid="lat-candidates-truncated" className="text-warning">
              top-K truncado — el recorte es visible, no silencioso
            </div>
          )}
        </div>
      )}

      {/* State 2 — honest empty tick: the finder found 0 routes. */}
      {rows.length === 0 ? (
        <p className="text-xs italic text-muted-foreground/70" data-testid="lat-candidates-empty">
          0 candidatos en este tick — un tick honesto sin rutas (sampled 0),
          no un error ni un "no computado".
        </p>
      ) : (
        <div className="space-y-3" data-testid="lat-candidates-rows">
          {rows.map((row, i) => (
            <div
              key={row.route_hash}
              data-testid={`lat-candidate-${i}`}
              className="rounded-lg border border-border/50 p-2"
            >
              <div className="mb-1 flex items-baseline justify-between gap-2 font-mono text-xs">
                <span>
                  <span className="text-muted-foreground/60">#{i + 1} </span>
                  <span
                    className="text-foreground/80"
                    title={row.route_hash}
                    data-testid={`lat-candidate-${i}-hash`}
                  >
                    {shortHash(row.route_hash)}
                  </span>
                </span>
                <span className="text-muted-foreground">
                  {row.route_kind} · {row.hops} hops
                </span>
              </div>
              <StageBar
                label="gates"
                us={row.stages.gates_us}
                total={row.total_us}
                testId={`lat-candidate-${i}-gates`}
              />
              <StageBar
                label="reprice"
                us={row.stages.reprice_us ?? null}
                total={row.total_us}
                note={
                  row.stages.reprice_us == null
                    ? "no atravesó el adapter este tick (ausencia = estado, R8)"
                    : undefined
                }
                testId={`lat-candidate-${i}-reprice`}
              />
              {/* §79: total is the wire's Σ (superRefine-checked in the
                  mirror) — rendered, not re-derived; it is NOT the tick's
                  wall-clock (schema doc). */}
              <div
                className="mt-1 flex items-baseline justify-between border-t border-border/50 pt-1 font-mono text-xs"
                data-testid={`lat-candidate-${i}-total`}
                title={`${row.total_us} µs — Σ stages (superRefine-checked); NO wall-clock del tick`}
              >
                <span className="text-muted-foreground">total (Σ stages)</span>
                <span className="font-semibold">{fmtMs(row.total_us)}</span>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
