import React from "react";
import { DeterministicAvatar } from "@/components/DeterministicAvatar";
import { TokenIcon } from "@/components/ui/TokenIcon";
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
 * R8 fail-honest branching:
 *   A. info.logo_url present  → <img> layered OVER a DeterministicAvatar base.
 *      The logo shows when the external URL loads; if it ever fails (network
 *      hiccup, ad-blocker, 404, transient) the avatar is revealed underneath —
 *      a logo/avatar ALWAYS renders, never a blank gap (logos never "disappear").
 *   B. no payload logo        → <TokenIcon> resolves via the token-icon route
 *      cascade (edge → api-server: Redis → Registry → PG logo_url → DexScreener
 *      → avatar), filling the long-tail gap the payload's TrustWallet-only logos
 *      leave. TokenIcon owns its own R1-safe client resolution + avatar fallback.
 */

/**
 * Token Validation Engine Phase 1 — composite multi-validator output.
 * Replaces the binary verified/unverified pill with one of seven statuses.
 */
export interface TokenValidationBlock {
  status:
    | "VERIFIED"
    | "VIABLE"
    | "LOW_LIQUIDITY"
    | "ILLIQUID"
    | "NO_DATA"
    | "INVALID"
    | "PENDING";
  score: number; // 0-100
  liquidity_usd: number | null;
  volume_24h_usd: number | null;
  pair_count: number | null;
  primary_dex: string | null;
  registry_source: string | null;
  validated_at: string;
  reasons:
    | Array<{ key: string; delta: number; note: string }>
    | null;
}

