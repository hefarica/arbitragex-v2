/**
 * FE-MASTER · Pair lifecycle state flow (FE-0020 — P5, §17).
 *
 * The Event→PoolDirty→PairDirty→Prefilter→HotSeed pipeline instrumented with
 * the counters the route-discovery tick ALREADY publishes (DirtyDrain +
 * FePrefilter wire groups, EMIT-05) plus the live pair census of the panel's
 * own universe (CLEAN/DIRTY ride the EMIT-06 payload boolean).
 *
 * Partition approved with 7b (2026-08-24): the per-pair states QUEUED / HOT /
 * EXPANDING / COOLED are searcher-side and NOT published — they render as
 * dimmed gap slots labeled FE-0020-pending-EMIT-06c (the EMIT-06c follow-up
 * publishes a `pair_state` enum from the same worker block that writes the
 * alpha hash). NEVER a fabricated count (RULE 00).
 *
 * R8 honesty model:
 *   - Every stage counter is a VERBATIM wire key — no derivation beyond the
 *     census count of payload booleans (the §13 header pattern).
 *   - tick null ⇒ all stages "—" + "sin tick servido" (searcher down / fetch
 *     failed); the census stays independent (it reads the pairs payload).
 *   - A present group with 0 renders "0" (computed-and-zero); an ABSENT
 *     group is a real backend state rendered as "—", and an absent
 *     fe_prefilter group means the knob is OFF ("knob OFF", dormant ≠ 0).
 */
"use client";

// SSR-test support (repo pattern, cf. TokenIcon/ChainsAdminClient).
import * as React from "react";

import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type { PairView, RouteDiscoveryTickSummary } from "@/lib/apex/schemas";

const DASH = "—";

/** Per-pair states the searcher holds but does NOT publish (EMIT-06c). */
const PENDING_STATES = ["QUEUED", "HOT", "EXPANDING", "COOLED"] as const;

/** Present group ⇒ the number (0 is a real zero); absent group ⇒ "—". */
const fmt = (v: number | undefined): string => (v === undefined ? DASH : String(v));

interface Props {
  /** The panel's live universe — CLEAN/DIRTY census reads its `dirty` flags. */
  pairs: PairView[] | null;
  /** Latest route-discovery tick summary (EMIT-05 wire mirror). */
  tick: RouteDiscoveryTickSummary | null;
  /** Honest fetch error for the tick surface (null = none). */
  tickError: string | null;
}

export function PairStateFlowPanel({ pairs, tick, tickError }: Props) {
  // Census over payload booleans — presentation aggregation only (§79: the
  // flags come from the wire; the panel never infers dirtiness).
  const dirtyCount = pairs === null ? null : pairs.filter((p) => p.dirty).length;
  const cleanCount = pairs === null || dirtyCount === null ? null : pairs.length - dirtyCount;

  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-base">
          Ciclo de vida del par (§17)
          <span className="ml-2 text-sm font-normal text-muted-foreground">
            Event → PoolDirty → PairDirty → Prefilter → HotSeed
          </span>
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        {tickError && (
          <p className="text-sm text-destructive" role="alert">
            {tickError}
          </p>
        )}
        {!tick && !tickError && (
          <p className="text-sm text-muted-foreground">sin tick servido</p>
        )}
        <div className="flex flex-wrap items-stretch gap-2">
          {/* Event — drain intake (verbatim drain_* wire keys). */}
          <div className="min-w-28 rounded-md border border-border/60 p-2" title="drain_drained · drain_unknown_pool · drain_invalid_pair">
            <div className="text-xs font-medium">Event</div>
            <div className="text-lg tabular-nums">{fmt(tick?.drain_drained)}</div>
            <div className="text-[10px] text-muted-foreground tabular-nums">
              unknown_pool {fmt(tick?.drain_unknown_pool)} · invalid_pair {fmt(tick?.drain_invalid_pair)}
            </div>
          </div>
          <div className="self-center text-muted-foreground" aria-hidden="true">→</div>
          {/* PoolDirty — the pool was already in the dirty set (dedupe). */}
          <div className="min-w-28 rounded-md border border-border/60 p-2" title="drain_already_dirty · drain_register_reject">
            <div className="text-xs font-medium">PoolDirty</div>
            <div className="text-lg tabular-nums">{fmt(tick?.drain_already_dirty)}</div>
            <div className="text-[10px] text-muted-foreground tabular-nums">
              register_reject {fmt(tick?.drain_register_reject)}
            </div>
          </div>
          <div className="self-center text-muted-foreground" aria-hidden="true">→</div>
          {/* PairDirty — LIVE census of the EMIT-06 payload's dirty flags. */}
          <div className="min-w-28 rounded-md border border-border/60 p-2" title="pares con dirty=true en el universo servido">
            <div className="text-xs font-medium">PairDirty</div>
            <div className="text-lg tabular-nums">
              {dirtyCount === null ? DASH : String(dirtyCount)}
            </div>
            <div className="text-[10px] text-muted-foreground">
              universo servido {pairs === null ? DASH : String(pairs.length)} pares
            </div>
          </div>
          <div className="self-center text-muted-foreground" aria-hidden="true">→</div>
          {/* Prefilter — F_e signal (knob ON only; OFF emits NOTHING). */}
          <div className="min-w-28 rounded-md border border-border/60 p-2" title="fe_prefilter_evaluated · pass · below_reference · uncomputed">
            <div className="text-xs font-medium">Prefilter</div>
            <div className="text-lg tabular-nums">
              {/* tick null = no snapshot at all ("—"); a served tick WITHOUT
                  the group = the knob is OFF (dormant ≠ absent ≠ zero). */}
              {tick === null
                ? DASH
                : tick.fe_prefilter_evaluated === undefined
                  ? "knob OFF"
                  : String(tick.fe_prefilter_evaluated)}
            </div>
            <div className="text-[10px] text-muted-foreground tabular-nums">
              pass {fmt(tick?.fe_prefilter_pass)} · below {fmt(tick?.fe_prefilter_below_reference)} · uncomputed {fmt(tick?.fe_prefilter_uncomputed)}
            </div>
          </div>
          <div className="self-center text-muted-foreground" aria-hidden="true">→</div>
          {/* HotSeed — current seeds + ring bookkeeping. */}
          <div className="min-w-28 rounded-md border border-border/60 p-2" title="dirty_seeds · drain_seeded · drain_evicted">
            <div className="text-xs font-medium">HotSeed</div>
            <div className="text-lg tabular-nums">{fmt(tick?.dirty_seeds)}</div>
            <div className="text-[10px] text-muted-foreground tabular-nums">
              seeded {fmt(tick?.drain_seeded)} · evicted {fmt(tick?.drain_evicted)}
            </div>
          </div>
        </div>

        {/* State census: published states carry REAL counts; the unpublished
            enum renders as dimmed gap slots — never a fabricated number. */}
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-xs text-muted-foreground">Estados:</span>
          <Badge variant="secondary">CLEAN {cleanCount === null ? DASH : cleanCount}</Badge>
          <Badge variant="destructive">DIRTY {dirtyCount === null ? DASH : dirtyCount}</Badge>
          {PENDING_STATES.map((s) => (
            <Badge
              key={s}
              variant="outline"
              className="opacity-50"
              title="Estado searcher-side no publicado — pendiente EMIT-06c (pair_state enum)"
            >
              {s} {DASH}
            </Badge>
          ))}
          <span className="text-[10px] text-muted-foreground">
            QUEUED/HOT/EXPANDING/COOLED: FE-0020-pending-EMIT-06c
          </span>
        </div>
      </CardContent>
    </Card>
  );
}
