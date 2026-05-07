/**
 * Phase 2 Route Finder — Pool browser tab (read-only MVP).
 *
 * Fetches /api/pools?chain_id=N&dex_id=X&limit=200 on mount and when
 * the DEX filter changes (R1: useEffect + useState).
 * Loads DEX list from /api/dexes for the filter dropdown.
 *
 * Filters (all client-side after initial load):
 *   - DEX dropdown (triggers new server request — different DEX = different dataset)
 *   - Protocol type chips (multiselect, client-side)
 *   - Token search (symbol or address prefix, client-side)
 *
 * Pagination: loads LIMIT=200 per request, "Load more" increments offset.
 * (Offset-based: re-fetches with limit+200 on each Load more click.)
 *
 * R8 fail-honest: empty items = explicit "no pools" message, never hidden.
 * Errors shown verbatim.
 *
 * Local types mirror shared-ts — no cross-package import required.
 */
"use client";

import { useEffect, useMemo, useState } from "react";

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
import { shortAddr } from "@/lib/format";

// ── Local type mirrors (no cross-package import) ───────────────────────────

/** Mirrors DexInfo from shared-ts — no cross-package import */
interface DexInfo {
  id: string;
  name: string;
  protocol_type: string;
  chain_id: number;
}

interface DexesResponse {
  count: number;
  chain_id: number;
  items: DexInfo[];
}

/** Mirrors PoolInfo from shared-ts — no cross-package import */
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

const EDGE_URL = process.env.NEXT_PUBLIC_EDGE_URL ?? "http://localhost:8787";
const LOAD_PAGE = 200;
const DASH = "—";

// All known protocol types for the multiselect chips.
const KNOWN_PROTOCOLS = ["UNISWAP_V2", "UNISWAP_V3", "CURVE", "BALANCER"];

// ── Helpers ────────────────────────────────────────────────────────────────

function fmtFeeTier(tier: number | null | undefined): string {
  if (tier == null) return DASH;
  // fee_tier is in basis-points (e.g. 500 = 0.05%, 3000 = 0.3%, 10000 = 1%)
  return `${(tier / 10_000).toFixed(tier % 100 === 0 ? 2 : 4)}%`;
}

function tokenPairLabel(pool: PoolInfo): string {
  const t0 = pool.token0_symbol ?? shortAddr(pool.token0_address);
  const t1 = pool.token1_symbol ?? shortAddr(pool.token1_address);
  return `${t0}/${t1}`;
}

