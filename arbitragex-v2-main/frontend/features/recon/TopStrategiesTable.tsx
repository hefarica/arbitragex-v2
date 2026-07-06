"use client";

import { useMemo } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import { TrendingUpIcon } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Card, CardContent } from "@/components/ui/card";
import { DataTable } from "@/features/table/DataTable";
import type { ReconSummary } from "@/lib/api-client";
import { fmtMoney, fmtPct01 } from "@/lib/formatters";

type Row = ReconSummary["top_strategies"][number];

export function TopStrategiesTable({ rows }: { rows: Row[] }) {
  const columns = useMemo<ColumnDef<Row, unknown>[]>(
    () => [
      {
        accessorKey: "strategy_kind",
        header: "Strategy",
        cell: ({ row }) => <Badge variant="info">{row.original.strategy_kind}</Badge>,
      },
      { accessorKey: "chain_id", header: "Chain" },
      {
        accessorKey: "sample_count",
        header: "Samples",
        meta: { align: "right", className: "font-mono tabular-nums" },
      },
      {
        accessorKey: "success_rate",
        header: "Success",
        meta: { align: "right", className: "font-mono tabular-nums" },
        cell: ({ row }) => fmtPct01(row.original.success_rate),
        sortUndefined: "last",
      },
      {
        accessorKey: "revert_rate",
        header: "Revert",
        meta: { align: "right", className: "font-mono tabular-nums" },
        cell: ({ row }) => fmtPct01(row.original.revert_rate),
        sortUndefined: "last",
      },
      {
        accessorKey: "avg_profit_usd",
        header: "Avg PnL",
        meta: { align: "right", className: "font-mono tabular-nums" },
        cell: ({ row }) => fmtMoney(row.original.avg_profit_usd),
        sortUndefined: "last",
      },
      {
        accessorKey: "score",
        header: "Score",
        meta: { align: "right", className: "font-mono tabular-nums font-semibold" },
        cell: ({ row }) => (row.original.score != null ? row.original.score.toFixed(2) : "—"),
        sortUndefined: "last",
      },
    ],
    [],
  );

  if (rows.length === 0) {
    return (
      <Card className="py-14">
        <CardContent className="flex flex-col items-center gap-3 text-center">
          <div className="flex size-12 items-center justify-center rounded-full bg-muted text-muted-foreground">
            <TrendingUpIcon className="size-6" />
          </div>
          <div className="text-lg font-medium">No strategy scores yet.</div>
          <p className="max-w-md text-sm text-muted-foreground">
            The recon learning loop populates this after the first aggregation window closes.
          </p>
        </CardContent>
      </Card>
    );
  }

  return (
    <DataTable
      columns={columns}
      data={rows}
      searchPlaceholder="Filter strategies…"
      initialSort={[{ id: "score", desc: true }]}
    />
  );
}
