"use client";

import { useMemo } from "react";
import {
  CartesianGrid,
  Line,
  LineChart,
  ReferenceLine,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import type { ReconTimeseriesResponse } from "@/lib/api-client";
import { fmtMoney, fmtTime } from "@/lib/formatters";

export function ReconPnLChart({ response }: { response: ReconTimeseriesResponse }) {
  const data = useMemo(
    () =>
      response.points.map((p) => ({
        t: p.bucket_start,
        tLabel: fmtTime(p.bucket_start),
        pnl: p.avg_pnl_included_usd,
        attempts: p.attempts,
        included: p.included,
      })),
    [response.points],
  );

  const hasAnyPnl = data.some((d) => d.pnl != null);

  return (
    <Card>
      <CardHeader>
        <CardTitle>Realised PnL over time</CardTitle>
        <CardDescription>
          Average PnL per included bundle, bucketed every {response.bucket_minutes}{" "}
          minutes over the last {response.window_hours}h. Empty buckets
          (no attempts) render as gaps — never as zero.
        </CardDescription>
      </CardHeader>
      <CardContent>
        {!hasAnyPnl ? (
          <div className="flex h-64 items-center justify-center text-sm text-muted-foreground">
            No included executions in this window. The line will draw once the
            first bundle confirms.
          </div>
        ) : (
          <div className="h-64 w-full">
            <ResponsiveContainer width="100%" height="100%">
              <LineChart data={data} margin={{ top: 4, right: 16, bottom: 4, left: 0 }}>
                <CartesianGrid stroke="var(--border)" strokeDasharray="3 3" vertical={false} />
                <XAxis
                  dataKey="tLabel"
                  stroke="var(--muted-foreground)"
                  fontSize={11}
                  tickLine={false}
                  minTickGap={24}
                />
                <YAxis
                  stroke="var(--muted-foreground)"
                  fontSize={11}
                  tickLine={false}
                  tickFormatter={(v: number) => `$${v.toFixed(0)}`}
                />
                <ReferenceLine y={0} stroke="var(--muted-foreground)" strokeOpacity={0.4} />
                <Tooltip
                  contentStyle={{
                    background: "var(--popover)",
                    border: "1px solid var(--border)",
                    borderRadius: "var(--radius-md)",
                    fontSize: "12px",
                    color: "var(--popover-foreground)",
                  }}
                  formatter={(value: number | null, name: string) => {
                    if (name === "pnl") return [fmtMoney(value), "Avg PnL (incl.)"];
                    return [value, name];
                  }}
                />
                <Line
                  type="monotone"
                  dataKey="pnl"
                  stroke="var(--primary)"
                  strokeWidth={2}
                  dot={false}
                  activeDot={{ r: 4, fill: "var(--primary)" }}
                  connectNulls={false}
                  isAnimationActive={false}
                />
              </LineChart>
            </ResponsiveContainer>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
