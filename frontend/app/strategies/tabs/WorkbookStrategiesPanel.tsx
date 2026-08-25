/**
 * FE-MASTER · Workbook strategies panel (FE-0021/FE-0022 — P6, §21/§22).
 *
 * The 264-row workbook canon (11_STRATEGY_HOP_MAP) over the omni-store's
 * StrategiesSlice (EMIT-07 consumer, fetch-once — static-per-canon: only a
 * `force` refresh after a workbook re-ingestion refetches it).
 *
 * §22 honest states: every row carries its DispatchStatus badge verbatim
 * (color is never the only signal) — ROUTE_READY / NEEDS_ROUTE_DATA /
 * OBSERVE_ONLY / NO_COMPATIBLE_ROUTE. Status filter chips show the counts
 * DERIVED from the payload (never pinned: the 79/174/8/3 split is a canon
 * contract test, not a runtime guarantee the FE may assume).
 *
 * §21 column note: the workbook's p95 latency column is NOT part of the
 * static catalog wire — no value is fabricated; the FE-0024 drawer lists it
 * as an honest gap (RULE 00 / R8).
 *
 * List|Matrix (§23) rides an inner segmented control; both views filter over
 * the SAME filtered set (presentation subsetting only — payload order is
 * never recomputed). Clicking a row/cell opens the FE-0024 drawer.
 */
"use client";

