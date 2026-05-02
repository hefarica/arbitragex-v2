"use client";
import React, { useEffect, useState } from "react";
import { Zap, WifiOff, ShieldAlert } from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";
import { io } from "socket.io-client";
import { createOpportunitySocket, type Opportunity } from "@/features/opportunities/socket-lifecycle";

export default function OpportunitiesPage() {
  const [opportunities, setOpportunities] = useState<Opportunity[]>([]);
  const [wsStatus, setWsStatus] = useState<"CONNECTING" | "LIVE" | "STALE">("CONNECTING");

  const API_URL =
    process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:3000";

  useEffect(() => {
    const handle = createOpportunitySocket({
      url: API_URL,
      ioFactory: io,
      onStatus: setWsStatus,
      onOpportunity: (opp) => setOpportunities((prev) => [opp, ...prev].slice(0, 20)),
    });
    return () => handle.dispose();
    // Effect owns the socket for the component lifetime — see socket-lifecycle.ts.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <div className={`p-8 min-h-screen transition-colors duration-500 ${wsStatus === 'STALE' ? 'bg-rose-950/20' : 'bg-[#020617]'} text-slate-200`}>
      <div className="flex justify-between items-center border-b border-slate-800 pb-4 mb-8">
        <div>
          <h1 className={`text-4xl font-extrabold tracking-tight bg-clip-text text-transparent ${wsStatus === 'STALE' ? 'bg-gradient-to-r from-rose-500 to-red-600' : 'bg-gradient-to-r from-emerald-400 to-teal-400'}`}>
            Live MEV Feed
          </h1>
          <p className="text-slate-500 mt-2 text-sm">Differential Snapshots via WebSockets</p>
        </div>
        
        <div className={`flex items-center gap-2 px-4 py-2 rounded-full border shadow-lg ${
          wsStatus === 'LIVE' ? 'bg-emerald-900/30 border-emerald-500/50 text-emerald-400' : 
          wsStatus === 'STALE' ? 'bg-rose-900/50 border-rose-500/80 text-rose-400 shadow-rose-900/50 animate-pulse' : 
          'bg-amber-900/30 border-amber-500/50 text-amber-400'
        }`}>
          {wsStatus === 'LIVE' ? <Zap size={18} className="animate-pulse" /> : wsStatus === 'STALE' ? <ShieldAlert size={18} /> : <WifiOff size={18} />}
          <span className="text-sm font-bold tracking-widest">{wsStatus}</span>
        </div>
      </div>

      {wsStatus === 'STALE' && (
        <div className="mb-8 p-4 bg-rose-950/50 border border-rose-800 rounded-xl flex items-center gap-4 text-rose-300">
          <ShieldAlert size={24} />
          <div>
            <h3 className="font-bold">ZERO-TRUST PROTOCOL TRIGGERED</h3>
            <p className="text-sm">WebSocket connection lost. Displayed data is considered STALE. Manual execution is LOCKED to prevent financial loss.</p>
          </div>
        </div>
      )}

      <div className="bg-[#0f172a]/80 backdrop-blur-md border border-slate-800 rounded-2xl shadow-2xl overflow-hidden">
        <table className="w-full text-left border-collapse">
          <thead>
            <tr className="bg-slate-900 text-slate-400 text-sm uppercase tracking-wider">
              <th className="p-4 border-b border-slate-800">Time</th>
              <th className="p-4 border-b border-slate-800">Route</th>
              <th className="p-4 border-b border-slate-800 text-right">Net Profit (USD)</th>
              <th className="p-4 border-b border-slate-800 text-right">Net ROI</th>
              <th className="p-4 border-b border-slate-800 text-center">Score</th>
              <th className="p-4 border-b border-slate-800 text-center">Action</th>
            </tr>
          </thead>
          <tbody>
            <AnimatePresence>
              {opportunities.map((opp) => (
                <motion.tr 
                  key={opp.id}
                  initial={{ opacity: 0, x: -20, backgroundColor: "rgba(16,185,129,0.2)" }}
                  animate={{ opacity: 1, x: 0, backgroundColor: "transparent" }}
                  exit={{ opacity: 0 }}
                  transition={{ duration: 0.5 }}
                  className="border-b border-slate-800/50 hover:bg-slate-800/30"
                >
                  <td className="p-4 font-mono text-xs text-slate-500">{new Date(opp.timestamp).toLocaleTimeString()}</td>
                  <td className="p-4 font-medium text-blue-300">{opp.route}</td>
                  <td className="p-4 text-right font-mono font-bold text-emerald-400">${opp.expected_profit_usd}</td>
                  <td className="p-4 text-right font-mono text-slate-300">{opp.net_roi_pct}%</td>
                  <td className="p-4 text-center">
                    <span className={`px-2 py-1 rounded text-xs font-bold ${opp.score > 90 ? 'bg-emerald-500/20 text-emerald-400' : 'bg-blue-500/20 text-blue-400'}`}>
                      {opp.score}
                    </span>
                  </td>
                  <td className="p-4 text-center">
                    <button 
                      disabled={wsStatus === 'STALE'}
                      className="px-4 py-1.5 rounded bg-indigo-600 hover:bg-indigo-500 disabled:opacity-50 disabled:cursor-not-allowed text-white text-xs font-bold transition-colors shadow-lg shadow-indigo-900/20"
                    >
                      SIMULATE
                    </button>
                  </td>
                </motion.tr>
              ))}
            </AnimatePresence>
            {opportunities.length === 0 && (
              <tr>
                <td colSpan={6} className="p-8 text-center text-slate-500 italic">No opportunities detected. Waiting for mempool deltas...</td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
