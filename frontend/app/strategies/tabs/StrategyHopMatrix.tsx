/**
 * FE-MASTER · Strategy×Hop matrix (FE-0023 — P6, §23).
 *
 * Pure presentational component: rows = the workbook strategies the payload
 * carries, columns = the DISTINCT hops that appear in those rows (derived
 * from `allowed_hops`, never a hardcoded [2..7] — the wire arrives ALREADY
 * EXPANDED from HopMask_u8, so the FE only answers membership per cell.
 * It never decodes bits and never computes the matrix (§79).
 *
 * Cell semantics:
 *   - hop ∈ allowed_hops ⇒ filled cell, tinted by DispatchStatus (FE-0022);
 *   - hop ∉ allowed_hops ⇒ empty cell (the absence IS the data, R8);
 *   - both carry a full `title` so hover reads without color.
 *
 * Row identity is mev_id (workbook canon, unique ascending). Clicking a row
 * opens the FE-0024 drawer via the parent's onRowSelect.
 */
"use client";

// SSR-test support (repo pattern, cf. TokenIcon/ChainsAdminClient): the node
// test env renders this component via react-dom/server with jsx preserved, so
// React must be in module scope.
import * as React from "react";
import { useMemo } from "react";

import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import type { DispatchStatus, StrategyCatalogRow } from "@/lib/apex/schemas";

/** Token-based tint per dispatch status — color is never the only signal (§22). */
export const STATUS_CELL_CLASS: Record<DispatchStatus, string> = {
  ROUTE_READY: "bg-success/25 text-success",
  NEEDS_ROUTE_DATA: "bg-warning/25 text-warning",
  OBSERVE_ONLY: "bg-info/25 text-info",
  NO_COMPATIBLE_ROUTE: "bg-destructive/20 text-destructive",
};

interface Props {
  rows: StrategyCatalogRow[];
  onRowSelect: (row: StrategyCatalogRow) => void;
  selectedId: string | null;
}

export function StrategyHopMatrix({ rows, onRowSelect, selectedId }: Props) {
  // Column set is DATA-DRIVEN: the distinct hops the payload actually carries
  // (today's canon is 2..7, but a re-ingestion could change it — the FE must
  // follow the payload, never pin the workbook of yesterday).
  const hops = useMemo(
    () => Array.from(new Set(rows.flatMap((r) => r.allowed_hops))).sort((a, b) => a - b),
    [rows],
  );

  if (rows.length === 0) {
    return (
      <p className="text-sm text-muted-foreground">
        Sin filas que mostrar — el filtro no dejó ninguna estrategia (R8: el
        cero es honesto, no un error).
      </p>
    );
  }

  return (
    <div className="overflow-x-auto">
      <Table>
        <TableHeader>
          <TableRow className="text-left text-muted-foreground">
            <TableHead className="font-medium">MEV_ID</TableHead>
            <TableHead className="font-medium">Estrategia</TableHead>
            {hops.map((h) => (
              <TableHead key={h} className="text-center font-medium" title={`hop ${h}`}>
                h{h}
              </TableHead>
            ))}
          </TableRow>
        </TableHeader>
        <TableBody>
          {rows.map((r) => (
            <TableRow
              key={r.mev_id}
              className={selectedId === r.mev_id ? "bg-muted/60" : undefined}
            >
              <TableCell className="font-mono text-xs whitespace-nowrap">{r.mev_id}</TableCell>
              <TableCell className="font-medium">
                <button
                  type="button"
                  className="text-left underline-offset-2 hover:underline focus-visible:outline focus-visible:outline-2 focus-visible:outline-ring"
                  title={`Detalle ${r.mev_id} (§24)`}
                  onClick={() => onRowSelect(r)}
                >
                  {r.name}
                </button>
              </TableCell>
              {hops.map((h) => {
                const allowed = r.allowed_hops.includes(h);
                return (
                  <TableCell key={h} className="p-0 text-center">
                    {allowed ? (
                      <div
                        className={`mx-auto h-5 w-5 rounded-sm ${STATUS_CELL_CLASS[r.status]}`}
                        title={`${r.mev_id} · hop ${h} permitido · ${r.status}`}
                        aria-label={`${r.mev_id} hop ${h} permitido ${r.status}`}
                      />
                    ) : (
                      <div
                        className="mx-auto h-5 w-5 rounded-sm border border-dashed border-border/60"
                        title={`${r.mev_id} · hop ${h} NO permitido por el canon`}
                        aria-label={`${r.mev_id} hop ${h} no permitido`}
                      />
                    )}
                  </TableCell>
                );
              })}
            </TableRow>
          ))}
        </TableBody>
      </Table>
      <p className="mt-2 text-xs text-muted-foreground">
        Celdas = membership del payload (`allowed_hops` ya expandido backend-side
        desde HopMask_u8 — §23/§79: el frontend nunca decodifica bits ni calcula
        la matriz). Tinte = DispatchStatus (§22); hueco punteado = el canon NO
        autoriza ese hop.
      </p>
    </div>
  );
}
