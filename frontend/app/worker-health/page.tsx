"use client";
import React, { useEffect, useState } from "react";
import { Activity, Cpu, HardDrive, Clock, TerminalSquare } from "lucide-react";
import { motion } from "framer-motion";

export default function WorkerHealthPage() {
  const [metrics, setMetrics] = useState<any>(null);
  // ZERO MOCKS DOCTRINE: Hold the real error. NEVER fabricate worker metrics.
  const [error, setError] = useState<string | null>(null);

  // ZERO MOCKS DOCTRINE (rule_00): NEVER inject fallback/dummy data in catch blocks.
  // If the API fails, display the error honestly. The operator must fix the backend.
  const API_URL = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:3000";

  useEffect(() => {
    fetch(`${API_URL}/api/metrics`)
      .then(res => res.json())
      .then(data => setMetrics(data.data))
      .catch((err) => {
        setError(err.message ?? "API unreachable");
      });
  }, []);

  if (!metrics && !error) return <div className="p-8 text-emerald-400">Booting Telemetry...</div>;

  if (error) return (
    <div className="p-8 min-h-screen bg-[#020617] text-slate-200">
      <div className="p-4 bg-rose-950/50 border border-rose-800 rounded-xl text-rose-300">
        <h3 className="font-bold">API ERROR — ZERO TRUST</h3>
        <p className="text-sm mt-1">{error}</p>
        <p className="text-xs mt-2 text-rose-400/70">The API server is unreachable. No fabricated telemetry will be shown.</p>
      </div>
    </div>
  );

  return (
    <div className="p-8 min-h-screen bg-[#020617] text-slate-200 space-y-8">
      <div className="flex justify-between items-center border-b border-slate-800 pb-4">
        <h1 className="text-4xl font-extrabold tracking-tight text-white flex items-center gap-3">
          <TerminalSquare className="text-emerald-400" size={32} />
          Worker Telemetry
        </h1>
        <div className="text-xs font-mono text-slate-400 bg-slate-900 px-3 py-1 rounded border border-slate-800">
          Uptime: {(metrics.uptime_seconds / 3600).toFixed(1)}h
        </div>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
        <motion.div initial={{ scale: 0.9, opacity: 0 }} animate={{ scale: 1, opacity: 1 }} className="bg-slate-900 p-6 rounded-2xl border border-slate-800 shadow-xl relative overflow-hidden">
          <div className="absolute top-0 right-0 p-4 opacity-10"><Activity size={64} /></div>
          <p className="text-slate-400 text-sm font-bold uppercase tracking-wider mb-2">Tokio Tasks (Workers)</p>
          <p className="text-4xl font-mono text-emerald-400">{metrics.active_workers}</p>
        </motion.div>

        <motion.div initial={{ scale: 0.9, opacity: 0 }} animate={{ scale: 1, opacity: 1 }} transition={{ delay: 0.1 }} className="bg-slate-900 p-6 rounded-2xl border border-slate-800 shadow-xl relative overflow-hidden">
          <div className="absolute top-0 right-0 p-4 opacity-10"><Cpu size={64} /></div>
          <p className="text-slate-400 text-sm font-bold uppercase tracking-wider mb-2">CPU Utilization</p>
          <p className="text-4xl font-mono text-blue-400">{metrics.cpu_usage_pct}%</p>
        </motion.div>

        <motion.div initial={{ scale: 0.9, opacity: 0 }} animate={{ scale: 1, opacity: 1 }} transition={{ delay: 0.2 }} className="bg-slate-900 p-6 rounded-2xl border border-slate-800 shadow-xl relative overflow-hidden">
          <div className="absolute top-0 right-0 p-4 opacity-10"><HardDrive size={64} /></div>
          <p className="text-slate-400 text-sm font-bold uppercase tracking-wider mb-2">Memory Allocation</p>
          <p className="text-4xl font-mono text-indigo-400">{metrics.memory_usage_mb} MB</p>
        </motion.div>

        <motion.div initial={{ scale: 0.9, opacity: 0 }} animate={{ scale: 1, opacity: 1 }} transition={{ delay: 0.3 }} className={`p-6 rounded-2xl border shadow-xl relative overflow-hidden ${metrics.kernel_bypass_active ? 'bg-emerald-950/20 border-emerald-900/50' : 'bg-slate-900 border-slate-800'}`}>
          <div className="absolute top-0 right-0 p-4 opacity-10"><Clock size={64} /></div>
          <p className="text-slate-400 text-sm font-bold uppercase tracking-wider mb-2">Kernel Bypassing</p>
          <p className={`text-2xl font-bold ${metrics.kernel_bypass_active ? 'text-emerald-400' : 'text-slate-500'}`}>
            {metrics.kernel_bypass_active ? 'ACTIVE (Skill 082)' : 'DISABLED'}
          </p>
        </motion.div>
      </div>
    </div>
  );
}
