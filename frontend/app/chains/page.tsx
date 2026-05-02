"use client";
import React, { useEffect, useState } from "react";
import { Activity, Server, AlertCircle } from "lucide-react";
import { motion } from "framer-motion";

export default function ChainsPage() {
  const [chains, setChains] = useState<any[]>([]);
  const [rpcs, setRpcs] = useState<any[]>([]);
  const [loading, setLoading] = useState(true);
  // ZERO MOCKS DOCTRINE: This state holds the real API error message.
  // If the API is unreachable, we show the error — NEVER fabricate data.
  const [error, setError] = useState<string | null>(null);

  // ZERO MOCKS DOCTRINE (rule_00): NEVER inject fallback/dummy data in catch blocks.
  // If the API fails, display the error honestly. The operator must fix the backend.
  const API_URL = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:3000";

  useEffect(() => {
    Promise.all([
      fetch(`${API_URL}/api/chains`).then(res => res.json()),
      fetch(`${API_URL}/api/rpcs`).then(res => res.json()),
    ]).then(([chainsRes, rpcsRes]) => {
      setChains(chainsRes.data ?? []);
      setRpcs(rpcsRes.data ?? []);
      setLoading(false);
    }).catch((err) => {
      setError(err.message ?? "API unreachable");
      setLoading(false);
    });
  }, []);

  if (loading) return <div className="p-8 text-emerald-400 animate-pulse">Loading Chain Topology...</div>;

  if (error) return (
    <div className="p-8 min-h-screen bg-[#020617] text-slate-200">
      <div className="p-4 bg-rose-950/50 border border-rose-800 rounded-xl text-rose-300">
        <h3 className="font-bold">API ERROR — ZERO TRUST</h3>
        <p className="text-sm mt-1">{error}</p>
        <p className="text-xs mt-2 text-rose-400/70">The API server is unreachable. No fabricated data will be shown. Fix the backend connection.</p>
      </div>
    </div>
  );

  return (
    <div className="p-8 space-y-8 min-h-screen bg-[#020617] text-slate-200">
      <div className="flex justify-between items-center border-b border-slate-800 pb-4">
        <h1 className="text-4xl font-extrabold tracking-tight bg-gradient-to-r from-blue-400 to-emerald-400 bg-clip-text text-transparent">
          Network Topology
        </h1>
        <div className="flex items-center gap-2 bg-emerald-950/40 text-emerald-400 px-3 py-1.5 rounded-full border border-emerald-800/50">
          <Activity size={16} className="animate-pulse" />
          <span className="text-sm font-semibold tracking-wide">LIVE</span>
        </div>
      </div>

      <div className="grid grid-cols-1 xl:grid-cols-2 gap-8">
        <motion.div 
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          className="bg-[#0f172a]/80 backdrop-blur-md border border-slate-800 rounded-2xl shadow-[0_0_40px_-15px_rgba(59,130,246,0.2)] p-6"
        >
          <div className="flex items-center gap-3 mb-6">
            <Server className="text-blue-400" />
            <h2 className="text-xl font-bold">Monitored Chains</h2>
          </div>
          <table className="w-full text-left border-collapse">
            <thead>
              <tr className="text-slate-500 border-b border-slate-800 text-sm">
                <th className="pb-3">Chain ID</th>
                <th className="pb-3">Name</th>
                <th className="pb-3">Native</th>
                <th className="pb-3">Status</th>
              </tr>
            </thead>
            <tbody>
              {chains.map((chain, i) => (
                <tr key={i} className="border-b border-slate-800/50 hover:bg-slate-800/30 transition-colors">
                  <td className="py-4 font-mono text-blue-300">{chain.chain_id}</td>
                  <td className="py-4 font-medium">{chain.name}</td>
                  <td className="py-4 text-slate-400">{chain.native_currency}</td>
                  <td className="py-4">
                    <span className="px-2 py-1 rounded bg-emerald-500/10 text-emerald-400 text-xs font-bold border border-emerald-500/20">ACTIVE</span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </motion.div>

        <motion.div 
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.1 }}
          className="bg-[#0f172a]/80 backdrop-blur-md border border-slate-800 rounded-2xl shadow-[0_0_40px_-15px_rgba(16,185,129,0.15)] p-6"
        >
          <div className="flex items-center gap-3 mb-6">
            <AlertCircle className="text-emerald-400" />
            <h2 className="text-xl font-bold">RPC Health (Kernel Bypass)</h2>
          </div>
          <table className="w-full text-left border-collapse">
            <thead>
              <tr className="text-slate-500 border-b border-slate-800 text-sm">
                <th className="pb-3">URL</th>
                <th className="pb-3">Type</th>
                <th className="pb-3">Latency</th>
                <th className="pb-3">Status</th>
              </tr>
            </thead>
            <tbody>
              {rpcs.map((rpc, i) => (
                <tr key={i} className="border-b border-slate-800/50 hover:bg-slate-800/30 transition-colors">
                  <td className="py-4 font-mono text-xs text-slate-400 truncate max-w-[150px]">{rpc.url}</td>
                  <td className="py-4 font-medium text-slate-300">{rpc.type}</td>
                  <td className="py-4">
                    <span className={`font-mono font-bold ${rpc.latency_ms < 50 ? 'text-emerald-400' : 'text-amber-400'}`}>
                      {rpc.latency_ms}ms
                    </span>
                  </td>
                  <td className="py-4">
                    <span className={`px-2 py-1 rounded text-xs font-bold border ${rpc.health_status === 'HEALTHY' ? 'bg-emerald-500/10 text-emerald-400 border-emerald-500/20' : 'bg-rose-500/10 text-rose-400 border-rose-500/20'}`}>
                      {rpc.health_status}
                    </span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </motion.div>
      </div>
    </div>
  );
}
