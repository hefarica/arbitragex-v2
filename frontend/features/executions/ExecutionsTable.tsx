"use client";

import { useMemo } from "react";
import type { ColumnDef } from "@tanstack/react-table";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { DataTable } from "@/features/table/DataTable";
import type { ExecutionRow } from "@/lib/api-client";
import { fmtMoney, fmtTime } from "@/lib/formatters";
import { exportToCSV, exportToJSON } from "@/lib/csv-export";

type StatusVariant = "success" | "destructive" | "warning" | "info" | "outline";

function statusVariant(s: ExecutionRow["status"]): StatusVariant {
  if (s === "included") return "success";
  if (s === "reverted") return "destructive";
  if (s === "dropped" || s === "replaced") return "warning";
  if (s === "submitted") return "info";
  return "outline";
}

export function ExecutionsTable({ items }: { items: ExecutionRow[] }) {
  const columns = useMemo<ColumnDef<ExecutionRow, unknown>[]>(
    () => [
      {
        accessorKey: "submitted_at",
        header: "Submitted",
        cell: ({ row }) => (
          <span className="text-muted-foreground">{fmtTime(row.original.submitted_at)}</span>
        ),
      },
      { accessorKey: "chain_id", header: "Chain" },
      { accessorKey: "strategy_kind", header: "Strategy" },
      {
        accessorKey: "pair_symbol",
        header: "Pair",
        cell: ({ row }) => row.original.pair_symbol ?? "—",
      },
      {
        accessorKey: "relay_name",
        header: "Relay",
        cell: ({ row }) => <Badge variant="outline">{row.original.relay_name}</Badge>,
      },
      {
        accessorKey: "status",
        header: "Status",
        cell: ({ row }) => (
          <Badge variant={statusVariant(row.original.status)}>{row.original.status}</Badge>
        ),
      },
      {
        accessorKey: "tx_hash",
        header: "Tx",
        enableSorting: false,
        meta: { className: "font-mono text-xs text-muted-foreground" },
        cell: ({ row }) => (row.original.tx_hash ? `${row.original.tx_hash.slice(0, 10)}…` : "—"),
      },
      {
        accessorKey: "block_included",
        header: "Block",
        meta: { align: "right", className: "font-mono tabular-nums" },
        cell: ({ row }) => row.original.block_included ?? "—",
        sortUndefined: "last",
      },
      {
        accessorKey: "actual_profit_usd",
        header: "PnL",
        meta: { align: "right", className: "font-mono tabular-nums" },
        cell: ({ row }) => fmtMoney(row.original.actual_profit_usd),
        sortUndefined: "last",
      },
      {
        accessorKey: "error_message",
        header: "Error",
        enableSorting: false,
        meta: { className: "max-w-[240px] truncate text-muted-foreground" },
        cell: ({ row }) => row.original.error_message ?? "",
      },
    ],
    [],
  );

  return (
    <div className="space-y-3">
      <div className="flex justify-end gap-2">
        <Button
          variant="outline"
          size="sm"
          onClick={() => exportToCSV(items as unknown as Record<string, unknown>[], `executions_${Date.now()}.csv`)}
        >
          Export CSV
        </Button>
        <Button
          variant="outline"
          size="sm"
          onClick={() => exportToJSON(items, `executions_${Date.now()}.json`)}
        >
          Export JSON
        </Button>
      </div>
      <DataTable
        columns={columns}
        data={items}
        searchPlaceholder="Filter executions (chain, strategy, relay, status, error)…"
        initialSort={[{ id: "submitted_at", desc: true }]}
      />
    </div>
  );
}
