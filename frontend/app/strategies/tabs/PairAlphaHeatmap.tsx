/**
 * FE-MASTER · Pair alpha heatmap (FE-0018 — P5, §14).
 *
 * Token×token matrix of the directed net prefilter signal: cell[i][j] =
 * (F_e(i→j) − 1)×10⁴ bps, the display form of the payload's `alpha_forward`/
 * `alpha_reverse` (EMIT-06b). NOT a spot spread: reserves are never touched
 * and no rate is recomputed (§79 — bps is unit formatting of the payload
 * number, exactly the form agreed on the wire review).
 *
 * r15 by construction: (i,j) and (j,i) are DIFFERENT cells fed by DIFFERENT
 * payload fields — the two triangles of the diagonal never collapse.
 *
 * RULE 00 / R8: null alpha renders an empty cell ("·"), never 0; when NO
 * pair carries a computed alpha (fe_prefilter OFF / TTL lapsed) the whole
 * matrix is replaced by the honest no-data message — an all-gray grid would
 * read as "all zero", which would be a fabrication.
 *
 * Dimension honesty: the matrix spans the tokens of the FILTERED pair set
 * (the panel's symbol filter subsets it — the operator controls the size).
 * Beyond MAX_DIMENSION tokens the matrix is not rendered at all: a cropped
 * 40×40 of a 300-token universe would silently misrepresent coverage.
 *
 * Identity by ADDRESS (R3 7b fidelity note, folded 2026-08-24): axes and
 * cells key on the token address — the ONLY identity PG guarantees. Symbols
 * are display labels; when two distinct addresses share a symbol on the
 * same chain (the scam-token clone pattern) each keeps its OWN axis,
 * disambiguated as `SYM·<short>`, and never collapse into one cell.
 */
"use client";

// SSR-test support (repo pattern, cf. TokenIcon/ChainsAdminClient).
import * as React from "react";
import { useMemo } from "react";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { shortAddr } from "@/lib/format";
import type { PairView } from "@/lib/apex/schemas";

/** Beyond this token count the matrix is honestly refused (see header). */
const MAX_DIMENSION = 40;

/** Display-only bps form of the payload's F_e — (F_e−1)×10⁴, never a rate recomputation. */
const cellBps = (fe: number): number => (fe - 1) * 10_000;

/** Color bucket by |bps| — the sign picks the hue, the magnitude the depth. */
function cellClass(bps: number): string {
  const a = Math.abs(bps);
  if (bps > 0) {
    if (a >= 20) return "bg-emerald-600/70 text-white";
    if (a >= 5) return "bg-emerald-500/50 text-emerald-950";
    return "bg-emerald-400/25 text-emerald-900";
  }
  if (a >= 20) return "bg-rose-600/50 text-white";
  if (a >= 5) return "bg-rose-500/30 text-rose-950";
  return "bg-muted text-muted-foreground";
}

interface Props {
  /** The SAME filtered set the table renders (filter owns the dimension). */
  pairs: PairView[];
}

