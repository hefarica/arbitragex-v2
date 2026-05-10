/**
 * Sprint 3 Task 3.4 — Cumulative S-curve (actual vs target) over 24h.
 *
 * Client component because recharts ResponsiveContainer mounts to the DOM.
 * Time conversion happens inside `useMemo` — never inside render — so the
 * Mounted Snapshot Pattern (R1) is preserved: no Date.now()/locale calls
 * during SSR.
 */
"use client";

import { useMemo } from "react";
import {
  CartesianGrid,
  Legend,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type { SCurvePayload } from "@/lib/operations-schemas";

interface Props {
  data: SCurvePayload;
}

export function SCurveChart({ data }: Props) {
  const points = useMemo(
    () =>
      data.buckets.map((b) => ({
        ts: new Date(b.ts).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" }),
        actual: b.profit_cumulative_usd,
        target: b.target_cumulative_usd,
      })),
    [data.buckets],
  );

  return (
    <Card>
      <CardHeader>
        <CardTitle>S-Curve · cumulative PnL vs target (24h)</CardTitle>
      </CardHeader>
      <CardContent style={{ height: 360 }}>
        <ResponsiveContainer width="100%" height="100%">
          <LineChart data={points} margin={{ top: 8, right: 16, left: 0, bottom: 4 }}>
            <CartesianGrid strokeDasharray="3 3" stroke="hsl(var(--border))" />
            <XAxis dataKey="ts" tick={{ fontSize: 11 }} />
            <YAxis tick={{ fontSize: 11 }} tickFormatter={(v) => `$${v}`} />
            <Tooltip formatter={(v: number) => `$${v.toFixed(2)}`} />
            <Legend />
            <Line type="monotone" dataKey="actual" stroke="var(--color-success)" name="actual PnL" dot={false} strokeWidth={2} />
            <Line type="monotone" dataKey="target" stroke="var(--color-muted-foreground)" name="target" strokeDasharray="4 4" dot={false} />
          </LineChart>
        </ResponsiveContainer>
      </CardContent>
    </Card>
  );
}
