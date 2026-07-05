// ZERO MOCKS DOCTRINE (rule_00): This page fetches data ONLY through the edge.
// NEVER hardcode fallback data or bypass the edge layer.
"use client";
import React, { useCallback, useEffect, useState } from "react";
import { Activity } from "lucide-react";
import { motion } from "framer-motion";
import { getDefiRpcs } from "@/lib/api-client";
import type { DefiRpcsResponse } from "@/lib/schemas";
import { EdgeState } from "@/components/EdgeState";
import { RpcSyncPanel } from "@/components/RpcSyncPanel";
import { BundleSyncPanel } from "@/components/BundleSyncPanel";

const RPC_COLUMNS = ["Chain ID", "URL", "Type", "Status"];

export default function RpcHealthPage() {
  const [result, setResult] = useState<{ ok: true; data: DefiRpcsResponse } | { ok: false; error: string } | null>(null);
  const [retrying, setRetrying] = useState(false);

  const load = useCallback(async () => {
    setRetrying(true);
    const r = await getDefiRpcs();
    setResult(r);
    setRetrying(false);
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  // Legacy RPC-health table (defi_rpcs / migration 021) — distinct from the
  // rpc_endpoints catalog registry shown in <RpcSyncPanel/> above. Rendered as a
  // section below the panel so the catalog sync is always available (even when
  // the legacy table is loading/empty/unreachable).
  let legacy: React.ReactNode;
  if (!result) {
    legacy = (
      <EdgeState variant="loading" title="Loading RPC registry…" description="Querying the edge for RPC health." endpoint="GET /api/v1/rpcs" ghost="table" ghostColumns={RPC_COLUMNS} />
    );
  } else if (!result.ok) {
    legacy = (
      <EdgeState
        variant="error"
        title="RPC registry unreachable"
        description="The edge endpoint refused or could not be reached. No fabricated RPC data is shown — zero-trust."
        endpoint="GET /api/v1/rpcs"
        reasons={[result.error]}
        onRetry={load}
        retrying={retrying}
        ghost="table"
        ghostColumns={RPC_COLUMNS}
      />
    );
  } else if (result.data.data.length === 0) {
    legacy = (
      <EdgeState variant="empty" title="No RPCs registered yet" description="The registry is reachable but empty — no RPC endpoints are configured." endpoint="GET /api/v1/rpcs" onRetry={load} retrying={retrying} ghost="table" ghostColumns={RPC_COLUMNS} />
    );
  } else {
    const rpcs = result.data.data;
    legacy = (
      <div data-slot="card" className="bg-card text-card-foreground border border-border rounded-xl overflow-hidden shadow-2xl">
        <table className="w-full text-left border-collapse">
          <thead>
            <tr className="bg-muted text-muted-foreground text-sm uppercase tracking-wider">
              <th className="p-4 border-b border-border">Chain ID</th>
              <th className="p-4 border-b border-border">URL</th>
              <th className="p-4 border-b border-border">Type</th>
              <th className="p-4 border-b border-border">Latency</th>
              <th className="p-4 border-b border-border">Status</th>
            </tr>
          </thead>
          <tbody className="text-foreground">
            {rpcs.map((rpc, i) => (
              <motion.tr
                key={`${rpc.chain_id ?? "?"}-${rpc.url ?? i}`}
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                transition={{ delay: i * 0.05 }}
                className="hover:bg-muted/40 transition-colors"
              >
                <td className="p-4 border-b border-border">{rpc.chain_id ?? "—"}</td>
                <td className="p-4 border-b border-border font-mono text-sm text-info truncate max-w-[250px]">{rpc.url ?? "—"}</td>
                <td className="p-4 border-b border-border">{rpc.type ?? "—"}</td>
                <td className="p-4 border-b border-border">
                  <span className={`font-mono font-bold ${(rpc.latency_ms ?? 999) < 50 ? 'text-success' : 'text-warning'}`}>
                    {rpc.latency_ms ?? "—"}ms
                  </span>
                </td>
                <td className="p-4 border-b border-border">
                  <span className={`inline-flex items-center px-2 py-1 rounded-full text-xs font-medium border ${rpc.health_status === 'HEALTHY' ? 'bg-success/10 text-success border-success/40' : 'bg-destructive/10 text-destructive border-destructive/40'}`}>
                    {rpc.health_status ?? "UNKNOWN"}
                  </span>
                </td>
              </motion.tr>
            ))}
          </tbody>
        </table>
      </div>
    );
  }

  return (
    <div className="p-8 space-y-6 min-h-screen text-foreground">
      <div className="flex justify-between items-center">
        <h1 className="text-3xl font-bold tracking-tight text-foreground">RPC Health &amp; Registry</h1>
        <div className="flex items-center gap-2 bg-success/10 text-success px-3 py-1.5 rounded-full border border-success/40">
          <Activity size={16} className="motion-safe:animate-pulse" />
          <span className="text-sm font-semibold tracking-wide">LIVE</span>
        </div>
      </div>

      <RpcSyncPanel />

      <BundleSyncPanel />

      {legacy}
    </div>
  );
}
