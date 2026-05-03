/**
 * Sprint 2 Task 2.3 — Audit tab.
 *
 * Filters audit_log to target_kind='trading_config' and shows the most
 * recent N entries. Fetched client-side so the operator's httpOnly cookie
 * goes with the request (avoids needing to forward auth from Server
 * Component).
 */
"use client";

import { useEffect, useState } from "react";
import { AlertCircleIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { getAuditLogs } from "@/lib/api-client";
import type { AuditLogRow } from "@/lib/schemas";

export function AuditTab() {
  const [rows, setRows] = useState<AuditLogRow[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      const res = await getAuditLogs(50, undefined, undefined, undefined, "trading_config");
      if (cancelled) return;
      if (res.ok) setRows(res.data.items);
      else setError(res.error);
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  if (error) {
    return (
      <Alert variant="destructive">
        <AlertCircleIcon />
        <AlertTitle>audit endpoint error</AlertTitle>
        <AlertDescription className="font-mono text-xs">{error}</AlertDescription>
      </Alert>
    );
  }

  if (rows === null) {
    return (
      <Card>
        <CardContent className="py-6 text-sm text-muted-foreground">Loading audit log…</CardContent>
      </Card>
    );
  }

  if (rows.length === 0) {
    return (
      <Card>
        <CardContent className="py-6 text-sm text-muted-foreground">
          No <code>trading_config</code> audit entries yet. Edit a value in another tab and save to see the
          first row appear here.
        </CardContent>
      </Card>
    );
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>Audit log · target_kind = trading_config</CardTitle>
      </CardHeader>
      <CardContent className="p-0">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>when</TableHead>
              <TableHead>actor</TableHead>
              <TableHead>action</TableHead>
              <TableHead>before</TableHead>
              <TableHead>after</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {rows.map((row) => (
              <TableRow key={row.id}>
                <TableCell className="font-mono text-xs">{row.created_at}</TableCell>
                <TableCell className="font-mono text-xs">{row.actor}</TableCell>
                <TableCell><code className="text-xs">{row.action}</code></TableCell>
                <TableCell className="font-mono text-xs max-w-xs truncate">
                  {summarise(row.before_state)}
                </TableCell>
                <TableCell className="font-mono text-xs max-w-xs truncate">
                  {summarise(row.after_state)}
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </CardContent>
    </Card>
  );
}

function summarise(state: unknown): string {
  if (state === null || state === undefined) return "—";
  if (typeof state !== "object") return String(state);
  const obj = state as Record<string, unknown>;
  const keys = ["capital_usd", "min_profit_usd", "min_roi_pct", "max_slippage_pct", "enabled_strategies"];
  const parts = keys.filter((k) => k in obj).map((k) => `${k}=${JSON.stringify(obj[k])}`);
  return parts.length > 0 ? parts.join(" · ") : JSON.stringify(obj).slice(0, 80);
}
