"use client";

/**
 * =============================================================================
 * HomeStoreAggregation — FE-0042 (FE-MASTER P10-DRIFT-HOME §58 §59)
 * =============================================================================
 *
 * The Home aggregation: eight axes, EVERY value read from the Omni-Store via
 * selectors — the container fetches NOTHING and recomputes NO business logic
 * (§58: aggregation = counting/grouping store state that other surfaces
 * already own; §59: a store that has not been fed renders its honest empty,
 * never an invented value).
 *
 * Structure (repo pattern, same as RegistryCoherenceStrip/FE-0040):
 *   HomeStoreAggregation          — PURE view, real-typed props (testable via
 *                                   renderToStaticMarkup with zero seams)
 *   HomeStoreAggregationContainer — the ONLY place selectors are read; page.tsx
 *                                   mounts this one
 *
 * Hydration is FREE: ArbxRealtimeProvider (root layout, FE-0008) already
 * feeds tick/pairs/quoteAnchor/channels on its cadence, and the opportunities
 * stream feeds the live slice. The island only reads. Production SSR note:
 * zustand v5's server snapshot reads getInitialState(), so the server HTML is
 * the honest blank skeleton and the client fills it after hydration (R1).
 *
 * Axis → source (all OBSERVED shapes, no invented fields):
 *   posture      RealtimeSlice.channels — status tally per channel
 *   funnel       TelemetrySlice.tick — drain_seeded → fe_prefilter_* →
 *                routes_dispatched (each key knob-conditional: absent key =
 *                real backend state, renders "—")
 *   hot pairs    PairsSlice.pairs — dirty tally (null = sin snapshot, R8)
 *   strategies   tick.strategy_status_counts — backend-named slugs (§21: the
 *                FE never hardcodes the status vocabulary)
 *   EV           DECLARED GAP — no EV slice exists (nivel-(b))
 *   p95          tick.lat_pass_p95 + lat_stages[lat.total].p95_us (§44: null
 *                pass = sin ciclos completos, never a fabricated PASS)
 *   risk         DECLARED GAP — no risk slice in the store (nivel-(b))
 *   ejecuciones  OpportunitySlice.opportunities — status tally (backend-named)
 */

// SSR-test support (repo pattern): classic JSX path needs the React namespace.
import * as React from "react";

import {
  useRealtimeChannels,
  useRouteTick,
  usePairs,
  useOpportunities,
} from "@/lib/store/omni-store";
import type { RouteDiscoveryTickSummary, PairView } from "@/lib/apex/schemas";
import type { OmniOpportunity } from "@/lib/store/types";
import type {
  RealtimeChannelId,
  RealtimeChannelState,
} from "@/lib/store/realtime-slices";

// ---------------------------------------------------------------------------
// Pure derivations (exported for tests — counting only, §58)
// ---------------------------------------------------------------------------

/** Tally channel statuses; keyed by the store's own status vocabulary. */
export function tallyStatuses(
  statuses: readonly string[],
): Array<[string, number]> {
  const counts = new Map<string, number>();
  for (const s of statuses) counts.set(s, (counts.get(s) ?? 0) + 1);
  return [...counts.entries()].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]));
}

/** µs → ms display; null stays null (R8 — never a fabricated 0). */
export function usToMs(us: number | null | undefined): string {
  if (us == null) return "—";
  return `${(us / 1000).toFixed(2)} ms`;
}

/** The p95 of the whole cycle, from the tick's lat.total row. */
export function totalP95Ms(tick: RouteDiscoveryTickSummary | null): string {
  const row = tick?.lat_stages?.find((r) => r.key === "lat.total");
  return usToMs(row?.p95_us ?? null);
}

// ---------------------------------------------------------------------------
// Pure view
// ---------------------------------------------------------------------------

export interface HomeAggregationProps {
  channels: Record<RealtimeChannelId, RealtimeChannelState>;
  tick: RouteDiscoveryTickSummary | null;
  pairs: PairView[] | null;
  opps: OmniOpportunity[];
}

function AxisRow({
  axis,
  value,
  note,
  testId,
}: {
  axis: string;
  value: React.ReactNode;
  note?: string;
  testId: string;
}) {
  return (
    <div
      data-testid={testId}
      className="flex flex-wrap items-baseline justify-between gap-x-3 gap-y-0.5 border-b border-border/50 py-1.5 last:border-0"
    >
      <span className="font-mono text-[10px] font-semibold uppercase tracking-widest text-muted-foreground">
        {axis}
      </span>
      <span className="font-mono text-xs">
        {value}
        {note && <span className="ml-2 text-[10px] text-muted-foreground/80">{note}</span>}
      </span>
    </div>
  );
}

