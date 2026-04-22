"use client";

import { useMemo } from "react";
import type { ColumnDef } from "@tanstack/react-table";

import { Badge } from "@/components/ui/badge";
import { DataTable } from "@/features/table/DataTable";
import type { OpportunityRow } from "@/lib/api-client";
import { fmtMoney, fmtPct100, fmtTime } from "@/lib/formatters";

export function OpportunitiesTable({ items }: { items: OpportunityRow[] }) {
  const columns = useMemo<ColumnDef<OpportunityRow, unknown>[]>(
    () => [
      {
        accessorKey: "detected_at",
        header: "Detected",
        cell: ({ row }) => (
          <span className="text-muted-foreground">{fmtTime(row.original.detected_at)}</span>
        ),
      },
      { accessorKey: "chain_id", header: "Chain" },
      {
        accessorKey: "strategy_kind",
        header: "Strategy",
        cell: ({ row }) => <Badge variant="info">{row.original.strategy_kind}</Badge>,
      },
      {
        accessorKey: "pair_symbol",
        header: "Pair",
        cell: ({ row }) => row.original.pair_symbol ?? "—",
      },
      {
        accessorKey: "expected_profit_usd",
        header: "Est. PnL",
        meta: { align: "right", className: "font-mono tabular-nums" },
        cell: ({ row }) => fmtMoney(row.original.expected_profit_usd),
        sortUndefined: "last",
      },
      {
        accessorKey: "roi_pct",
        header: "ROI",
        meta: { align: "right", className: "font-mono tabular-nums" },
        cell: ({ row }) => fmtPct100(row.original.roi_pct),
        sortUndefined: "last",
      },
      {
        accessorKey: "status",
        header: "Status",
        cell: ({ row }) => <Badge variant="outline">{row.original.status}</Badge>,
      },
    ],
    [],
  );

  return (
    <DataTable
      columns={columns}
      data={items}
      searchPlaceholder="Filter opportunities (chain, strategy, pair, status)…"
      initialSort={[{ id: "detected_at", desc: true }]}
    />
  );
}
