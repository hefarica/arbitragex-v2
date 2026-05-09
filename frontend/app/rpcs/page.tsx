// ZERO MOCKS DOCTRINE (rule_00): This page fetches data ONLY through the edge.
// NEVER hardcode fallback data or bypass the edge layer.
"use client";
import React, { useEffect, useState } from "react";
import { Activity, AlertTriangle } from "lucide-react";
import { motion } from "framer-motion";
import { getDefiRpcs } from "@/lib/api-client";
import type { DefiRpcsResponse } from "@/lib/schemas";

export default function RpcHealthPage() {
  const [result, setResult] = useState<{ ok: true; data: DefiRpcsResponse } | { ok: false; error: string } | null>(null);

  useEffect(() => {
    getDefiRpcs().then(setResult);
  }, []);

  if (!result) return <div className="p-8 text-success animate-pulse">Loading RPC Registry...</div>;

  if (!result.ok) return (
    <div className="p-8 min-h-screen text-foreground">
      <div className="p-4 bg-destructive/10 border border-destructive/30 rounded-xl text-destructive">
        <h3 className="font-bold flex items-center gap-2"><AlertTriangle size={18} /> EDGE ERROR — ZERO TRUST</h3>
        <p className="text-sm mt-1">{result.error}</p>
        <p className="text-xs mt-2 text-destructive/70">The edge/API server is unreachable. No fabricated RPC data will be shown.</p>
      </div>
    </div>
  );

  const rpcs = result.data.data;

  return (
    <div className="p-8 space-y-6 min-h-screen text-foreground">
      <div className="flex justify-between items-center">
        <h1 className="text-3xl font-bold tracking-tight text-foreground">RPC Health &amp; Registry</h1>
        <div className="flex items-center gap-2 bg-success/10 text-success px-3 py-1.5 rounded-full border border-success/40">
          <Activity size={16} className="animate-pulse" />
          <span className="text-sm font-semibold tracking-wide">LIVE</span>
        </div>
      </div>

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
                key={i}
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
            {rpcs.length === 0 && (
              <tr>
                <td colSpan={5} className="p-8 text-center text-muted-foreground italic">No RPCs registered. Waiting for backend data...</td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
