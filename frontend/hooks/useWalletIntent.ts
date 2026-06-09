"use client";

/**
 * useWalletIntent — registers a transaction INTENT with the backend. The intent
 * pipeline ALWAYS terminates at BROADCAST_DISABLED; broadcast_allowed is always
 * false. This hook NEVER sends a transaction — it only records what the operator
 * WOULD do for review. There is no code path from here to a network send.
 */

import { useState } from "react";

import { getApiBaseUrl } from "@/lib/api-client";

export type IntentState =
  | "DRAFT"
  | "SIMULATION_REQUIRED"
  | "SIMULATION_FAILED"
  | "SIMULATION_PASSED"
  | "PAPER_ONLY"
  | "BLOCKED_BY_RISK"
  | "BLOCKED_BY_LIVE_OFF"
  | "READY_FOR_OPERATOR_APPROVAL"
  | "BROADCAST_DISABLED";

export interface IntentDraft {
  chain_id?: number;
  kind?: string;
  to?: string;
  value?: string;
  note?: string;
  /** Whether the operator claims a prior simulation. Never enables broadcast. */
  simulated?: boolean;
}

export interface IntentResult {
  status: string;
  intent_id: string;
  state: IntentState;
  lifecycle: IntentState[];
  broadcast_allowed: boolean;
  reason: string;
  live_enabled: boolean;
  capital_exposed: number;
  broadcast: boolean;
  message: string;
}

export function useWalletIntent() {
  const [result, setResult] = useState<IntentResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  async function submitIntent(draft: IntentDraft): Promise<void> {
    setSubmitting(true);
    setError(null);
    try {
      const base = getApiBaseUrl();
      const res = await fetch(`${base}/api/wallet/intent`, {
        method: "POST",
        credentials: "include",
        headers: { "content-type": "application/json", accept: "application/json" },
        body: JSON.stringify(draft),
      });
      const body = (await res.json()) as IntentResult;
      // Mirror Law (RULE 00): the backend is the authority on the safe posture — do NOT
      // synthesize safe values client-side. The intent endpoint is verified to always
      // return broadcast_allowed:false / live_enabled:false / capital_exposed:0 and to
      // terminate at BROADCAST_DISABLED; rendering its real values means a backend
      // regression would surface here as a visible alarm instead of being masked. The
      // frontend has NO broadcast path regardless of what this object says.
      setResult(body);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setSubmitting(false);
    }
  }

  return { result, error, submitting, submitIntent };
}
