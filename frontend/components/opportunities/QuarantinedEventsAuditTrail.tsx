"use client";

/**
 * FE-0032 (§31) — Audit Trail: Invalid / Quarantined Events.
 *
 * The §30 quarantine is visible per-card (QuarantineStrip); this subsection
 * aggregates the SAME snapshot into the §31 audit columns. Pure props over
 * the store's opportunities — no second fetch, no parallel model: the rows
 * are exactly the ones the grid holds, filtered to
 * `semantic_violations.length > 0` (the mapper is the only constructor).
 *
 * §31 column → wire source (wire-grade only, never the §29 synthetic view):
 *   timestamp      detected_at (null ⇒ "sin fecha" — the card tri-state)
 *   candidate_id   NOT EMITTED on the wire — column stays "no emitido"
 *   reason         rejection_reason (verbatim)
 *   source         trace_id (the row's provenance id)
 *   payload version NOT EMITTED on the wire — column stays "no emitido"
 *   strategy       strategy_kind (null ⇒ "—")
 *   route          dex_a → dex_b + hop_count from route_metadata only
 *   block          block_number (null ⇒ "—")
 *   errores        semantic_violations codes joined " · "
 *
 * R8: the empty state is a COMPUTED zero over this snapshot ("0 eventos en
 * cuarentena"), never a fabricated all-clear. Rows cap at AUDIT_TRAIL_LIMIT
 * with an explicit "+N" disclosure — no silent truncation.
 */

// SSR-test support (repo pattern): classic JSX path needs the React namespace.
import * as React from "react";

import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import type { OmniOpportunity } from "@/lib/store/types";

const DASH = "—";
/** §31 columns the wire does not carry yet (nivel-(b)) — stated, never faked. */
export const NOT_EMITTED = "no emitido";
/** Disclosure cap — matches the backend analytics LIMIT 25 convention. */
export const AUDIT_TRAIL_LIMIT = 25;

export function QuarantinedEventsAuditTrail({
  opportunities,
}: {
  opportunities: OmniOpportunity[];
}) {
  const quarantined = opportunities.filter(
    (o) => o.semantic_violations.length > 0,
  );
  const shown = quarantined.slice(0, AUDIT_TRAIL_LIMIT);
  const hidden = quarantined.length - shown.length;

  const routeCell = (o: OmniOpportunity) => {
    const dex = o.dex_b ? `${o.dex_a} → ${o.dex_b}` : o.dex_a;
    // hop_count is route_metadata-grade only (FE-0028): null without a
    // persisted topology — never the §29 synthetic leg count.
    const hops =
      o.hop_count != null ? ` (${o.hop_count} hops)` : ` (hops ${DASH})`;
    return `${dex}${hops}`;
  };

  return (
    <section
      aria-label="Audit Trail — Invalid / Quarantined Events (§31)"
      data-testid="quarantined-events-audit-trail"
      className="mt-8 space-y-2"
    >
      <h2 className="text-sm font-semibold uppercase tracking-wide text-muted-foreground">
        Audit Trail · Invalid / Quarantined Events (§31)
      </h2>

      {quarantined.length === 0 ? (
        <p className="text-sm text-muted-foreground" data-testid="audit-trail-empty">
          0 eventos en cuarentena en este snapshot — recuento computado sobre
          las filas servidas, no una garantía de integridad del pipeline.
        </p>
      ) : (
        <>
          <div className="overflow-x-auto rounded-lg border">
            <Table>
              <TableHeader>
                <TableRow className="text-left text-muted-foreground">
                  <TableHead className="font-medium">Timestamp</TableHead>
                  <TableHead className="font-medium">candidate_id</TableHead>
                  <TableHead className="font-medium">Reason</TableHead>
                  <TableHead className="font-medium">Source</TableHead>
                  <TableHead className="font-medium">Payload ver.</TableHead>
                  <TableHead className="font-medium">Strategy</TableHead>
                  <TableHead className="font-medium">Route</TableHead>
                  <TableHead className="font-medium">Block</TableHead>
                  <TableHead className="font-medium">Errores (§30)</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {shown.map((o) => (
                  <TableRow key={o.id}>
                    <TableCell className="py-1.5 pr-3 font-mono text-xs">
                      {o.detected_at ?? "sin fecha"}
                    </TableCell>
                    <TableCell className="py-1.5 pr-3 text-xs text-muted-foreground">
                      {NOT_EMITTED}
                    </TableCell>
                    <TableCell className="py-1.5 pr-3 font-mono text-xs">
                      {o.rejection_reason ?? DASH}
                    </TableCell>
                    <TableCell
                      className="py-1.5 pr-3 font-mono text-xs"
                      title={o.trace_id ?? ""}
                    >
                      {o.trace_id
                        ? `${o.trace_id.slice(0, 10)}…`
                        : DASH}
                    </TableCell>
                    <TableCell className="py-1.5 pr-3 text-xs text-muted-foreground">
                      {NOT_EMITTED}
                    </TableCell>
                    <TableCell className="py-1.5 pr-3 font-mono text-xs">
                      {o.strategy_kind ?? DASH}
                    </TableCell>
                    <TableCell className="py-1.5 pr-3 font-mono text-xs">
                      {routeCell(o)}
                    </TableCell>
                    <TableCell className="py-1.5 pr-3 tabular-nums">
                      {o.block_number ?? DASH}
                    </TableCell>
                    <TableCell className="py-1.5 font-mono text-xs text-destructive">
                      {o.semantic_violations.join(" · ")}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
          <p className="text-[10px] text-muted-foreground">
            candidate_id y payload version: {NOT_EMITTED} en el wire
            (nivel-(b)) — la columna existe para el día en que el backend los
            emita, jamás para fabricarlos.
            {hidden > 0 && (
              <span className="font-semibold">
                {" "}
                +{hidden} evento(s) en cuarentena no mostrado(s) — cap{" "}
                {AUDIT_TRAIL_LIMIT} por vista.
              </span>
            )}
          </p>
        </>
      )}
    </section>
  );
}
