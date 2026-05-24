"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import { getApiBaseUrl } from "@/lib/api-client";
import { rehydrateSystemStore, useSystemStore, type ActiveChain } from "@/store/useSystemStore";

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

function snapshotToActiveChains(topology: TopologySnapshot | null | undefined): ActiveChain[] {
  if (!topology || typeof topology.chain_id !== "number") return [];
  return [{
    chainId: topology.chain_id,
    name: `Chain ${topology.chain_id}`,
    rpcHttpHost: topology.rpc_http_1?.[0]?.host ?? "",
    rpcWsHost: topology.rpc_ws_1?.[0]?.host ?? "",
    versionId: topology.version_id ?? 0,
    validatedAt: topology.updated_at ?? new Date().toISOString(),
  }];
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
  const storeTopologyReady = useSystemStore((state) => state.topology.isReady);
  const activeChains = useSystemStore((state) => state.topology.activeChains);
  const storeCredentialsReady = useSystemStore((state) => state.credentials.isReady);
  const credentialEntries = useSystemStore((state) => state.credentials.entries);
  const storeMarketsReady = useSystemStore((state) => state.markets.isReady);
  const storeMarketsValidatedAt = useSystemStore((state) => state.markets.validatedAt);
  const storeEnginesReady = useSystemStore((state) => state.engines.isReady);
  const enabledStrategies = useSystemStore((state) => state.engines.enabledStrategies);
  const setTopologyReady = useSystemStore((state) => state.setTopologyReady);
  const clearTopology = useSystemStore((state) => state.clearTopology);

  const refresh = useCallback(async () => {
    setIsLoading(true);
    try {
      const envelope = await fetchTopologySnapshot();
      if (!envelope.ok) {
        setError(envelope.error ?? "topology_readiness_unavailable");
        setTopology(null);
        return;
      }
      const nextTopology = envelope.topology ?? null;
      setTopology(nextTopology);
      if (hasActiveWss(nextTopology)) {
        const chains = snapshotToActiveChains(nextTopology);
        setTopologyReady(chains, nextTopology?.version_id ?? 0, nextTopology?.updated_at ?? new Date().toISOString());
      } else if ((envelope.source ?? "") === "empty_vault") {
        clearTopology();
      }
      setError(null);
      setLastCheckedAt(new Date().toISOString());
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setIsLoading(false);
    }
  }, [clearTopology, setTopologyReady]);

  useEffect(() => {
    rehydrateSystemStore();
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

  const isTopologyReady = hasActiveWss(topology) || storeTopologyReady;
  const isCredentialsReady = storeCredentialsReady;
  const isMarketsReady = storeMarketsReady;
  const isEnginesReady = storeEnginesReady;
  const validCredentialCount = Object.values(credentialEntries).filter((entry) => entry.status === "valid").length;

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
          ? `${topology?.rpc_ws_1?.length ?? activeChains.length} WSS confirmado(s) · versión ${topology?.version_id ?? activeChains[0]?.versionId ?? "n/a"}`
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
        status: isCredentialsReady ? "complete" : isTopologyReady ? "pending" : "blocked",
        evidence: isCredentialsReady
          ? `${validCredentialCount} credencial(es) RPC/WSS validada(s) para ${activeChains.length} cadena(s).`
          : isTopologyReady
            ? "Pendiente de validar credenciales requeridas para las cadenas activas."
            : "Bloqueado hasta publicar Topology Vault.",
      },
      {
        id: "markets",
        index: 3,
        title: "Topología de Mercados",
        label: "Chains & DEXes",
        description: "Validará que el registro de exchanges esté inicializado.",
        href: "/dex-registry",
        ready: isMarketsReady,
        status: isMarketsReady ? "complete" : isCredentialsReady ? "pending" : "blocked",
        evidence: isMarketsReady
          ? `Mercados validados${storeMarketsValidatedAt ? ` · ${storeMarketsValidatedAt}` : ""}.`
          : isCredentialsReady
            ? "Pendiente de validar DEXes y mercados activos."
            : "Bloqueado hasta validar credenciales RPC/WSS.",
      },
      {
        id: "engines",
        index: 4,
        title: "Motores de Resolución",
        label: "SVS / DLP / Backrun",
        description: "Validará que al menos un motor de resolución esté activado.",
        href: "/strategies",
        ready: isEnginesReady,
        status: isEnginesReady ? "complete" : isMarketsReady ? "pending" : "blocked",
        evidence: isEnginesReady
          ? `${enabledStrategies.length} estrategia(s) habilitada(s).`
          : isMarketsReady
            ? "Pendiente de habilitar al menos un motor de resolución."
            : "Bloqueado hasta validar mercados y DEXes.",
      },
    ];
    return base;
  }, [activeChains, enabledStrategies.length, error, isCredentialsReady, isEnginesReady, isMarketsReady, isTopologyReady, storeMarketsValidatedAt, topology, validCredentialCount]);

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
