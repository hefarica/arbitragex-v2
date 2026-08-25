/**
 * FE-MASTER · Pair detail drawer (FE-0019 — P5, §15/§16).
 *
 * Opens from a Pair Intelligence row: the full directed view of ONE pair —
 * the r15 invariant front and center (forward and reverse alpha are
 * independent computations that NEVER collapse; the two cards render side
 * by side even when one is null and the other is not).
 *
 * RULE 00 / §79: every value renders what a payload carries.
 *   - alpha cards: `alpha_forward` / `alpha_reverse` (EMIT-06b F_e) verbatim
 *     — bps is DISPLAY formatting of that number ((F_e−1)×10⁴), never a
 *     recomputed rate; null renders "—", never 0.
 *   - reserves ride as §62 decimal strings, untouched.
 *   - `hot` (HotSeed) is not published per pair today — the honest "no
 *     publicado (FE-0020)", never a fabricated state.
 *   - Quote context comes from the quote-anchor slice (EMIT-02); when the
 *     anchor is not served yet the drawer shows "—", it never guesses.
 *
 * `PairDetailBody` is the pure presentational core (SSR-testable without
 * the Radix portal); the Sheet wrapper owns open/close.
 */
"use client";

// SSR-test support (repo pattern, cf. TokenIcon/ChainsAdminClient).
import * as React from "react";
import { useEffect } from "react";

import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
  SheetDescription,
} from "@/components/ui/sheet";
import { shortAddr } from "@/lib/format";
import { useOmniStore } from "@/lib/store/omni-store";
import type { PairView } from "@/lib/apex/schemas";

const DASH = "—";

const feFmt = (v: number | null): string =>
  v === null || !Number.isFinite(v) ? DASH : v.toFixed(6);

/** Display-only bps form of the payload's F_e — (F_e−1)×10⁴, never a rate recomputation. */
const bpsFmt = (v: number | null): string =>
  v === null || !Number.isFinite(v) ? DASH : ((v - 1) * 10_000).toFixed(1);

/** Quote context the body renders — supplied by the wrapper from the slice. */
export interface QuoteContext {
  quote_symbol: string;
  quote_version: number;
  graph_version: number;
}

interface BodyProps {
  pair: PairView;
  quote: QuoteContext | null;
}

