"use client";

import Link from "next/link";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useRouter } from "next/navigation";
import { toast } from "sonner";
import { ReadinessStepper } from "@/components/ReadinessStepper";
import { emitSystemReadinessRefresh, useSystemReadiness } from "@/hooks/useSystemReadiness";
import { hasAdminSession } from "@/lib/admin-token";

export type MempoolMode = "auto" | "filtered" | "firehose";

interface TopologyProviderSnapshot {
  name: string;
  url_masked: string;
  scheme: "http" | "https" | "ws" | "wss";
  host: string;
  provider_kind: "alchemy" | "standard";
}

interface TopologySnapshot {
  scope: string;
  chain_id: number;
  mempool_mode: MempoolMode;
  rpc_http_1: TopologyProviderSnapshot[];
  rpc_ws_1: TopologyProviderSnapshot[];
  checksum: string;
  version_id: number;
  updated_at: string;
}

export interface TopologyInitialSnapshot {
  topology: TopologySnapshot | null;
  source: string;
  error: string | null;
}

interface SnapshotEnvelope {
  ok?: boolean;
  topology?: TopologySnapshot | null;
  source?: string;
  error?: string;
}

interface MutationEnvelope {
  ok?: boolean;
  mutation_id?: string;
  version_id?: number;
  subscribers_notified?: number;
  topology?: TopologySnapshot;
  error?: string;
}

interface ClientProps {
  initialSnapshot: TopologyInitialSnapshot;
  edgeUrl: string;
}

const HTTP_EXAMPLE = "alchemy=https://eth-mainnet.g.alchemy.com/v2/TU_API_KEY,publicnode=https://ethereum-rpc.publicnode.com";
const WS_EXAMPLE = "alchemy=wss://eth-mainnet.g.alchemy.com/v2/TU_API_KEY,publicnode=wss://ethereum-rpc.publicnode.com";

function formatErrorCode(code: string): string {
  const labels: Record<string, string> = {
    filtered_requires_first_ws_alchemy: "El modo filtered exige que el primer proveedor WebSocket sea Alchemy.",
    proxy_forbidden: "Se rechazó una URL que parece proxy de terceros. Usa el endpoint RPC/WSS directo del proveedor.",
    userinfo_forbidden: "No se permiten credenciales en userinfo de URL. Usa el formato provider=url.",
    invalid_http_scheme: "RPC HTTP debe usar http:// o https://.",
    invalid_ws_scheme: "RPC WebSocket debe usar ws:// o wss://.",
    invalid_provider_name: "El nombre del proveedor solo puede usar letras, números, guion y underscore.",
    invalid_url: "Una URL de proveedor no es válida.",
    invalid_request: "La solicitud no cumple el contrato del Topology Vault.",
  };
  return labels[code] ?? code;
}

