"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import { getApiBaseUrl } from "@/lib/api-client";

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

interface TopologyProviderSnapshot {
  name?: string;
  url_masked?: string;
  scheme?: string;
  host?: string;
  provider_kind?: string;
}

interface TopologySnapshot {
  scope?: string;
  chain_id?: number;
  mempool_mode?: string;
  rpc_http_1?: TopologyProviderSnapshot[];
  rpc_ws_1?: TopologyProviderSnapshot[];
  version_id?: number;
  updated_at?: string;
}

interface TopologyEnvelope {
  ok?: boolean;
  topology?: TopologySnapshot | null;
  source?: string;
  error?: string;
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
  topology: TopologySnapshot | null;
  steps: SystemReadinessStep[];
  refresh: () => Promise<void>;
}

const POLL_MS = 20_000;
const REFRESH_EVENT = "arbx:system-readiness:refresh";

function hasActiveWss(topology: TopologySnapshot | null): boolean {
  if (!topology) return false;
  const wsProviders = Array.isArray(topology.rpc_ws_1) ? topology.rpc_ws_1 : [];
  return wsProviders.some((provider) => provider.scheme === "wss" && Boolean(provider.host));
}

async function fetchTopologySnapshot(): Promise<TopologyEnvelope> {
  const res = await fetch(`${getApiBaseUrl()}/api/admin/topology/snapshot`, {
    cache: "no-store",
    credentials: "include",
    headers: { accept: "application/json" },
  });
  const data = (await res.json().catch(() => ({}))) as TopologyEnvelope;
  if (!res.ok || !data.ok) {
    return { ok: false, error: data.error ?? `HTTP ${res.status}`, topology: null };
  }
  return data;
}

export function emitSystemReadinessRefresh(): void {
  if (typeof window === "undefined") return;
  window.dispatchEvent(new CustomEvent(REFRESH_EVENT));
}

export function useSystemReadiness(): SystemReadinessState {
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [lastCheckedAt, setLastCheckedAt] = useState<string | null>(null);
  const [topology, setTopology] = useState<TopologySnapshot | null>(null);

  const refresh = useCallback(async () => {
    setIsLoading(true);
    try {
      const envelope = await fetchTopologySnapshot();
      if (!envelope.ok) {
        setError(envelope.error ?? "topology_readiness_unavailable");
        setTopology(null);
        return;
      }
      setTopology(envelope.topology ?? null);
      setError(null);
      setLastCheckedAt(new Date().toISOString());
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    let alive = true;
    const tick = async () => {
      if (!alive) return;
      await refresh();
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

  const isTopologyReady = hasActiveWss(topology);
  const isCredentialsReady = false;
  const isMarketsReady = false;
  const isEnginesReady = false;

  const steps = useMemo<SystemReadinessStep[]>(() => {
    const base: SystemReadinessStep[] = [
      {
        id: "topology",
        index: 1,
        title: "Manifolds de Ingesta",
        label: "Topology Vault",
        description: "Valida que exista al menos un proveedor WSS activo registrado por el backend.",
        href: "/admin/topology",
        ready: isTopologyReady,
        status: isTopologyReady ? "complete" : error ? "error" : "pending",
        evidence: isTopologyReady
          ? `${topology?.rpc_ws_1?.length ?? 0} WSS confirmado(s) · versión ${topology?.version_id ?? "n/a"}`
          : error
            ? error
            : "Pendiente de snapshot no vacío con WSS activo.",
      },
      {
        id: "credentials",
        index: 2,
        title: "Firmas Cuánticas",
        label: "Credentials / Wallets",
        description: "Validará presencia server-side de una private key o signer autorizado.",
        href: "/settings/credentials",
        ready: isCredentialsReady,
        status: isTopologyReady ? "pending" : "blocked",
        evidence: "Endpoint de verificación pendiente; no se marca verde por simulación.",
      },
      {
        id: "markets",
        index: 3,
        title: "Topología de Mercados",
        label: "Chains & DEXes",
        description: "Validará que el registro de exchanges esté inicializado.",
        href: "/dex-registry",
        ready: isMarketsReady,
        status: isCredentialsReady ? "pending" : "blocked",
        evidence: "Endpoint de verificación pendiente; permanece bloqueado hasta implementar la fuente real.",
      },
      {
        id: "engines",
        index: 4,
        title: "Motores de Resolución",
        label: "SVS / DLP / Backrun",
        description: "Validará que al menos un motor de resolución esté activado.",
        href: "/strategies",
        ready: isEnginesReady,
        status: isMarketsReady ? "pending" : "blocked",
        evidence: "Endpoint de verificación pendiente; sin verde falso.",
      },
    ];
    return base;
  }, [error, isCredentialsReady, isEnginesReady, isMarketsReady, isTopologyReady, topology]);

  const completedCount = steps.filter((step) => step.ready).length;
  const totalCount = steps.length;
  const allReady = completedCount === totalCount;

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
    topology,
    steps,
    refresh,
  };
}