export function PairDetailBody({ pair, quote }: BodyProps) {
  const canonicalKey = `${pair.token_a.address}|${pair.token_b.address}`;
  return (
    <div className="space-y-4">
      {/* ── §15 identity: PairIndex + quote context ──────────────────── */}
      <div className="space-y-1">
        <p className="text-xs uppercase text-muted-foreground">PairIndex (canónico a|b)</p>
        <p className="break-all font-mono text-xs text-muted-foreground">{canonicalKey}</p>
      </div>
      <div className="flex flex-wrap items-center gap-2">
        {quote ? (
          <>
            <Badge variant="outline">quote {quote.quote_symbol}</Badge>
            <Badge variant="outline">quote_version {quote.quote_version}</Badge>
            <Badge variant="outline">graph_version {quote.graph_version}</Badge>
          </>
        ) : (
          <span className="text-xs text-muted-foreground">
            quote anchor {DASH} (sin snapshot servido)
          </span>
        )}
        <Badge variant={pair.dirty ? "destructive" : "secondary"}>
          {pair.dirty ? "DIRTY" : "CLEAN"}
        </Badge>
      </div>

      {/* ── r15: forward and reverse NEVER collapse ──────────────────── */}
      <div>
        <p className="mb-2 text-xs font-medium uppercase text-muted-foreground">
          α dirigido (r15 — direcciones independientes)
        </p>
        <div className="grid grid-cols-2 gap-3">
          <div className="rounded-lg border p-3">
            <p className="text-xs text-muted-foreground">
              forward {shortAddr(pair.token_a.address)}→{shortAddr(pair.token_b.address)}
            </p>
            <p className="text-lg font-semibold tabular-nums">{feFmt(pair.alpha_forward)}</p>
            <p className="text-xs text-muted-foreground tabular-nums">
              {bpsFmt(pair.alpha_forward)} bps
            </p>
          </div>
          <div className="rounded-lg border p-3">
            <p className="text-xs text-muted-foreground">
              reverse {shortAddr(pair.token_b.address)}→{shortAddr(pair.token_a.address)}
            </p>
            <p className="text-lg font-semibold tabular-nums">{feFmt(pair.alpha_reverse)}</p>
            <p className="text-xs text-muted-foreground tabular-nums">
              {bpsFmt(pair.alpha_reverse)} bps
            </p>
          </div>
        </div>
        <p className="mt-1 text-[11px] text-muted-foreground">
          F_e normalizado contra el anchor — reverse NUNCA es −forward. null = no computado (R8).
        </p>
      </div>

      <Separator />

      {/* ── §16 pools paralelos (full list, no dedupe across directions) ── */}
      <div>
        <p className="mb-2 text-xs font-medium uppercase text-muted-foreground">
          Pools paralelos ({pair.pools.length} · {pair.venue_count} venues)
        </p>
        {pair.pools.length === 0 ? (
          <p className="text-sm text-muted-foreground">{DASH}</p>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-xs">
              <thead>
                <tr className="border-b text-left text-muted-foreground">
                  <th className="py-1.5 pr-3 font-medium">Pool</th>
                  <th className="py-1.5 pr-3 font-medium">Venue</th>
                  <th className="py-1.5 pr-3 text-right font-medium">Fee bps</th>
                  <th className="py-1.5 pr-3 text-right font-medium">Reserves a (§62)</th>
                  <th className="py-1.5 text-right font-medium">Reserves b (§62)</th>
                </tr>
              </thead>
              <tbody>
                {pair.pools.map((p) => (
                  <tr key={p.pool_address} className="border-b last:border-0">
                    <td className="py-1.5 pr-3 font-mono text-muted-foreground">
                      {shortAddr(p.pool_address)}
                    </td>
                    <td className="py-1.5 pr-3">{p.venue}</td>
                    <td className="py-1.5 pr-3 text-right tabular-nums">{p.fee_bps}</td>
                    <td className="py-1.5 pr-3 text-right font-mono tabular-nums">{p.reserves_a}</td>
                    <td className="py-1.5 text-right font-mono tabular-nums">{p.reserves_b}</td>
                  </tr>
                ))}
              </tbody>
            </table>
            <p className="mt-1 text-[11px] text-muted-foreground">
              Reservas sobre las patas canónicas (address-asc) — decimal strings §62 verbatim.
            </p>
          </div>
        )}
      </div>

      <Separator />

      {/* ── Estado: dirty + freshness + hot (honest FE-0020 gap) ─────── */}
      <div className="grid gap-2 text-xs sm:grid-cols-3">
        <div>
          <p className="text-muted-foreground">Dirty (SET no drenado)</p>
          <p className="font-medium">{pair.dirty ? "DIRTY" : "CLEAN"}</p>
        </div>
        <div>
          <p className="text-muted-foreground">Last reserve update</p>
          <p className="font-medium">
            {pair.last_reserve_update === null
              ? DASH
              : new Date(pair.last_reserve_update).toLocaleString()}
          </p>
        </div>
        <div>
          <p className="text-muted-foreground">Hot seed</p>
          <p className="font-medium">{DASH} no publicado (FE-0020)</p>
        </div>
      </div>
    </div>
  );
}

interface Props {
  pair: PairView | null;
  onClose: () => void;
}

export function PairDetailDrawer({ pair, onClose }: Props) {
  // Quote context from the anchor slice — same store the Quote/Base panel
  // feeds; when absent the body renders its honest "—" (never a guess).
  const anchor = useOmniStore((s) => s.quoteAnchor);
  const quote: QuoteContext | null = anchor
    ? {
        quote_symbol: anchor.quote_symbol,
        quote_version: anchor.quote_version,
        graph_version: anchor.graph_version,
      }
    : null;

  // Close on Escape is Radix-native; this keeps the pair state in sync when
  // the open prop flips so a reopen never shows the previous pair.
  useEffect(() => {
    if (pair === null) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [pair, onClose]);

  return (
    <Sheet open={pair !== null} onOpenChange={(open) => !open && onClose()}>
      <SheetContent side="right" className="w-full overflow-y-auto sm:max-w-lg">
        <SheetHeader>
          <SheetTitle className="font-mono text-base">
            {pair ? `${pair.token_a.symbol}/${pair.token_b.symbol}` : DASH}
          </SheetTitle>
          <SheetDescription>
            Detalle dirigido del par — chain {pair?.chain_id ?? DASH}
          </SheetDescription>
        </SheetHeader>
        {pair && <PairDetailBody pair={pair} quote={quote} />}
      </SheetContent>
    </Sheet>
  );
}
