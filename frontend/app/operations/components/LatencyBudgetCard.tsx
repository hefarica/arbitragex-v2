/**
 * ARBX-QB-07-008 (REQ-QB-015, workbook 10_LATENCY) — discovery latency
 * budget panel: one row per lat.* stage with the workbook's Target_ms /
 * Actual_p50 / Actual_p95 / Headroom_p95 columns (p90/p99 exist on the wire
 * but are NOT 10_LATENCY columns — they stay out of the display).
 *
 * Pure presentational (props in — no store, no fetch): the boot snapshot
 * arrives from the /operations Server Component, the live overlay is the
 * thin LatencyBudgetPanel wrapper over useRouteTick. Rows render in WIRE
 * order (backend-fixed stage order); the aggregate lat.total row is
 * emphasized because PASS_p95 is decided on it.
 *
 * R8 everywhere: null percentile → "—", never 0; absent snapshot → the
 * honest absence note; a failed fetch renders the error verbatim. The
 * caption discloses the two honest lags (Emit and lat.total trail one tick;
 * boot snapshot serves until the live tick carries lat.*).
 */
import * as React from "react";
import { TimerIcon } from "lucide-react";

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import type { LatencyStageRow } from "@/lib/apex/schemas";

import {
  headroomMsText,
  headroomOverBudget,
  LAT_ABSENCE_NOTE,
  LAT_TOTAL_KEY,
  usToMsText,
} from "./latency-budget";

interface Props {
  stages: LatencyStageRow[] | null;
  /** PASS_p95 vs the canonical discovery SLA. null = no completed cycles. */
  passP95: boolean | null;
  /** Completed cycles the window aggregates over. */
  cycles: number;
  /** Honest fetch/absence reason rendered when stages is null (R8). */
  error: string | null;
}

export function LatencyBudgetCard({ stages, passP95, cycles, error }: Props) {
  return (
    <Card data-slot="latency-budget-card">
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-base">
          <TimerIcon className="size-4 text-primary" aria-hidden />
          Discovery latency budget · lat.* stages (10_LATENCY)
        </CardTitle>
        <CardDescription>
          windowed nearest-rank p50/p95 · headroom = target − p95 (negative =
          over budget) · null = not computed (R8) · Emit and lat.total lag one
          tick · snapshot source: boot fetch until the live tick carries lat.*
        </CardDescription>
      </CardHeader>
      <CardContent>
        {error !== null ? (
          <div role="alert" className="break-all font-mono text-xs text-destructive">
            {error}
          </div>
        ) : stages === null || stages.length === 0 ? (
          <p className="text-xs text-muted-foreground">{LAT_ABSENCE_NOTE}</p>
        ) : (
          <>
            <div className="mb-3 flex flex-wrap items-center gap-2 text-xs">
              {passP95 === null ? (
                <span className="rounded-full border border-border px-2 py-0.5 text-muted-foreground">
                  PASS_p95: no completed cycles yet
                </span>
              ) : passP95 ? (
                <span className="rounded-full border border-primary/30 bg-primary/10 px-2 py-0.5 font-semibold text-primary">
                  PASS_p95
                </span>
              ) : (
                <span className="rounded-full border border-destructive/30 bg-destructive/10 px-2 py-0.5 font-semibold text-destructive">
                  FAIL_p95 — lat.total over SLA
                </span>
              )}
              <span className="text-muted-foreground">
                cycles: <span className="tabular-nums">{cycles}</span>
              </span>
            </div>
            <div className="overflow-x-auto">
              <Table>
                <TableHeader>
                  <TableRow className="text-left text-muted-foreground">
                    <TableHead className="font-medium">Stage</TableHead>
                    <TableHead className="text-right font-medium">Target ms</TableHead>
                    <TableHead className="text-right font-medium">p50 ms</TableHead>
                    <TableHead className="text-right font-medium">p95 ms</TableHead>
                    <TableHead className="text-right font-medium">Headroom ms</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {stages.map((row) => {
                    const total = row.key === LAT_TOTAL_KEY;
                    const over = headroomOverBudget(row.headroom_p95_us);
                    return (
                      <TableRow key={row.key}>
                        <TableCell className={total ? "font-semibold" : "font-medium"}>
                          {row.key}
                        </TableCell>
                        <TableCell className="text-right tabular-nums">{row.target_ms}</TableCell>
                        <TableCell
                          className="text-right tabular-nums"
                          title={row.p50_us === null ? undefined : `${row.p50_us} µs raw`}
                        >
                          {usToMsText(row.p50_us)}
                        </TableCell>
                        <TableCell
                          className="text-right tabular-nums"
                          title={row.p95_us === null ? undefined : `${row.p95_us} µs raw`}
                        >
                          {usToMsText(row.p95_us)}
                        </TableCell>
                        <TableCell
                          className={`text-right tabular-nums ${over ? "font-semibold text-destructive" : ""}`}
                        >
                          {headroomMsText(row.headroom_p95_us)}
                        </TableCell>
                      </TableRow>
                    );
                  })}
                </TableBody>
              </Table>
            </div>
          </>
        )}
      </CardContent>
    </Card>
  );
}
