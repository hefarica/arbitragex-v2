"use client";

import { useMemo, useState } from "react";
import type { ColumnDef } from "@tanstack/react-table";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { DataTable } from "@/features/table/DataTable";
import { Sheet, SheetContent, SheetHeader, SheetTitle, SheetDescription } from "@/components/ui/sheet";
import type { AuditLogRow } from "@/lib/api-client";
import { fmtTime } from "@/lib/formatters";
import { exportToCSV, exportToJSON } from "@/lib/csv-export";

export function AuditLogsTable({ items }: { items: AuditLogRow[] }) {
  const [selectedRow, setSelectedRow] = useState<AuditLogRow | null>(null);

  const columns = useMemo<ColumnDef<AuditLogRow, unknown>[]>(
    () => [
      {
        accessorKey: "created_at",
        header: "Timestamp",
        cell: ({ row }) => (
          <span className="text-muted-foreground">{fmtTime(row.original.created_at)}</span>
        ),
      },
      {
        accessorKey: "action",
        header: "Action",
        cell: ({ row }) => <Badge variant="outline">{row.original.action}</Badge>,
      },
      {
        accessorKey: "actor",
        header: "Actor",
        cell: ({ row }) => <span className="font-mono text-sm">{row.original.actor}</span>,
      },
      {
        accessorKey: "target_kind",
        header: "Target",
        cell: ({ row }) => (
          row.original.target_kind ? (
            <span className="flex flex-col">
              <span>{row.original.target_kind}</span>
              <span className="text-xs text-muted-foreground max-w-[200px] truncate">{row.original.target_id}</span>
            </span>
          ) : "—"
        ),
      },
      {
        accessorKey: "ip_address",
        header: "IP",
        cell: ({ row }) => row.original.ip_address ?? "—",
      },
      {
        id: "details",
        header: "State",
        cell: ({ row }) => {
          const hasState = row.original.before_state != null || row.original.after_state != null;
          if (!hasState) return <span className="text-muted-foreground">—</span>;
          return (
            <Button variant="secondary" size="sm" onClick={() => setSelectedRow(row.original)}>
              View JSON
            </Button>
          );
        },
      },
    ],
    [],
  );

  // Flatten before_state / after_state for CSV export (they are objects).
  const exportRows = useMemo(
    () =>
      items.map((r) => ({
        ...r,
        before_state: r.before_state != null ? JSON.stringify(r.before_state) : null,
        after_state: r.after_state != null ? JSON.stringify(r.after_state) : null,
      })),
    [items],
  );

  return (
    <>
      <div className="flex justify-end gap-2 mb-3">
        <Button
          variant="outline"
          size="sm"
          onClick={() => exportToCSV(exportRows as unknown as Record<string, unknown>[], `audit_logs_${Date.now()}.csv`)}
        >
          Export CSV
        </Button>
        <Button
          variant="outline"
          size="sm"
          onClick={() => exportToJSON(items, `audit_logs_${Date.now()}.json`)}
        >
          Export JSON
        </Button>
      </div>
      <DataTable
        columns={columns}
        data={items}
        searchPlaceholder="Filter logs (action, actor, target)…"
        initialSort={[{ id: "created_at", desc: true }]}
      />
      <Sheet open={!!selectedRow} onOpenChange={(open) => !open && setSelectedRow(null)}>
        <SheetContent className="w-[400px] sm:w-[540px] overflow-y-auto">
          <SheetHeader className="mb-6">
            <SheetTitle>Audit Log Details</SheetTitle>
            <SheetDescription>
              {selectedRow?.action} by {selectedRow?.actor}
            </SheetDescription>
          </SheetHeader>
          
          {selectedRow && (
            <div className="space-y-6">
              <div className="grid grid-cols-2 gap-2 text-sm">
                <div><span className="text-muted-foreground">ID:</span> <br/>{selectedRow.id.split("-")[0]}...</div>
                <div><span className="text-muted-foreground">Time:</span> <br/>{fmtTime(selectedRow.created_at)}</div>
                {selectedRow.target_kind && (
                  <div><span className="text-muted-foreground">Target Kind:</span> <br/>{selectedRow.target_kind}</div>
                )}
                {selectedRow.target_id && (
                  <div><span className="text-muted-foreground">Target ID:</span> <br/>{selectedRow.target_id}</div>
                )}
                {selectedRow.ip_address && (
                  <div><span className="text-muted-foreground">IP:</span> <br/>{selectedRow.ip_address}</div>
                )}
                {selectedRow.trace_id && (
                  <div><span className="text-muted-foreground">Trace ID:</span> <br/>{selectedRow.trace_id.split("-")[0]}...</div>
                )}
              </div>

              {selectedRow.before_state != null && (
                <div>
                  <h4 className="text-sm font-semibold mb-2">Before State</h4>
                  <pre className="p-4 bg-muted rounded-md overflow-x-auto text-xs font-mono">
                    {JSON.stringify(selectedRow.before_state, null, 2)}
                  </pre>
                </div>
              )}
              {selectedRow.after_state != null && (
                <div>
                  <h4 className="text-sm font-semibold mb-2">After State</h4>
                  <pre className="p-4 bg-muted rounded-md overflow-x-auto text-xs font-mono">
                    {JSON.stringify(selectedRow.after_state, null, 2)}
                  </pre>
                </div>
              )}
            </div>
          )}
        </SheetContent>
      </Sheet>
    </>
  );
}
