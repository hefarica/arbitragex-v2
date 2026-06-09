"use client";

/**
 * useTransactionSimulation — asks the backend to simulate a transaction intent.
 * Simulation is the mandatory pre-flight (no transaction without simulation),
 * but a PASS NEVER unlocks a broadcast. broadcast_allowed is always false. The
 * backend is fail-honest: without a wired fork-simulation runtime it reports
 * `unavailable` instead of a fabricated "passed" result.
 */

import { useState } from "react";

import { getApiBaseUrl } from "@/lib/api-client";
import type { IntentDraft, IntentState } from "@/hooks/useWalletIntent";

export interface SimulationResult {
  status: string;
  reason?: string;
  intent_id: string;
  state: IntentState;
  simulated: boolean;
  broadcast_allowed: boolean;
  broadcast_reason: string;
  live_enabled: boolean;
  capital_exposed: number;
  broadcast: boolean;
  message: string;
}

export function useTransactionSimulation() {
  const [result, setResult] = useState<SimulationResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [running, setRunning] = useState(false);

  async function simulate(draft: IntentDraft): Promise<void> {
    setRunning(true);
    setError(null);
    try {
      const base = getApiBaseUrl();
      const res = await fetch(`${base}/api/wallet/simulate`, {
        method: "POST",
        credentials: "include",
        headers: { "content-type": "application/json", accept: "application/json" },
        body: JSON.stringify(draft),
      });
      const body = (await res.json()) as SimulationResult;
      // Mirror Law (RULE 00): reflect the backend's real safe posture (verified always
      // broadcast_allowed:false / capital 0 / live false; honest "unavailable" without a
      // wired sim runtime) rather than synthesizing it client-side — a regression then
      // surfaces as a visible alarm. The frontend never broadcasts regardless.
      setResult(body);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setRunning(false);
    }
  }

  return { result, error, running, simulate };
}
