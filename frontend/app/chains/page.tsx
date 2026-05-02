// ZERO MOCKS DOCTRINE (rule_00): This page fetches data ONLY through the edge.
// NEVER hardcode fallback data or bypass the edge layer.
// If the edge/api-server is unreachable, show the error honestly.
"use client";
import React, { useEffect, useState } from "react";
import { Activity, AlertTriangle } from "lucide-react";
import { motion } from "framer-motion";
import { getDefiChains } from "@/lib/api-client";
import type { DefiChainsResponse } from "@/lib/schemas";

export default function ChainsPage() {
  const [result, setResult] = useState<{ ok: true; data: DefiChainsResponse } | { ok: false; error: string } | null>(null);

  useEffect(() => {
    getDefiChains().then(setResult);
  }, []);

  if (!result) return <div className="p-8 text-emerald-400 animate-pulse">Loading Chains Registry...</div>;

  if (!result.ok) return (
    <div className="p-8 min-h-screen bg-[#020617] text-slate-200">
      <div className="p-4 bg-rose-950/50 border border-rose-800 rounded-xl text-rose-300">
        <h3 className="font-bold flex items-center gap-2"><AlertTriangle size={18} /> EDGE ERROR — ZERO TRUST</h3>
        <p className="text-sm mt-1">{result.error}</p>
        <p className="text-xs mt-2 text-rose-400/70">The edge/API server is unreachable. No fabricated chain data will be shown.</p>
      </div>
    </div>
  );

  const chains = result.data.data;

  return (
    <div className="p-8 space-y-6 min-h-screen bg-[#020617] text-slate-200">
      <div className="flex justify-between items-center">
        <h1 className="text-3xl font-bold tracking-tight text-slate-100">Chain Registry</h1>
        <div className="flex items-center gap-2 bg-emerald-950/40 text-emerald-400 px-3 py-1.5 rounded-full border border-emerald-800/50">
          <Activity size={16} className="animate-pulse" />
          <span className="text-sm font-semibold tracking-wide">LIVE</span>
        </div>
      </div>

      <div className="bg-slate-900 border border-slate-800 rounded-xl overflow-hidden shadow-2xl">
        <table className="w-full text-left border-collapse">
          <thead>
            <tr className="bg-slate-800 text-slate-400 text-sm uppercase tracking-wider">
              <th className="p-4 border-b border-slate-700">Chain ID</th>
              <th className="p-4 border-b border-slate-700">Name</th>
              <th className="p-4 border-b border-slate-700">RPC URL</th>
              <th className="p-4 border-b border-slate-700">Status</th>
            </tr>
          </thead>
          <tbody className="text-slate-300">
            {chains.map((chain, i) => (
              <motion.tr
                key={chain.chain_id}
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                transition={{ delay: i * 0.05 }}
                className="hover:bg-slate-800/50 transition-colors"
              >
                <td className="p-4 border-b border-slate-800 font-mono text-blue-400">{chain.chain_id}</td>
                <td className="p-4 border-b border-slate-800 font-medium">{chain.name}</td>
                <td className="p-4 border-b border-slate-800 font-mono text-sm text-slate-400 truncate max-w-[300px]">{chain.rpc_url ?? "—"}</td>
                <td className="p-4 border-b border-slate-800">
                  <span className={`inline-flex items-center px-2 py-1 rounded-full text-xs font-medium border ${chain.is_active ? 'bg-emerald-900/30 text-emerald-400 border-emerald-800/50' : 'bg-rose-900/30 text-rose-400 border-rose-800/50'}`}>
                    {chain.is_active ? "ACTIVE" : "DISABLED"}
                  </span>
                </td>
              </motion.tr>
            ))}
            {chains.length === 0 && (
              <tr>
                <td colSpan={4} className="p-8 text-center text-slate-500 italic">No chains registered. Waiting for backend data...</td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
