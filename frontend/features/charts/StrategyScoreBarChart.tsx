"use client";

import { useMemo } from "react";
import { Bar, BarChart, CartesianGrid, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import type { ReconSummary } from "@/lib/api-client";

type Row = ReconSummary["top_strategies"][number];

export function StrategyScoreBarChart({ rows, limit = 8 }: { rows: Row[]; limit?: number }) {
  const data = useMemo(
    () =>
      rows
        .filter((r) => r.score != null)
        .slice(0, limit)
        .map((r) => ({
          label: `${r.strategy_kind} · ${r.chain_id}`,
          score: Number(r.score ?? 0),
        }))
        .sort((a, b) => b.score - a.score),
    [rows, limit],
  );

  return (
    <Card>
      <CardHeader>
        <CardTitle>Strategy score</CardTitle>
        <CardDescription>
          Adaptive scoring — higher is better. Top {data.length} strategies by score.
        </CardDescription>
      </CardHeader>
      <CardContent>
        {data.length === 0 ? (
          <div className="flex h-56 items-center justify-center text-sm text-muted-foreground">
            No scored strategies yet.
          </div>
        ) : (
          <div className="h-64 w-full">
            <ResponsiveContainer width="100%" height="100%">
              <BarChart data={data} layout="vertical" margin={{ top: 4, right: 16, bottom: 4, left: 0 }}>
                <CartesianGrid horizontal={false} stroke="var(--border)" />
                <XAxis
                  type="number"
                  stroke="var(--muted-foreground)"
                  fontSize={11}
                  tickLine={false}
                />
                <YAxis
                  type="category"
                  dataKey="label"
                  stroke="var(--muted-foreground)"
                  fontSize={11}
                  tickLine={false}
                  width={140}
                />
                <Tooltip
                  cursor={{ fill: "var(--accent)", opacity: 0.4 }}
                  contentStyle={{
                    background: "var(--popover)",
                    border: "1px solid var(--border)",
                    borderRadius: "var(--radius-md)",
                    fontSize: "12px",
                    color: "var(--popover-foreground)",
                  }}
                  formatter={(value: number) => value.toFixed(2)}
                />
                <Bar dataKey="score" fill="var(--primary)" radius={[0, 4, 4, 0]} />
              </BarChart>
            </ResponsiveContainer>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