// Protocol badge colours — reuse same palette as DexesTab.
const PROTOCOL_CLASSES: Record<string, string> = {
  UNISWAP_V3: "bg-violet-500/15 text-violet-300 border-violet-500/40",
  UNISWAP_V2: "bg-pink-500/15 text-pink-300 border-pink-500/40",
  CURVE: "bg-amber-500/15 text-amber-300 border-amber-500/40",
  BALANCER: "bg-sky-500/15 text-sky-300 border-sky-500/40",
};
const PROTOCOL_FALLBACK = "bg-slate-500/15 text-slate-300 border-slate-500/40";

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

  // ── DEX list for filter dropdown ──
  const [dexes, setDexes] = useState<DexInfo[]>([]);
  const [dexesLoading, setDexesLoading] = useState(true);

  useEffect(() => {
    const ctrl = new AbortController();
    setDexesLoading(true);
    fetch(`${EDGE_URL}/api/dexes?chain_id=${chainId}`, {
      signal: ctrl.signal,
      credentials: "include",
      headers: { accept: "application/json" },
    })
      .then((r) => {
        if (!r.ok) throw new Error(`HTTP ${r.status}`);
        return r.json() as Promise<DexesResponse>;
      })
      .then((d) => {
        setDexes(d.items ?? []);
        setDexesLoading(false);
      })
      .catch((e: Error) => {
        if (e.name !== "AbortError") setDexesLoading(false);
      });
    return () => ctrl.abort();
  }, [chainId]);

  // ── Filter state ──
  const [selectedDexId, setSelectedDexId] = useState<string>("__all__");
  const [activeProtocols, setActiveProtocols] = useState<Set<string>>(new Set());
  const [search, setSearch] = useState("");

  // ── Pool fetch state ──
  const [pools, setPools] = useState<PoolInfo[]>([]);
  const [totalCount, setTotalCount] = useState(0);
  const [limit, setLimit] = useState(LOAD_PAGE);
  const [poolsLoading, setPoolsLoading] = useState(true);
  const [fetchError, setFetchError] = useState<string | null>(null);

  // Fetch whenever dex filter or limit changes.
  useEffect(() => {
    const ctrl = new AbortController();
    setPoolsLoading(true);
    setFetchError(null);

    const url = new URL(`${EDGE_URL}/api/pools`);
    url.searchParams.set("chain_id", String(chainId));
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
  }, [chainId, selectedDexId, limit]);

  // ── Client-side filtering (protocol chips + search) ──
  const filteredPools = useMemo(() => {
    let result = pools;

    // Protocol filter — only active if at least one chip is toggled.
    if (activeProtocols.size > 0) {
      result = result.filter((p) => activeProtocols.has(p.protocol_type));
    }

    // Search by token symbol or address (prefix, case-insensitive).
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

  const onDexChange = (value: string) => {
    setSelectedDexId(value);
    setLimit(LOAD_PAGE); // reset pagination on DEX change
  };

  const loadMore = () => {
    setLimit((prev) => prev + LOAD_PAGE);
  };

  // Whether there are more results on the server side.
  const hasMore = pools.length < totalCount && !poolsLoading;

  // ── Render ─────────────────────────────────────────────────────────────

  return (
    <Card>
      <CardHeader>
        <div className="flex items-start justify-between gap-4">
          <div>
            <CardTitle>Pools · chain {chainId}</CardTitle>
            <p className="text-xs text-muted-foreground mt-1">
              Browse — selection / allowlist in next phase
            </p>
          </div>
          {!poolsLoading && !fetchError && (
            <span className="text-xs text-muted-foreground font-mono whitespace-nowrap">
              {filteredPools.length} shown
              {filteredPools.length !== pools.length && ` of ${pools.length} loaded`}
              {` · ${totalCount} total`}
            </span>
          )}
        </div>
      </CardHeader>
      <CardContent className="space-y-4">

        {/* ── Filters bar ── */}
        <div className="flex flex-wrap items-center gap-3">

          {/* DEX dropdown */}
          <Select
            value={selectedDexId}
            onValueChange={onDexChange}
            disabled={dexesLoading}
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

          {/* Protocol type chips (multiselect) */}
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
                  title={active ? `Remove ${proto} filter` : `Filter by ${proto}`}
                >
                  {proto}
                </button>
              );
            })}
            {activeProtocols.size > 0 && (
              <button
                type="button"
                onClick={() => setActiveProtocols(new Set())}
                className="text-[10px] text-muted-foreground underline ml-1"
              >
                clear
              </button>
            )}
          </div>

          {/* Token search */}
          <Input
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Search token symbol or address…"
            className="h-8 text-xs w-56 ml-auto"
          />
        </div>

        {/* ── Error ── */}
        {fetchError && (
          <p className="text-xs text-destructive font-mono bg-destructive/10 px-3 py-2 rounded">
            Failed to load pools: {fetchError}
          </p>
        )}

        {/* ── Loading ── */}
        {poolsLoading && !fetchError && (
          <p className="text-xs text-muted-foreground">Loading pools from edge…</p>
        )}

        {/* ── Empty state (R8) ── */}
        {!poolsLoading && !fetchError && filteredPools.length === 0 && (
          <p className="text-xs text-muted-foreground py-6 text-center">
            {pools.length === 0
              ? `No pools available for chain ${chainId}${selectedDexId !== "__all__" ? " + selected DEX" : ""} — check backend indexer.`
              : "No pools match the current filters."}
          </p>
        )}

        {/* ── Pool table ── */}
        {!poolsLoading && filteredPools.length > 0 && (
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>DEX</TableHead>
                <TableHead>Protocol</TableHead>
                <TableHead>Pair</TableHead>
                <TableHead>Fee</TableHead>
                <TableHead>Pool address</TableHead>
                <TableHead className="text-center">Status</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {filteredPools.map((pool) => (
                <TableRow key={pool.id}>
                  <TableCell className="text-xs font-medium">{pool.dex_name}</TableCell>
                  <TableCell>
                    <ProtocolBadge type={pool.protocol_type} />
                  </TableCell>
                  <TableCell className="font-mono text-xs font-medium">
                    {tokenPairLabel(pool)}
                  </TableCell>
                  <TableCell className="font-mono text-xs text-muted-foreground">
                    {fmtFeeTier(pool.fee_tier)}
                  </TableCell>
                  <TableCell className="font-mono text-xs text-muted-foreground">
                    <span title={pool.address}>{shortAddr(pool.address)}</span>
                  </TableCell>
                  <TableCell className="text-center">
                    {pool.is_active ? (
                      <span className="text-[10px] text-emerald-400 font-mono">active</span>
                    ) : (
                      <span className="text-[10px] text-muted-foreground font-mono">inactive</span>
                    )}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        )}

        {/* ── Load more ── */}
        {hasMore && (
          <div className="pt-2 flex justify-center">
            <Button
              variant="outline"
              size="sm"
              onClick={loadMore}
              disabled={poolsLoading}
            >
              {poolsLoading ? "Loading…" : `Load more (${pools.length} / ${totalCount})`}
            </Button>
          </div>
        )}

      </CardContent>
    </Card>
  );
}
