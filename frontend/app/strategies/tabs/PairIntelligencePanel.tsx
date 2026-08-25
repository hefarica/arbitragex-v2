/**
 * FE-MASTER · Pair Intelligence panel (FE-0017 — P5, workbook 05 §13/§14).
 *
 * The pair table over REAL runtime data (EMIT-06 → GET /api/pairs, PairsSlice
 * of the omni-store): one row per canonical {A,B} pair of the effective
 * universe (backend-fixed address-asc order — §13: dedupe/dir is the
 * backend's job, the panel renders payload order).
 *
 * RULE 00 / §79: every cell renders what the payload carries. alpha_forward
 * / alpha_reverse are NULL in v1 (PairAlpha r15 is not published until
 * EMIT-06b) → the honest "—" (R8), NEVER a spot spread recomputed from
 * reserves. fee_bps cells join the per-pool fees the payload already lists.
 *
 * §12 deep-links: Route Discovery links here as `#pair=<aAddr>-<bAddr>` —
 * the matching row scrolls into view and highlights.
 *
 * FE-0019 (pair drawer) and FE-0020 (state machine viz CLEAN/DIRTY/QUEUED/
 * HOT/…) layer ON this table later; today `dirty` renders as the binary
 * badge the payload carries.
 *
 * FE-0008: refresh cadence lives in the root ArbxRealtimeProvider (WS push
 * + REST poll) — this panel no longer polls on its own; only the Refresh
 * button and the chain-≠1 back-fill fetch manually.
 */
"use client";

// SSR-test support (repo pattern, cf. TokenIcon/ChainsAdminClient): the node
// test env renders this panel via react-dom/server with jsx preserved, so
// React must be in module scope.
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
import { TokenPairIcon } from "@/components/ui/TokenIcon";
import { shortAddr } from "@/lib/format";
import { useOmniStore } from "@/lib/store/omni-store";
import type { PairView } from "@/lib/apex/schemas";

import { PairAlphaHeatmap } from "./PairAlphaHeatmap";
import { PairDetailDrawer } from "./PairDetailDrawer";
import { PairStateFlowPanel } from "./PairStateFlowPanel";

const DASH = "—";

/** Row identity == the deep-link key (§12): canonical `aAddr-bAddr`. */
export const pairKey = (p: PairView): string => `${p.token_a.address}-${p.token_b.address}`;

/** Distinct per-pool fee_bps joined for display — payload values, no math. */
export const feeList = (p: PairView): string => {
  const fees = Array.from(new Set(p.pools.map((pool) => pool.fee_bps)));
  return fees.length > 0 ? fees.sort((x, y) => x - y).join(" · ") : DASH;
};

interface Props {
  chainId: number;
}

