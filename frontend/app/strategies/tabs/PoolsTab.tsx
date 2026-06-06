/**
 * Phase 2 Route Finder — Pool browser tab (read-only MVP).
 *
 * Migrated to Omni-Store (Phase 8):
 * - Consumes chains and dexes from useOmniStore.
 * - Centralized registry fetch reduces redundant API calls.
 * - Preserves R8 fail-honest error reporting.
 */
"use client";

import { useEffect, useMemo, useState } from "react";
import { useShallow } from "zustand/react/shallow";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { TokenPairIcon } from "@/components/ui/TokenIcon";
import { useOmniStore } from "@/lib/store/omni-store";
import { shortAddr } from "@/lib/format";
import { getApiBaseUrl } from "@/lib/api-client";

// ── Local type mirrors (no cross-package import) ───────────────────────────

interface PoolInfo {
  id: string;
  chain_id: number;
  address: string;
  dex_id: string;
  dex_name: string;
  protocol_type: string;
  fee_tier: number | null;
  token0_symbol: string | null;
  token0_address: string;
  token1_symbol: string | null;
  token1_address: string;
  is_active: boolean;
}

interface PoolsResponse {
  count: number;
  chain_id: number;
  filters: { dex_id: string | null; protocol_type: string | null };
  items: PoolInfo[];
}

// ── Constants ──────────────────────────────────────────────────────────────

const LOAD_PAGE = 200;
const DASH = "—";

// All known protocol types for the multiselect chips.
const KNOWN_PROTOCOLS = ["UNISWAP_V2", "UNISWAP_V3", "CURVE", "BALANCER"];

// ── Helpers ────────────────────────────────────────────────────────────────

function fmtFeeTier(tier: number | null | undefined): string {
  if (tier == null) return DASH;
  return `${(tier / 10_000).toFixed(tier % 100 === 0 ? 2 : 4)}%`;
}

function tokenPairLabel(pool: PoolInfo): string {
  const t0 = pool.token0_symbol ?? shortAddr(pool.token0_address);
  const t1 = pool.token1_symbol ?? shortAddr(pool.token1_address);
  return `${t0}/${t1}`;
}

const PROTOCOL_CLASSES: Record<string, string> = {
  UNISWAP_V3: "bg-chart-1/15 text-chart-1 border-chart-1/40",
  UNISWAP_V2: "bg-chart-2/15 text-chart-2 border-chart-2/40",
  CURVE: "bg-chart-4/15 text-chart-4 border-chart-4/40",
  BALANCER: "bg-chart-3/15 text-chart-3 border-chart-3/40",
};
const PROTOCOL_FALLBACK = "bg-muted text-muted-foreground border-border";

function ProtocolBadge({ type }: { type: string }) {
  const cls = PROTOCOL_CLASSES[type] ?? PROTOCOL_FALLBACK;
  return (
    <Badge variant="outline" className={`text-[10px] font-mono px-1.5 py-0 border ${cls}`}>
      {type}
    </Badge>
  );
}

// ── Props ──────────────────────────────────────────────────────────────────

interface Props {
  chainId: number;
}

// ── Component ──────────────────────────────────────────────────────────────

