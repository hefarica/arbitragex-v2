/**
 * FE-MASTER · Detector detail drawer (FE-0025 — P7, §25).
 *
 * Opens from a Detector Policy tier row: the full 14-key canon record of ONE
 * detector family — the EXACT workbook sentences (discovery criterion,
 * required data, DO_NOT_RULES), the ops chips, the family hop envelope, and
 * the frontend_config phrases (display-only per the d9 amendment: the knob
 * VALUES live in runtime config, never in this catalog — nothing here derives
 * types or units heuristically).
 *
 * Honest gaps (RULE 00 / §28 / R8): per-detector runtime state (detections/h,
 * p95, last fire) is NOT part of the static catalog wire — each renders as an
 * explicit "no emitido" line. The tier badge repeats the DP-005 fold
 * (tierForExecutionClass, the TS mirror of the Rust rule) so the drawer never
 * disagrees with the panel; a class outside the closed vocabulary shows the
 * honest unknown badge, never a default tier.
 *
 * `DetectorDetailBody` is the pure presentational core (SSR-testable without
 * the Radix portal); the Sheet wrapper owns open/close — same split as
 * StrategyDetailDrawer (FE-0024) / PairDetailDrawer (FE-0019).
 */
"use client";

// SSR-test support (repo pattern, cf. StrategyDetailDrawer).
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
import { tierForExecutionClass } from "@/lib/apex/signal-tier";
import type { DetectorPolicyView, HotSeed } from "@/lib/apex/schemas";

/** Badge variant per hot_seed — color is never the only signal. */
export const HOT_SEED_VARIANT: Record<HotSeed, "secondary" | "outline"> = {
  SEED_CANDIDATE: "secondary",
  OBSERVE_EVIDENCE: "outline",
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

export function DetectorDetailBody({ row }: { row: DetectorPolicyView }) {
  const tier = tierForExecutionClass(row.execution_class);
  return (
    <div className="space-y-4">
      {/* ── identity + badges ────────────────────────────────────────── */}
      <div className="space-y-1">
        <p className="font-mono text-xs text-muted-foreground">{row.detector_id}</p>
        <p className="text-lg font-semibold leading-tight">{row.example_surface}</p>
        <p className="text-sm text-muted-foreground">
          ejemplo <span className="font-mono">{row.example_mev}</span>
        </p>
      </div>
      <div className="flex flex-wrap gap-1.5">
        <Badge variant={HOT_SEED_VARIANT[row.hot_seed]}>{row.hot_seed}</Badge>
        <Badge variant="outline">{row.execution_class}</Badge>
        {tier === null ? (
          <Badge variant="destructive">tier desconocido (drift)</Badge>
        ) : (
          <Badge variant="outline">tier {tier}</Badge>
        )}
      </div>

      <Separator />

      {/* ── metadata grid — wire values verbatim ─────────────────────── */}
      <div className="grid grid-cols-2 gap-3">
        <Meta label="Estrategias (Σ=264 canon)" value={String(row.strategies_count)} />
        <Meta label="Graph policy" value={row.graph_policy} />
      </div>
      <div>
        <p className="text-[10px] uppercase tracking-wide text-muted-foreground">
          Hop envelope (familia)
        </p>
        <p className="text-sm tabular-nums">
          {row.hop_envelope.min}–{row.hop_envelope.max}
        </p>
        <p className="mt-0.5 text-[10px] text-muted-foreground">
          La envolvente de familia INTERSECTA los min/max_legs de cada estrategia
          miembro — una estrategia nunca escapa a su familia (invariante backend;
          el FE solo muestra).
        </p>
      </div>

      {/* ── policies — workbook sentences verbatim ───────────────────── */}
      <div className="space-y-2">
        <div>
          <p className="text-[10px] uppercase tracking-wide text-muted-foreground">
            Criterio de discovery (exacto del workbook)
          </p>
          <p className="rounded-md bg-muted/60 px-2 py-1.5 font-mono text-xs">
            {row.exact_discovery_criterion}
          </p>
        </div>
        <Meta label="Datos requeridos" value={row.required_data} />
      </div>

      <div>
        <p className="text-[10px] uppercase tracking-wide text-muted-foreground">
          NO hacer (DO_NOT_RULES — regla universal)
        </p>
        <p className="rounded-md border border-destructive/40 bg-destructive/10 px-2 py-1.5 text-xs text-destructive">
          {row.do_not_do}
        </p>
      </div>

      {/* ── ops — payload chips ──────────────────────────────────────── */}
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
      <div className="space-y-1.5">
        <p className="text-[10px] uppercase tracking-wide text-muted-foreground">
          Secondary ops
        </p>
        {row.secondary_ops.length === 0 ? (
          <p className="text-xs text-muted-foreground">—</p>
        ) : (
          <div className="flex flex-wrap gap-1.5">
            {row.secondary_ops.map((op) => (
              <Badge key={op} variant="secondary" className="font-mono text-[10px]">
                {op}
              </Badge>
            ))}
          </div>
        )}
      </div>

      {/* ── frontend_config — display-only phrases (enmienda d9) ─────── */}
      <div className="space-y-1.5">
        <p className="text-[10px] uppercase tracking-wide text-muted-foreground">
          Frontend config (frases del workbook)
        </p>
        <div className="flex flex-wrap gap-1.5">
          {row.frontend_config.map((phrase) => (
            <Badge key={phrase} variant="outline" className="text-[10px]">
              {phrase}
            </Badge>
          ))}
        </div>
        <p className="mt-0.5 text-[10px] text-muted-foreground">
          Frases EXACTAS del workbook — los VALORES de estos knobs viven en
          runtime config; jamás se derivan tipos ni unidades aquí (§28/R8).
        </p>
      </div>

      <Separator />

      {/* ── honest gaps — NOT in the static catalog wire (§28/R8) ────── */}
      <div className="space-y-1">
        <p className="text-[10px] uppercase tracking-wide text-muted-foreground">
          No emitido en el catálogo (gaps honestos)
        </p>
        <ul className="list-disc space-y-0.5 pl-4 text-xs text-muted-foreground">
          <li>
            Estado runtime por detector (detecciones/h, p95, último disparo) —
            requieren emisión runtime; el wire EMIT-08 es estático-per-canon.
          </li>
          <li>
            Estrategias miembro — el join detector_id→MEV_ID vive en el catálogo
            de estrategias (tab «Engine Catalog · Workbook 264»), no en este row.
          </li>
        </ul>
      </div>
    </div>
  );
}

interface Props {
  row: DetectorPolicyView | null;
  onClose: () => void;
}

export function DetectorDetailDrawer({ row, onClose }: Props) {
  return (
    <Sheet open={row !== null} onOpenChange={(open) => !open && onClose()}>
      <SheetContent side="right" className="w-full overflow-y-auto sm:max-w-md">
        {row && (
          <>
            <SheetHeader>
              <SheetTitle className="font-mono text-sm">{row.detector_id}</SheetTitle>
              <SheetDescription>
                Familia de detectores del workbook 05_DETECTOR_POLICY — registro
                canónico (§25).
              </SheetDescription>
            </SheetHeader>
            <div className="px-4 pb-6">
              <DetectorDetailBody row={row} />
            </div>
          </>
        )}
      </SheetContent>
    </Sheet>
  );
}
