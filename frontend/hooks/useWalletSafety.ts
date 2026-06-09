"use client";

/**
 * useWalletSafety — fetches the backend safety posture for the wallet surface.
 *
 * The safety posture is the institutional "why you cannot broadcast" matrix:
 * live_enabled=false, capital_exposed=0, broadcast=false, plus the gate list.
 * It is read-only and never enables anything. On fetch failure we expose the
 * error verbatim and keep the local safe defaults (the UI must NEVER imply that
 * a fetch failure could mean live/broadcast is enabled).
 */

import { useEffect, useState } from "react";

import { getApiBaseUrl } from "@/lib/api-client";

export interface SafetyGate {
  name: string;
  status: string;
  value: string | number | boolean;
  reason: string;
}

export interface WalletSafety {
  status: string;
  live_enabled: boolean;
  capital_exposed: number;
  broadcast: boolean;
  submit_enabled: boolean;
  private_relay_enabled: boolean;
  blind_signing_enabled: boolean;
  simulation_required: boolean;
  session_mode: string;
  gates: SafetyGate[];
}

/** Local safe defaults — used until/if the backend answers. Always the safe posture. */
export const SAFE_DEFAULTS: WalletSafety = {
  status: "ok",
  live_enabled: false,
  capital_exposed: 0,
  broadcast: false,
  submit_enabled: false,
  private_relay_enabled: false,
  blind_signing_enabled: false,
  simulation_required: true,
  session_mode: "wallet_identity_only",
  gates: [],
};

export function useWalletSafety(): { safety: WalletSafety; error: string | null; loading: boolean } {
  const [safety, setSafety] = useState<WalletSafety>(SAFE_DEFAULTS);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState<boolean>(true);

  useEffect(() => {
    const ctrl = new AbortController();
    const base = getApiBaseUrl();
    fetch(`${base}/api/wallet/safety`, { signal: ctrl.signal, credentials: "include", headers: { accept: "application/json" } })
      .then((r) => {
        if (!r.ok) throw new Error(`HTTP ${r.status}`);
        return r.json() as Promise<WalletSafety>;
      })
      .then((j) => {
        // Defensive: even if the backend somehow returned otherwise, the UI
        // pins the safe invariants. Trust but verify.
        setSafety({
          ...j,
          live_enabled: false,
          capital_exposed: 0,
          broadcast: false,
          submit_enabled: false,
        });
        setError(null);
      })
      .catch((e: Error) => {
        if (e.name === "AbortError") return;
        setError(e.message);
      })
      .finally(() => setLoading(false));
    return () => ctrl.abort();
  }, []);

  return { safety, error, loading };
}