// SSR-test support (repo pattern, cf. TokenIcon/ChainsAdminClient): the node
// test env renders the pure exports of this module via react-dom/server with
// jsx preserved, so React must be in module scope.
import * as React from "react";
import { useEffect, useMemo, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { useOmniStore } from "@/lib/store/omni-store";
import type { DispatchStatus, StrategyCatalogRow } from "@/lib/apex/schemas";

import { StrategyDetailDrawer, STATUS_VARIANT } from "./StrategyDetailDrawer";
import { StrategyHopMatrix } from "./StrategyHopMatrix";

const DASH = "—";

const ALL_STATUSES: DispatchStatus[] = [
  "ROUTE_READY",
  "NEEDS_ROUTE_DATA",
  "OBSERVE_ONLY",
  "NO_COMPATIBLE_ROUTE",
];

/**
 * Presentation filter over payload rows — pure subsetting for readability.
 * Text matches mev_id/name/family/surface/detector_id/ops (case-insensitive);
 * an EMPTY status set means "no status filter" (all rows pass).
 */
export function filterStrategies(
  rows: StrategyCatalogRow[],
  query: string,
  statuses: ReadonlySet<DispatchStatus>,
): StrategyCatalogRow[] {
  const q = query.trim().toUpperCase();
  return rows.filter((r) => {
    if (statuses.size > 0 && !statuses.has(r.status)) return false;
    if (!q) return true;
    const hay = [
      r.mev_id,
      r.name,
      r.family,
      r.surface,
      r.detector_id,
      r.backend_module,
      ...r.primary_ops,
    ].join(" ");
    return hay.toUpperCase().includes(q);
  });
}

type View = "list" | "matrix";

export function WorkbookStrategiesPanel() {
  const rows = useOmniStore((s) => s.strategyCatalog);
  const status = useOmniStore((s) => s.strategyCatalogStatus);
  const error = useOmniStore((s) => s.strategyCatalogError);
  const fetchStrategyCatalog = useOmniStore((s) => s.fetchStrategyCatalog);

  // Static-per-canon: fetch-once on mount — the slice's own guard makes this
  // a no-op when the catalog is already in the store.
  useEffect(() => {
    void fetchStrategyCatalog();
  }, [fetchStrategyCatalog]);

  const [view, setView] = useState<View>("list");
  const [query, setQuery] = useState("");
  const [statusFilter, setStatusFilter] = useState<ReadonlySet<DispatchStatus>>(new Set());
  const [selected, setSelected] = useState<StrategyCatalogRow | null>(null);

  const filtered = useMemo(
    () => (rows ? filterStrategies(rows, query, statusFilter) : null),
    [rows, query, statusFilter],
  );

  // Counts derived from the PAYLOAD — never the workbook's pinned 79/174/8/3
  // (a re-ingestion may legitimately change them; §28).
  const statusCounts = useMemo(() => {
    const counts = new Map<DispatchStatus, number>();
    for (const r of rows ?? []) counts.set(r.status, (counts.get(r.status) ?? 0) + 1);
    return counts;
  }, [rows]);

  const toggleStatus = (st: DispatchStatus) => {
    setStatusFilter((prev) => {
      const next = new Set(prev);
      if (next.has(st)) next.delete(st);
      else next.add(st);
      return next;
    });
  };

  return (
    <Card>
      <CardHeader className="space-y-3">
        <CardTitle className="text-base">
          Workbook 264 · catálogo canónico (§21)
          {rows && (
            <span className="ml-2 text-sm font-normal text-muted-foreground">
              {filtered?.length ?? 0} de {rows.length} filas servidas
            </span>
          )}
        </CardTitle>
        {/* §22: status filter chips with PAYLOAD-derived counts */}
        <div className="flex flex-wrap items-center gap-2">
          {ALL_STATUSES.map((st) => {
            const on = statusFilter.has(st);
            return (
              <button
                key={st}
                type="button"
                aria-pressed={on}
                onClick={() => toggleStatus(st)}
                className={`rounded-md border px-2 py-0.5 text-[11px] font-medium transition-colors focus-visible:outline focus-visible:outline-2 focus-visible:outline-ring ${
                  on ? "border-primary bg-primary/10 text-primary" : "border-border text-muted-foreground hover:bg-muted/50"
                }`}
                title={`Filtrar por ${st} (§22)`}
              >
                {st} {statusCounts.get(st) ?? 0}
              </button>
            );
          })}
          {statusFilter.size > 0 && (
            <Button
              variant="ghost"
              size="sm"
              className="h-6 px-2 text-[11px]"
              onClick={() => setStatusFilter(new Set())}
            >
              limpiar filtros
            </Button>
          )}
          <Input
            aria-label="Filtrar estrategias"
            placeholder="Filtrar MEV_ID / nombre / familia / detector…"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            className="h-8 w-56"
          />
          {/* List|Matrix segmented (§23) */}
          <div
            role="tablist"
            aria-label="Vista del catálogo"
            className="inline-flex h-8 items-center rounded-lg bg-muted p-0.5 text-muted-foreground"
          >
            {(["list", "matrix"] as const).map((v) => (
              <button
                key={v}
                role="tab"
                type="button"
                aria-selected={view === v}
                onClick={() => setView(v)}
                className={`rounded-md px-2.5 py-0.5 text-xs font-medium transition-colors focus-visible:outline focus-visible:outline-2 focus-visible:outline-ring ${
                  view === v ? "bg-background text-foreground shadow-sm" : "hover:text-foreground"
                }`}
              >
                {v === "list" ? "Lista" : "Matriz ×hop"}
              </button>
            ))}
          </div>
          <Button
            variant="outline"
            size="sm"
            onClick={() => void fetchStrategyCatalog({ force: true })}
            disabled={status === "loading"}
            title="Catálogo estático-per-canon — solo refresca tras re-ingestión del workbook"
          >
            {status === "loading" ? "Cargando…" : "Refrescar canon"}
          </Button>
        </div>
      </CardHeader>
      <CardContent>
        {status === "error" && (
          <p className="text-sm text-destructive" role="alert">
            {error ?? "strategy catalog unavailable"}
          </p>
        )}
        {status !== "error" && !rows && (
          <p className="text-sm text-muted-foreground">{DASH}</p>
        )}
        {rows && rows.length === 0 && (
          <p className="text-sm text-muted-foreground">
            Catálogo servido vacío — entries: [] (R8: el cero es honesto; el
            generador canon valida 264 en CI, no en runtime).
          </p>
        )}
        {status === "ready" && filtered && filtered.length === 0 && rows && rows.length > 0 && (
          <p className="text-sm text-muted-foreground">
            Ninguna estrategia coincide con el filtro actual.
          </p>
        )}
        {filtered && filtered.length > 0 && view === "list" && (
          <div className="overflow-x-auto">
            <Table>
              <TableHeader>
                <TableRow className="text-left text-muted-foreground">
                  <TableHead className="font-medium">MEV_ID</TableHead>
                  <TableHead className="font-medium">Estrategia</TableHead>
                  <TableHead className="font-medium">Familia</TableHead>
                  <TableHead className="font-medium">Surface</TableHead>
                  <TableHead className="font-medium">Detector</TableHead>
                  <TableHead className="text-right font-medium">Legs</TableHead>
                  <TableHead className="text-right font-medium">Hops</TableHead>
                  <TableHead className="text-center font-medium">Estado (§22)</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {filtered.map((r) => (
                  <TableRow
                    key={r.mev_id}
                    className={selected?.mev_id === r.mev_id ? "bg-muted/60" : undefined}
                  >
                    <TableCell className="font-mono text-xs whitespace-nowrap">{r.mev_id}</TableCell>
                    <TableCell className="font-medium">
                      <button
                        type="button"
                        className="text-left underline-offset-2 hover:underline focus-visible:outline focus-visible:outline-2 focus-visible:outline-ring"
                        title={`Detalle ${r.mev_id} (§24)`}
                        onClick={() => setSelected(r)}
                      >
                        {r.name}
                      </button>
                    </TableCell>
                    <TableCell className="text-xs">{r.family}</TableCell>
                    <TableCell className="text-xs">{r.surface}</TableCell>
                    <TableCell className="font-mono text-xs">{r.detector_id}</TableCell>
                    <TableCell className="text-right tabular-nums text-xs">
                      {r.min_legs}–{r.max_legs}
                    </TableCell>
                    <TableCell className="text-right tabular-nums text-xs">
                      {r.allowed_hops.join("·")}
                    </TableCell>
                    <TableCell className="text-center">
                      <Badge variant={STATUS_VARIANT[r.status]}>{r.status}</Badge>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
        )}
        {filtered && filtered.length > 0 && view === "matrix" && (
          <StrategyHopMatrix
            rows={filtered}
            onRowSelect={setSelected}
            selectedId={selected?.mev_id ?? null}
          />
        )}
      </CardContent>
      {/* FE-0024: full canon record + honest gaps (financing/KPIs/p95). */}
      <StrategyDetailDrawer row={selected} onClose={() => setSelected(null)} />
    </Card>
  );
}
