"use client";

/**
 * useWalletBalances — READ-ONLY native balance of the connected account via
 * wagmi's useBalance. This is a pure read (eth_getBalance); it never moves
 * funds, never signs, never broadcasts.
 *
 * The balance is gated behind a `mounted` flag (see useWalletStatus) so first
 * paint is deterministic. When disconnected, the hook reports nothing — honest
 * empty, never a fabricated number.
 */

import { useEffect, useState } from "react";
import { useAccount, useBalance } from "wagmi";

export interface WalletBalance {
  /** Human-readable formatted balance (e.g. "1.234"). */
  formatted: string | null;
  symbol: string | null;
  decimals: number | null;
  loading: boolean;
  error: string | null;
}

export function useWalletBalances(): WalletBalance {
  const [mounted, setMounted] = useState(false);
  useEffect(() => setMounted(true), []);

  const { address, isConnected } = useAccount();
  const enabled = mounted && isConnected && Boolean(address);

  const { data, isLoading, isError, error } = useBalance({
    address,
    query: { enabled },
  });

  if (!enabled) {
    return { formatted: null, symbol: null, decimals: null, loading: false, error: null };
  }
  return {
    formatted: data ? data.formatted : null,
    symbol: data ? data.symbol : null,
    decimals: data ? data.decimals : null,
    loading: isLoading,
    error: isError ? (error?.message ?? "balance_read_error") : null,
  };
}
