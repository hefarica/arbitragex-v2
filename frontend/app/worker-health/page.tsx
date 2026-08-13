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

  if (!result) return <div className="p-8 text-success animate-pulse">Booting Telemetry...</div>;

  if (!result.ok) return (
    <div className="p-8 min-h-screen text-foreground">
      <div className="p-4 bg-destructive/10 border border-destructive/30 rounded-xl text-destructive">
        <h3 className="font-bold flex items-center gap-2"><AlertTriangle size={18} /> EDGE ERROR — ZERO TRUST</h3>
        <p className="text-sm mt-1">{result.error}</p>
        <p className="text-xs mt-2 text-destructive/70">The edge/API server is unreachable. No fabricated telemetry will be shown.</p>
      </div>
    </div>
  );

  const metrics = result.data.data;

  return (
    <div className="p-8 min-h-screen text-foreground space-y-8">
      <div className="flex justify-between items-center">
        <h1 className="text-3xl font-bold tracking-tight text-foreground">Worker Health &amp; Telemetry</h1>
        <div className="flex items-center gap-2 bg-success/10 text-success px-3 py-1.5 rounded-full border border-success/40">
          <Activity size={16} className="animate-pulse" />
          <span className="text-sm font-semibold tracking-wide">{metrics.active_workers > 0 ? "LIVE" : "NO ACTIVE WORKERS"}</span>
        </div>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
        <motion.div data-slot="card" initial={{ scale: 0.9, opacity: 0 }} animate={{ scale: 1, opacity: 1 }} className="p-6 rounded-2xl border shadow-xl bg-card text-card-foreground border-border">
          <div className="flex items-center gap-3 mb-3"><Zap className="text-warning" size={20} /><span className="text-muted-foreground text-sm uppercase">Active Workers</span></div>
          <p className="text-3xl font-bold text-foreground">{metrics.active_workers}</p>
        </motion.div>

        <motion.div data-slot="card" initial={{ scale: 0.9, opacity: 0 }} animate={{ scale: 1, opacity: 1 }} transition={{ delay: 0.1 }} className="p-6 rounded-2xl border shadow-xl bg-card text-card-foreground border-border">
          <div className="flex items-center gap-3 mb-3"><Cpu className="text-info" size={20} /><span className="text-muted-foreground text-sm uppercase">CPU Usage</span></div>
          <p className="text-3xl font-bold text-foreground">{metrics.cpu_usage_pct.toFixed(1)}%</p>
        </motion.div>

        <motion.div data-slot="card" initial={{ scale: 0.9, opacity: 0 }} animate={{ scale: 1, opacity: 1 }} transition={{ delay: 0.2 }} className="p-6 rounded-2xl border shadow-xl bg-card text-card-foreground border-border">
          <div className="flex items-center gap-3 mb-3"><HardDrive className="text-chart-3" size={20} /><span className="text-muted-foreground text-sm uppercase">Memory</span></div>
          <p className="text-3xl font-bold text-foreground">{metrics.memory_usage_mb.toFixed(0)} MB</p>
        </motion.div>

        <motion.div data-slot="card" initial={{ scale: 0.9, opacity: 0 }} animate={{ scale: 1, opacity: 1 }} transition={{ delay: 0.3 }} className={`p-6 rounded-2xl border shadow-xl bg-card text-card-foreground ${metrics.kernel_bypass_active ? 'border-success/40' : 'border-border'}`}>
          <div className="flex items-center gap-3 mb-3"><Clock className="text-success" size={20} /><span className="text-muted-foreground text-sm uppercase">Kernel Bypass</span></div>
          <p className={`text-2xl font-bold ${metrics.kernel_bypass_active ? 'text-success' : 'text-muted-foreground'}`}>
            {metrics.kernel_bypass_active ? 'ACTIVE' : 'DISABLED'}
          </p>
        </motion.div>
      </div>

      <div data-slot="card" className="p-6 bg-card text-card-foreground border border-border rounded-2xl">
        <h2 className="text-lg font-semibold text-foreground mb-2">Uptime</h2>
        <p className="text-4xl font-mono font-bold text-success">{(metrics.uptime_seconds / 3600).toFixed(1)}h</p>
      </div>
    </div>
  );
}
