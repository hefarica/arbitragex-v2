"use client";

import { PolarAngleAxis, RadialBar, RadialBarChart, ResponsiveContainer } from "recharts";

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";

export function RevertRateGauge({ rate }: { rate: number | null }) {
  const hasData = rate != null && !Number.isNaN(rate);
  const clamped = hasData ? Math.max(0, Math.min(1, rate)) : 0;
  const pct = clamped * 100;

  const tone =
    !hasData ? "var(--muted-foreground)"
      : pct < 5 ? "var(--success)"
        : pct < 15 ? "var(--warning)"
          : "var(--destructive)";

  const data = [{ name: "revert", value: pct, fill: tone }];

  return (
    <Card>
      <CardHeader>
        <CardTitle>Revert rate</CardTitle>
        <CardDescription>Share of submitted bundles that reverted on-chain. Lower is better.</CardDescription>
      </CardHeader>
      <CardContent>
        <div className="relative flex h-64 w-full items-center justify-center">
          <ResponsiveContainer width="100%" height="100%">
            <RadialBarChart
              cx="50%"
              cy="50%"
              innerRadius="70%"
              outerRadius="100%"
              startAngle={220}
              endAngle={-40}
              data={data}
            >
              <PolarAngleAxis type="number" domain={[0, 100]} tick={false} />
              <RadialBar background={{ fill: "var(--muted)" }} dataKey="value" cornerRadius={8} />
            </RadialBarChart>
          </ResponsiveContainer>
          <div className="pointer-events-none absolute inset-0 flex flex-col items-center justify-center">
            <span className="font-mono text-4xl font-medium tabular-nums" style={{ color: tone }}>
              {hasData ? `${pct.toFixed(1)}%` : "—"}
            </span>
            <span className="mt-1 text-[11px] uppercase tracking-widest text-muted-foreground">
              {hasData ? (pct < 5 ? "healthy" : pct < 15 ? "elevated" : "critical") : "no data"}
            </span>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}
