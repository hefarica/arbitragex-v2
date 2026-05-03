"use client";
import React, { useEffect, useState, useCallback, startTransition } from "react";
import { Zap, WifiOff, ShieldAlert, RefreshCw, Radio, Clock, AlertTriangle } from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";

interface Opportunity {
  id: string;
  detected_at: string;
  pair_symbol: string;
  dex_a: string;
  dex_b: string;
  expected_profit_usd: number;
  roi_pct: number;
  risk_score: number;
}

type FeedStatus = "POLLING" | "LIVE" | "ERROR";

const POLL_INTERVAL_MS = 4_000;

export type OpportunitiesSnapshot = {
  opportunities: Opportunity[];
  serverTime: string | null;
  source: string;
};

export default function OpportunitiesClient({
  initialSnapshot,
}: {
  initialSnapshot: OpportunitiesSnapshot;
}) {
  const [snapshot, setSnapshot] = useState<OpportunitiesSnapshot>(initialSnapshot);
  const [isMounted, setIsMounted] = useState(false);
  const [feedStatus, setFeedStatus] = useState<FeedStatus>("POLLING");
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const [now, setNow] = useState<number>(0);

  const EDGE_URL = process.env.NEXT_PUBLIC_EDGE_URL ?? "http://localhost:8787";

  const fetchOpportunities = useCallback(async () => {
    try {
      const res = await fetch(`${EDGE_URL}/api/opportunities/live`, {
        headers: { accept: "application/json" },
        signal: AbortSignal.timeout(4000),
        cache: "no-store",
      });
      if (!res.ok) {
        if (feedStatus !== "LIVE") setFeedStatus("ERROR");
        setErrorMsg(`Edge returned ${res.status}`);
        return;
      }
      const data = await res.json();
      startTransition(() => {
        setSnapshot(prev => ({
          opportunities: Array.isArray(data?.items) ? data.items : Array.isArray(data) ? data : [],
          serverTime: new Date().toISOString(),
          source: "client-rest-fallback",
        }));
      });
      setErrorMsg(null);
    } catch (e) {
      if (feedStatus !== "LIVE") setFeedStatus("ERROR");
      setErrorMsg((e as Error).message);
    }
  }, [EDGE_URL, feedStatus]);

  useEffect(() => {
    setIsMounted(true);
    setNow(Date.now());

    // HTTP polling only — Socket.IO removed: edge worker has no /socket.io
    // upgrade handler, which produced an endless reconnect storm in production.
    let alive = true;
    fetchOpportunities();
    const timer = setInterval(() => {
      if (alive) fetchOpportunities();
    }, POLL_INTERVAL_MS);

    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, [fetchOpportunities]);

  useEffect(() => {
    const ticker = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(ticker);
  }, []);

  const opportunities = snapshot.opportunities;
  const lastRefresh = snapshot.serverTime ? new Date(snapshot.serverTime) : null;

  return (
    <div className={`p-8 min-h-screen transition-colors duration-500 ${feedStatus === 'ERROR' ? 'bg-rose-950/20' : 'bg-[#020617]'} text-slate-200`}>
      <div className="flex justify-between items-center border-b border-slate-800 pb-4 mb-8">
        <div>
          <h1 className={`text-4xl font-extrabold tracking-tight bg-clip-text text-transparent ${feedStatus === 'ERROR' ? 'bg-gradient-to-r from-rose-500 to-red-600' : 'bg-gradient-to-r from-emerald-400 to-teal-400'}`}>
            Live MEV Feed
          </h1>
          <p className="text-slate-500 mt-2 text-sm" suppressHydrationWarning>
            Polling edge every {POLL_INTERVAL_MS / 1000}s · {isMounted && lastRefresh ? `Last: ${lastRefresh.toLocaleTimeString()}` : "Loading..."}
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
        <div className="mb-8 p-4 bg-slate-800/50 border border-slate-700 rounded-xl flex items-center gap-4 text-slate-400 shadow-inner">
          <div className="relative flex h-3 w-3">
            <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-emerald-400 opacity-75"></span>
            <span className="relative inline-flex rounded-full h-3 w-3 bg-emerald-500"></span>
          </div>
          <div>
            <h3 className="font-bold text-emerald-400 tracking-wide">SCANNING MEMPOOL IN REAL-TIME</h3>
            <p className="text-sm mt-1">Searcher-rs is actively hunting for arbitrage routes. Opportunities will appear here instantly.</p>
          </div>
        </div>
      )}

      <div className="bg-[#0f172a]/80 backdrop-blur-md border border-slate-800 rounded-2xl shadow-2xl overflow-hidden">
        <table className="w-full text-left border-collapse">
          <thead>
            <tr className="bg-slate-900 text-slate-400 text-sm uppercase tracking-wider">
              <th className="p-4 border-b border-slate-800">Age / Time</th>
              <th className="p-4 border-b border-slate-800">Route</th>
              <th className="p-4 border-b border-slate-800 text-right">Net Profit (USD)</th>
              <th className="p-4 border-b border-slate-800 text-right">Net ROI</th>
              <th className="p-4 border-b border-slate-800 text-center">Score</th>
              <th className="p-4 border-b border-slate-800 text-center">Action</th>
            </tr>
          </thead>
          <tbody>
            <AnimatePresence>
              {opportunities.map((opp) => {
                const detectedTime = new Date(opp.detected_at).getTime();
                const ageSecs = isMounted ? Math.floor((now - detectedTime) / 1000) : 0;
                const isStale = ageSecs > 12;
                const scorePercent = Number(opp.risk_score ?? 0) * 100;
                const isCriticalTriage = scorePercent > 95;
                
                return (
                  <motion.tr 
                    key={opp.id}
                    initial={{ opacity: 0, x: -20, backgroundColor: "rgba(16,185,129,0.2)" }}
                    animate={{ opacity: 1, x: 0, backgroundColor: isCriticalTriage ? "rgba(234,179,8,0.05)" : "transparent" }}
                    exit={{ opacity: 0 }}
                    transition={{ duration: 0.5 }}
                    className={`border-b hover:bg-slate-800/30 transition-all ${isCriticalTriage ? 'border-yellow-500/30 shadow-[inset_0_0_15px_rgba(234,179,8,0.05)] relative' : 'border-slate-800/50'}`}
                  >
                    <td className="p-4 font-mono text-xs">
                      {isCriticalTriage && (
                        <div className="absolute left-0 top-0 bottom-0 w-1 bg-gradient-to-b from-yellow-400 to-emerald-400 animate-pulse"></div>
                      )}
                      <div className="flex flex-col gap-1">
                        <div className={`flex items-center gap-1.5 font-bold ${isStale ? 'text-rose-400' : 'text-emerald-400'}`}>
                          {isStale ? <AlertTriangle size={12} className="animate-pulse" /> : <Clock size={12} />}
                          <span suppressHydrationWarning>{isMounted ? `${ageSecs}s ago` : '--'}</span>
                        </div>
                        <div className="text-slate-500" suppressHydrationWarning>
                          {isMounted ? new Date(opp.detected_at).toLocaleTimeString([], { hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit' }) : '--:--:--'}
                        </div>
                      </div>
                    </td>
                    <td className="p-4">
                      <div className="flex flex-col">
                        <span className="font-semibold text-slate-200 text-sm">{opp.pair_symbol || 'Unknown Pair'}</span>
                        <span className="text-xs font-mono text-indigo-400">
                          {opp.dex_a} <span className="text-slate-500">→</span> {opp.dex_b}
                        </span>
                      </div>
                    </td>
                    <td className="p-4 text-right">
                      <div className="group relative inline-block cursor-help">
                        <span className="font-mono font-bold text-emerald-400 text-base shadow-emerald-500/10 drop-shadow-md border-b border-dashed border-emerald-500/30">
                          ${Number(opp.expected_profit_usd).toFixed(2)}
                        </span>
                        {/* ZERO MOCKS TOOLTIP */}
                        <div className="absolute bottom-full right-0 mb-2 w-64 p-3 bg-slate-900 border border-slate-700 rounded-lg shadow-xl opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none z-10 text-left">
                          <div className="text-xs text-slate-300 font-sans">
                            <div className="flex justify-between border-b border-slate-700 pb-1 mb-1">
                              <span>Ganancia Neta (Est):</span>
                              <span className="text-emerald-400 font-mono">${Number(opp.expected_profit_usd).toFixed(2)}</span>
                            </div>
                            <div className="flex justify-between text-slate-500">
                              <span>Desglose de Gas:</span>
                              <span className="italic">Pendiente Sim.</span>
                            </div>
                            <div className="flex justify-between text-slate-500">
                              <span>Bribe (MEV):</span>
                              <span className="italic">Pendiente Sim.</span>
                            </div>
                          </div>
                        </div>
                      </div>
                    </td>
                    <td className="p-4 text-right font-mono text-slate-300">
                      <span className="bg-slate-800/80 px-2 py-1 rounded border border-slate-700/50">
                        {Number(opp.roi_pct ?? 0).toFixed(2)}%
                      </span>
                    </td>
                    <td className="p-4 text-center">
                      <span className={`px-3 py-1 rounded-full text-xs font-bold border ${scorePercent > 95 ? 'bg-yellow-500/20 text-yellow-400 border-yellow-500/50 shadow-[0_0_15px_rgba(234,179,8,0.3)] animate-pulse' : scorePercent > 70 ? 'bg-emerald-500/10 text-emerald-400 border-emerald-500/30' : 'bg-blue-500/10 text-blue-400 border-blue-500/30'}`}>
                        {scorePercent.toFixed(1)}%
                      </span>
                    </td>
                    <td className="p-4 text-center">
                      <button 
                        className={`px-4 py-1.5 rounded text-white text-xs font-bold transition-colors shadow-lg ${isCriticalTriage ? 'bg-gradient-to-r from-yellow-600 to-amber-500 hover:from-yellow-500 hover:to-amber-400 shadow-yellow-900/40' : 'bg-indigo-600 hover:bg-indigo-500 shadow-indigo-900/20'}`}
                      >
                        SIMULATE
                      </button>
                    </td>
                  </motion.tr>
                );
              })}
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
