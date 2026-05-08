/**
 * Phase 2 Route Finder — DEX selector tab.
 *
 * Fetches /api/dexes?chain_id=N on mount (R1: useEffect + useState([])).
 * Renders one checkbox row per DEX. The operator selects which DEXes the
 * searcher scans; the selection is persisted via PUT /admin/trading-config/:chainId
 * with enabled_dex_ids: string[] | null (null = all enabled).
 *
 * Chain selector (Phase 2 multichain): operator can browse DEXes for any of the
 * 6 supported chains. Save still persists only for config.chain_id (the parent
 * config row). TODO(Phase 3): support per-chain trading_config rows so Save can
 * target the selected chain.
 *
 * Save button is gated on hasAdminSession() (per Task 12 pattern).
 * R8 fail-honest: empty array shows "No DEXes seeded for chain N".
 * Errors are shown verbatim; never hidden.
 *
 * Local types mirror shared-ts — no cross-package import required.
 */
"use client";

import { useEffect, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { hasAdminSession } from "@/lib/admin-token";
import { putTradingConfig } from "@/lib/api-client";
import type { TradingConfigConfigured } from "@/lib/schemas";

// ── Local type mirrors (no cross-package import) ───────────────────────────

/** Mirrors DexInfo from shared-ts — no cross-package import */
interface DexInfo {
  id: string;
  name: string;
  protocol_type: string;
  is_active: boolean;
  chain_id: number;
  factory_count: number | null;
  pool_count: number | null;
  volume_24h_usd: number | null;
  tvl_usd: number | null;
  enabled: boolean;
}

interface DexesResponse {
  count: number;
  chain_id: number;
  items: DexInfo[];
}

// ── Chain catalog (doctrinal infrastructure constants — not productive data) ──

const SUPPORTED_CHAINS = [
  { chain_id: 1,     name: "Ethereum", short: "ETH"  },
  { chain_id: 42161, name: "Arbitrum", short: "ARB"  },
  { chain_id: 10,    name: "Optimism", short: "OP"   },
  { chain_id: 8453,  name: "Base",     short: "BASE" },
  { chain_id: 137,   name: "Polygon",  short: "MATIC"},
  { chain_id: 56,    name: "BSC",      short: "BSC"  },
] as const;

// ── Helpers ────────────────────────────────────────────────────────────────

const EDGE_URL = process.env.NEXT_PUBLIC_EDGE_URL ?? "http://localhost:8787";

const DASH = "—";

function fmtUsdCompact(n: number | null | undefined): string {
  if (n == null || !Number.isFinite(n)) return DASH;
  const abs = Math.abs(n);
  if (abs >= 1e9) return `$${(n / 1e9).toFixed(1)}B`;
  if (abs >= 1e6) return `$${(n / 1e6).toFixed(0)}M`;
  if (abs >= 1e3) return `$${(n / 1e3).toFixed(0)}K`;
  return `$${n.toFixed(0)}`;
}

function fmtCount(n: number | null | undefined): string {
  if (n == null) return DASH;
  return String(n);
}

/**
 * R8 fail-honest: pool_sync_worker (Phase 3) has not run yet.
 * SQL COUNT(*) returns 0 — a true integer — but the semantic meaning is
 * "not yet indexed", NOT "verified zero pools". Render em-dash until >0.
 * TODO(Phase 3): remove this guard once pool_sync_worker ships and
 * pool_count can be a genuine measurement.
 */
function fmtPoolCount(n: number | null | undefined): { value: string; muted: boolean } {
  if (n == null || n === 0) return { value: DASH, muted: true };
  return { value: String(n), muted: false };
}

// Protocol badge colours — tuned for dark dashboards.
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
  config: TradingConfigConfigured;
  onSaved: (next: TradingConfigConfigured) => void;
  adminToken: string;
  actor: string;
}

// ── Component ──────────────────────────────────────────────────────────────

