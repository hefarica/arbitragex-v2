"use client";

import React, { useEffect, useState } from "react";
import { AlertCircle, CheckCircle2, Clock, Activity, ShieldAlert, Cpu } from "lucide-react";
import { getApiBaseUrl } from "@/lib/api-client";

type DataDependenciesStatus =
  | "ok"
  | "partial"
  | "armed_waiting_for_impact"
  | "waiting_for_profitable_base"
  | "missing_lending_watchlist"
  | "unknown_pool_map_entries"
  | string;

interface StrategyStatus {
  strategy_kind: string;
  enabled: boolean;
  engine_loaded: boolean;
  engine_invoked: boolean;
  last_invoked_at: string | null;
  last_candidate_at: string | null;
  candidates_1h: number;
  rejections_1h: number;
  last_rejection_reason: string | null;
  data_dependencies_status: DataDependenciesStatus;
  triangular_pool_map_entries?: number | null;
  pool_index_entries?: number | null;
  reserves_cache_entries?: number | null;
  base_candidates_seen_1h?: number | null;
  lending_watchlist_size?: number | null;
}

interface RuntimeStatusResponse {
  chain_id: number;
  source: any;
  strategies: StrategyStatus[];
  ts: string;
}

export function StrategyStatusCards() {
  const [data, setData] = useState<RuntimeStatusResponse | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    async function fetchStatus() {
      try {
        const EDGE_URL = process.env.NEXT_PUBLIC_EDGE_URL || getApiBaseUrl();
        const res = await fetch(`${EDGE_URL}/api/strategies/runtime-status?chain_id=1`);
        if (!res.ok) throw new Error("Failed to fetch strategy status");
        const json = await res.json();
        setData(json);
      } catch (e: any) {
        setError(e.message);
      }
    }
    
    fetchStatus();
    const interval = setInterval(fetchStatus, 15000); // Poll every 15s
    return () => clearInterval(interval);
  }, []);

  if (error) {
    return (
      <div className="p-4 mb-6 border border-destructive/50 rounded-md bg-destructive/10 text-destructive text-sm flex items-center gap-2">
        <AlertCircle className="w-4 h-4" />
        Failed to load strategy runtime status: {error}
      </div>
    );
  }

  if (!data) {
    return (
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4 mb-6">
        {[1, 2, 3, 4].map((i) => (
          <div key={i} className="h-32 bg-muted/20 animate-pulse rounded-md border border-border"></div>
        ))}
      </div>
    );
  }

  return (
    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4 mb-6">
      {data.strategies.map((strat) => (
        <StrategyCard key={strat.strategy_kind} strat={strat} />
      ))}
    </div>
  );
}

function StrategyCard({ strat }: { strat: StrategyStatus }) {
  let statusColor = "bg-muted border-border text-muted-foreground"; // Gray
  let statusIcon = <Clock className="w-4 h-4" />;
  let statusText = "Desconocido";

  // Evaluador de semáforo según reglas
  if (!strat.engine_loaded || !strat.engine_invoked) {
    statusColor = "bg-muted/10 border-muted text-muted-foreground";
    statusText = "Sin señal viva";
    statusIcon = <Activity className="w-4 h-4" />;
  } else if (strat.data_dependencies_status === "missing_lending_watchlist") {
    statusColor = "bg-destructive/10 border-destructive/30 text-destructive";
    statusText = "Requiere watchlist lending";
    statusIcon = <ShieldAlert className="w-4 h-4" />;
  } else if (strat.data_dependencies_status === "unknown_pool_map_entries") {
    statusColor = "bg-destructive/10 border-destructive/30 text-destructive";
    statusText = "Fallo Redis/DB";
    statusIcon = <AlertCircle className="w-4 h-4" />;
  } else if (
    strat.strategy_kind === "triangular_arb" &&
    strat.data_dependencies_status === "armed_waiting_for_impact"
  ) {
    statusColor = "bg-yellow-500/10 border-yellow-500/30 text-yellow-500";
    statusText = "Armado, esperando impacto rentable";
    statusIcon = <Clock className="w-4 h-4" />;
  } else if (
    strat.strategy_kind === "flashloan_arb" &&
    strat.data_dependencies_status === "waiting_for_profitable_base"
  ) {
    statusColor = "bg-yellow-500/10 border-yellow-500/30 text-yellow-500";
    statusText = "Esperando base profitable";
    statusIcon = <Clock className="w-4 h-4" />;
  } else if (strat.candidates_1h > 0) {
    statusColor = "bg-success/10 border-success/30 text-success";
    statusText = "Activo y detectando";
    statusIcon = <CheckCircle2 className="w-4 h-4" />;
  } else {
    // Default yellow if loaded but 0 candidates (for dex_arb mostly)
    statusColor = "bg-yellow-500/10 border-yellow-500/30 text-yellow-500";
    statusText = "Inactivo 1h";
    statusIcon = <Activity className="w-4 h-4" />;
  }

  const titleMap: Record<string, string> = {
    dex_arb: "DEX Arb",
    triangular_arb: "Triangular Arb",
    flashloan_arb: "Flashloan Arb",
    liquidation: "Liquidation",
  };

  return (
    <div className="flex flex-col p-4 rounded-md border bg-card text-card-foreground shadow-sm">
      <div className="flex items-center justify-between mb-2">
        <h3 className="font-semibold text-sm flex items-center gap-2">
          <Cpu className="w-4 h-4 text-muted-foreground" />
          {titleMap[strat.strategy_kind] || strat.strategy_kind}
        </h3>
        <div
          className={`flex items-center gap-1.5 px-2 py-0.5 rounded-full text-xs font-medium border ${statusColor}`}
          title={statusText}
        >
          {statusIcon}
          <span className="truncate max-w-[120px]">{statusText}</span>
        </div>
      </div>
      
      <div className="space-y-1.5 mt-2 text-xs text-muted-foreground">
        <div className="flex justify-between">
          <span>Candidates (1h)</span>
          <span className="font-mono text-foreground">{strat.candidates_1h}</span>
        </div>
        <div className="flex justify-between">
          <span>Rejections (1h)</span>
          <span className="font-mono text-foreground">{strat.rejections_1h}</span>
        </div>
        
        {strat.strategy_kind === "triangular_arb" && strat.pool_index_entries !== undefined && (
          <div className="flex justify-between border-t border-border/50 pt-1 mt-1">
            <span>Pool Index (Redis)</span>
            <span className="font-mono text-foreground">{strat.pool_index_entries ?? "0"}</span>
          </div>
        )}
        {strat.strategy_kind === "flashloan_arb" && strat.base_candidates_seen_1h !== undefined && (
          <div className="flex justify-between border-t border-border/50 pt-1 mt-1">
            <span>Base Seen (1h)</span>
            <span className="font-mono text-foreground">{strat.base_candidates_seen_1h ?? "0"}</span>
          </div>
        )}
        {strat.strategy_kind === "liquidation" && strat.lending_watchlist_size !== undefined && (
          <div className="flex justify-between border-t border-border/50 pt-1 mt-1">
            <span>Watchlist Size</span>
            <span className="font-mono text-foreground">{strat.lending_watchlist_size ?? "0"}</span>
          </div>
        )}
      </div>
    </div>
  );
}
