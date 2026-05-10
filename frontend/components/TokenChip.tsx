import React from "react";
import { DeterministicAvatar } from "@/components/DeterministicAvatar";
import { shortAddr } from "@/lib/format";

/**
 * TokenChip.tsx — Pure display component for a single token in an
 * opportunity row.
 *
 * 2026-05-10 layout refresh (operator request):
 *   Always show BOTH the symbol (when known) AND the truncated address.
 *   Previous behaviour collapsed to address-only when symbol was missing,
 *   which made it impossible to tell at a glance whether a row had been
 *   enriched. The new layout uses two stacked spans:
 *     line 1:  SYMBOL   (bold, foreground)
 *     line 2:  0xabc…def (mono, muted)
 *   When the enricher has no symbol yet, line 1 renders "—" so the operator
 *   sees that the metadata is pending rather than missing the token entirely.
 *
 * R1 Mounted Snapshot compliance:
 *   - No Date.now(), no Math.random(), no window/document, no hooks.
 *   - SSR render === CSR render. Zero hydration risk.
 *
 * R8 fail-honest branching (4 cases):
 *   A. info.logo_url present         → <img> + symbol + address.
 *   B. info present, no logo, symbol → DeterministicAvatar + symbol + address.
 *   C. info.resolved_via = "failed"  → DeterministicAvatar + "—" + address.
 *   D. info = null                   → DeterministicAvatar + "—" + address.
 */

/** Mirrors TokenInfoSchema from shared-ts/src/api-contracts.ts — no cross-package import. */
export interface TokenInfo {
  symbol: string | null;
  decimals: number | null;
  logo_url: string | null;
  resolved_via: "onchain_full" | "onchain_partial" | "trustwallet_only" | "failed";
}

export interface TokenChipProps {
  /** EVM address, 42-char hex. Used as DeterministicAvatar seed and shortAddr fallback. */
  token_address: string;
  /** Chain id — reserved for future chain-icon enrichment. Currently unused in render
   *  (chain identity surfaces via a sibling ChainBadge in the parent layout). */
  chain_id: number;
  /** Nullable per R8: enricher may not have resolved metadata yet. */
  info: TokenInfo | null;
}

function SymbolPlusAddress({
  avatar,
  symbol,
  token_address,
}: {
  avatar: React.ReactNode;
  symbol: string | null;
  token_address: string;
}) {
  const displaySymbol = symbol ?? "—";
  const symbolCls = symbol
    ? "text-foreground font-semibold"
    : "text-muted-foreground/70 italic";
  return (
    <span className="inline-flex items-center gap-2 min-w-0">
      {avatar}
      <span className="flex flex-col min-w-0 leading-tight">
        <span className={`text-xs ${symbolCls} truncate`} title={symbol ?? "metadata pending"}>
          {displaySymbol}
        </span>
        <span
          className="font-mono text-[10px] text-muted-foreground/80 truncate"
          title={token_address}
        >
          {shortAddr(token_address)}
        </span>
      </span>
    </span>
  );
}

export function TokenChip({ token_address, info }: TokenChipProps) {
  // Case D: enricher pending OR missing field (old API shape) — info not available.
  if (info == null) {
    return (
      <SymbolPlusAddress
        avatar={
          <DeterministicAvatar
            seed={token_address}
            className="size-6 rounded-full shrink-0"
          />
        }
        symbol={null}
        token_address={token_address}
      />
    );
  }

  const hasLogo = info.logo_url !== null && info.logo_url.length > 0;
  const hasSymbol = info.symbol !== null && info.symbol.length > 0;

  // Case A: logo available — render img with graceful onError fallback (browser-only).
  if (hasLogo) {
    const altLabel = hasSymbol ? info.symbol! : shortAddr(token_address);
    return (
      <SymbolPlusAddress
        avatar={
          <img
            src={info.logo_url!}
            alt={altLabel}
            width={24}
            height={24}
            className="size-6 rounded-full shrink-0 object-cover"
            onError={(e) => {
              (e.currentTarget as HTMLImageElement).style.display = "none";
            }}
          />
        }
        symbol={hasSymbol ? info.symbol : null}
        token_address={token_address}
      />
    );
  }

  // Case C: explicit failure OR Case B: symbol-only OR fallback.
  // All three share the same avatar+symbol+address layout; the only
  // difference is whether `symbol` is non-null.
  return (
    <SymbolPlusAddress
      avatar={
        <DeterministicAvatar
          seed={token_address}
          className="size-6 rounded-full shrink-0"
        />
      }
      symbol={hasSymbol ? info.symbol : null}
      token_address={token_address}
    />
  );
}
