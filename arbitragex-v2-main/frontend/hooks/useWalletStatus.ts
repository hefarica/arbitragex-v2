"use client";

/**
 * useWalletStatus — combines the live wagmi connection state (address, chain,
 * connection status) with the backend wallet runtime posture. Everything that
 * is non-deterministic at first paint (address, chainId, isConnected) is gated
 * behind a `mounted` flag so the server renders a stable disconnected shell and
 * the client fills in after hydration (avoids React #418).
 *
 * No capital, no signer — this is connection identity + safe posture only.
 */

import { useEffect, useState } from "react";
import { useAccount, useChainId } from "wagmi";

import { getApiBaseUrl } from "@/lib/api-client";
import { isSupportedChain } from "@/lib/web3/chains";

export interface WalletRuntimeStatus {
  status: string;
  mode: string;
  read_only: boolean;
  live_enabled: boolean;
  capital_exposed: number;
  broadcast: boolean;
  broadcast_allowed: boolean;
  simulation_required: boolean;
}

export interface UseWalletStatus {
  /** True only after the client mounts — gate all wallet-derived live values. */
  mounted: boolean;
  isConnected: boolean;
  address: `0x${string}` | undefined;
  chainId: number | undefined;
  supportedChain: boolean;
  /** Backend runtime posture (safe). Null until fetched. */
  runtime: WalletRuntimeStatus | null;
  runtimeError: string | null;
}

export function useWalletStatus(): UseWalletStatus {
  const [mounted, setMounted] = useState(false);
  const [runtime, setRuntime] = useState<WalletRuntimeStatus | null>(null);
  const [runtimeError, setRuntimeError] = useState<string | null>(null);

  // wagmi hooks are safe to call during SSR (return disconnected), but we still
  // gate the rendered output behind `mounted` to keep paint deterministic.
  const { address, isConnected } = useAccount();
  const chainId = useChainId();

  useEffect(() => {
    setMounted(true);
    const ctrl = new AbortController();
    const base = getApiBaseUrl();
    fetch(`${base}/api/wallet/status`, { signal: ctrl.signal, credentials: "include", headers: { accept: "application/json" } })
      .then((r) => {
        if (!r.ok) throw new Error(`HTTP ${r.status}`);
        return r.json() as Promise<WalletRuntimeStatus>;
      })
      .then((j) => {
        setRuntime({ ...j, live_enabled: false, capital_exposed: 0, broadcast: false, broadcast_allowed: false });
        setRuntimeError(null);
      })
      .catch((e: Error) => {
        if (e.name === "AbortError") return;
        setRuntimeError(e.message);
      });
    return () => ctrl.abort();
  }, []);

  return {
    mounted,
    isConnected: mounted ? isConnected : false,
    address: mounted ? address : undefined,
    chainId: mounted ? chainId : undefined,
    supportedChain: mounted ? isSupportedChain(chainId) : false,
    runtime,
    runtimeError,
  };
}
