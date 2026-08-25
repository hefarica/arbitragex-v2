"use client";

/**
 * FE-MASTER · Route Discovery Performance panel (FE-0036 — §43/§44).
 *
 * The latency half of the route-discovery tick: the nine 10_LATENCY stage
 * rows (decode → total) with target_ms from the workbook budgets and the
 * windowed p50/p90/p95/p99 the worker computes — values straight from the
 * tick (props in; the caller owns the store read, same split as the §18
 * header). The wire carries percentiles in µs; the table displays ms
 * (`usToMs` — an exact ÷1000 with µs precision kept, formatting not math).
 *
 * §44 — the PASS rule: `lat_pass_p95` is the ONLY verdict authority and it
 * is null until enough completed cycles exist. null renders "SIN PASS —
 * muestra insuficiente", never PASS, never FAIL. A percentile that has no
 * samples renders the honest dash. Nothing here recomputes a verdict from
 * the rows (§79): headroom_p95 is read from the wire, signed — negative =
 * over budget, and the minus sign lives in the text (color is never the
 * only signal).
 *
 * RULE 00: the latency group is path-conditional on the tick (every
 * aggregate key is optional) — a tick without `lat_stages` renders the
 * honest absence note, never an empty table dressed as zero.
 */

// SSR-test support (repo pattern): classic JSX path needs the React namespace.
import * as React from "react";

import { Badge } from "@/components/ui/badge";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  LatencyStageKeySchema,
  type RouteDiscoveryTickSummary,
} from "@/lib/apex/schemas";

const DASH = "—";

/** Canonical 10_LATENCY order, read from the schema enum itself (no drift). */
export const STAGE_ORDER: readonly string[] = LatencyStageKeySchema.options;

/** Exact µs→ms display string (÷1000, µs precision preserved). */
export function usToMs(us: number): string {
  return (us / 1000).toFixed(3);
}

/** Signed headroom text — explicit "+"/"-" so the sign never rides on color. */
export function formatHeadroom(us: number): string {
  return us > 0 ? `+${us}` : String(us);
}

interface Props {
  tick: RouteDiscoveryTickSummary | null;
}

export function PerformancePanel({ tick }: Props) {
  const rows = tick?.lat_stages;
  const verdict = tick?.lat_pass_p95;
  const cycles = tick?.lat_cycles;

  return (
    <section
      aria-label="Route Discovery Performance"
      data-testid="performance-panel"
      className="space-y-3"
    >
      {/* ── §44 verdict strip — lat_pass_p95 is the only PASS authority ── */}
      <div className="flex flex-wrap items-center gap-2">
        {verdict === undefined && (
          <Badge variant="outline" className="text-muted-foreground">
            latencia: grupo ausente en este tick
          </Badge>
        )}
        {verdict === null && (
          <Badge variant="outline" className="border-border/60 text-muted-foreground">
            SIN PASS — muestra insuficiente (§44)
          </Badge>
        )}
        {verdict === true && <Badge variant="secondary">PASS p95 vs SLA</Badge>}
        {verdict === false && (
          <Badge variant="destructive">FAIL p95 vs SLA</Badge>
        )}
        {cycles !== undefined && (
          <span className="text-xs text-muted-foreground tabular-nums">
            ciclos completados: {cycles}
          </span>
        )}
        <span className="text-xs text-muted-foreground">
          SLA global p95 &lt; 30 ms · targets por stage = 10_LATENCY (workbook)
        </span>
      </div>

      {/* ── stage table — wire rows in canonical order ─────────────────── */}
      {rows === undefined || rows.length === 0 ? (
        <p className="text-sm text-muted-foreground">
          {tick === null
            ? DASH
            : "Este tick no trae filas de latencia (grupo lat.* ausente — ausencia real del backend, R8)."}
        </p>
      ) : (
        <div className="overflow-x-auto">
          <Table>
            <TableHeader>
              <TableRow className="text-left text-muted-foreground">
                <TableHead className="font-medium">Stage</TableHead>
                <TableHead className="text-right font-medium">Target (ms)</TableHead>
                <TableHead className="text-right font-medium">p50 (ms)</TableHead>
                <TableHead className="text-right font-medium">p90 (ms)</TableHead>
                <TableHead className="text-right font-medium">p95 (ms)</TableHead>
                <TableHead className="text-right font-medium">p99 (ms)</TableHead>
                <TableHead className="text-right font-medium">Headroom p95 (µs)</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {[...rows]
                .sort(
                  (a, b) =>
                    STAGE_ORDER.indexOf(a.key) - STAGE_ORDER.indexOf(b.key),
                )
                .map((row) => {
                  const isTotal = row.key === "lat.total";
                  const over = row.headroom_p95_us !== null && row.headroom_p95_us < 0;
                  return (
                    <TableRow
                      key={row.key}
                      className={isTotal ? "border-t font-semibold" : undefined}
                    >
                      <TableCell className="py-1.5 pr-3 font-mono text-xs">
                        {row.key}
                      </TableCell>
                      <TableCell className="py-1.5 text-right tabular-nums">
                        {row.target_ms}
                      </TableCell>
                      <TableCell className="py-1.5 text-right tabular-nums">
                        {row.p50_us === null ? DASH : usToMs(row.p50_us)}
                      </TableCell>
                      <TableCell className="py-1.5 text-right tabular-nums">
                        {row.p90_us === null ? DASH : usToMs(row.p90_us)}
                      </TableCell>
                      <TableCell className="py-1.5 text-right tabular-nums">
                        {row.p95_us === null ? DASH : usToMs(row.p95_us)}
                      </TableCell>
                      <TableCell className="py-1.5 text-right tabular-nums">
                        {row.p99_us === null ? DASH : usToMs(row.p99_us)}
                      </TableCell>
                      <TableCell
                        className={`py-1.5 text-right tabular-nums ${over ? "text-destructive" : ""}`}
                      >
                        {row.headroom_p95_us === null
                          ? DASH
                          : formatHeadroom(row.headroom_p95_us)}
                      </TableCell>
                    </TableRow>
                  );
                })}
            </TableBody>
          </Table>
          <p className="mt-1 text-[10px] text-muted-foreground">
            Percentiles ventana del worker en µs, mostrados en ms (÷1000 exacto);
            headroom p95 en µs con signo (negativo = sobre presupuesto). null = sin
            muestras — el guion nunca es un cero (R8).
          </p>
        </div>
      )}
    </section>
  );
}
