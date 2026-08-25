/**
 * FE-MASTER · Detector Policy taxonomy panel (ARBX-DP-005 — P7 §25).
 *
 * The four emission tiers as DISTINCT FEEDS over the detector catalog
 * (GET /api/detectors/catalog, EMIT-08): OBSERVATION / SIGNAL / CANDIDATE /
 * EXECUTABLE — never one flattened "arbitrage" feed. The tier derives
 * client-side from the row's `execution_class` via the TS mirror of the
 * Rust rule (lib/apex/signal-tier.ts; parity pinned differentially against
 * the same generated catalog).
 *
 * RULE 00 / §79: every cell renders what the payload carries — detector_id,
 * execution_class, example_surface, hot_seed, example_mev,
 * strategies_count. No recomputation beyond the tier fold (which IS the
 * doctrine, not a score). R8: null catalog = "—" (never served); error
 * verbatim; a class outside the closed vocabulary lands in an honest
 * "unknown" bucket, never a default tier.
 *
 * The catalog is static-per-canon: fetch on mount, no polling.
 *
 * FE-0025 (d9): every tier row opens the DetectorDetailDrawer (§25) — the
 * full 14-key canon record. Row selection is panel state; the drawer is a
 * controlled Sheet (open = selected !== null). Nothing else about the DP-005
 * tier fold changes.
 */
"use client";