/** Mirrors TokenInfoSchema from shared-ts/src/api-contracts.ts — no cross-package import. */
export interface TokenInfo {
  symbol: string | null;
  decimals: number | null;
  logo_url: string | null;
  resolved_via: "onchain_full" | "onchain_partial" | "trustwallet_only" | "failed";
  /**
   * Curated-registry verification flag set by the api-server. Optional for
   * backward compatibility with payloads emitted before 2026-05-11.
   *   true  → in Uniswap default token list AND on-chain symbol matches.
   *   false → either unknown address OR symbol-mismatch (impersonation).
   *   undefined → field not present on older payloads (treat as unknown).
   */
  verified?: boolean;
  registry_symbol?: string | null;
  registry_name?: string | null;
  verified_notes?: string[] | null;
  /**
   * Phase 1 Token Validation Engine result. When present, supersedes the
   * binary `verified` flag for rendering: the badge color + label come
   * from `validation.status` + `validation.score`. Null when the validator
   * hasn't run yet (cold start; populated by next refresh).
   */
  validation?: TokenValidationBlock | null;
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

/**
 * Format USD shortened with k/M suffix. Used in validation badges to show
 * liquidity amount inline ("$1.2M", "$50k", "$0.01").
 */
function formatUsdCompact(v: number | null | undefined): string {
  if (v == null || !Number.isFinite(v)) return "—";
  const abs = Math.abs(v);
  if (abs >= 1_000_000) return `$${(v / 1_000_000).toFixed(2)}M`;
  if (abs >= 1_000)     return `$${(v / 1_000).toFixed(1)}k`;
  if (abs >= 1)         return `$${v.toFixed(2)}`;
  if (abs >= 0.01)      return `$${v.toFixed(2)}`;
  return `$${v.toFixed(4)}`;
}

interface BadgeSpec {
  label: string;
  className: string;
  emoji: string;
  title: string;
}

/**
 * Resolve which badge to render for a given TokenInfo. Priority:
 *   1. Validation Engine status when present (covers all 7 cases).
 *   2. Legacy `verified` boolean as fallback (pre-validation-engine payloads).
 *   3. Unknown (no badge).
 */
function resolveBadge(info: TokenInfo | null | undefined, symbol: string | null): BadgeSpec | null {
  const v = info?.validation;
  if (v) {
    // The Validation Engine has run. Use its status + score directly.
    const liqLabel = v.liquidity_usd != null ? ` ${formatUsdCompact(v.liquidity_usd)}` : "";
    const topReason = v.reasons && v.reasons.length > 0 ? v.reasons[0]!.note : "";
    switch (v.status) {
      case "VERIFIED":
        return {
          label: "✓",
          emoji: "✓",
          className: "bg-success/15 text-success border-success/40",
          title:
            `VERIFIED (score ${v.score}) — ${info?.registry_name ?? info?.registry_symbol ?? "in curated registry"}`
            + (v.registry_source ? ` · source: ${v.registry_source}` : ""),
        };
      case "VIABLE":
        return {
          label: `LIQ${liqLabel}`,
          emoji: "💧",
          className: "bg-info/15 text-info border-info/40",
          title:
            `VIABLE (score ${v.score}) — has real market activity` +
            (v.liquidity_usd != null ? ` · liquidity ${formatUsdCompact(v.liquidity_usd)}` : "") +
            (v.volume_24h_usd != null ? ` · 24h vol ${formatUsdCompact(v.volume_24h_usd)}` : "") +
            ` · ${v.pair_count ?? 0} pair(s)`,
        };
      case "LOW_LIQUIDITY":
        return {
          label: `LOW${liqLabel}`,
          emoji: "⚠",
          className: "bg-warning/15 text-warning border-warning/40",
          title:
            `LOW LIQUIDITY (score ${v.score}) — below trading threshold` +
            (v.liquidity_usd != null ? ` · liquidity ${formatUsdCompact(v.liquidity_usd)} < $5,000` : ""),
        };
      case "ILLIQUID":
        return {
          label: `ILLIQ${liqLabel}`,
          emoji: "🚫",
          className: "bg-destructive/15 text-destructive border-destructive/40",
          title:
            `ILLIQUID (score ${v.score}) — likely scam / wash trading / rugpull` +
            (v.liquidity_usd != null ? ` · liquidity ${formatUsdCompact(v.liquidity_usd)}` : "") +
            (topReason ? ` · ${topReason}` : ""),
        };
      case "NO_DATA":
        return {
          label: "NO MARKET",
          emoji: "❓",
          className: "bg-muted/40 text-muted-foreground border-border/60",
          title: `NO_DATA (score ${v.score}) — no DEX pair indexed; token might be brand-new or unlisted`,
        };
      case "INVALID":
        return {
          label: "NOT ERC20",
          emoji: "⛔",
          className: "bg-destructive/25 text-destructive border-destructive/60",
          title: `INVALID — address has no contract bytecode OR does not implement the ERC-20 standard`,
        };
      case "PENDING":
        return {
          label: "validating…",
          emoji: "⏳",
          className: "bg-muted/30 text-muted-foreground/80 border-border/40 italic",
          title: "PENDING — validation queued; result will appear on next refresh",
        };
    }
  }

  // Legacy fallback: payload predates the validation engine (no `validation`
  // field). Use the binary `verified` flag.
  if (info?.verified === true) {
    return {
      label: "✓",
      emoji: "✓",
      className: "bg-success/15 text-success border-success/40",
      title: info?.registry_name
        ? `Verified — ${info.registry_name} (${info.registry_symbol ?? symbol})`
        : "Verified by curated registry",
    };
  }
  if (info?.verified === false) {
    const isMismatch = info?.verified_notes?.includes("symbol-mismatch");
    return {
      label: "⚠ unverified",
      emoji: "⚠",
      className: "bg-destructive/15 text-destructive border-destructive/40",
      title: isMismatch
        ? `⚠ IMPERSONATION RISK — contract claims "${symbol}" but the canonical token at this address is "${info?.registry_symbol ?? "unknown"}". Do NOT trade.`
        : "⚠ UNVERIFIED — address not in curated registry.",
    };
  }
  return null;
}

function SymbolPlusAddress({
  avatar,
  symbol,
  token_address,
  info,
}: {
  avatar: React.ReactNode;
  symbol: string | null;
  token_address: string;
  /** Optional — when present, drives the validation badge. */
  info?: TokenInfo | null;
}) {
  const displaySymbol = symbol ?? "—";
  const badge = resolveBadge(info, symbol);

  // Symbol styling: green/normal for VERIFIED/VIABLE; warning/italic for
  // LOW_LIQUIDITY/PENDING; destructive/italic for ILLIQUID/INVALID/unverified.
  const status = info?.validation?.status;
  const symbolCls = symbol
    ? status === "VERIFIED" || status === "VIABLE"
      ? "text-foreground font-semibold"
      : status === "INVALID" || status === "ILLIQUID"
        ? "text-destructive font-semibold italic"
        : status === "LOW_LIQUIDITY"
          ? "text-warning font-semibold italic"
          : status === "NO_DATA"
            ? "text-muted-foreground font-semibold italic"
            : status === "PENDING"
              ? "text-muted-foreground/70 italic"
              // Legacy fallback path (no validation block)
              : info?.verified === false
                ? "text-muted-foreground/90 font-semibold italic"
                : "text-foreground font-semibold"
    : "text-muted-foreground/70 italic";

  return (
    <span className="inline-flex items-center gap-2 min-w-0">
      {avatar}
      <span className="flex flex-col min-w-0 leading-tight">
        <span className="flex items-center gap-1 min-w-0 flex-wrap">
          <span className={`text-xs ${symbolCls} truncate`} title={badge?.title ?? symbol ?? "metadata pending"}>
            {displaySymbol}
          </span>
          {badge && (
            <span
              className={`text-[8px] font-bold px-1 py-px rounded border uppercase tracking-wider shrink-0 ${badge.className}`}
              title={badge.title}
              aria-label={badge.title}
            >
              {badge.label}
            </span>
          )}
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

export function TokenChip({ token_address, chain_id, info }: TokenChipProps) {
  const hasLogo = info?.logo_url != null && info.logo_url.length > 0;
  const hasSymbol = info?.symbol != null && info.symbol.length > 0;
  // F1 (audit §11 RC2): registry_symbol is REAL curated-list data resolved
  // by the api-server for every token — surface it before rendering "—" so
  // long-tail tokens still show their code. (The impersonation case always
  // has a non-null on-chain symbol, so this fallback never masks it.)
  const hasRegistrySymbol =
    !hasSymbol && info?.registry_symbol != null && info.registry_symbol.length > 0;
  const symbol = hasSymbol
    ? info!.symbol
    : hasRegistrySymbol
      ? info!.registry_symbol!
      : null;

  // Case A: payload logo — <img> layered over a DeterministicAvatar base. The
  // logo shows when the external URL loads; onError hides only the <img>,
  // revealing the avatar beneath. A logo/avatar ALWAYS renders — never blank.
  if (hasLogo) {
    const altLabel = hasSymbol ? info!.symbol! : shortAddr(token_address);
    return (
      <SymbolPlusAddress
        avatar={
          <span className="relative inline-block size-6 shrink-0">
            <DeterministicAvatar
              seed={token_address}
              className="absolute inset-0 size-6 rounded-full"
            />
            <img
              src={info!.logo_url!}
              alt={altLabel}
              width={24}
              height={24}
              className="absolute inset-0 size-6 rounded-full object-cover"
              onError={(e) => {
                const img = e.currentTarget as HTMLImageElement;
                img.style.display = "none";
                // PERF (2026-08-10): remove the failed image node from the DOM
                // so the browser can release the network/bitmap resources instead
                // of retaining a hidden <img> for every token logo that 404s.
                img.remove();
              }}
            />
          </span>
        }
        symbol={symbol}
        token_address={token_address}
        info={info}
      />
    );
  }

  // Case B/C/D: no payload logo → resolve via the token-icon route cascade
  // (edge → api-server Redis→Registry→PG logo_url→DexScreener→avatar). TokenIcon
  // owns its R1-safe client resolution + avatar fallback, so this also never
  // renders blank. chain_id now drives the per-chain route lookup.
  return (
    <SymbolPlusAddress
      avatar={
        <TokenIcon
          address={token_address}
          chainId={chain_id}
          symbol={symbol ?? undefined}
          size={24}
        />
      }
      symbol={symbol}
      token_address={token_address}
      info={info ?? null}
    />
  );
}
