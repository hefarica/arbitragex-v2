"use client";

/**
 * FE-MASTER · Route Outcomes analytics panel (FE-0038 — §47).
 *
 * The §47 group-bys over the SAME outcomes sink the Gate-C panel reads:
 * by-strategy (cartridge_id IS the strategy key the table persists) and
 * by-pair (raw token addresses, verbatim + short form for display). By-chain
 * already lives in the Gate-C panel above — repeated here only as a pointer,
 * never a second table over the same wire.
 *
 * Honest gaps (nivel-(b), RULE 00/§28): hop, detector and DEX are NOT
 * columns of `route_discovery_outcomes` — each renders as an explicit
 * "no emitido en el sink" line. The FE never invents a join to fake those
 * dimensions, and never derives one strategy_kind/detector from another.
 *
 * R8: STALE without data surfaces the upstream reason verbatim (503
 * db_unavailable / query_failed); an empty window is an honest empty state;
 * absent groupings (api-server older than the FE-0038 deploy) parse as []
 * — displayed as "no servido por esta api-server", not as a zero count.
 */

// SSR-test support (repo pattern): classic JSX path needs the React namespace.
import * as React from "react";
import { AlertCircleIcon, LayersIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  useRouteDiscoveryOutcomes,
  type OutcomeCartridgeRow,
  type OutcomePairRow,
} from "@/lib/hooks/useRouteDiscoveryOutcomes";
import { Freshness, ReadOnlyBadge, StatusPill } from "./premium-ui";

const WINDOWS: Array<{ label: string; hours: number }> = [
  { label: "24h", hours: 24 },
  { label: "3d", hours: 72 },
  { label: "7d", hours: 168 },
  { label: "14d", hours: 336 },
];

const DASH = "—";

/** Display-only short form of an address (never parsed, never re-derived). */
export function shortAddr(a: string): string {
  if (a === DASH || a === "(null)") return a;
  return a.length <= 16 ? a : `${a.slice(0, 8)}…${a.slice(-6)}`;
}

interface BodyProps {
  byCartridge: OutcomeCartridgeRow[];
  byPair: OutcomePairRow[];
  /** null = grouping not served by this api-server (pre-FE-0038 deploy). */
  groupingsServed: boolean;
  windowHours: number | null;
}

