// ZERO MOCKS DOCTRINE (rule_00): This page fetches data ONLY through the edge.
"use client";
import React, { useEffect, useState } from "react";
import { Cpu, AlertTriangle, Zap, Clock, HardDrive, Activity } from "lucide-react";
import { motion } from "framer-motion";
import { getDefiMetrics } from "@/lib/api-client";
import type { DefiMetricsResponse } from "@/lib/schemas";

export default function WorkerHealthPage() {
  const [result, setResult] = useState<{ ok: true; data: DefiMetricsResponse } | { ok: false; error: string } | null>(null);

  useEffect(() => {
    getDefiMetrics().then(setResult);
  }, []);

  if (!result) return <div className="p-8 text-emerald-400 animate-pulse">Booting Telemetry...</div>;

  if (!result.ok) return (
    <div className="p-8 min-h-screen bg-[#020617] text-slate-200">
      <div className="p-4 bg-rose-950/50 border border-rose-800 rounded-xl text-rose-300">
        <h3 className="font-bold flex items-center gap-2"><AlertTriangle size={18} /> EDGE ERROR — ZERO TRUST</h3>
        <p className="text-sm mt-1">{result.error}</p>
        <p className="text-xs mt-2 text-rose-400/70">The edge/API server is unreachable. No fabricated telemetry will be shown.</p>
      </div>
    </div>
  );

  const metrics = result.data.data;

  return (
    <div className="p-8 min-h-screen bg-[#020617] text-slate-200 space-y-8">
      <div className="flex justify-between items-center">
        <h1 className="text-3xl font-bold tracking-tight text-slate-100">Worker Health &amp; Telemetry</h1>
        <div className="flex items-center gap-2 bg-emerald-950/40 text-emerald-400 px-3 py-1.5 rounded-full border border-emerald-800/50">
          <Activity size={16} className="animate-pulse" />
          <span className="text-sm font-semibold tracking-wide">LIVE</span>
        </div>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
        <motion.div initial={{ scale: 0.9, opacity: 0 }} animate={{ scale: 1, opacity: 1 }} className="p-6 rounded-2xl border shadow-xl bg-slate-900 border-slate-800">
          <div className="flex items-center gap-3 mb-3"><Zap className="text-amber-400" size={20} /><span className="text-slate-400 text-sm uppercase">Active Workers</span></div>
          <p className="text-3xl font-bold text-slate-100">{metrics.active_workers}</p>
        </motion.div>

        <motion.div initial={{ scale: 0.9, opacity: 0 }} animate={{ scale: 1, opacity: 1 }} transition={{ delay: 0.1 }} className="p-6 rounded-2xl border shadow-xl bg-slate-900 border-slate-800">
          <div className="flex items-center gap-3 mb-3"><Cpu className="text-blue-400" size={20} /><span className="text-slate-400 text-sm uppercase">CPU Usage</span></div>
          <p className="text-3xl font-bold text-slate-100">{metrics.cpu_usage_pct.toFixed(1)}%</p>
        </motion.div>

        <motion.div initial={{ scale: 0.9, opacity: 0 }} animate={{ scale: 1, opacity: 1 }} transition={{ delay: 0.2 }} className="p-6 rounded-2xl border shadow-xl bg-slate-900 border-slate-800">
          <div className="flex items-center gap-3 mb-3"><HardDrive className="text-purple-400" size={20} /><span className="text-slate-400 text-sm uppercase">Memory</span></div>
          <p className="text-3xl font-bold text-slate-100">{metrics.memory_usage_mb.toFixed(0)} MB</p>
        </motion.div>

        <motion.div initial={{ scale: 0.9, opacity: 0 }} animate={{ scale: 1, opacity: 1 }} transition={{ delay: 0.3 }} className={`p-6 rounded-2xl border shadow-xl ${metrics.kernel_bypass_active ? 'bg-emerald-950/20 border-emerald-900/50' : 'bg-slate-900 border-slate-800'}`}>
          <div className="flex items-center gap-3 mb-3"><Clock className="text-emerald-400" size={20} /><span className="text-slate-400 text-sm uppercase">Kernel Bypass</span></div>
          <p className={`text-2xl font-bold ${metrics.kernel_bypass_active ? 'text-emerald-400' : 'text-slate-500'}`}>
            {metrics.kernel_bypass_active ? 'ACTIVE' : 'DISABLED'}
          </p>
        </motion.div>
      </div>

      <div className="p-6 bg-slate-900 border border-slate-800 rounded-2xl">
        <h2 className="text-lg font-semibold text-slate-300 mb-2">Uptime</h2>
        <p className="text-4xl font-mono font-bold text-emerald-400">{(metrics.uptime_seconds / 3600).toFixed(1)}h</p>
      </div>
    </div>
  );
}
