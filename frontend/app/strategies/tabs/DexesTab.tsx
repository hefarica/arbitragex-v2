/**
 * Phase 2 Route Finder — DEX selector tab.
 *
 * Migrated to Omni-Store (Phase 8):
 * - Consumes dexes from useOmniStore.
 * - Preserves admin session gating and R8 fail-honest.
 */
"use client";

import { useEffect, useMemo, useState } from "react";
import { useShallow } from "zustand/react/shallow";

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
import { putTradingConfig, getApiBaseUrl } from "@/lib/api-client";
import { useOmniStore } from "@/lib/store/omni-store";
import type { TradingConfigConfigured } from "@/lib/schemas";

// ── Local type mirrors (no cross-package import) ───────────────────────────

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

async function fetchTradingConfigForChain(
  chainId: number,
): Promise<TradingConfigConfigured | null> {
  const baseUrl = getApiBaseUrl();
  const r = await fetch(`${baseUrl}/api/trading-config?chain_id=${chainId}`, {
    credentials: "include",
    headers: { accept: "application/json" },
  });
  if (!r.ok) {
    throw new Error(`fetch trading-config chain=${chainId}: HTTP ${r.status}`);
  }
  const j = (await r.json()) as { configured?: boolean } & TradingConfigConfigured;
  return j.configured === true ? (j as TradingConfigConfigured) : null;
}

const DASH = "—";

function ProtocolBadge({ type }: { type: string }) {
  const PROTOCOL_CLASSES: Record<string, string> = {
    UNISWAP_V3: "bg-chart-1/15 text-chart-1 border-chart-1/40",
    UNISWAP_V2: "bg-chart-2/15 text-chart-2 border-chart-2/40",
    CURVE: "bg-chart-4/15 text-chart-4 border-chart-4/40",
    BALANCER: "bg-chart-3/15 text-chart-3 border-chart-3/40",
  };
  const cls = PROTOCOL_CLASSES[type] ?? "bg-muted text-muted-foreground border-border";
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
  const configChainId = config.chain_id;

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

  const [selectedChainId, setSelectedChainId] = useState<number>(configChainId);

  // Selection state
  const [selected, setSelected] = useState<Set<string>>(() => {
    const ids = config.enabled_dex_ids;
    return ids ? new Set(ids) : new Set<string>();
  });
  const [restrictMode, setRestrictMode] = useState<boolean>(
    Array.isArray(config.enabled_dex_ids),
  );

  // Admin session
  const [hasSession, setHasSession] = useState(false);
  useEffect(() => {
    setHasSession(hasAdminSession());
    const id = setInterval(() => setHasSession(hasAdminSession()), 30_000);
    return () => clearInterval(id);
  }, []);

  // Fetch DEXes on chain change
  useEffect(() => {
    fetchRegistry(selectedChainId);
    setSelected(new Set<string>());
    setRestrictMode(false);
  }, [selectedChainId, fetchRegistry]);

  // Save state
  const [saving, setSaving] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);

  // Handlers
  const toggleDex = (id: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
    setRestrictMode(true);
  };

  const selectAll = () => {
    setSelected(new Set(dexes.map((d) => d.id)));
    setRestrictMode(false);
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

    const enabled_dex_ids: string[] | null = restrictMode
      ? Array.from(selected)
      : null;

    let baseConfig: TradingConfigConfigured = config;
    if (selectedChainId !== configChainId) {
      try {
        const remote = await fetchTradingConfigForChain(selectedChainId);
        if (remote === null) {
          setSaving(false);
          setMsg(`Chain ${selectedChainId} has no trading_config row yet.`);
          return;
        }
        baseConfig = remote;
      } catch (e) {
        setSaving(false);
        setMsg(`Failed to load chain ${selectedChainId} config: ${(e as Error).message}`);
        return;
      }
    }

    const { configured: _c, chain_id: _cid, updated_at: _ua, updated_by: _ub, ...rest } = baseConfig;
    const body = { ...rest, enabled_dex_ids };
    const res = await putTradingConfig(selectedChainId, body, adminToken, actor);
    setSaving(false);

    if (res.ok) {
      if (selectedChainId === configChainId) {
        onSaved({
          ...config,
          enabled_dex_ids,
          updated_at: res.data.updated_at,
          updated_by: res.data.updated_by,
        });
      }
      setMsg(`Saved chain ${selectedChainId} · searcher reloads in ≤1s`);
    } else {
      setMsg(res.error);
    }
  };

  const selectedChainName =
    chainsMap.get(selectedChainId)?.name ?? String(selectedChainId);

  return (
    <Card>
      <CardHeader>
        <div className="flex items-center justify-between gap-4 flex-wrap">
          <CardTitle>
            DEX Selection · {selectedChainName}
            {selectedChainId !== configChainId && (
              <span className="ml-2 text-xs font-normal text-info font-mono">
                (Save will PUT chain {selectedChainId})
              </span>
            )}
          </CardTitle>

          <Select
            value={String(selectedChainId)}
            onValueChange={(v: string) => setSelectedChainId(Number(v))}
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
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        {registryError && (
          <p className="text-xs text-destructive font-mono bg-destructive/10 px-3 py-2 rounded">
            Error: {registryError}
          </p>
        )}

        {registryStatus === "loading" && (
          <p className="text-xs text-muted-foreground">Loading DEXes from edge…</p>
        )}

          {registryStatus !== "loading" && dexes.length === 0 && (
          <p className="text-xs text-muted-foreground py-4 text-center">
            No DEXes seeded for {selectedChainName}.
          </p>
        )}

          {registryStatus !== "loading" && dexes.length > 0 && (
          <>
            <div className="flex items-center gap-2 text-xs text-muted-foreground">
              <button type="button" onClick={selectAll} className="underline hover:text-foreground">select all</button>
              <span>·</span>
              <button type="button" onClick={clearAll} className="underline hover:text-foreground">clear</button>
              <span className="ml-auto font-mono">{selected.size} selected</span>
            </div>

            <div className="rounded-md border divide-y">
              {dexes.map((dex) => {
                const isSelected = selected.has(dex.id);
                return (
                  <div key={dex.id} className="flex items-center gap-4 px-4 py-3 hover:bg-muted/30 transition-colors group">
                    <input
                      type="checkbox"
                      checked={isSelected}
                      onChange={() => toggleDex(dex.id)}
                      className="h-4 w-4 rounded border-border text-primary focus:ring-primary/50"
                    />
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2">
                        <span className="text-sm font-medium">{dex.name}</span>
                        <ProtocolBadge type={dex.protocol_type} />
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>

            <div className="flex items-center justify-between pt-2">
              <p className="text-[10px] text-muted-foreground max-w-[60%]">
                {restrictMode 
                  ? "Restrict mode active: only selected DEXes will be scanned." 
                  : "All enabled mode: every seeded DEX on this chain is active."}
              </p>
              <Button size="sm" onClick={onSave} disabled={saving} className="font-mono h-8">
                {saving ? "Saving…" : "Save Selection"}
              </Button>
            </div>
            {msg && <p className="text-[10px] font-mono text-info text-right">{msg}</p>}
          </>
        )}
      </CardContent>
    </Card>
  );
}