function ProviderTable({ title, providers }: { title: string; providers: TopologyProviderSnapshot[] }) {
  return (
    <div className="rounded-xl border border-slate-200 bg-white">
      <div className="border-b border-slate-200 px-4 py-3">
        <h3 className="text-sm font-semibold text-slate-950">{title}</h3>
      </div>
      <div className="overflow-x-auto">
        <table className="w-full text-left text-sm">
          <thead className="bg-slate-50 text-xs uppercase tracking-wide text-slate-500">
            <tr>
              <th className="px-4 py-3">Proveedor</th>
              <th className="px-4 py-3">Tipo</th>
              <th className="px-4 py-3">Host</th>
              <th className="px-4 py-3">URL redactada</th>
            </tr>
          </thead>
          <tbody>
            {providers.length === 0 ? (
              <tr>
                <td colSpan={4} className="px-4 py-5 text-center text-slate-500">
                  Sin proveedores registrados.
                </td>
              </tr>
            ) : providers.map((p) => (
              <tr key={`${title}:${p.name}:${p.host}:${p.scheme}`} className="border-t border-slate-100">
                <td className="px-4 py-3 font-semibold text-slate-900">{p.name}</td>
                <td className="px-4 py-3">
                  <span className={p.provider_kind === "alchemy" ? "rounded-full bg-emerald-50 px-2 py-1 text-xs font-semibold text-emerald-700" : "rounded-full bg-slate-100 px-2 py-1 text-xs font-semibold text-slate-700"}>
                    {p.provider_kind}
                  </span>
                </td>
                <td className="px-4 py-3 font-mono text-xs text-slate-700">{p.host}</td>
                <td className="max-w-[420px] break-all px-4 py-3 font-mono text-xs text-slate-700">{p.url_masked}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

export function TopologyVaultClient({ initialSnapshot, edgeUrl }: ClientProps) {
  const router = useRouter();
  const [snapshot, setSnapshot] = useState<TopologyInitialSnapshot>(initialSnapshot);
  const [scope, setScope] = useState(initialSnapshot.topology?.scope ?? "staging");
  const [chainId, setChainId] = useState(String(initialSnapshot.topology?.chain_id ?? 1));
  const [mempoolMode, setMempoolMode] = useState<MempoolMode>(initialSnapshot.topology?.mempool_mode ?? "auto");
  const [rpcHttp, setRpcHttp] = useState("");
  const [rpcWs, setRpcWs] = useState("");
  const [actor, setActor] = useState("operator");
  const [isMounted, setIsMounted] = useState(false);
  const [hasSession, setHasSession] = useState(false);
  const [isLoading, setIsLoading] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const readiness = useSystemReadiness();

  useEffect(() => {
    setIsMounted(true);
    setHasSession(hasAdminSession());
  }, []);

  const topology = snapshot.topology;

  const canSubmit = useMemo(() => {
    const parsedChain = Number(chainId);
    return scope.trim().length > 0 && Number.isInteger(parsedChain) && parsedChain > 0 && rpcHttp.trim().length >= 8 && rpcWs.trim().length >= 8 && !isSaving;
  }, [chainId, isSaving, rpcHttp, rpcWs, scope]);

  const refresh = useCallback(async () => {
    setIsLoading(true);
    try {
      const res = await fetch(`${edgeUrl}/api/admin/topology/snapshot`, {
        credentials: "include",
        cache: "no-store",
        headers: { accept: "application/json" },
      });
      const data = (await res.json().catch(() => ({}))) as SnapshotEnvelope;
      if (!res.ok || !data.ok) {
        setSnapshot((prev) => ({ ...prev, error: data.error ?? `HTTP ${res.status}` }));
        return;
      }
      setSnapshot({ topology: data.topology ?? null, source: data.source ?? "vault", error: null });
      if (data.topology) {
        setScope(data.topology.scope);
        setChainId(String(data.topology.chain_id));
        setMempoolMode(data.topology.mempool_mode);
      }
    } catch (e) {
      setSnapshot((prev) => ({ ...prev, error: (e as Error).message }));
    } finally {
      setIsLoading(false);
    }
  }, [edgeUrl]);

  const applyMutation = useCallback(async () => {
    if (!canSubmit) {
      toast.error("Completa scope, chain_id, RPC HTTP y RPC WebSocket antes de aplicar.");
      return;
    }
    setIsSaving(true);
    try {
      const res = await fetch(`${edgeUrl}/api/admin/topology/mutations`, {
        method: "POST",
        credentials: "include",
        headers: {
          "content-type": "application/json",
          "x-arbx-actor": actor.trim() || "operator",
        },
        body: JSON.stringify({
          scope: scope.trim(),
          chain_id: Number(chainId),
          mempool_mode: mempoolMode,
          rpc_http_1: rpcHttp.trim(),
          rpc_ws_1: rpcWs.trim(),
        }),
      });
      const data = (await res.json().catch(() => ({}))) as MutationEnvelope;
      if (!res.ok || !data.ok || !data.topology) {
        const msg = formatErrorCode(data.error ?? `HTTP ${res.status}`);
        toast.error("Topology Vault rechazó la mutación", { description: msg });
        setSnapshot((prev) => ({ ...prev, error: msg }));
        return;
      }
      setSnapshot({ topology: data.topology, source: "mutation", error: null });
      setScope(data.topology.scope);
      setChainId(String(data.topology.chain_id));
      setMempoolMode(data.topology.mempool_mode);
      setRpcHttp("");
      setRpcWs("");
      toast.success("Topology Vault actualizado", {
        description: `Versión ${data.version_id}; subscribers notificados: ${data.subscribers_notified ?? 0}`,
      });
      emitSystemReadinessRefresh();
      await readiness.refresh();
    } catch (e) {
      const msg = (e as Error).message;
      toast.error("No se pudo aplicar la topología", { description: msg });
      setSnapshot((prev) => ({ ...prev, error: msg }));
    } finally {
      setIsSaving(false);
    }
  }, [actor, canSubmit, chainId, edgeUrl, mempoolMode, readiness, rpcHttp, rpcWs, scope]);

  if (isMounted && !hasSession) {
    return (
      <main className="min-h-screen bg-white p-8 text-slate-950">
        <div className="max-w-4xl rounded-xl border border-amber-200 bg-amber-50 p-5 text-amber-900">
          <h1 className="text-2xl font-bold">Topology Vault requiere sesión administrativa</h1>
          <p className="mt-2 text-sm">
            Inicia sesión primero en <button onClick={() => router.push("/admin/signin?next=/admin/topology")} className="font-semibold underline">/admin/signin</button>. El token administrativo queda en cookie httpOnly y no se expone a JavaScript.
          </p>
        </div>
      </main>
    );
  }

  return (
    <main className="min-h-screen bg-white p-8 text-slate-950">
      <ReadinessStepper readiness={readiness} className="mb-8" />

      <div className="mb-8 border-b border-slate-200 pb-6">
        <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
          <div>
            <p className="text-xs font-semibold uppercase tracking-[0.25em] text-slate-500">Control Plane · RPC/WSS</p>
            <h1 className="mt-2 text-3xl font-bold tracking-tight text-slate-950">Paso 1 de 4: Inicialización de Ingesta (Topology Vault)</h1>
            <p className="mt-2 max-w-3xl text-sm leading-6 text-slate-600">
              Este es el punto operativo donde el operador ingresa credenciales de Alchemy, PublicNode u otros proveedores. Los secretos se envían al API Server una sola vez; la UI solo vuelve a mostrar hosts y URLs redactadas.
            </p>
          </div>
          <button
            type="button"
            onClick={refresh}
            disabled={isLoading}
            className="rounded-lg border border-slate-300 bg-white px-4 py-2 text-sm font-semibold text-slate-800 shadow-sm hover:bg-slate-50 disabled:opacity-50"
          >
            {isLoading ? "Actualizando…" : "Actualizar snapshot"}
          </button>
        </div>
        <div className="mt-5 rounded-2xl border border-slate-200 bg-white/70 p-4 text-sm leading-6 text-slate-700 shadow-sm backdrop-blur-xl">
          <strong className="text-slate-950">Atención:</strong> La plataforma requiere la validación de los pasos posteriores —Firmas, DEXes y Motores— para iniciar el hot-path. Completa este paso para desbloquear la telemetría de red y avanzar hacia credenciales.
        </div>
        {snapshot.error && (
          <div className="mt-4 rounded-lg border border-red-200 bg-red-50 p-3 text-sm text-red-700">
            {snapshot.error}
          </div>
        )}
      </div>

      <div className="grid gap-6 xl:grid-cols-[minmax(0,1fr)_420px]">
        <section className="space-y-5 rounded-2xl border border-slate-200 bg-white p-5 shadow-sm">
          <div>
            <h2 className="text-lg font-semibold text-slate-950">Aplicar nueva topología RPC</h2>
            <p className="mt-1 text-sm text-slate-600">
              Usa formato CSV `nombre=url`. Para `filtered`, el primer WSS debe ser Alchemy porque el searcher usa `alchemy_pendingTransactions`; en `auto`, Rust decidirá filtered solo si detecta Alchemy compatible.
            </p>
          </div>

          <div className="grid gap-4 md:grid-cols-3">
            <label className="block">
              <span className="text-xs font-semibold uppercase tracking-wide text-slate-500">Scope</span>
              <input
                value={scope}
                onChange={(e) => setScope(e.target.value)}
                className="mt-1 w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm text-slate-950 shadow-sm outline-none focus:border-slate-900"
              />
            </label>
            <label className="block">
              <span className="text-xs font-semibold uppercase tracking-wide text-slate-500">Chain ID</span>
              <input
                value={chainId}
                onChange={(e) => setChainId(e.target.value)}
                inputMode="numeric"
                className="mt-1 w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm text-slate-950 shadow-sm outline-none focus:border-slate-900"
              />
            </label>
            <label className="block">
              <span className="text-xs font-semibold uppercase tracking-wide text-slate-500">Mempool mode</span>
              <select
                value={mempoolMode}
                onChange={(e) => setMempoolMode(e.target.value as MempoolMode)}
                className="mt-1 w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm text-slate-950 shadow-sm outline-none focus:border-slate-900"
              >
                <option value="auto">auto</option>
                <option value="filtered">filtered</option>
                <option value="firehose">firehose</option>
              </select>
            </label>
          </div>

          <label className="block">
            <span className="text-xs font-semibold uppercase tracking-wide text-slate-500">RPC HTTP providers</span>
            <textarea
              value={rpcHttp}
              onChange={(e) => setRpcHttp(e.target.value)}
              placeholder={HTTP_EXAMPLE}
              rows={4}
              spellCheck={false}
              className="mt-1 w-full rounded-lg border border-slate-300 bg-white px-3 py-2 font-mono text-xs text-slate-950 shadow-sm outline-none focus:border-slate-900"
            />
          </label>

          <label className="block">
            <span className="text-xs font-semibold uppercase tracking-wide text-slate-500">RPC WebSocket providers</span>
            <textarea
              value={rpcWs}
              onChange={(e) => setRpcWs(e.target.value)}
              placeholder={WS_EXAMPLE}
              rows={4}
              spellCheck={false}
              className="mt-1 w-full rounded-lg border border-slate-300 bg-white px-3 py-2 font-mono text-xs text-slate-950 shadow-sm outline-none focus:border-slate-900"
            />
          </label>

          <label className="block max-w-sm">
            <span className="text-xs font-semibold uppercase tracking-wide text-slate-500">Actor de auditoría</span>
            <input
              value={actor}
              onChange={(e) => setActor(e.target.value)}
              className="mt-1 w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm text-slate-950 shadow-sm outline-none focus:border-slate-900"
            />
          </label>

          <div className="rounded-lg border border-slate-200 bg-slate-50 p-4 text-sm text-slate-700">
            <strong className="text-slate-950">Regla de seguridad:</strong> no pegues proxys intermedios ni URLs con userinfo. El backend rechazará dominios tipo túnel/proxy y jamás devolverá la URL completa en el snapshot.
          </div>

          <div className="flex flex-col gap-3 sm:flex-row sm:items-center">
            <button
              type="button"
              onClick={applyMutation}
              disabled={!canSubmit}
              className="rounded-lg bg-slate-950 px-5 py-2.5 text-sm font-semibold text-white shadow-sm hover:bg-slate-800 disabled:cursor-not-allowed disabled:opacity-50"
            >
              {isSaving ? "Aplicando hot-swap…" : "Aplicar topología y publicar hot-swap"}
            </button>
            <Link
              href="/settings/credentials"
              aria-disabled={!readiness.isTopologyReady}
              className={readiness.isTopologyReady
                ? "rounded-lg border border-emerald-200 bg-emerald-50 px-5 py-2.5 text-sm font-semibold text-emerald-800 shadow-sm hover:bg-emerald-100"
                : "pointer-events-none rounded-lg border border-slate-200 bg-slate-100 px-5 py-2.5 text-sm font-semibold text-slate-400"
              }
              title={readiness.isTopologyReady ? "Backend confirmó WSS activo" : "Requiere snapshot backend con WSS activo"}
            >
              Topología Confirmada → Continuar al Paso 2 (Credenciales)
            </Link>
          </div>
        </section>

        <aside className="space-y-4 rounded-2xl border border-slate-200 bg-white p-5 shadow-sm">
          <h2 className="text-lg font-semibold text-slate-950">Snapshot actual</h2>
          {topology ? (
            <div className="space-y-3 text-sm text-slate-700">
              <div className="grid grid-cols-2 gap-3">
                <div className="rounded-lg bg-slate-50 p-3"><div className="text-xs uppercase text-slate-500">Scope</div><div className="font-semibold text-slate-950">{topology.scope}</div></div>
                <div className="rounded-lg bg-slate-50 p-3"><div className="text-xs uppercase text-slate-500">Chain</div><div className="font-semibold text-slate-950">{topology.chain_id}</div></div>
                <div className="rounded-lg bg-slate-50 p-3"><div className="text-xs uppercase text-slate-500">Mode</div><div className="font-semibold text-slate-950">{topology.mempool_mode}</div></div>
                <div className="rounded-lg bg-slate-50 p-3"><div className="text-xs uppercase text-slate-500">Version</div><div className="font-semibold text-slate-950">{topology.version_id}</div></div>
              </div>
              <div className="rounded-lg bg-slate-50 p-3">
                <div className="text-xs uppercase text-slate-500">Updated</div>
                <div className="font-mono text-xs text-slate-950">{new Date(topology.updated_at).toLocaleString()}</div>
              </div>
              <div className="rounded-lg bg-slate-50 p-3">
                <div className="text-xs uppercase text-slate-500">Checksum</div>
                <div className="break-all font-mono text-xs text-slate-950">{topology.checksum}</div>
              </div>
            </div>
          ) : (
            <p className="rounded-lg border border-dashed border-slate-300 p-4 text-sm text-slate-500">
              Todavía no hay topología guardada en el vault. Aplica una mutación para inicializarlo.
            </p>
          )}
        </aside>
      </div>

      {topology && (
        <section className="mt-6 grid gap-6 xl:grid-cols-2">
          <ProviderTable title="RPC HTTP actuales" providers={topology.rpc_http_1} />
          <ProviderTable title="RPC WebSocket actuales" providers={topology.rpc_ws_1} />
        </section>
      )}
    </main>
  );
}
