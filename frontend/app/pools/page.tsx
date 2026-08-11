// ZERO MOCKS DOCTRINE (rule_00): This page fetches data ONLY through the edge.
"use client";
import React, { useCallback, useEffect, useState } from "react";
import { Activity } from "lucide-react";
import { motion } from "framer-motion";
import { getDefiPools } from "@/lib/api-client";
import type { DefiPoolsResponse } from "@/lib/schemas";
import { EdgeState } from "@/components/EdgeState";

const POOL_COLUMNS = ["Pair", "DEX", "Address", "Status"];

export default function PoolsPage() {
  const [result, setResult] = useState<{ ok: true; data: DefiPoolsResponse } | { ok: false; error: string } | null>(null);
  const [retrying, setRetrying] = useState(false);

  const load = useCallback(async () => {
    setRetrying(true);
    const r = await getDefiPools();
    setResult(r);
    setRetrying(false);
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  if (!result) {
    return (
      <div className="p-8 min-h-screen text-foreground">
        <EdgeState
          variant="loading"
          title="Loading pool registry…"
          description="Querying the edge for the live pool set."
          endpoint="GET /api/v1/pools"
          ghost="table"
          ghostColumns={POOL_COLUMNS}
        />
      </div>
    );
  }

  if (!result.ok) {
    return (
      <div className="p-8 min-h-screen text-foreground">
        <EdgeState
          variant="error"
          title="Pool registry unreachable"
          description="The edge endpoint refused or could not be reached. No fabricated pool data is shown — zero-trust."
          endpoint="GET /api/v1/pools"
          reasons={[result.error]}
          onRetry={load}
          retrying={retrying}
          ghost="table"
          ghostColumns={POOL_COLUMNS}
        />
      </div>
    );
  }

  const pools = result.data.data;

  if (pools.length === 0) {
    return (
      <div className="p-8 min-h-screen text-foreground">
        <EdgeState
          variant="empty"
          title="No pools registered yet"
          description="The registry is reachable but empty — the enumerator hasn't seeded pools for this chain."
          endpoint="GET /api/v1/pools"
          onRetry={load}
          retrying={retrying}
          ghost="table"
          ghostColumns={POOL_COLUMNS}
        />
      </div>
    );
  }

  return (
    <div className="p-8 space-y-6 min-h-screen text-foreground">
      <div className="flex justify-between items-center">
        <h1 className="text-3xl font-bold tracking-tight text-foreground">Pool Registry</h1>
        <div className="flex items-center gap-2 bg-success/10 text-success px-3 py-1.5 rounded-full border border-success/40">
          <Activity size={16} className="motion-safe:animate-pulse" />
          <span className="text-sm font-semibold tracking-wide">LIVE</span>
        </div>
      </div>

      <div data-slot="card" className="bg-card text-card-foreground border border-border rounded-xl overflow-hidden shadow-2xl">
        <table className="w-full text-left border-collapse">
          <thead>
            <tr className="bg-muted text-muted-foreground text-sm uppercase tracking-wider">
              <th className="p-4 border-b border-border">Pair</th>
              <th className="p-4 border-b border-border">DEX</th>
              <th className="p-4 border-b border-border">Address</th>
              <th className="p-4 border-b border-border">Status</th>
            </tr>
          </thead>
          <tbody className="text-foreground">
            {pools.map((pool, i) => (
              <motion.tr
                key={pool.address ?? `pool-${i}`}
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                transition={{ delay: i * 0.05 }}
                className="hover:bg-muted/40 transition-colors"
              >
                <td className="p-4 border-b border-border font-medium text-info">
                  {pool.token0_symbol ?? "?"}/{pool.token1_symbol ?? "?"}
                </td>
                <td className="p-4 border-b border-border">{pool.dex_name ?? "—"}</td>
                <td className="p-4 border-b border-border font-mono text-xs text-muted-foreground truncate max-w-[200px]">{pool.address ?? "—"}</td>
                <td className="p-4 border-b border-border">
                  <span className={`inline-flex items-center px-2 py-1 rounded-full text-xs font-medium border ${pool.is_active ? 'bg-success/10 text-success border-success/40' : 'bg-destructive/10 text-destructive border-destructive/40'}`}>
                    {pool.is_active ? "ACTIVE" : "DISABLED"}
                  </span>
                </td>
              </motion.tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
