// ZERO MOCKS DOCTRINE (rule_00): This page MUST fetch data exclusively from the API.
// NEVER hardcode table rows, dummy data, or fallback values.
// If the API is unreachable, show the real error — the operator must fix the backend.
"use client";
import React, { useEffect, useState } from "react";
import { Activity } from "lucide-react";
import { motion } from "framer-motion";

export const metadata = {
  title: "RPC Health Registry | ArbitrageX",
};

export default function RpcHealthPage() {
  const [rpcs, setRpcs] = useState<any[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const API_URL = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:3000";

  useEffect(() => {
    fetch(`${API_URL}/api/rpcs`)
      .then(res => res.json())
      .then(data => {
        setRpcs(data.data ?? []);
        setLoading(false);
      })
      .catch((err) => {
        setError(err.message ?? "API unreachable");
        setLoading(false);
      });
  }, []);

  if (loading) return <div className="p-8 text-emerald-400 animate-pulse">Loading RPC Registry...</div>;

  if (error) return (
    <div className="p-8 min-h-screen bg-[#020617] text-slate-200">
      <div className="p-4 bg-rose-950/50 border border-rose-800 rounded-xl text-rose-300">
        <h3 className="font-bold">API ERROR — ZERO TRUST</h3>
        <p className="text-sm mt-1">{error}</p>
        <p className="text-xs mt-2 text-rose-400/70">The API server is unreachable. No fabricated RPC data will be shown.</p>
      </div>
    </div>
  );

  return (
    <div className="p-8 space-y-6 min-h-screen bg-[#020617] text-slate-200">
      <div className="flex justify-between items-center">
        <h1 className="text-3xl font-bold tracking-tight text-slate-100">
          RPC Health &amp; Registry
        </h1>
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
              <th className="p-4 border-b border-slate-700">URL</th>
              <th className="p-4 border-b border-slate-700">Type</th>
              <th className="p-4 border-b border-slate-700">Latency</th>
              <th className="p-4 border-b border-slate-700">Status</th>
            </tr>
          </thead>
          <tbody className="text-slate-300">
            {rpcs.map((rpc, i) => (
              <motion.tr
                key={rpc.id ?? i}
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                transition={{ delay: i * 0.05 }}
                className="hover:bg-slate-800/50 transition-colors"
              >
                <td className="p-4 border-b border-slate-800">{rpc.chain_id ?? "—"}</td>
                <td className="p-4 border-b border-slate-800 font-mono text-sm text-blue-400 truncate max-w-[250px]">{rpc.url}</td>
                <td className="p-4 border-b border-slate-800">{rpc.type}</td>
                <td className="p-4 border-b border-slate-800">
                  <span className={`font-mono font-bold ${(rpc.latency_ms ?? 999) < 50 ? 'text-emerald-400' : 'text-amber-400'}`}>
                    {rpc.latency_ms ?? "—"}ms
                  </span>
                </td>
                <td className="p-4 border-b border-slate-800">
                  <span className={`inline-flex items-center px-2 py-1 rounded-full text-xs font-medium border ${rpc.health_status === 'HEALTHY' ? 'bg-emerald-900/30 text-emerald-400 border-emerald-800/50' : 'bg-rose-900/30 text-rose-400 border-rose-800/50'}`}>
                    {rpc.health_status ?? "UNKNOWN"}
                  </span>
                </td>
              </motion.tr>
            ))}
            {rpcs.length === 0 && (
              <tr>
                <td colSpan={5} className="p-8 text-center text-slate-500 italic">No RPCs registered. Waiting for backend data...</td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
