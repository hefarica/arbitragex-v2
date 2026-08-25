"use client";

/**
 * TokenIcon / TokenPairIcon — reliable, layout-stable token avatars.
 *
 * Resolution is delegated to `useTokenIcon` (Known → Redis → DexScreener →
 * Jazzicon). This component owns only the *presentation* contract:
 *
 *   - Zero layout shift: a fixed-size box is reserved up front; the shimmer,
 *     image, and avatar all occupy the exact same footprint.
 *   - Never a broken image: the deterministic avatar (`DeterministicAvatar`,
 *     the repo's existing seeded SVG — reused, not re-implemented) is the base
 *     layer; the `<img>` fades in over it on load and is removed on `onError`,
 *     so a failed/blocked image silently reveals the avatar beneath.
 *   - Accessible: meaningful `alt`/`aria-label`; decorative layers are hidden.
 *
 * No external icon dependency, no Jazzicon library — the fallback is a pure SVG.
 */

import * as React from "react";

import { cn } from "@/lib/utils";
import { Skeleton } from "@/components/ui/skeleton";
import { DeterministicAvatar } from "@/components/DeterministicAvatar";
import { useTokenIcon } from "@/lib/hooks/useTokenIcon";

export interface TokenIconProps {
  address: string;
  /** FE-0029 (§28): null = no chain claim — no per-chain icon route, the
   *  deterministic avatar layer remains. */
  chainId: number | string | null;
  symbol?: string;
  /** Pixel size of the (square) icon. Default 24. */
  size?: number;
  /** Render the symbol text beside the icon. Default false. */
  showSymbol?: boolean;
  className?: string;
  imgClassName?: string;
  title?: string;
}

export function TokenIcon({
  address,
  chainId,
  symbol,
  size = 24,
  showSymbol = false,
  className,
  imgClassName,
  title,
}: TokenIconProps) {
  const { iconUrl, loading, isFallback, seed } = useTokenIcon({
    address,
    chainId,
    symbol,
  });
  const [imgFailed, setImgFailed] = React.useState(false);
  const [imgLoaded, setImgLoaded] = React.useState(false);

  const box = { width: size, height: size } as const;
  const altLabel = symbol ? `${symbol} token icon` : "Token icon";
  const showImg = !loading && iconUrl != null && !isFallback && !imgFailed;

  return (
    <span
      className={cn("inline-flex items-center gap-1.5 align-middle", className)}
      title={title ?? symbol ?? undefined}
    >
      <span
        className="relative inline-block shrink-0 overflow-hidden rounded-full bg-muted/40 ring-1 ring-border/40"
        style={box}
      >
        {loading ? (
          <Skeleton className="absolute inset-0 size-full rounded-full" />
        ) : (
          <>
            {/* Base layer — always present so the box is never empty/broken. */}
            <DeterministicAvatar
              seed={seed}
              className="absolute inset-0 size-full rounded-full"
            />
            {showImg && (
              <img
                src={iconUrl ?? undefined}
                alt={showSymbol ? "" : altLabel}
                width={size}
                height={size}
                loading="lazy"
                decoding="async"
                draggable={false}
                onLoad={() => setImgLoaded(true)}
                onError={() => setImgFailed(true)}
                className={cn(
                  "absolute inset-0 size-full rounded-full object-cover transition-opacity duration-200",
                  imgLoaded ? "opacity-100" : "opacity-0",
                  imgClassName,
                )}
              />
            )}
          </>
        )}
      </span>
      {showSymbol && symbol ? (
        <span className="text-sm font-medium leading-none">{symbol}</span>
      ) : null}
    </span>
  );
}

export interface TokenPairIconProps {
  tokenInAddress: string;
  tokenOutAddress: string;
  tokenInSymbol?: string;
  tokenOutSymbol?: string;
  chainId: number | string;
  /** Pixel size of each (square) coin. Default 28. */
  size?: number;
  className?: string;
}

/**
 * Two overlapping token icons. The first token sits in front (top-left) with a
 * background-colored ring carving it out from the second, which peeks from
 * behind-right. Works when the two tokens are identical (renders both, offset).
 */
export function TokenPairIcon({
  tokenInAddress,
  tokenOutAddress,
  tokenInSymbol,
  tokenOutSymbol,
  chainId,
  size = 28,
  className,
}: TokenPairIconProps) {
  const overlap = Math.round(size * 0.42);
  const totalWidth = size * 2 - overlap;
  const inLabel = tokenInSymbol ?? "token";
  const outLabel = tokenOutSymbol ?? "token";

  return (
    <span
      className={cn("relative inline-flex shrink-0 align-middle", className)}
      style={{ width: totalWidth, height: size }}
      role="img"
      aria-label={`${inLabel} / ${outLabel} pair`}
    >
      {/* Back coin (token out) — lower z, offset right. */}
      <span
        className="absolute top-0 rounded-full"
        style={{ left: size - overlap, zIndex: 1 }}
        aria-hidden="true"
      >
        <TokenIcon
          address={tokenOutAddress}
          chainId={chainId}
          symbol={tokenOutSymbol}
          size={size}
        />
      </span>
      {/* Front coin (token in) — higher z, ring cut-out against the page bg. */}
      <span
        className="absolute left-0 top-0 rounded-full ring-2 ring-background"
        style={{ zIndex: 2 }}
        aria-hidden="true"
      >
        <TokenIcon
          address={tokenInAddress}
          chainId={chainId}
          symbol={tokenInSymbol}
          size={size}
        />
      </span>
    </span>
  );
}
