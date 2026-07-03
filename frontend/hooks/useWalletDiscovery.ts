"use client";

/**
 * useWalletDiscovery — EIP-6963 multi-injected provider discovery. On mount it dispatches
 * `eip6963:requestProvider` and continuously listens for `eip6963:announceProvider`. SSR-safe (no window
 * access on the server). Read-only: it never calls an announced provider and never auto-connects.
 * `redetect()` re-runs discovery (for the "Volver a detectar" affordance).
 */

import { useCallback, useEffect, useState } from "react";
import { requestProviders, type EIP6963ProviderDetail } from "@/lib/web3/wallet-discovery";

export interface UseWalletDiscovery {
  providers: EIP6963ProviderDetail[];
  loading: boolean;
  redetect: () => void;
}

export function useWalletDiscovery(): UseWalletDiscovery {
  const [providers, setProviders] = useState<EIP6963ProviderDetail[]>([]);
  const [loading, setLoading] = useState(false);

  const scan = useCallback(() => {
    if (typeof window === "undefined") return;
    setLoading(true);
    void requestProviders().then((p) => {
      setProviders((prev) => {
        const byId = new Map(prev.map((x) => [x.info.uuid, x]));
        for (const d of p) byId.set(d.info.uuid, d);
        return [...byId.values()];
      });
      setLoading(false);
    });
  }, []);

  useEffect(() => {
    if (typeof window === "undefined") return;
    const onAnnounce = (ev: Event): void => {
      const detail = (ev as CustomEvent<EIP6963ProviderDetail>).detail;
      if (detail && detail.info && typeof detail.info.uuid === "string") {
        setProviders((prev) => (prev.some((x) => x.info.uuid === detail.info.uuid) ? prev : [...prev, detail]));
      }
    };
    window.addEventListener("eip6963:announceProvider", onAnnounce as EventListener);
    scan();
    return () => window.removeEventListener("eip6963:announceProvider", onAnnounce as EventListener);
  }, [scan]);

  return { providers, loading, redetect: scan };
}
