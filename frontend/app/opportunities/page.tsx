"use client";
import React, { useEffect, useState, useCallback, useRef } from "react";
import { Zap, WifiOff, ShieldAlert, RefreshCw, Radio } from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";

interface Opportunity {
  id: string;
  timestamp: number | string;
  route: string;
  expected_profit_usd: number;
  net_roi_pct: number;
  score: number;
}

type FeedStatus = "POLLING" | "LIVE" | "ERROR";

const POLL_INTERVAL_MS = 5_000; // 5s — matches searcher-rs scan cycle

export default function OpportunitiesPage() {
  const [opportunities, setOpportunities] = useState<Opportunity[]>([]);
  const [feedStatus, setFeedStatus] = useState<FeedStatus>("POLLING");
  const [lastRefresh, setLastRefresh] = useState<Date | null>(null);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const EDGE_URL =
    process.env.NEXT_PUBLIC_EDGE_URL ?? "http://localhost:8787";

  const fetchOpportunities = useCallback(async () => {
    try {
      const res = await fetch(`${EDGE_URL}/api/opportunities/live`, {
        headers: { accept: "application/json" },
        signal: AbortSignal.timeout(4000),
      });
      if (!res.ok) {
        setFeedStatus("ERROR");
        setErrorMsg(`Edge returned ${res.status}`);
        return;
      }
      const data = await res.json();
      setOpportunities(data.items ?? []);
      setFeedStatus("POLLING");
      setLastRefresh(new Date());
      setErrorMsg(null);
    } catch (e) {
      setFeedStatus("ERROR");
      setErrorMsg((e as Error).message);
    }
  }, [EDGE_URL]);

  useEffect(() => {
    fetchOpportunities();
    intervalRef.current = setInterval(fetchOpportunities, POLL_INTERVAL_MS);
    return () => {
      if (intervalRef.current) clearInterval(intervalRef.current);
    };
  }, [fetchOpportunities]);

  return (
    <div className={`p-8 min-h-screen transition-colors duration-500 ${feedStatus === 'ERROR' ? 'bg-rose-950/20' : 'bg-[#020617]'} text-slate-200`}>
      <div className="flex justify-between items-center border-b border-slate-800 pb-4 mb-8">
        <div>
          <h1 className={`text-4xl font-extrabold tracking-tight bg-clip-text text-transparent ${feedStatus === 'ERROR' ? 'bg-gradient-to-r from-rose-500 to-red-600' : 'bg-gradient-to-r from-emerald-400 to-teal-400'}`}>
            Live MEV Feed
          </h1>
          <p className="text-slate-500 mt-2 text-sm">
            Polling edge every {POLL_INTERVAL_MS / 1000}s · {lastRefresh ? `Last: ${lastRefresh.toLocaleTimeString()}` : "Loading..."}
          </p>
        </div>
        
        <div className="flex items-center gap-3">
          <button 
            onClick={fetchOpportunities}
            className="p-2 rounded-lg bg-slate-800 hover:bg-slate-700 transition-colors border border-slate-700"
            title="Force refresh"
          >
            <RefreshCw size={16} className="text-slate-400" />
          </button>
          <div className={`flex items-center gap-2 px-4 py-2 rounded-full border shadow-lg ${
            feedStatus === 'POLLING' ? 'bg-emerald-900/30 border-emerald-500/50 text-emerald-400' : 
            feedStatus === 'ERROR' ? 'bg-rose-900/50 border-rose-500/80 text-rose-400 shadow-rose-900/50 animate-pulse' : 
            'bg-cyan-900/30 border-cyan-500/50 text-cyan-400'
          }`}>
            {feedStatus === 'POLLING' ? <Radio size={18} className="animate-pulse" /> : feedStatus === 'ERROR' ? <ShieldAlert size={18} /> : <Zap size={18} />}
            <span className="text-sm font-bold tracking-widest">{feedStatus}</span>
          </div>
        </div>
      </div>

      {feedStatus === 'ERROR' && (
        <div className="mb-8 p-4 bg-rose-950/50 border border-rose-800 rounded-xl flex items-center gap-4 text-rose-300">
          <ShieldAlert size={24} />
          <div>
            <h3 className="font-bold">EDGE CONNECTION ERROR</h3>
            <p className="text-sm">Cannot reach edge API: {errorMsg}. Retrying every {POLL_INTERVAL_MS / 1000}s.</p>
          </div>
        </div>
      )}

      {feedStatus === 'POLLING' && opportunities.length === 0 && (
        <div className="mb-8 p-4 bg-slate-800/50 border border-slate-700 rounded-xl flex items-center gap-4 text-slate-400">
          <Radio size={24} />
          <div>
            <h3 className="font-bold">SCANNING MEMPOOL</h3>
            <p className="text-sm">Searcher is actively monitoring. Paper-mode enabled — no capital at risk. Opportunities appear here when detected.</p>
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
                      className="px-4 py-1.5 rounded bg-indigo-600 hover:bg-indigo-500 text-white text-xs font-bold transition-colors shadow-lg shadow-indigo-900/20"
                    >
                      SIMULATE
                    </button>
                  </td>
                </motion.tr>
              ))}
            </AnimatePresence>
            {opportunities.length === 0 && (
              <tr>
                <td colSpan={6} className="p-8 text-center text-slate-500 italic">No opportunities detected. Searcher scanning mempool...</td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}

