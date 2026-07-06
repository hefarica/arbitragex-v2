"use client";

import { useMemo } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import { ShieldCheckIcon } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Card, CardContent } from "@/components/ui/card";
import { DataTable } from "@/features/table/DataTable";
import type { RiskAlertRow } from "@/lib/api-client";
import { fmtTime } from "@/lib/formatters";

type SeverityVariant = "destructive" | "warning" | "info";

function sevVariant(s: RiskAlertRow["severity"]): SeverityVariant {
  if (s === "critical") return "destructive";
  if (s === "warning") return "warning";
  return "info";
}

function safePayloadPreview(payload: unknown, max = 200): string {
  try {
    return JSON.stringify(payload).slice(0, max);
  } catch {
    return "[unserializable]";
  }
}

export function RiskAlertsTable({ alerts, windowHours }: { alerts: RiskAlertRow[]; windowHours: number }) {
  const columns = useMemo<ColumnDef<RiskAlertRow, unknown>[]>(
    () => [
      {
        accessorKey: "created_at",
        header: "Time",
        cell: ({ row }) => (
          <span className="text-muted-foreground">{fmtTime(row.original.created_at)}</span>
        ),
      },
      {
        accessorKey: "severity",
        header: "Severity",
        cell: ({ row }) => (
          <Badge variant={sevVariant(row.original.severity)}>{row.original.severity}</Badge>
        ),
      },
      { accessorKey: "event_type", header: "Type" },
      {
        accessorKey: "source_service",
        header: "Source",
        meta: { className: "font-mono text-xs" },
      },
      {
        id: "payload",
        accessorFn: (row) => safePayloadPreview(row.payload),
        header: "Details",
        enableSorting: false,
        meta: { className: "max-w-[360px] truncate font-mono text-xs text-muted-foreground" },
      },
    ],
    [],
  );

  if (alerts.length === 0) {
    return (
      <Card className="py-14">
        <CardContent className="flex flex-col items-center gap-3 text-center">
          <div className="flex size-12 items-center justify-center rounded-full bg-success/10 text-success">
            <ShieldCheckIcon className="size-6" />
          </div>
          <div className="text-lg font-medium">All clear.</div>
          <p className="max-w-md text-sm text-muted-foreground">
            No warnings or criticals logged in the last {windowHours}h.
          </p>
        </CardContent>
      </Card>
    );
  }

  return (
    <DataTable
      columns={columns}
      data={alerts}
      searchPlaceholder="Filter alerts (severity, type, source, payload)…"
      initialSort={[{ id: "created_at", desc: true }]}
    />
  );
}