export function PairAlphaHeatmap({ pairs }: Props) {
  const model = useMemo(() => {
    // Token universe in payload first-seen order keyed by ADDRESS (the only
    // identity the registry guarantees); symbols are display labels and get
    // a shortAddr suffix when the same symbol names several addresses.
    const addrOf: string[] = [];
    const addrSeen = new Set<string>();
    const symbolOf = new Map<string, string>();
    const cells = new Map<string, number>();
    let anyComputed = false;
    for (const p of pairs) {
      for (const [addr, sym] of [
        [p.token_a.address, p.token_a.symbol],
        [p.token_b.address, p.token_b.symbol],
      ] as const) {
        if (!symbolOf.has(addr)) symbolOf.set(addr, sym);
        if (!addrSeen.has(addr)) {
          addrSeen.add(addr);
          addrOf.push(addr);
        }
      }
      if (p.alpha_forward !== null && Number.isFinite(p.alpha_forward)) {
        cells.set(`${p.token_a.address}|${p.token_b.address}`, cellBps(p.alpha_forward));
        anyComputed = true;
      }
      if (p.alpha_reverse !== null && Number.isFinite(p.alpha_reverse)) {
        cells.set(`${p.token_b.address}|${p.token_a.address}`, cellBps(p.alpha_reverse));
        anyComputed = true;
      }
    }
    const symFreq = new Map<string, number>();
    for (const sym of symbolOf.values()) symFreq.set(sym, (symFreq.get(sym) ?? 0) + 1);
    const labelOf = (addr: string): string => {
      const sym = symbolOf.get(addr) ?? addr;
      return (symFreq.get(sym) ?? 0) > 1 ? `${sym}·${shortAddr(addr)}` : sym;
    };
    return { addrOf, labelOf, cells, anyComputed };
  }, [pairs]);

  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-base">
          Heatmap α dirigido (§14) · net prefilter bps
          <span className="ml-2 text-sm font-normal text-muted-foreground">
            {model.addrOf.length} tokens · {model.cells.size} celdas computadas
          </span>
        </CardTitle>
      </CardHeader>
      <CardContent>
        {model.addrOf.length === 0 && (
          <p className="text-sm text-muted-foreground">—</p>
        )}
        {model.addrOf.length > 0 && !model.anyComputed && (
          <p className="text-sm text-muted-foreground">
            Ningún par con α computado (fe_prefilter OFF o TTL lapsed) — la
            matriz no se fabrica (R8). Enciende el knob del lado searcher para
            poblarla.
          </p>
        )}
        {model.addrOf.length > MAX_DIMENSION && (
          <p className="text-sm text-muted-foreground">
            {model.addrOf.length} tokens exceden la dimensión renderizable
            (≤{MAX_DIMENSION}) — afina el filtro por símbolo: la matriz cubre
            exactamente lo filtrado, nunca un recorte silencioso.
          </p>
        )}
        {model.addrOf.length > 0 &&
          model.addrOf.length <= MAX_DIMENSION &&
          model.anyComputed && (
            <div className="overflow-x-auto">
              <table className="border-collapse text-[10px] tabular-nums">
                <thead>
                  <tr>
                    <th className="sticky left-0 z-10 bg-card p-1" aria-label="origen → destino" />
                    {model.addrOf.map((a) => (
                      <th key={a} className="p-1 font-medium text-muted-foreground" title={a}>
                        <span className="inline-block max-w-14 truncate">{model.labelOf(a)}</span>
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {model.addrOf.map((rowAddr) => (
                    <tr key={rowAddr}>
                      <th
                        className="sticky left-0 z-10 bg-card p-1 pr-2 font-medium text-muted-foreground"
                        title={rowAddr}
                      >
                        <span className="inline-block max-w-14 truncate">{model.labelOf(rowAddr)}</span>
                      </th>
                      {model.addrOf.map((colAddr) => {
                        if (rowAddr === colAddr) {
                          return (
                            <td key={colAddr} className="border border-border/40 p-0" aria-label="diagonal" />
                          );
                        }
                        const bps = model.cells.get(`${rowAddr}|${colAddr}`);
                        return (
                          <td
                            key={colAddr}
                            title={`${model.labelOf(rowAddr)}→${model.labelOf(colAddr)}: ${bps === undefined ? "no computado" : `${bps.toFixed(1)} bps`}`}
                            className={`border border-border/40 px-1.5 py-0.5 text-center ${bps === undefined ? "text-muted-foreground/40" : cellClass(bps)}`}
                          >
                            {bps === undefined ? "·" : bps.toFixed(1)}
                          </td>
                        );
                      })}
                    </tr>
                  ))}
                </tbody>
              </table>
              <p className="mt-2 text-[11px] text-muted-foreground">
                Fila → columna = dirección dirigida (r15: (i,j) y (j,i) son celdas
                independientes). Identidad por address (símbolos repetidos se
                desambiguan ·short). Verde = supera la referencia del anchor;
                valor = (F_e−1)×10⁴ del payload, jamás un spot spread
                recomputado (§79).
              </p>
            </div>
          )}
      </CardContent>
    </Card>
  );
}