export function PoolsTab({ chainId }: Props) {
  // ── Omni-Store Selectors ──
  const { 
    chainsMap, 
    dexesMap, 
    registryStatus, 
    registryError, 
    fetchRegistry 
  } = useOmniStore(
    useShallow((state) => ({
      chainsMap: state.chains,
      dexesMap: state.dexes,
      registryStatus: state.registryStatus,
      registryError: state.registryError,
      fetchRegistry: state.fetchRegistry,
    }))
  );

  const SUPPORTED_CHAINS = useMemo(() => Array.from(chainsMap.values()), [chainsMap]);
  const dexes = useMemo(() => Array.from(dexesMap.values()), [dexesMap]);

  // ── Chain selector state ──
  const [selectedChainId, setSelectedChainId] = useState<number>(chainId);

  // Initial registry fetch
  useEffect(() => {
    fetchRegistry(selectedChainId);
  }, [selectedChainId, fetchRegistry]);

  // ── Filter state ──
  const [selectedDexId, setSelectedDexId] = useState<string>("__all__");
  const [activeProtocols, setActiveProtocols] = useState<Set<string>>(new Set());
  const [search, setSearch] = useState("");

  // ── Pool fetch state (Still local as it's a large paginated dataset) ──
  const [pools, setPools] = useState<PoolInfo[]>([]);
  const [totalCount, setTotalCount] = useState(0);
  const [limit, setLimit] = useState(LOAD_PAGE);
  const [poolsLoading, setPoolsLoading] = useState(true);
  const [fetchError, setFetchError] = useState<string | null>(null);

  // Fetch whenever chain, dex filter, or limit changes.
  useEffect(() => {
    const ctrl = new AbortController();
    setPoolsLoading(true);
    setFetchError(null);

    const baseUrl = getApiBaseUrl();
    const url = new URL(`${baseUrl}/api/pools`);
    url.searchParams.set("chain_id", String(selectedChainId));
    if (selectedDexId !== "__all__") url.searchParams.set("dex_id", selectedDexId);
    url.searchParams.set("limit", String(limit));

    fetch(url.toString(), {
      signal: ctrl.signal,
      credentials: "include",
      headers: { accept: "application/json" },
    })
      .then((r) => {
        if (!r.ok) throw new Error(`edge HTTP ${r.status}`);
        return r.json() as Promise<PoolsResponse>;
      })
      .then((d) => {
        setPools(d.items ?? []);
        setTotalCount(d.count ?? 0);
        setPoolsLoading(false);
      })
      .catch((e: Error) => {
        if (e.name === "AbortError") return;
        setFetchError(e.message);
        setPoolsLoading(false);
      });
    return () => ctrl.abort();
  }, [selectedChainId, selectedDexId, limit]);

  // ── Client-side filtering ──
  const filteredPools = useMemo(() => {
    let result = pools;
    if (activeProtocols.size > 0) {
      result = result.filter((p) => activeProtocols.has(p.protocol_type));
    }
    const q = search.trim().toLowerCase();
    if (q.length > 0) {
      result = result.filter((p) => {
        const pair = tokenPairLabel(p).toLowerCase();
        const addr = p.address.toLowerCase();
        return pair.includes(q) || addr.startsWith(q);
      });
    }
    return result;
  }, [pools, activeProtocols, search]);

  // ── Handlers ──

  const toggleProtocol = (proto: string) => {
    setActiveProtocols((prev) => {
      const next = new Set(prev);
      if (next.has(proto)) next.delete(proto);
      else next.add(proto);
      return next;
    });
  };

  const onChainChange = (value: string) => {
    setSelectedChainId(Number(value));
    setSelectedDexId("__all__");
    setLimit(LOAD_PAGE);
    setActiveProtocols(new Set());
  };

  const onDexChange = (value: string) => {
    setSelectedDexId(value);
    setLimit(LOAD_PAGE);
  };

  const loadMore = () => {
    setLimit((prev) => prev + LOAD_PAGE);
  };

  const hasMore = pools.length < totalCount && !poolsLoading;

  const selectedChainName =
    chainsMap.get(selectedChainId)?.name ?? String(selectedChainId);

  return (
    <Card>
      <CardHeader>
        <div className="flex items-start justify-between gap-4 flex-wrap">
          <div>
            <CardTitle>Pools · {selectedChainName}</CardTitle>
            <p className="text-xs text-muted-foreground mt-1">
              Read-only browser. Operator controls pools indirectly today via
              <strong className="font-mono mx-1">DEXes</strong> +
              <strong className="font-mono mx-1">Tokens</strong> tabs.
            </p>
          </div>
          <div className="flex items-center gap-3">
            <Select
              value={String(selectedChainId)}
              onValueChange={onChainChange}
            >
              <SelectTrigger size="sm" className="w-48">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {SUPPORTED_CHAINS.map((c) => (
                  <SelectItem key={c.id} value={String(c.id)}>
                    {c.name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>

            {!poolsLoading && !fetchError && (
              <span className="text-xs text-muted-foreground font-mono whitespace-nowrap">
                {filteredPools.length} shown
                {filteredPools.length !== pools.length && ` of ${pools.length} loaded`}
                {` · ${totalCount} total`}
              </span>
            )}
          </div>
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="flex flex-wrap items-center gap-3">
          <Select
            value={selectedDexId}
            onValueChange={onDexChange}
            disabled={registryStatus === "loading"}
          >
            <SelectTrigger size="sm" className="w-48">
              <SelectValue placeholder="All DEXes" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="__all__">All DEXes</SelectItem>
              {dexes.map((d) => (
                <SelectItem key={d.id} value={d.id}>
                  {d.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>

          <div className="flex flex-wrap items-center gap-1.5">
            {KNOWN_PROTOCOLS.map((proto) => {
              const active = activeProtocols.has(proto);
              const cls = active
                ? (PROTOCOL_CLASSES[proto] ?? PROTOCOL_FALLBACK)
                : "bg-muted/30 text-muted-foreground border-border";
              return (
                <button
                  key={proto}
                  type="button"
                  onClick={() => toggleProtocol(proto)}
                  className={`text-[10px] font-mono px-2 py-0.5 rounded border transition-colors cursor-pointer ${cls}`}
                >
                  {proto}
                </button>
              );
            })}
          </div>

          <Input
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Search token symbol or address…"
            className="h-8 text-xs w-56 ml-auto"
          />
        </div>

        {(fetchError || registryError) && (
          <p className="text-xs text-destructive font-mono bg-destructive/10 px-3 py-2 rounded">
            Error: {fetchError || registryError}
          </p>
        )}

          {poolsLoading && !fetchError && !registryError && (
          <p className="text-xs text-muted-foreground">Loading from edge…</p>
        )}

        <div className="rounded-md border">
          <Table>
            <TableHeader>
              <TableRow className="hover:bg-transparent">
                <TableHead className="w-[180px]">Pair</TableHead>
                <TableHead>DEX</TableHead>
                <TableHead>Protocol</TableHead>
                <TableHead>Fee</TableHead>
                <TableHead className="text-right">Address</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {filteredPools.map((pool) => (
                <TableRow key={pool.id} className="group hover:bg-muted/30 transition-colors">
                  <TableCell className="font-medium">
                    <div className="flex items-center gap-2 min-w-0">
                      <TokenPairIcon
                        tokenInAddress={pool.token0_address}
                        tokenOutAddress={pool.token1_address}
                        tokenInSymbol={pool.token0_symbol ?? undefined}
                        tokenOutSymbol={pool.token1_symbol ?? undefined}
                        chainId={pool.chain_id}
                        size={22}
                      />
                      <span className="truncate">{tokenPairLabel(pool)}</span>
                    </div>
                  </TableCell>
                  <TableCell className="text-xs text-muted-foreground">
                    {pool.dex_name}
                  </TableCell>
                  <TableCell>
                    <ProtocolBadge type={pool.protocol_type} />
                  </TableCell>
                  <TableCell className="text-xs font-mono">
                    {fmtFeeTier(pool.fee_tier)}
                  </TableCell>
                  <TableCell className="text-right font-mono text-[10px] text-muted-foreground group-hover:text-foreground transition-colors">
                    {pool.address}
                  </TableCell>
                </TableRow>
              ))}
              {!poolsLoading && filteredPools.length === 0 && (
                <TableRow>
                  <TableCell colSpan={5} className="h-24 text-center text-xs text-muted-foreground">
                    No pools found matching your filters.
                  </TableCell>
                </TableRow>
              )}
            </TableBody>
          </Table>
        </div>

        {hasMore && (
          <div className="flex justify-center pt-2">
            <Button
              variant="outline"
              size="sm"
              onClick={loadMore}
              className="text-xs font-mono h-8"
            >
              Load more pools (+{LOAD_PAGE})
            </Button>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