// SSR-test support (repo pattern, cf. PairIntelligencePanel): react-dom/server
// with jsx preserved needs React in module scope.
import * as React from "react";
import { useEffect, useMemo, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { tierForExecutionClass, SIGNAL_TIER_TOKENS, type SignalTier } from "@/lib/apex/signal-tier";
import { useOmniStore } from "@/lib/store/omni-store";
import type { DetectorPolicyView } from "@/lib/apex/schemas";
import { DetectorDetailDrawer } from "./DetectorDetailDrawer";

const DASH = "—";

/** Doctrine order: ascending actionability. */
const TIER_META: Record<SignalTier, { title: string; blurb: string }> = {
  observation: {
    title: "OBSERVATION",
    blurb: "Evidencia informativa — jamás una Opportunity{confidence}.",
  },
  signal: {
    title: "SIGNAL",
    blurb: "Precondición fuera del envelope atómico — señal hasta evidencia firme.",
  },
  candidate: {
    title: "CANDIDATE",
    blurb: "Matemática exacta bajo precondición observable en runtime.",
  },
  executable: {
    title: "EXECUTABLE",
    blurb: "Determinista sin precondición externa — puede entrar al feed ejecutable.",
  },
};

export function DetectorPolicyPanel() {
  const catalog = useOmniStore((s) => s.detectorCatalog);
  const status = useOmniStore((s) => s.detectorCatalogStatus);
  const error = useOmniStore((s) => s.detectorCatalogError);
  const fetchDetectorCatalog = useOmniStore((s) => s.fetchDetectorCatalog);

  // FE-0025: selected detector for the §25 detail drawer.
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const selected =
    catalog?.find((row) => row.detector_id === selectedId) ?? null;

  // Static-per-canon: fetch on mount (the slice's ready-guard makes the
  // remount a no-op). No poll — workbook rows do not churn per tick.
  useEffect(() => {
    void fetchDetectorCatalog();
  }, [fetchDetectorCatalog]);

  const buckets = useMemo(() => {
    const byTier: Record<SignalTier, DetectorPolicyView[]> = {
      observation: [],
      signal: [],
      candidate: [],
      executable: [],
    };
    const unknown: DetectorPolicyView[] = [];
    if (catalog) {
      for (const row of catalog) {
        const tier = tierForExecutionClass(row.execution_class);
        if (tier === null) unknown.push(row);
        else byTier[tier].push(row);
      }
    }
    return { byTier, unknown };
  }, [catalog]);

  return (
    <div className="space-y-4">
      {status === "error" && (
        <p className="text-sm text-destructive" role="alert">
          {error ?? "detector catalog unavailable"}
        </p>
      )}
      {status !== "error" && !catalog && <p className="text-sm text-muted-foreground">{DASH}</p>}
      {catalog && (
        <>
          {SIGNAL_TIER_TOKENS.map((tier) => {
            const rows = buckets.byTier[tier];
            const meta = TIER_META[tier];
            return (
              <Card key={tier} data-tier={tier}>
                <CardHeader className="flex flex-row flex-wrap items-center justify-between gap-2 space-y-0">
                  <div>
                    <CardTitle className="text-base">
                      {meta.title}
                      <span className="ml-2 text-sm font-normal text-muted-foreground">
                        {rows.length} {rows.length === 1 ? "detector" : "detectores"}
                      </span>
                    </CardTitle>
                    <p className="mt-1 text-xs text-muted-foreground">{meta.blurb}</p>
                  </div>
                  <Badge variant="outline">{tier}</Badge>
                </CardHeader>
                <CardContent>
                  {rows.length === 0 && (
                    <p className="text-sm text-muted-foreground">
                      Sin detectores en este tier (cero honesto del canon).
                    </p>
                  )}
                  {rows.length > 0 && (
                    <div className="overflow-x-auto">
                      <Table>
                        <TableHeader>
                          <TableRow className="text-left text-muted-foreground">
                            <TableHead className="font-medium">Detector</TableHead>
                            <TableHead className="font-medium">Execution_Class</TableHead>
                            <TableHead className="font-medium">Surface</TableHead>
                            <TableHead className="font-medium">Hot seed</TableHead>
                            <TableHead className="font-medium">Ejemplo</TableHead>
                            <TableHead className="text-right font-medium">Estrategias</TableHead>
                          </TableRow>
                        </TableHeader>
                        <TableBody>
                          {rows.map((row) => (
                            <TableRow
                              key={row.detector_id}
                              className="cursor-pointer"
                              title={`Detalle ${row.detector_id} (§25)`}
                              onClick={() => setSelectedId(row.detector_id)}
                            >
                              <TableCell className="py-1.5 pr-3 font-mono text-xs">
                                {row.detector_id}
                              </TableCell>
                              <TableCell className="py-1.5 pr-3 text-xs">
                                {row.execution_class}
                              </TableCell>
                              <TableCell className="py-1.5 pr-3 text-xs">
                                {row.example_surface}
                              </TableCell>
                              <TableCell className="py-1.5 pr-3 text-xs">{row.hot_seed}</TableCell>
                              <TableCell className="py-1.5 pr-3 font-mono text-xs">
                                {row.example_mev}
                              </TableCell>
                              <TableCell className="py-1.5 text-right tabular-nums">
                                {row.strategies_count}
                              </TableCell>
                            </TableRow>
                          ))}
                        </TableBody>
                      </Table>
                    </div>
                  )}
                </CardContent>
              </Card>
            );
          })}
          {buckets.unknown.length > 0 && (
            <Card data-tier="unknown">
              <CardHeader>
                <CardTitle className="text-base">
                  UNKNOWN
                  <span className="ml-2 text-sm font-normal text-muted-foreground">
                    {buckets.unknown.length} {buckets.unknown.length === 1 ? "detector" : "detectores"}
                  </span>
                </CardTitle>
                <p className="mt-1 text-xs text-muted-foreground">
                  Execution_Class fuera del vocabulario cerrado de 29 tokens — drift honesto
                  (R8), jamás un tier por defecto.
                </p>
              </CardHeader>
              <CardContent>
                <ul className="list-disc pl-4 text-xs">
                  {buckets.unknown.map((row) => (
                    <li key={row.detector_id} className="font-mono">
                      {row.detector_id}: {row.execution_class}
                    </li>
                  ))}
                </ul>
              </CardContent>
            </Card>
          )}
        </>
      )}
      {status === "loading" && catalog === null && (
        <p className="text-sm text-muted-foreground">Cargando catálogo de detectores…</p>
      )}
      <DetectorDetailDrawer row={selected} onClose={() => setSelectedId(null)} />
    </div>
  );
}