function FunnelValue({ tick }: { tick: RouteDiscoveryTickSummary | null }) {
  // Each key is knob-conditional on the wire: absence renders "—", never 0.
  const seeded = tick?.drain_seeded;
  const evaluated = tick?.fe_prefilter_evaluated;
  const pass = tick?.fe_prefilter_pass;
  const dispatched = tick?.routes_dispatched;
  const stage = (v: number | undefined) => (v == null ? "—" : String(v));
  return (
    <span className="font-mono text-xs">
      seeds {stage(seeded)} → eval {stage(evaluated)} → pass {stage(pass)} → dispatch{" "}
      {stage(dispatched)}
    </span>
  );
}

export function HomeStoreAggregation({
  channels,
  tick,
  pairs,
  opps,
}: HomeAggregationProps): JSX.Element {
  // posture: tally the store's own channel statuses.
  const channelTally = tallyStatuses(
    Object.values(channels).map((c) => c.status),
  );
  const postureValue =
    channelTally.length === 0 ? (
      "sin canales"
    ) : (
      <span>
        {channelTally.map(([s, n], i) => (
          <React.Fragment key={s}>
            {i > 0 && <span className="text-muted-foreground/60"> · </span>}
            <span className={s === "live" ? "text-success" : "text-warning"}>{s} ×{n}</span>
          </React.Fragment>
        ))}
      </span>
    );

  // strategies: the tick's per-status census — backend-named slugs (§21).
  const census = tick?.strategy_status_counts;
  const strategiesValue =
    census == null ? (
      "sin censo en tick"
    ) : (
      <span>
        {Object.entries(census)
          .sort((a, b) => b[1] - a[1])
          .map(([slug, n], i) => (
            <React.Fragment key={slug}>
              {i > 0 && <span className="text-muted-foreground/60"> · </span>}
              {slug} {n}
            </React.Fragment>
          ))}
      </span>
    );

  // hot pairs: dirty tally over the effective-universe snapshot.
  const hotPairsValue =
    pairs == null ? (
      "sin snapshot (R8)"
    ) : (
      <>
        {pairs.filter((p) => p.dirty).length} / {pairs.length}{" "}
        <span className="text-muted-foreground/80">dirty</span>
      </>
    );

  // ejecuciones: status tally over the live opportunities the store holds.
  // status is wire-mandatory but nullable (§28): null renders as its own
  // honest "unknown" bucket, never dropped and never defaulted.
  const execTally = tallyStatuses(opps.map((o) => o.status ?? "unknown"));
  const executionsValue =
    opps.length === 0 ? (
      "feed vacío"
    ) : (
      <span>
        {execTally.map(([s, n], i) => (
          <React.Fragment key={s}>
            {i > 0 && <span className="text-muted-foreground/60"> · </span>}
            {s} {n}
          </React.Fragment>
        ))}
      </span>
    );

  // p95: §44 — null pass means no completed cycles; never a fabricated PASS.
  const passP95 = tick?.lat_pass_p95 ?? null;

  return (
    <div
      data-testid="home-store-aggregation"
      className="rounded-2xl border border-border bg-card/60 p-4"
    >
      <div className="mb-2 flex items-baseline justify-between">
        <h3 className="text-sm font-semibold">Agregación del sistema — desde stores (§58)</h3>
        <span className="font-mono text-[10px] uppercase tracking-widest text-muted-foreground">
          sin lógica propia · sólo lectura
        </span>
      </div>

      <AxisRow axis="posture" value={postureValue} testId="agg-posture" />
      <AxisRow axis="funnel" value={<FunnelValue tick={tick} />} testId="agg-funnel" />
      <AxisRow
        axis="hot pairs"
        value={hotPairsValue}
        testId="agg-hot-pairs"
      />
      <AxisRow
        axis="strategies"
        value={strategiesValue}
        note="censo per-status del tick"
        testId="agg-strategies"
      />
      <AxisRow
        axis="EV"
        value="no computado"
        note="sin slice de EV en el store (nivel-(b))"
        testId="agg-ev"
      />
      <AxisRow
        axis="p95 ciclo"
        value={totalP95Ms(tick)}
        note={
          passP95 == null
            ? "sin ciclos completos (§44)"
            : passP95
              ? "PASS p95 vs SLA"
              : "over budget"
        }
        testId="agg-p95"
      />
      <AxisRow
        axis="risk"
        value="no computado"
        note="sin slice de risk en el store (nivel-(b))"
        testId="agg-risk"
      />
      <AxisRow axis="ejecuciones" value={executionsValue} testId="agg-executions" />
    </div>
  );
}

// ---------------------------------------------------------------------------
// Container — the ONLY place selectors are read (§79: wiring stays trivial)
// ---------------------------------------------------------------------------

export function HomeStoreAggregationContainer(): JSX.Element {
  return (
    <HomeStoreAggregation
      channels={useRealtimeChannels()}
      tick={useRouteTick()}
      pairs={usePairs()}
      opps={useOpportunities()}
    />
  );
}
