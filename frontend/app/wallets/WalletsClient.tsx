"use client";

import { useState, useEffect, useCallback, useMemo } from "react";
import { useShallow } from "zustand/react/shallow";
import { AlertCircle, RefreshCw, Wallet } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { WalletDetailDialog } from "@/components/WalletDetailDialog";
import { getWallets, type WalletRow } from "@/lib/api/wallets";
import { useOmniStore } from "@/lib/store/omni-store";
import { getApiBaseUrl } from "@/lib/api-client";

export interface WalletsSnapshot {
  wallets: WalletRow[];
  source: string;
}

interface Props {
  initialSnapshot: WalletsSnapshot;
}

function EndpointNotice({ message }: { message: string }) {
  return (
    <div className="flex items-start gap-3 rounded-xl border border-warning/40 bg-warning/10 p-4 text-sm text-warning">
      <AlertCircle className="mt-0.5 size-4 shrink-0" />
      <div>
        <p className="font-semibold">Endpoint not available</p>
        <p className="text-xs mt-0.5 font-mono">{message}</p>
      </div>
    </div>
  );
}

export default function WalletsClient({ initialSnapshot }: Props) {
  // ── Omni-Store Selectors ──
  const { 
    walletsMap, 
    fetchWallets 
  } = useOmniStore(
    useShallow((state) => ({
      walletsMap: state.wallets,
      fetchWallets: state.fetchWallets,
    }))
  );

  // Initial fetch
  useEffect(() => {
    fetchWallets();
  }, [fetchWallets]);

  const wallets = useMemo(() => Array.from(walletsMap.values()), [walletsMap]);

  const [selected, setSelected] = useState<WalletRow | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [isMounted, setIsMounted] = useState(false);

  useEffect(() => {
    setIsMounted(true);
  }, []);

  const refresh = useCallback(async () => {
    setRefreshing(true);
    await fetchWallets();
    setRefreshing(false);
  }, [fetchWallets]);

  return (
    <div className="p-8 space-y-6">
      <div className="flex items-center justify-between border-b border-border pb-4">
        <div>
          <h1 className="text-3xl font-extrabold tracking-tight text-foreground">
            Operational Wallets
          </h1>
          <p className="text-sm text-muted-foreground mt-1">
            Executor signers, treasury addresses, per-chain balances and token allowances.
          </p>
        </div>
        <button
          type="button"
          onClick={refresh}
          disabled={refreshing}
          className="flex items-center gap-2 px-3 py-1.5 rounded-lg border border-border bg-muted text-muted-foreground hover:text-foreground hover:bg-accent transition-colors text-xs font-semibold disabled:opacity-50"
        >
          <RefreshCw className={`size-3.5 ${refreshing ? "animate-spin" : ""}`} />
          Refresh
        </button>
      </div>



      {isMounted && wallets.length === 0 && (
        <div className="flex flex-col items-center justify-center py-16 text-muted-foreground gap-3">
          <Wallet className="size-10 opacity-30" />
          <p className="text-sm">No wallets returned by the API.</p>
        </div>
      )}

      {wallets.length > 0 && (
        <div className="rounded-2xl border border-border overflow-hidden shadow-sm">
          <table className="w-full text-left border-collapse">
            <thead>
              <tr className="bg-muted text-muted-foreground text-xs uppercase tracking-wider">
                <th className="px-4 py-3 border-b border-border">Label</th>
                <th className="px-4 py-3 border-b border-border">Address</th>
                <th className="px-4 py-3 border-b border-border">Role</th>
                <th className="px-4 py-3 border-b border-border text-right">Details</th>
              </tr>
            </thead>
            <tbody>
              {wallets.map((w) => (
                <tr
                  key={w.address}
                  onClick={() => setSelected(w)}
                  className="border-b border-border/50 hover:bg-muted/40 transition-colors cursor-pointer"
                >
                  <td className="px-4 py-3 font-semibold text-sm">{w.label}</td>
                  <td className="px-4 py-3 font-mono text-xs text-muted-foreground">
                    {w.address.slice(0, 8)}…{w.address.slice(-6)}
                  </td>
                  <td className="px-4 py-3">
                    {w.role ? (
                      <Badge variant="outline">{w.role}</Badge>
                    ) : (
                      <span className="text-muted-foreground/50 italic text-xs">—</span>
                    )}
                  </td>
                  <td className="px-4 py-3 text-right">
                    <button
                      type="button"
                      onClick={(e) => {
                        e.stopPropagation();
                        setSelected(w);
                      }}
                      className="text-xs text-primary hover:underline"
                    >
                      View
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      <WalletDetailDialog
        wallet={selected}
        onClose={() => setSelected(null)}
      />
    </div>
  );
}
