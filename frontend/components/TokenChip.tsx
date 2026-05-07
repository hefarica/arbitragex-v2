import React from "react";
import { DeterministicAvatar } from "@/components/DeterministicAvatar";
import { shortAddr } from "@/lib/format";

/**
 * TokenChip.tsx — Pure display component for a single token in an opportunity row.
 *
 * R1 Mounted Snapshot compliance:
 *   - No Date.now(), no Math.random(), no window/document, no hooks.
 *   - SSR render === CSR render. Zero hydration risk.
 *
 * R8 fail-honest branching (4 cases):
 *   A. info.logo_url present         → <img> + symbol label. onError hides img (browser-only, untestable in node env).
 *   B. info present, no logo, symbol → DeterministicAvatar + symbol label.
 *   C. info.resolved_via = "failed"  → DeterministicAvatar + shortAddr (all metadata null).
 *   D. info = null                   → DeterministicAvatar + shortAddr (enricher pending, R8).
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
  /** Chain id — reserved for future chain-icon enrichment. Currently unused in render. */
  chain_id: number;
  /** Nullable per R8: enricher may not have resolved metadata yet. */
  info: TokenInfo | null;
}

export function TokenChip({ token_address, info }: TokenChipProps) {
  // Case D: enricher pending OR missing field (old API shape) — info not available.
  // Use loose equality (==) to catch BOTH null AND undefined defensively.
  if (info == null) {
    return (
      <span className="inline-flex items-center gap-1.5">
        <DeterministicAvatar seed={token_address} className="size-5 rounded-full shrink-0" />
        <span className="font-mono text-xs text-slate-400">{shortAddr(token_address)}</span>
      </span>
    );
  }

  const hasLogo = info.logo_url !== null && info.logo_url.length > 0;
  const hasSymbol = info.symbol !== null && info.symbol.length > 0;

  // Case A: logo available — render img with graceful onError fallback (browser-only).
  if (hasLogo) {
    const label = hasSymbol ? info.symbol! : shortAddr(token_address);
    return (
      <span className="inline-flex items-center gap-1.5">
        {/* onError hides broken img; browser-only behavior, untestable in node env (see Task 13). */}
        <img
          src={info.logo_url!}
          alt={label}
          width={20}
          height={20}
          className="size-5 rounded-full shrink-0 object-cover"
          onError={(e) => {
            (e.currentTarget as HTMLImageElement).style.display = "none";
          }}
        />
        <span className="font-mono text-xs text-slate-200">{label}</span>
      </span>
    );
  }

  // Case C: explicit failure — all metadata unavailable.
  if (info.resolved_via === "failed") {
    return (
      <span className="inline-flex items-center gap-1.5">
        <DeterministicAvatar seed={token_address} className="size-5 rounded-full shrink-0" />
        <span className="font-mono text-xs text-slate-400">{shortAddr(token_address)}</span>
      </span>
    );
  }

  // Case B: no logo but symbol present (partial resolution).
  if (hasSymbol) {
    return (
      <span className="inline-flex items-center gap-1.5">
        <DeterministicAvatar seed={token_address} className="size-5 rounded-full shrink-0" />
        <span className="font-mono text-xs text-slate-200">{info.symbol!}</span>
      </span>
    );
  }

  // Fallback: no logo, no symbol (e.g. onchain_partial with empty metadata).
  return (
    <span className="inline-flex items-center gap-1.5">
      <DeterministicAvatar seed={token_address} className="size-5 rounded-full shrink-0" />
      <span className="font-mono text-xs text-slate-400">{shortAddr(token_address)}</span>
    </span>
  );
}
