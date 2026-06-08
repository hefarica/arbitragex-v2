"use client";

/**
 * useSystemReadiness — SERVER-DRIVEN 4-step "Live Readiness" state.
 *
 * Audit 2026-06-08 rewire: this hook used to derive its N/4 count from
 * `localStorage` (Steps 2-4 via useSystemStore) + an ADMIN-gated topology
 * snapshot (Step 1, which returns 401 to a normal operator browser). That made
 * the readiness count a client-side fiction (4/4 in one browser, 0/4 in another,
 * same backend) — a zero-mocks / RULE 00 violation.
 *
 * It now reads ONE public, server-evaluated endpoint, `/api/readiness/steps`
 * (api-server readiness-steps.ts), through the resilient `getReadinessSteps()`
 * pipeline: 5s timeout + AbortController, retries on 5xx, Zod-validated, and it
 * NEVER throws (always resolves to a Result). Therefore:
 *   - the spinner can never hang (the old raw no-timeout fetch could),
 *   - a fabricated green can never slip through (server is the only source),
 *   - credentials stay redacted (present|null), live stays structurally off.
 *
 * The public interface (fields + exported types) is unchanged so existing
 * consumers (ReadinessStepper, SystemGuardBanner, TopologyVaultClient) keep
 * working untouched.
 */

import { useCallback, useEffect, useMemo, useState } from "react";
import { getReadinessSteps } from "@/lib/api-client";
import type {
  ReadinessStep as ServerReadinessStep,
  ReadinessStepStatus as ServerStepStatus,
} from "@/lib/schemas";

export type ReadinessStepId = "topology" | "credentials" | "markets" | "engines";
export type ReadinessStepStatus = "complete" | "pending" | "blocked" | "error";

export interface SystemReadinessStep {
  id: ReadinessStepId;
  index: number;
  title: string;
  label: string;
  description: string;
  href: string;
  ready: boolean;
  status: ReadinessStepStatus;
  evidence: string;
}

export interface SystemReadinessState {
  isLoading: boolean;
  error: string | null;
  lastCheckedAt: string | null;
  isTopologyReady: boolean;
  isCredentialsReady: boolean;
  isMarketsReady: boolean;
  isEnginesReady: boolean;
  allReady: boolean;
  completedCount: number;
  totalCount: number;
  steps: SystemReadinessStep[];
  refresh: () => Promise<void>;
}

const POLL_MS = 20_000;
const REFRESH_EVENT = "arbx:system-readiness:refresh";

// Static presentation metadata per step id. The STATE (ready/status/evidence)
// always comes from the server; only the human copy + deep-link live here.
const STEP_META: Record<ReadinessStepId, { index: number; title: string; label: string; description: string; href: string }> = {
  topology: {
    index: 1,
    title: "Manifolds de Ingesta",
    label: "Topology Vault",
    description: "≥1 proveedor RPC + ≥1 WSS activos en el Topology Vault (verificado server-side).",
    href: "/admin/topology",
  },
  credentials: {
    index: 2,
    title: "Firmas Cuánticas",
    label: "Credentials / Wallets",
    description: "Presencia server-side de signer/clave autorizada (redactado: present|null).",
    href: "/settings/credentials",
  },
  markets: {
    index: 3,
    title: "Topología de Mercados",
    label: "Chains & DEXes",
    description: "Chains, DEXes y pools registrados en el backend (conteos reales).",
    href: "/dex-registry",
  },
  engines: {
    index: 4,
    title: "Motores de Resolución",
    label: "SVS / DLP / Backrun",
    description: "≥1 motor de resolución habilitado (paper/shadow; live deshabilitado).",
    href: "/strategies",
  },
};

const STEP_ORDER: ReadinessStepId[] = ["topology", "credentials", "markets", "engines"];

function mapStatus(s: ServerStepStatus): ReadinessStepStatus {
  switch (s) {
    case "PASS":
      return "complete";
    case "BLOCKED":
      return "blocked";
    case "FAIL":
      return "error";
    default:
      return "pending";
  }
}

function toUiStep(server: ServerReadinessStep): SystemReadinessStep {
  const meta = STEP_META[server.id];
  return {
    id: server.id,
    index: meta.index,
    title: meta.title,
    label: meta.label,
    description: meta.description,
    href: meta.href,
    ready: server.ready,
    status: mapStatus(server.status),
    // The server reason IS the evidence line — honest, concise, redacted.
    evidence: server.reason,
  };
}

// Stable placeholder while the first fetch is in-flight or after an error: all
// PENDING/blocked, NEVER a fabricated green. Step 1 is the only reachable one.
function placeholderSteps(): SystemReadinessStep[] {
  return STEP_ORDER.map((id) => {
    const meta = STEP_META[id];
    return {
      id,
      index: meta.index,
      title: meta.title,
      label: meta.label,
      description: meta.description,
      href: meta.href,
      ready: false,
      status: id === "topology" ? ("pending" as const) : ("blocked" as const),
      evidence: "—",
    };
  });
}

export function emitSystemReadinessRefresh(): void {
  if (typeof window === "undefined") return;
  window.dispatchEvent(new CustomEvent(REFRESH_EVENT));
}

export function useSystemReadiness(): SystemReadinessState {
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [lastCheckedAt, setLastCheckedAt] = useState<string | null>(null);
  const [steps, setSteps] = useState<SystemReadinessStep[]>(placeholderSteps);
  const [completedCount, setCompletedCount] = useState(0);

  const refresh = useCallback(async () => {
    setIsLoading(true);
    const r = await getReadinessSteps();
    setIsLoading(false);
    if (!r.ok) {
      // Fail-honest: surface the error, KEEP the last good (or placeholder)
      // steps — never invent a green to paper over the failure.
      setError(r.error);
      return;
    }
    setError(null);
    setSteps(r.data.steps.map(toUiStep));
    setCompletedCount(r.data.completed_steps);
    setLastCheckedAt(r.data.generated_at);
  }, []);

  useEffect(() => {
    let alive = true;
    const tick = async () => {
      if (alive) await refresh();
    };
    void tick();
    const interval = window.setInterval(tick, POLL_MS);
    const onDemandRefresh = () => void tick();
    window.addEventListener(REFRESH_EVENT, onDemandRefresh);
    return () => {
      alive = false;
      window.clearInterval(interval);
      window.removeEventListener(REFRESH_EVENT, onDemandRefresh);
    };
  }, [refresh]);

  const byId = useMemo(() => new Map(steps.map((s) => [s.id, s] as const)), [steps]);

  const isTopologyReady = byId.get("topology")?.ready ?? false;
  const isCredentialsReady = byId.get("credentials")?.ready ?? false;
  const isMarketsReady = byId.get("markets")?.ready ?? false;
  const isEnginesReady = byId.get("engines")?.ready ?? false;
  const totalCount = steps.length;
  const allReady = totalCount > 0 && completedCount === totalCount;

  return {
    isLoading,
    error,
    lastCheckedAt,
    isTopologyReady,
    isCredentialsReady,
    isMarketsReady,
    isEnginesReady,
    allReady,
    completedCount,
    totalCount,
    steps,
    refresh,
  };
}
