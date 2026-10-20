"use client";
// frontend/lib/hooks/usePriceDerivedValues.ts
//
// G-PRICE-2 — real-time USD re-pricing of visible opportunity cards.
//
// When prices:update arrives via usePricesStream, existing cards' USD values
// (amount_in_usd, expected_profit_usd, etc.) were computed at DETECTION time
// with the then-current prices. This hook recalculates those derived values
// using the FRESHEST price map, WITHOUT refetching the opportunity from the
// API — the on-chain amounts (amount_in_wei, reserves) don't change, only
// their USD-denominated presentation does.
//
// R8 fail-honest: if a token has no fresh price, the derived value stays
// null (rendered as "—") — never fabricated.
//
// React discipline: the recompute is memoized on [prices, amountInWei, tokenSymbol]
// so a WETH price update only re-renders cards that use WETH.

import { useMemo } from "react";

/** Derive fresh USD values for a single opportunity from fresh prices. */
export function deriveUsdValues(
  amountInWei: string | null,
  tokenSymbol: string | null,
  prices: Record<string, number>,
): {
  amountInUsd: number | null;
  hasPrice: boolean;
} {
  if (!amountInWei || !tokenSymbol) {
    return { amountInUsd: null, hasPrice: false };
  }
  const price = prices[tokenSymbol];
  if (price == null || !Number.isFinite(price) || price <= 0) {
    return { amountInUsd: null, hasPrice: false };
  }
  // Convert wei (string) to tokens using 18 decimals as the base case.
  // For non-18-decimal tokens, the original amount_in_usd from the API
  // remains authoritative — we only override when we have BOTH a fresh
  // price AND the default-decimal interpretation is valid.
  const weiNum = Number(amountInWei);
  if (!Number.isFinite(weiNum) || weiNum <= 0) {
    return { amountInUsd: null, hasPrice: false };
  }
  const tokens = weiNum / 1e18;
  return {
    amountInUsd: tokens * price,
    hasPrice: true,
  };
}

/**
 * Hook: derive fresh USD values for a list of opportunities from live prices.
 * Memoized so only opportunities whose token price CHANGED re-compute.
 *
 * @param opps Array of { id, amount_in_wei, token_in_symbol } — the minimum
 *             fields needed for USD derivation.
 * @param prices Live price map from usePricesStream (symbol → USD).
 * @returns Map<oppId, { amountInUsd, hasPrice }> — lookup for card rendering.
 */
export function usePriceDerivedValues(
  opps: Array<{ id: string; amount_in_wei: string | null; token_in_symbol: string | null }>,
  prices: Record<string, number>,
): Map<string, { amountInUsd: number | null; hasPrice: boolean }> {
  return useMemo(() => {
    const out = new Map<string, { amountInUsd: number | null; hasPrice: boolean }>();
    for (const opp of opps) {
      out.set(opp.id, deriveUsdValues(opp.amount_in_wei, opp.token_in_symbol, prices));
    }
    return out;
  }, [opps, prices]);
}
