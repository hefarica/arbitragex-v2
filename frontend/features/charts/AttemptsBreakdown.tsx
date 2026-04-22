"use client";

import { useMemo } from "react";
import { Cell, Pie, PieChart, ResponsiveContainer, Tooltip } from "recharts";

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import type { ReconSummary } from "@/lib/api-client";

type Segment = { key: "included" | "reverted" | "dropped" | "other"; label: string; value: number; color: string };

export function AttemptsBreakdown({ totals }: { totals: ReconSummary["totals"] }) {
  const segments = useMemo<Segment[]>(() => {
    const included = totals.included;
    const reverted = totals.reverted;
    const dropped = totals.dropped;
    const other = Math.max(0, totals.total - included - reverted - dropped);
    const all: Segment[] = [
      { key: "included", label: "Included", value: included, color: "var(--success)" },
      { key: "reverted", label: "Reverted", value: reverted, color: "var(--destructive)" },
      { key: "dropped", label: "Dropped", value: dropped, color: "var(--warning)" },
      { key: "other", label: "Other", value: other, color: "var(--muted-foreground)" },
    ];
    return all.filter((s) => s.value > 0);
  }, [totals]);

  const hasData = totals.total > 0 && segments.length > 0;

  return (
    <Card>
      <CardHeader>
        <CardTitle>Attempts breakdown</CardTitle>
        <CardDescription>
          {hasData ? `${totals.total} total attempts in the current window.` : "Awaiting first attempt."}
        </CardDescription>
      </CardHeader>
      <CardContent>
        <div className="flex h-64 w-full flex-col items-center gap-4 sm:flex-row">
          <div className="relative h-full w-full sm:w-1/2">
            {hasData ? (
              <ResponsiveContainer width="100%" height="100%">
                <PieChart>
                  <Pie
                    data={segments}
                    dataKey="value"
                    nameKey="label"
                    innerRadius="60%"
                    outerRadius="90%"
                    stroke="var(--card)"
                    strokeWidth={2}
                  >
                    {segments.map((s) => (
                      <Cell key={s.key} fill={s.color} />
                    ))}
                  </Pie>
                  <Tooltip
                    contentStyle={{
                      background: "var(--popover)",
                      border: "1px solid var(--border)",
                      borderRadius: "var(--radius-md)",
                      fontSize: "12px",
                      color: "var(--popover-foreground)",
                    }}
                    formatter={(value: number, name: string) => [`${value}`, name]}
                  />
                </PieChart>
              </ResponsiveContainer>
            ) : (
              <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
                No attempts yet.
              </div>
            )}
            {hasData && (
              <div className="pointer-events-none absolute inset-0 flex flex-col items-center justify-center">
                <span className="font-mono text-3xl font-medium tabular-nums">{totals.total}</span>
                <span className="text-[11px] uppercase tracking-widest text-muted-foreground">attempts</span>
              </div>
            )}
          </div>
          <ul className="flex w-full flex-col gap-2 text-sm sm:w-1/2">
            {segments.map((s) => {
              const pct = totals.total > 0 ? (s.value / totals.total) * 100 : 0;
              return (
                <li key={s.key} className="flex items-center gap-3">
                  <span
                    className="size-2.5 shrink-0 rounded-sm"
                    style={{ background: s.color }}
                    aria-hidden
                  />
                  <span className="flex-1 text-muted-foreground">{s.label}</span>
                  <span className="font-mono tabular-nums">{s.value}</span>
                  <span className="w-14 text-right font-mono tabular-nums text-muted-foreground">
                    {pct.toFixed(1)}%
                  </span>
                </li>
              );
            })}
          </ul>
        </div>
      </CardContent>
    </Card>
  );
}
