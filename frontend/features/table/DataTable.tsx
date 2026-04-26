"use client";

import { useCallback, useEffect, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import {
  flexRender,
  getCoreRowModel,
  getFilteredRowModel,
  getSortedRowModel,
  useReactTable,
  type ColumnDef,
  type RowData,
  type SortingState,
} from "@tanstack/react-table";
import { ArrowDownIcon, ArrowUpDownIcon, ArrowUpIcon, SearchIcon } from "lucide-react";

import { Input } from "@/components/ui/input";
import { Card, CardContent } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { nextRowIndex } from "@/features/table/keyboard";
import { cn } from "@/lib/utils";

declare module "@tanstack/react-table" {
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  interface ColumnMeta<TData extends RowData, TValue> {
    align?: "left" | "right" | "center";
    className?: string;
  }
}

const ALIGN_CLASS = {
  left: "text-left",
  right: "text-right",
  center: "text-center",
} as const;

export function DataTable<TData>({
  columns,
  data,
  emptyState,
  searchPlaceholder = "Filter…",
  initialSort,
  enableFilter = true,
  ariaLabel,
}: {
  columns: ColumnDef<TData, unknown>[];
  data: TData[];
  emptyState?: ReactNode;
  searchPlaceholder?: string;
  initialSort?: SortingState;
  enableFilter?: boolean;
  ariaLabel?: string;
}) {
  const [sorting, setSorting] = useState<SortingState>(initialSort ?? []);
  const [globalFilter, setGlobalFilter] = useState("");
  const [activeRow, setActiveRow] = useState(0);
  const gridRef = useRef<HTMLDivElement>(null);

  const table = useReactTable({
    data,
    columns,
    state: { sorting, globalFilter },
    onSortingChange: setSorting,
    onGlobalFilterChange: setGlobalFilter,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
    getFilteredRowModel: getFilteredRowModel(),
  });

  const rows = table.getRowModel().rows;
  const rowCount = rows.length;
  const filteredCount = rowCount;
  const totalCount = data.length;

  // Clamp active row when data length shrinks (filter, sort) so focus stays valid.
  useEffect(() => {
    if (rowCount > 0 && activeRow >= rowCount) setActiveRow(rowCount - 1);
  }, [rowCount, activeRow]);

  const focusRowByIdx = useCallback((idx: number) => {
    const el = gridRef.current?.querySelector<HTMLTableRowElement>(`tr[data-row-idx="${idx}"]`);
    el?.focus();
  }, []);

  const onRowKeyDown = useCallback(
    (e: KeyboardEvent<HTMLTableRowElement>, idx: number) => {
      const next = nextRowIndex(e.key, idx, rowCount);
      if (next === null) return;
      e.preventDefault();
      if (next === idx) return;
      setActiveRow(next);
      // Defer focus to the next paint so the tabIndex flip has landed.
      queueMicrotask(() => focusRowByIdx(next));
    },
    [rowCount, focusRowByIdx],
  );

  return (
    <div className="space-y-3" ref={gridRef}>
      {enableFilter && (
        <div className="flex items-center justify-between gap-4">
          <div className="relative max-w-sm flex-1">
            <SearchIcon className="pointer-events-none absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
            <Input
              value={globalFilter}
              onChange={(e) => setGlobalFilter(e.target.value)}
              placeholder={searchPlaceholder}
              className="pl-8"
              aria-label={searchPlaceholder}
            />
          </div>
          <span className="text-xs uppercase tracking-widest text-muted-foreground/70">
            {globalFilter
              ? `${filteredCount} of ${totalCount} rows`
              : `${totalCount} rows`}
          </span>
        </div>
      )}

      <Card className="py-0">
        <CardContent className="p-0">
          <Table role="grid" aria-label={ariaLabel} aria-rowcount={rowCount}>
            <TableHeader>
              {table.getHeaderGroups().map((hg) => (
                <TableRow key={hg.id} role="row">
                  {hg.headers.map((h) => {
                    const meta = h.column.columnDef.meta;
                    const align = meta?.align ?? "left";
                    const canSort = h.column.getCanSort();
                    const sortDir = h.column.getIsSorted();
                    const ariaSort = sortDir === "asc" ? "ascending"
                      : sortDir === "desc" ? "descending"
                      : canSort ? "none" : undefined;
                    return (
                      <TableHead
                        key={h.id}
                        role="columnheader"
                        aria-sort={ariaSort}
                        className={cn(ALIGN_CLASS[align], meta?.className)}
                      >
                        {h.isPlaceholder ? null : canSort ? (
                          <button
                            type="button"
                            onClick={h.column.getToggleSortingHandler()}
                            className={cn(
                              "inline-flex items-center gap-1.5 hover:text-foreground transition-colors",
                              align === "right" && "w-full justify-end",
                              align === "center" && "w-full justify-center",
                            )}
                            aria-label={`Sort by ${String(h.column.columnDef.header ?? h.id)}`}
                          >
                            {flexRender(h.column.columnDef.header, h.getContext())}
                            <SortIcon dir={sortDir} />
                          </button>
                        ) : (
                          flexRender(h.column.columnDef.header, h.getContext())
                        )}
                      </TableHead>
                    );
                  })}
                </TableRow>
              ))}
            </TableHeader>
            <TableBody>
              {rows.length === 0 ? (
                <TableRow role="row">
                  <TableCell
                    role="gridcell"
                    colSpan={columns.length}
                    className="h-32 text-center text-muted-foreground"
                  >
                    {emptyState ??
                      (globalFilter
                        ? `No rows match "${globalFilter}".`
                        : "No data.")}
                  </TableCell>
                </TableRow>
              ) : (
                rows.map((row, i) => {
                  const isActive = i === activeRow;
                  return (
                    <TableRow
                      key={row.id}
                      role="row"
                      aria-rowindex={i + 1}
                      data-row-idx={i}
                      tabIndex={isActive ? 0 : -1}
                      onFocus={() => setActiveRow(i)}
                      onKeyDown={(e) => onRowKeyDown(e, i)}
                      className="focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50 focus-visible:bg-accent/50"
                    >
                      {row.getVisibleCells().map((cell) => {
                        const meta = cell.column.columnDef.meta;
                        const align = meta?.align ?? "left";
                        return (
                          <TableCell
                            key={cell.id}
                            role="gridcell"
                            className={cn(ALIGN_CLASS[align], meta?.className)}
                          >
                            {flexRender(cell.column.columnDef.cell, cell.getContext())}
                          </TableCell>
                        );
                      })}
                    </TableRow>
                  );
                })
              )}
            </TableBody>
          </Table>
        </CardContent>
      </Card>
    </div>
  );
}

function SortIcon({ dir }: { dir: false | "asc" | "desc" }) {
  if (dir === "asc") return <ArrowUpIcon className="size-3.5" />;
  if (dir === "desc") return <ArrowDownIcon className="size-3.5" />;
  return <ArrowUpDownIcon className="size-3.5 opacity-40" />;
}
