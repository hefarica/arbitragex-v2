/**
 * FE-MASTER · Strategy detail drawer (FE-0024 — P6, §24).
 *
 * Opens from a Workbook-264 row/list cell: the full canon record of ONE
 * strategy — discovery equation (display-only LaTeX-ish string, never
 * evaluated), Gate_LIVE sentence, search policy, primary ops, and the whole
 * metadata block the EMIT-07 wire carries.
 *
 * Honest gaps (RULE 00 / §28 / R8): financing modes, runtime KPIs (p95,
 * detections/h) and the per-chain active flag are NOT part of the static
 * catalog wire — each renders as an explicit "no emitido" line, never a
 * fabricated value. Runtime enabled state lives in trading_config (the
 * Runtime kinds view of this same tab), which the drawer points to.
 *
 * `StrategyDetailBody` is the pure presentational core (SSR-testable without
 * the Radix portal); the Sheet wrapper owns open/close — same split as
 * PairDetailDrawer (FE-0019).
 */
"use client";

// SSR-test support (repo pattern, cf. TokenIcon/ChainsAdminClient).
import * as React from "react";

import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
import type { DispatchStatus, StrategyCatalogRow } from "@/lib/apex/schemas";

const DASH = "—";

/** Badge variant per dispatch status — color is never the only signal (§22). */
export const STATUS_VARIANT: Record<DispatchStatus, "secondary" | "outline" | "destructive"> = {
  ROUTE_READY: "secondary",
  NEEDS_ROUTE_DATA: "outline",
  OBSERVE_ONLY: "outline",
  NO_COMPATIBLE_ROUTE: "destructive",
};

/** Row of the metadata grid. */
function Meta({ label, value }: { label: string; value: string }) {
  return (
    <div className="space-y-0.5">
      <p className="text-[10px] uppercase tracking-wide text-muted-foreground">{label}</p>
      <p className="break-words text-sm">{value}</p>
    </div>
  );
}

export function StrategyDetailBody({ row }: { row: StrategyCatalogRow }) {
  return (
    <div className="space-y-4">
      {/* ── identity + dispatch status (§22) ─────────────────────────── */}
      <div className="space-y-1">
        <p className="font-mono text-xs text-muted-foreground">{row.mev_id}</p>
        <p className="text-lg font-semibold leading-tight">{row.name}</p>
        <p className="text-sm text-muted-foreground">{row.family}</p>
      </div>
      <div className="flex flex-wrap gap-1.5">
        <Badge variant={STATUS_VARIANT[row.status]}>{row.status}</Badge>
        <Badge variant="outline">{row.execution_class}</Badge>
        <Badge variant="outline">group {row.group}</Badge>
      </div>

      <Separator />

      {/* ── metadata grid — wire values verbatim ─────────────────────── */}
      <div className="grid grid-cols-2 gap-3">
        <Meta label="Surface" value={row.surface} />
        <Meta label="Backend module" value={row.backend_module} />
        <Meta label="Detector (§25)" value={row.detector_id} />
        <Meta label="Graph model" value={row.graph_model} />
        <Meta label="QuoteBase role" value={row.quotebase_role} />
        <Meta label="Legs (canon)" value={`${row.min_legs}–${row.max_legs}`} />
      </div>
      <div>
        <p className="text-[10px] uppercase tracking-wide text-muted-foreground">
          Hops permitidos (canon expandido)
        </p>
        <p className="text-sm tabular-nums">{row.allowed_hops.join(" · ")}</p>
        <p className="mt-0.5 text-[10px] text-muted-foreground">
          El cap runtime de 7 hops es política del hot-path, no metadata del
          catálogo — el canon de legs llega hasta {row.max_legs}.
        </p>
      </div>

      {/* ── policies — workbook sentences verbatim ───────────────────── */}
      <div className="space-y-2">
        <div>
          <p className="text-[10px] uppercase tracking-wide text-muted-foreground">
            Discovery equation (display only)
          </p>
          <p className="rounded-md bg-muted/60 px-2 py-1.5 font-mono text-xs">
            {row.discovery_equation}
          </p>
        </div>
        <Meta label="Search policy" value={row.search_policy} />
        <Meta label="Gate_LIVE" value={row.gate_live} />
      </div>

      <div className="space-y-1.5">
        <p className="text-[10px] uppercase tracking-wide text-muted-foreground">
          Primary ops
        </p>
        <div className="flex flex-wrap gap-1.5">
          {row.primary_ops.map((op) => (
            <Badge key={op} variant="outline" className="font-mono text-[10px]">
              {op}
            </Badge>
          ))}
        </div>
      </div>

      <Separator />

      {/* ── honest gaps — NOT in the static catalog wire (§28/R8) ────── */}
      <div className="space-y-1">
        <p className="text-[10px] uppercase tracking-wide text-muted-foreground">
          No emitido en el catálogo (gaps honestos)
        </p>
        <ul className="list-disc space-y-0.5 pl-4 text-xs text-muted-foreground">
          <li>
            Financing modes — dimensión de ruta runtime, no metadata del canon
            (nivel-(b) pendiente de emisión).
          </li>
          <li>
            KPIs runtime (p95, detecciones/h) — requieren emisión runtime; el
            wire EMIT-07 es estático-per-canon.
          </li>
          <li>
            Activo por chain — vive en trading_config.enabled_strategies
            (vista «Runtime kinds» de este mismo tab), nunca en el catálogo.
          </li>
        </ul>
      </div>
    </div>
  );
}

interface Props {
  row: StrategyCatalogRow | null;
  onClose: () => void;
}

export function StrategyDetailDrawer({ row, onClose }: Props) {
  return (
    <Sheet open={row !== null} onOpenChange={(open) => !open && onClose()}>
      <SheetContent side="right" className="w-full overflow-y-auto sm:max-w-md">
        {row && (
          <>
            <SheetHeader>
              <SheetTitle className="font-mono text-sm">{row.mev_id}</SheetTitle>
              <SheetDescription>
                Estrategia del workbook 11_STRATEGY_HOP_MAP — registro canónico
                (§24).
              </SheetDescription>
            </SheetHeader>
            <div className="px-4 pb-6">
              <StrategyDetailBody row={row} />
            </div>
          </>
        )}
      </SheetContent>
    </Sheet>
  );
}

export { DASH };
