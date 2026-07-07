/**
 * Sprint 3 Task 3.4 — Cumulative S-curve (actual vs target) over 24h.
 *
 * Client component because recharts ResponsiveContainer mounts to the DOM.
 * R1 / React #418 fix: useMemo runs during render on BOTH server and client, so a
 * runtime-default-locale toLocaleTimeString(undefined, …) produced server-tz/locale at
 * SSR ≠ client-tz at hydration → hydration mismatch. We pin a DETERMINISTIC locale
 * ("en-US") + timeZone:"UTC" so the axis labels are byte-identical on SSR and client.
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
        ts: new Date(b.ts).toLocaleTimeString("en-US", { hour: "2-digit", minute: "2-digit", timeZone: "UTC" }),
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
