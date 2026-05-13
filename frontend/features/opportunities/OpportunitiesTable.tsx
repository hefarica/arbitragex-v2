"use client";

import { useMemo } from "react";
import type { ColumnDef } from "@tanstack/react-table";

import { Badge } from "@/components/ui/badge";
import { DataTable } from "@/features/table/DataTable";
import type { OpportunityRow } from "@/lib/api-client";
import { fmtMoney, fmtPct100, fmtTime } from "@/lib/formatters";
import { OpportunityEvidenceCell } from "./OpportunityEvidenceCell";

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
      // ─── P1 simulation/evidence enrichment (audit 2026-05-13) ───
      // These columns extract evidence-of-execution fields from the row. The
      // backend does not yet emit them on /api/opportunities/live — when the
      // payload omits a field, the cell shows "Unavailable" (never zero,
      // never fabricated). R8 fail-honest applies row-by-row.
      {
        id: "sim_classification",
        header: "Sim",
        cell: ({ row }) => (
          <OpportunityEvidenceCell.SimClassification value={row.original.sim_classification ?? null} />
        ),
      },
      {
        id: "net_profit_usd",
        header: "Net P&L",
        meta: { align: "right", className: "font-mono tabular-nums" },
        cell: ({ row }) => (
          <OpportunityEvidenceCell.NetProfit
            usd={row.original.simulated_net_profit_usd ?? null}
            wei={row.original.net_profit_wei ?? null}
          />
        ),
      },
      {
        id: "gas_used",
        header: "Gas",
        meta: { align: "right", className: "font-mono tabular-nums text-xs" },
        cell: ({ row }) => (
          <OpportunityEvidenceCell.GasUsed value={row.original.gas_used ?? null} />
        ),
      },
      {
        id: "trace_hash",
        header: "Trace",
        cell: ({ row }) => (
          <OpportunityEvidenceCell.Hash value={row.original.trace_hash ?? null} />
        ),
      },
      {
        id: "evidence_gate",
        header: "Gates",
        cell: ({ row }) => (
          <OpportunityEvidenceCell.Gates
            evidence={row.original.evidence_gate ?? null}
            netProfit={row.original.net_profit_gate ?? null}
          />
        ),
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