export function OutcomesAnalyticsBody({
  byCartridge,
  byPair,
  groupingsServed,
  windowHours,
}: BodyProps) {
  if (!groupingsServed) {
    return (
      <p className="text-sm text-muted-foreground">
        Agrupaciones §47 no servidas por esta api-server (despliegue FE-0038
        pendiente en el backend) — ausencia real, no un cero (R8).
      </p>
    );
  }
  return (
    <div className="space-y-4" data-testid="outcomes-analytics-body">
      {/* ── by strategy (cartridge_id) ─────────────────────────────────── */}
      <div className="space-y-2">
        <h4 className="text-sm font-semibold">
          By strategy
          <span className="ml-2 text-xs font-normal text-muted-foreground">
            cartridge_id — la llave de estrategia que el sink persiste
          </span>
        </h4>
        {byCartridge.length === 0 ? (
          <p className="text-xs text-muted-foreground">
            Sin filas en la ventana {windowHours ?? DASH}h (cero honesto).
          </p>
        ) : (
          <div className="overflow-x-auto rounded-lg border">
            <Table>
              <TableHeader>
                <TableRow className="text-left text-muted-foreground">
                  <TableHead className="font-medium">Cartridge (estrategia)</TableHead>
                  <TableHead className="text-right font-medium">Outcomes</TableHead>
                  <TableHead className="text-right font-medium">Opportunities</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {byCartridge.map((r) => (
                  <TableRow key={r.cartridge_id}>
                    <TableCell className="py-1.5 pr-3 font-mono text-xs">
                      {r.cartridge_id}
                    </TableCell>
                    <TableCell className="py-1.5 text-right tabular-nums">
                      {r.n.toLocaleString()}
                    </TableCell>
                    <TableCell className="py-1.5 text-right tabular-nums">
                      {r.opportunities.toLocaleString()}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
        )}
      </div>

      {/* ── by pair (raw addresses, verbatim + short display) ──────────── */}
      <div className="space-y-2">
        <h4 className="text-sm font-semibold">
          By pair
          <span className="ml-2 text-xs font-normal text-muted-foreground">
            token_in → token_out (direcciones del wire; título = dirección completa)
          </span>
        </h4>
        {byPair.length === 0 ? (
          <p className="text-xs text-muted-foreground">
            Sin filas en la ventana {windowHours ?? DASH}h (cero honesto).
          </p>
        ) : (
          <div className="overflow-x-auto rounded-lg border">
            <Table>
              <TableHeader>
                <TableRow className="text-left text-muted-foreground">
                  <TableHead className="font-medium">Par</TableHead>
                  <TableHead className="text-right font-medium">Outcomes</TableHead>
                  <TableHead className="text-right font-medium">Opportunities</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {byPair.map((r) => (
                  <TableRow key={`${r.token_in}-${r.token_out}`}>
                    <TableCell
                      className="py-1.5 pr-3 font-mono text-xs"
                      title={`${r.token_in} → ${r.token_out}`}
                    >
                      {shortAddr(r.token_in)} → {shortAddr(r.token_out)}
                    </TableCell>
                    <TableCell className="py-1.5 text-right tabular-nums">
                      {r.n.toLocaleString()}
                    </TableCell>
                    <TableCell className="py-1.5 text-right tabular-nums">
                      {r.opportunities.toLocaleString()}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
        )}
      </div>

      {/* ── honest gaps — dimensions the sink does NOT persist ─────────── */}
      <div className="space-y-1 rounded-lg border border-dashed p-3">
        <p className="text-[10px] uppercase tracking-wide text-muted-foreground">
          No emitido en el sink (gaps honestos §47 — nivel-(b))
        </p>
        <ul className="list-disc space-y-0.5 pl-4 text-xs text-muted-foreground">
          <li>
            <strong>Por hop</strong> — la tabla no persiste hops por outcome;
            requiere emisión de columna, jamás un JOIN inventado.
          </li>
          <li>
            <strong>Por detector</strong> — detector_id no es columna del sink;
            vive en el catálogo estático (§25), no en el outcome row.
          </li>
          <li>
            <strong>Por DEX</strong> — el outcome row no lleva venue/DEX.
          </li>
        </ul>
        <p className="pt-1 text-[10px] text-muted-foreground">
          By chain vive en el panel Gate-C de arriba — misma ventana, mismo
          wire; no se duplica.
        </p>
      </div>
    </div>
  );
}

export function RouteOutcomesAnalyticsPanel() {
  const [hours, setHours] = React.useState(24);
  // FE-0038: distinct query window ⇒ a distinct poll is legitimate (it is
  // not the same datum twice); the panel owns its window selector.
  const { byCartridge, byPair, groupingsServed, windowHours, status, updatedAt, unavailableReason } =
    useRouteDiscoveryOutcomes(hours);
  const hasData = status === "LIVE";

  return (
    <Card data-slot="route-outcomes-analytics-panel">
      <CardHeader className="gap-3">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <CardTitle className="flex items-center gap-2 text-base">
            <LayersIcon className="h-5 w-5 text-primary" />
            Route Outcomes Analytics — §47
          </CardTitle>
          <div className="flex items-center gap-2">
            <Freshness at={updatedAt} />
            <StatusPill status={status} />
          </div>
        </div>
        <div className="flex flex-wrap items-center justify-between gap-2">
          <ReadOnlyBadge label="shadow-only · read-only sobre el sink de outcomes" />
          {windowHours !== null ? (
            <Badge variant="outline" className="font-mono text-[10px]">
              window {windowHours}h
            </Badge>
          ) : null}
          <div className="flex items-center gap-1">
            {WINDOWS.map((w) => (
              <Button
                key={w.hours}
                type="button"
                size="sm"
                variant={hours === w.hours ? "default" : "outline"}
                className="h-7 px-2.5 text-[11px]"
                onClick={() => setHours(w.hours)}
              >
                {w.label}
              </Button>
            ))}
          </div>
        </div>
      </CardHeader>
      <CardContent>
        {status === "STALE" && !hasData ? (
          <Alert variant="destructive">
            <AlertCircleIcon className="h-4 w-4" />
            <AlertTitle>Analytics series unavailable</AlertTitle>
            <AlertDescription className="text-sm">
              The outcomes summary could not be read
              {unavailableReason ? (
                <>
                  {" "}(<span className="font-mono">{unavailableReason}</span>)
                </>
              ) : null}
              . Polling /api/route-discovery-outcomes/summary every 8s — never a
              fabricated row (R8).
            </AlertDescription>
          </Alert>
        ) : (
          <OutcomesAnalyticsBody
            byCartridge={byCartridge}
            byPair={byPair}
            groupingsServed={groupingsServed}
            windowHours={windowHours}
          />
        )}
      </CardContent>
    </Card>
  );
}
