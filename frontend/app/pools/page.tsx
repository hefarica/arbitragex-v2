"use client";
import React, { useEffect, useState } from "react";
import { Droplet, TrendingUp } from "lucide-react";
import { motion } from "framer-motion";

export default function PoolsPage() {
  const [pools, setPools] = useState<any[]>([]);
  const [loading, setLoading] = useState(true);
  // ZERO MOCKS DOCTRINE: Hold the real error. NEVER fabricate pool data.
  const [error, setError] = useState<string | null>(null);

  // ZERO MOCKS DOCTRINE (rule_00): NEVER inject fallback/dummy data in catch blocks.
  // If the API fails, display the error honestly. The operator must fix the backend.
  const API_URL = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:3000";

  useEffect(() => {
    fetch(`${API_URL}/api/pools`)
      .then(res => res.json())
      .then(data => {
        setPools(data.data ?? []);
        setLoading(false);
      })
      .catch((err) => {
        setError(err.message ?? "API unreachable");
        setLoading(false);
      });
  }, []);

  if (loading) return <div className="p-8 text-blue-400 animate-pulse">Loading Pool State...</div>;

  if (error) return (
    <div className="p-8 min-h-screen bg-[#020617] text-slate-200">
      <div className="p-4 bg-rose-950/50 border border-rose-800 rounded-xl text-rose-300">
        <h3 className="font-bold">API ERROR — ZERO TRUST</h3>
        <p className="text-sm mt-1">{error}</p>
        <p className="text-xs mt-2 text-rose-400/70">The API server is unreachable. No fabricated pool data will be shown.</p>
      </div>
    </div>
  );

  return (
    <div className="p-8 space-y-8 min-h-screen bg-[#020617] text-slate-200">
      <div className="flex justify-between items-center border-b border-slate-800 pb-4">
        <h1 className="text-4xl font-extrabold tracking-tight bg-gradient-to-r from-blue-400 to-indigo-400 bg-clip-text text-transparent">
          Live Pool Explorer
        </h1>
        <div className="flex items-center gap-2 bg-indigo-950/40 text-indigo-400 px-3 py-1.5 rounded-full border border-indigo-800/50">
          <Droplet size={16} />
          <span className="text-sm font-semibold tracking-wide">SYNC_WORKER ACTIVE</span>
        </div>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
        {pools.map((pool, i) => (
          <motion.div 
            key={i}
            initial={{ opacity: 0, scale: 0.95 }}
            animate={{ opacity: 1, scale: 1 }}
            transition={{ delay: i * 0.1 }}
            className="bg-[#0f172a] border border-slate-800 rounded-2xl shadow-xl overflow-hidden group hover:border-indigo-500/50 transition-all duration-300"
          >
            <div className="p-6 border-b border-slate-800/50 flex justify-between items-center">
              <div>
                <h3 className="font-bold text-lg text-slate-100 flex items-center gap-2">
                  {pool.token0} / {pool.token1}
                  <span className="px-2 py-0.5 rounded bg-slate-800 text-xs text-slate-400 font-mono border border-slate-700">{pool.fee / 10000}%</span>
                </h3>
                <p className="font-mono text-xs text-slate-500 mt-1">{pool.address}</p>
              </div>
              <div className="bg-blue-500/10 p-2 rounded-lg text-blue-400">
                <TrendingUp size={20} />
              </div>
            </div>
            
            <div className="p-6 grid grid-cols-2 gap-4 bg-slate-900/50">
              <div>
                <p className="text-xs text-slate-500 uppercase tracking-wider mb-1">Reserve 0 ({pool.token0})</p>
                <p className="font-mono font-semibold text-slate-300 text-lg">{Number(pool.reserve0).toLocaleString()}</p>
              </div>
              <div>
                <p className="text-xs text-slate-500 uppercase tracking-wider mb-1">Reserve 1 ({pool.token1})</p>
                <p className="font-mono font-semibold text-slate-300 text-lg">{Number(pool.reserve1).toLocaleString()}</p>
              </div>
            </div>
          </motion.div>
        ))}
      </div>
    </div>
  );
}