export function DexesTab({ config, onSaved, adminToken, actor }: Props) {
  // config.chain_id is the parent config chain (used for Save).
  // selectedChainId drives the DEX browser — operator can switch freely.
  const configChainId = config.chain_id;

  // ── Chain selector state (R1: init = config.chain_id) ──
  const [selectedChainId, setSelectedChainId] = useState<number>(configChainId);

  // ── Fetch state (R1: init=[]) ──
  const [dexes, setDexes] = useState<DexInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [fetchError, setFetchError] = useState<string | null>(null);

  // ── Selection state ──
  // null means "all enabled" (backend default). Initialise from config if present.
  const [selected, setSelected] = useState<Set<string>>(() => {
    const ids = config.enabled_dex_ids;
    return ids ? new Set(ids) : new Set<string>();
  });
  // Whether the operator explicitly chose to restrict (vs null=all).
  // If enabled_dex_ids is null → "select all" mode; else explicit mode.
  const [restrictMode, setRestrictMode] = useState<boolean>(
    Array.isArray(config.enabled_dex_ids),
  );

  // ── Admin session (R1: useEffect) ──
  const [hasSession, setHasSession] = useState(false);
  useEffect(() => {
    setHasSession(hasAdminSession());
    const id = setInterval(() => setHasSession(hasAdminSession()), 30_000);
    return () => clearInterval(id);
  }, []);

  // ── Fetch DEXes on mount / selectedChainId change ──
  // When chain changes reset selection (different chain = different DEX set).
  useEffect(() => {
    setSelected(new Set<string>());
    setRestrictMode(false);

    const ctrl = new AbortController();
    setLoading(true);
    setFetchError(null);
    fetch(`${EDGE_URL}/api/dexes?chain_id=${selectedChainId}`, {
      signal: ctrl.signal,
      credentials: "include",
      headers: { accept: "application/json" },
    })
      .then((r) => {
        if (!r.ok) throw new Error(`edge HTTP ${r.status}`);
        return r.json() as Promise<DexesResponse>;
      })
      .then((d) => {
        setDexes(d.items ?? []);
        setLoading(false);
      })
      .catch((e: Error) => {
        if (e.name === "AbortError") return;
        setFetchError(e.message);
        setLoading(false);
      });
    return () => ctrl.abort();
  }, [selectedChainId]);

  // ── Save state ──
  const [saving, setSaving] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);

  // ── Handlers ──

  const toggleDex = (id: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
    // As soon as the operator clicks a checkbox, we're in restrict mode.
    setRestrictMode(true);
  };

  const selectAll = () => {
    setSelected(new Set(dexes.map((d) => d.id)));
    setRestrictMode(false); // all selected = null payload = "all enabled"
  };

  const clearAll = () => {
    setSelected(new Set());
    setRestrictMode(true);
  };

  const onSave = async () => {
    if (!hasAdminSession()) {
      setHasSession(false);
      setMsg("Login required: open /killswitch and unlock an admin session first.");
      return;
    }
    setSaving(true);
    setMsg(null);

    // null = all enabled (backend default). Array = restricted to listed IDs.
    const enabled_dex_ids: string[] | null = restrictMode
      ? Array.from(selected)
      : null;

    const { configured: _c, chain_id: _cid, updated_at: _ua, updated_by: _ub, ...rest } = config;
    void _c; void _cid; void _ua; void _ub;

    const body = { ...rest, enabled_dex_ids };
    // TODO(Phase 3): support per-chain config rows; for now Save always targets
    // configChainId (the parent config row, typically chain 1).
    const res = await putTradingConfig(configChainId, body, adminToken, actor);
    setSaving(false);

    if (res.ok) {
      onSaved({
        ...config,
        enabled_dex_ids,
        updated_at: res.data.updated_at,
        updated_by: res.data.updated_by,
      });
      const summary = enabled_dex_ids === null
        ? "all DEXes enabled (null)"
        : `${enabled_dex_ids.length} DEX(es) restricted`;
      setMsg(`Saved · ${summary} · searcher reloads in ≤1s`);
    } else {
      setMsg(res.error);
    }
  };

  // ── Derived ──
  const allSelectedCount = selected.size;
  const totalCount = dexes.length;

  // ── Render ─────────────────────────────────────────────────────────────

  const selectedChainName =
    SUPPORTED_CHAINS.find((c) => c.chain_id === selectedChainId)?.name ?? String(selectedChainId);

  return (
    <Card>
      <CardHeader>
        <div className="flex items-center justify-between gap-4 flex-wrap">
          <CardTitle>
            DEX Selection · {selectedChainName}
            {selectedChainId !== configChainId && (
              <span className="ml-2 text-xs font-normal text-amber-400 font-mono">
                (browsing — Save targets chain {configChainId})
              </span>
            )}
          </CardTitle>

          {/* Chain selector */}
          <Select
            value={String(selectedChainId)}
            onValueChange={(v) => setSelectedChainId(Number(v))}
          >
            <SelectTrigger size="sm" className="w-48">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {SUPPORTED_CHAINS.map((c) => (
                <SelectItem key={c.chain_id} value={String(c.chain_id)}>
                  {c.name} ({c.short})
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      </CardHeader>
      <CardContent className="space-y-4">

        {/* Error banner */}
        {fetchError && (
          <p className="text-xs text-destructive font-mono bg-destructive/10 px-3 py-2 rounded">
            Failed to load DEXes: {fetchError}
          </p>
        )}

        {/* Loading skeleton */}
        {loading && !fetchError && (
          <p className="text-xs text-muted-foreground">Loading DEXes from edge…</p>
        )}

        {/* Empty state (R8 fail-honest) */}
        {!loading && !fetchError && dexes.length === 0 && (
          <p className="text-xs text-muted-foreground py-4 text-center">
            No DEXes seeded for {selectedChainName} (chain {selectedChainId}) — migration 043 seeds top 6 DEXes per chain.
          </p>
        )}

        {/* DEX rows */}
        {!loading && dexes.length > 0 && (
          <>
            {/* Bulk controls */}
            <div className="flex items-center gap-2 text-xs text-muted-foreground">
              <button
                type="button"
                onClick={selectAll}
                className="underline hover:text-foreground transition-colors"
              >
                select all
              </button>
              <span>·</span>
              <button
                type="button"
                onClick={clearAll}
                className="underline hover:text-foreground transition-colors"
              >
                clear all
              </button>
              <span className="ml-auto">
                {restrictMode
                  ? `${allSelectedCount} / ${totalCount} selected`
                  : `all ${totalCount} enabled (null)`}
              </span>
            </div>

            {/* Table */}
            <div className="rounded-md border overflow-hidden">
              <table className="w-full text-sm">
                <thead className="bg-muted/50">
                  <tr>
                    <th className="w-8 px-3 py-2 text-left" aria-label="Select DEX"></th>
                    <th className="px-3 py-2 text-left text-xs text-muted-foreground font-medium uppercase tracking-wider">DEX</th>
                    <th className="px-3 py-2 text-left text-xs text-muted-foreground font-medium uppercase tracking-wider">Protocol</th>
                    <th className="px-3 py-2 text-right text-xs text-muted-foreground font-medium uppercase tracking-wider">Pools</th>
                    <th className="px-3 py-2 text-right text-xs text-muted-foreground font-medium uppercase tracking-wider">Factories</th>
                    <th className="px-3 py-2 text-right text-xs text-muted-foreground font-medium uppercase tracking-wider">Vol 24h</th>
                    <th className="px-3 py-2 text-right text-xs text-muted-foreground font-medium uppercase tracking-wider">TVL</th>
                    <th className="px-3 py-2 text-center text-xs text-muted-foreground font-medium uppercase tracking-wider">Status</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-border">
                  {dexes.map((dex) => {
                    const isChecked = restrictMode ? selected.has(dex.id) : true;
                    return (
                      <tr
                        key={dex.id}
                        className="hover:bg-muted/30 transition-colors cursor-pointer"
                        onClick={() => toggleDex(dex.id)}
                      >
                        <td className="px-3 py-2.5">
                          <input
                            type="checkbox"
                            aria-label={`Enable ${dex.name} for scanning`}
                            checked={isChecked}
                            onChange={() => toggleDex(dex.id)}
                            onClick={(e) => e.stopPropagation()}
                            className="h-4 w-4 accent-primary cursor-pointer"
                          />
                        </td>
                        <td className="px-3 py-2.5 font-medium">{dex.name}</td>
                        <td className="px-3 py-2.5">
                          <ProtocolBadge type={dex.protocol_type} />
                        </td>
                        <td className={`px-3 py-2.5 text-right tabular-nums${fmtPoolCount(dex.pool_count).muted ? " text-slate-400" : ""}`}>
                          {fmtPoolCount(dex.pool_count).value}
                        </td>
                        <td className="px-3 py-2.5 text-right tabular-nums">
                          {fmtCount(dex.factory_count)}
                        </td>
                        <td className="px-3 py-2.5 text-right tabular-nums text-muted-foreground">
                          {fmtUsdCompact(dex.volume_24h_usd)}
                        </td>
                        <td className="px-3 py-2.5 text-right tabular-nums text-muted-foreground">
                          {fmtUsdCompact(dex.tvl_usd)}
                        </td>
                        <td className="px-3 py-2.5 text-center">
                          {dex.is_active ? (
                            <span className="text-[10px] text-emerald-400 font-mono">active</span>
                          ) : (
                            <span className="text-[10px] text-muted-foreground font-mono">inactive</span>
                          )}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          </>
        )}

        {/* Save bar */}
        <div className="flex items-center gap-3 pt-2 border-t">
          <Button
            onClick={onSave}
            disabled={saving || !hasSession || loading}
            title={!hasSession ? "Admin session required — open /killswitch first" : undefined}
          >
            {saving ? "Saving…" : "Save DEX selection"}
          </Button>
          {msg && <span className="text-xs font-mono text-muted-foreground">{msg}</span>}
          {!hasSession && !msg && (
            <span className="text-xs text-amber-400 font-mono">
              No admin session —{" "}
              <a href="/killswitch" className="underline">unlock at /killswitch</a>
            </span>
          )}
        </div>

      </CardContent>
    </Card>
  );
}
