"use client";

import { useState, useEffect, useCallback } from "react";
import { AlertCircle, RefreshCw, ToggleLeft, ToggleRight, Database } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { DexDetailDialog } from "@/components/DexDetailDialog";
import { getDexes, toggleDexActive, type DexRow } from "@/lib/api/dexes";

export interface DexRegistrySnapshot {
  dexes: DexRow[];
  source: string;
}

interface Props {
  initialSnapshot: DexRegistrySnapshot;
}

const EDGE_URL =
  process.env.NEXT_PUBLIC_EDGE_URL ?? "http://localhost:8787";

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

function fmtUsd(v: number | null): string {
  if (v == null) return "—";
  if (v >= 1_000_000_000) return `$${(v / 1_000_000_000).toFixed(1)}B`;
  if (v >= 1_000_000) return `$${(v / 1_000_000).toFixed(1)}M`;
  if (v >= 1_000) return `$${(v / 1_000).toFixed(1)}K`;
  return `$${v.toFixed(2)}`;
}

const ALL_CHAINS = "all";

export default function DexRegistryClient({ initialSnapshot }: Props) {
  // R1: useState initialised from server-provided snapshot
  const [dexes, setDexes] = useState<DexRow[]>(initialSnapshot.dexes);
  const [error, setError] = useState<string | null>(
    initialSnapshot.source === "endpoint-not-implemented"
      ? "API endpoint not yet implemented — wire backend route to populate"
      : null,
  );
  const [selected, setSelected] = useState<DexRow | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [toggling, setToggling] = useState<string | null>(null);
  const [toggleError, setToggleError] = useState<string | null>(null);
  // R1: isMounted to avoid SSR/CSR mismatch on derived values
  const [isMounted, setIsMounted] = useState(false);
  const [chainFilter, setChainFilter] = useState<string>(ALL_CHAINS);

  useEffect(() => {
    setIsMounted(true);
  }, []);

  const refresh = useCallback(async () => {
    setRefreshing(true);
    const res = await getDexes(EDGE_URL);
    if (res.ok) {
      setDexes(res.data.dexes);
      setError(null);
    } else {
      setError(res.error);
    }
    setRefreshing(false);
  }, []);

  const handleToggle = useCallback(
    async (dex: DexRow, e: React.MouseEvent) => {
      e.stopPropagation();
      setToggling(dex.id);
      setToggleError(null);
      // R8: admin token not in localStorage yet — surface gap if endpoint returns 404
      const adminToken =
        (typeof window !== "undefined" &&
          localStorage.getItem("arbx-admin-token")) ||
        "";
      const res = await toggleDexActive(EDGE_URL, dex.id, !dex.is_active, adminToken);
      if (res.ok) {
        setDexes((prev) =>
          prev.map((d) =>
            d.id === dex.id ? { ...d, is_active: res.data.is_active } : d,
          ),
        );
      } else {
        setToggleError(`Toggle failed: ${res.error}`);
      }
      setToggling(null);
    },
    [],
  );

  // Derive unique chain IDs for filter tabs
  const allChainIds: number[] = isMounted
    ? Array.from(new Set(dexes.flatMap((d) => d.chain_ids))).sort(
        (a, b) => a - b,
      )
    : [];

  const filtered =
    chainFilter === ALL_CHAINS
      ? dexes
      : dexes.filter((d) =>
          d.chain_ids.includes(Number(chainFilter)),
        );

  return (
    <div className="p-8 space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between border-b border-border pb-4">
        <div>
          <h1 className="text-3xl font-extrabold tracking-tight text-foreground">
            DEX Registry
          </h1>
          <p className="text-sm text-muted-foreground mt-1">
            Authorised DEX protocols — volume, TVL, enable/disable per
            operator.
          </p>
        </div>
        <button
          type="button"
          onClick={refresh}
          disabled={refreshing}
          className="flex items-center gap-2 px-3 py-1.5 rounded-lg border border-border bg-muted text-muted-foreground hover:text-foreground hover:bg-accent transition-colors text-xs font-semibold disabled:opacity-50"
          title="Refresh DEX list"
        >
          <RefreshCw
            className={`size-3.5 ${refreshing ? "animate-spin" : ""}`}
          />
          Refresh
        </button>
      </div>

      {/* R8 fail-honest notices */}
      {error && <EndpointNotice message={error} />}
      {toggleError && (
        <div className="flex items-start gap-3 rounded-xl border border-destructive/40 bg-destructive/10 p-3 text-xs text-destructive">
          <AlertCircle className="mt-0.5 size-3.5 shrink-0" />
          {toggleError}
        </div>
      )}

      {/* Chain filter tabs — only rendered after mount to avoid SSR mismatch */}
      {isMounted && allChainIds.length > 0 && (
        <div className="flex gap-2 flex-wrap">
          <button
            type="button"
            onClick={() => setChainFilter(ALL_CHAINS)}
            className={`px-3 py-1 rounded-full text-xs font-semibold border transition-colors ${
              chainFilter === ALL_CHAINS
                ? "bg-primary text-primary-foreground border-primary"
                : "border-border text-muted-foreground hover:text-foreground hover:border-foreground/30"
            }`}
          >
            All chains
          </button>
          {allChainIds.map((c) => (
            <button
              key={c}
              type="button"
              onClick={() => setChainFilter(String(c))}
              className={`px-3 py-1 rounded-full text-xs font-semibold border transition-colors ${
                chainFilter === String(c)
                  ? "bg-primary text-primary-foreground border-primary"
                  : "border-border text-muted-foreground hover:text-foreground hover:border-foreground/30"
              }`}
            >
              Chain {c}
            </button>
          ))}
        </div>
      )}

      {/* Empty state */}
      {!error && isMounted && filtered.length === 0 && (
        <div className="flex flex-col items-center justify-center py-16 text-muted-foreground gap-3">
          <Database className="size-10 opacity-30" />
          <p className="text-sm">No DEXes returned by the API.</p>
          <p className="text-xs">
            Populate the{" "}
            <code className="font-mono text-xs">dexes</code> table via
            migration 044/045 and wire the endpoint.
          </p>
        </div>
      )}

      {/* Table */}
      {filtered.length > 0 && (
        <div className="rounded-2xl border border-border overflow-hidden shadow-sm">
          <table className="w-full text-left border-collapse">
            <thead>
              <tr className="bg-muted text-muted-foreground text-xs uppercase tracking-wider">
                <th className="px-4 py-3 border-b border-border">Name</th>
                <th className="px-4 py-3 border-b border-border">Protocol</th>
                <th className="px-4 py-3 border-b border-border">Chains</th>
                <th className="px-4 py-3 border-b border-border text-right">
                  Vol 24h
                </th>
                <th className="px-4 py-3 border-b border-border text-right">
                  TVL
                </th>
                <th className="px-4 py-3 border-b border-border text-center">
                  Status
                </th>
                <th className="px-4 py-3 border-b border-border text-center">
                  Toggle
                </th>
              </tr>
            </thead>
            <tbody>
              {filtered.map((d) => (
                <tr
                  key={d.id}
                  onClick={() => setSelected(d)}
                  className="border-b border-border/50 hover:bg-muted/40 transition-colors cursor-pointer"
                >
                  <td className="px-4 py-3 font-semibold text-sm">{d.name}</td>
                  <td className="px-4 py-3 text-xs text-muted-foreground font-mono">
                    {d.protocol_type}
                  </td>
                  <td className="px-4 py-3">
                    <div className="flex flex-wrap gap-1">
                      {d.chain_ids.map((c) => (
                        <Badge key={c} variant="outline" className="text-[10px]">
                          {c}
                        </Badge>
                      ))}
                    </div>
                  </td>
                  <td className="px-4 py-3 text-right font-mono text-xs">
                    {fmtUsd(d.volume_24h_usd)}
                  </td>
                  <td className="px-4 py-3 text-right font-mono text-xs">
                    {fmtUsd(d.tvl_usd)}
                  </td>
                  <td className="px-4 py-3 text-center">
                    {d.is_active ? (
                      <Badge variant="success">Active</Badge>
                    ) : (
                      <Badge variant="secondary">Disabled</Badge>
                    )}
                  </td>
                  <td className="px-4 py-3 text-center">
                    <button
                      type="button"
                      disabled={toggling === d.id}
                      onClick={(e) => handleToggle(d, e)}
                      className="flex items-center justify-center mx-auto text-muted-foreground hover:text-foreground transition-colors disabled:opacity-40"
                      title={d.is_active ? "Disable DEX" : "Enable DEX"}
                      aria-label={d.is_active ? `Disable ${d.name}` : `Enable ${d.name}`}
                    >
                      {d.is_active ? (
                        <ToggleRight className="size-5 text-success" />
                      ) : (
                        <ToggleLeft className="size-5" />
                      )}
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {/* Detail sheet */}
      <DexDetailDialog dex={selected} onClose={() => setSelected(null)} />
    </div>
  );
}