export function PairIntelligencePanel({ chainId }: Props) {
  const pairs = useOmniStore((s) => s.pairs);
  const status = useOmniStore((s) => s.pairsStatus);
  const error = useOmniStore((s) => s.pairsError);
  const updatedAt = useOmniStore((s) => s.pairsUpdatedAt);
  const fetchPairs = useOmniStore((s) => s.fetchPairs);
  // FE-0020: the §17 state flow rides the SAME cadence as the table (the
  // tick snapshot instruments Event→…→HotSeed; its fetch has its own guard).
  const tick = useOmniStore((s) => s.tick);
  const tickError = useOmniStore((s) => s.tickError);
  const fetchTick = useOmniStore((s) => s.fetchTick);

  // FE-0019: the selected pair opens the detail drawer (directed view).
  const [selected, setSelected] = useState<PairView | null>(null);

  // FE-0008 strip (2026-08-24): the cadence belongs to the root realtime
  // provider (REST initial pass + 30s interval; tick via WS push with poll
  // fallback). This panel only back-fills the UNCOMMON chain — the provider's
  // initial pass fetches chain 1; a differently-configured chain would
  // otherwise never load here.
  useEffect(() => {
    if (chainId !== 1) {
      void fetchPairs(chainId);
      void fetchTick(chainId);
    }
  }, [chainId, fetchPairs, fetchTick]);

  // Client-side presentation filter (symbol substring) over payload rows —
  // order is never recomputed, only subsetting for readability.
  const [query, setQuery] = useState("");
  const filtered = useMemo(() => {
    if (!pairs) return null;
    const q = query.trim().toUpperCase();
    if (!q) return pairs;
    return pairs.filter(
      (p) =>
        p.token_a.symbol.toUpperCase().includes(q) ||
        p.token_b.symbol.toUpperCase().includes(q),
    );
  }, [pairs, query]);

  // §12 deep-link target: `#pair=<aAddr>-<bAddr>` scrolls + highlights.
  const [highlighted, setHighlighted] = useState<string | null>(null);
  useEffect(() => {
    const sync = () => {
      const hash = window.location.hash;
      const key = hash.startsWith("#pair=") ? hash.slice("#pair=".length) : null;
      setHighlighted(key);
      if (key) {
        window.requestAnimationFrame(() => {
          document.getElementById(`pair-row-${key}`)?.scrollIntoView({ block: "center" });
        });
      }
    };
    sync();
    window.addEventListener("hashchange", sync);
    return () => window.removeEventListener("hashchange", sync);
  }, []);

  const dirtyCount = useMemo(
    () => pairs?.filter((p) => p.dirty).length ?? 0,
    [pairs],
  );

  return (
    <div className="space-y-4">
    <Card>
      <CardHeader className="flex flex-row flex-wrap items-center justify-between gap-2 space-y-0">
        <CardTitle className="text-base">
          Pair Intelligence · universo efectivo (§13)
          {pairs && (
            <span className="ml-2 text-sm font-normal text-muted-foreground">
              {pairs.length} pares · {dirtyCount} dirty
            </span>
          )}
        </CardTitle>
        <div className="flex items-center gap-2 text-xs text-muted-foreground">
          {updatedAt && <span>updated {updatedAt}</span>}
          <Input
            aria-label="Filtrar por símbolo"
            placeholder="Filtrar símbolo…"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            className="h-8 w-40"
          />
          <Button
            variant="outline"
            size="sm"
            onClick={() => {
              void fetchPairs(chainId);
              void fetchTick(chainId);
            }}
            disabled={status === "loading"}
          >
            Refresh
          </Button>
        </div>
      </CardHeader>
      <CardContent>
        {status === "error" && (
          <p className="text-sm text-destructive" role="alert">
            {error ?? "pairs unavailable"}
          </p>
        )}
        {status !== "error" && !pairs && <p className="text-sm text-muted-foreground">{DASH}</p>}
        {pairs && pairs.length === 0 && (
          <p className="text-sm text-muted-foreground">
            Sin pares activos con reserves para la chain {chainId} — universo
            efectivo vacío (R8: el cero es honesto, no un error).
          </p>
        )}
        {pairs && pairs.length > 0 && filtered && filtered.length === 0 && (
          <p className="text-sm text-muted-foreground">
            Ningún par coincide con “{query.trim()}”.
          </p>
        )}
        {filtered && filtered.length > 0 && (
          <div className="overflow-x-auto">
            <Table>
              <TableHeader>
                <TableRow className="text-left text-muted-foreground">
                  <TableHead className="font-medium">Par (A→B canónico)</TableHead>
                  <TableHead className="text-right font-medium">Venues</TableHead>
                  <TableHead className="text-right font-medium">Pools</TableHead>
                  <TableHead className="text-right font-medium">Fee bps</TableHead>
                  <TableHead
                    className="text-right font-medium"
                    title="PairAlpha r15 — se publica con EMIT-06b; null = no computado (R8)"
                  >
                    α fwd
                  </TableHead>
                  <TableHead
                    className="text-right font-medium"
                    title="NUNCA = −forward (r15): se publica con EMIT-06b"
                  >
                    α rev
                  </TableHead>
                  <TableHead className="text-center font-medium">Estado</TableHead>
                  <TableHead className="text-right font-medium">Last reserve</TableHead>
                  <TableHead className="w-20 font-medium" aria-label="Acciones">
                    <span className="sr-only">Acciones</span>
                  </TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {filtered.map((p) => {
                  const key = pairKey(p);
                  return (
                    <TableRow
                      key={key}
                      id={`pair-row-${key}`}
                      className={highlighted === key ? "bg-muted/60" : undefined}
                    >
                      <TableCell className="font-medium">
                        <div className="flex items-center gap-2 min-w-0">
                          <TokenPairIcon
                            tokenInAddress={p.token_a.address}
                            tokenOutAddress={p.token_b.address}
                            tokenInSymbol={p.token_a.symbol}
                            tokenOutSymbol={p.token_b.symbol}
                            chainId={p.chain_id}
                            size={22}
                          />
                          <span
                            className="truncate"
                            title={`${p.token_a.address} / ${p.token_b.address}`}
                          >
                            {p.token_a.symbol}/{p.token_b.symbol}
                          </span>
                          <span className="font-mono text-[10px] text-muted-foreground">
                            {shortAddr(p.token_a.address)}·{shortAddr(p.token_b.address)}
                          </span>
                        </div>
                      </TableCell>
                      <TableCell className="text-right tabular-nums">{p.venue_count}</TableCell>
                      <TableCell className="text-right tabular-nums">{p.pools.length}</TableCell>
                      <TableCell className="text-right tabular-nums">{feeList(p)}</TableCell>
                      <TableCell className="text-right tabular-nums text-muted-foreground">
                        {p.alpha_forward === null ? DASH : p.alpha_forward.toFixed(4)}
                      </TableCell>
                      <TableCell className="text-right tabular-nums text-muted-foreground">
                        {p.alpha_reverse === null ? DASH : p.alpha_reverse.toFixed(4)}
                      </TableCell>
                      <TableCell className="text-center">
                        <Badge variant={p.dirty ? "destructive" : "secondary"}>
                          {p.dirty ? "DIRTY" : "CLEAN"}
                        </Badge>
                      </TableCell>
                      <TableCell className="text-right tabular-nums text-muted-foreground">
                        {p.last_reserve_update === null
                          ? DASH
                          : new Date(p.last_reserve_update).toLocaleTimeString()}
                      </TableCell>
                      <TableCell>
                        <Button
                          variant="ghost"
                          size="sm"
                          aria-label={`Detalles ${p.token_a.symbol}/${p.token_b.symbol}`}
                          aria-haspopup="dialog"
                          onClick={() => setSelected(p)}
                        >
                          Detalles
                        </Button>
                      </TableCell>
                    </TableRow>
                  );
                })}
              </TableBody>
            </Table>
          </div>
        )}
      </CardContent>
      {/* FE-0019: directed pair detail (§15/§16) — r15 forward/reverse
          never collapse; pools listed in full, reserves §62 verbatim. */}
      <PairDetailDrawer pair={selected} onClose={() => setSelected(null)} />
    </Card>
    {/* FE-0020: §17 lifecycle state flow — tick counters verbatim + live
        CLEAN/DIRTY census; QUEUED/HOT/EXPANDING/COOLED stay honest gaps
        until EMIT-06c publishes the pair_state enum. */}
    <PairStateFlowPanel pairs={pairs} tick={tick} tickError={tickError} />
    {/* FE-0018: directed α heatmap over the SAME filtered set — the symbol
        filter owns the matrix dimension (honest, never a silent crop). */}
    {filtered !== null && <PairAlphaHeatmap pairs={filtered} />}
    </div>
  );
}
