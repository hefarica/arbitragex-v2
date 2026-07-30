/**
 * Runtime Cartridges tab — the REAL set of .rhai strategy cartridges loaded on
 * the searcher hot-path (the 264-strategy library + core pack).
 *
 * Data source: GET /api/cartridges/runtime (searcher-rs registry snapshot via
 * Redis). NOT the curated strategy_catalog table — this is what the searcher
 * actually compiled at boot, with live state (Active/Paused).
 *
 * Toggle: POST /api/cartridges/runtime/:id/pause|resume publishes a hot-reload
 * event to the searcher (Redis `arbx:cartridge:injection`), which pauses /
 * resumes the cartridge WITHOUT a restart. Admin-gated; optimistic state is
 * reconciled against the next registry poll.
 *
 * R8 fail-honest: empty/unavailable registry → explicit empty state, never
 * fabricated cartridges. R1: non-deterministic state (poll, toggles) lives in
 * this client component; SSR snapshot passed as prop.
 */
"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { getRuntimeCartridges, toggleRuntimeCartridge } from "@/lib/api-client";
import { hasAdminSession } from "@/lib/admin-token";
import type { RuntimeCartridge, TradingConfigConfigured } from "@/lib/schemas";
import { STRATEGY_MAPPING, strategyOperators } from "@/lib/math-operator-mapping";
import { StrategySettingsSheet } from "@/components/strategy-settings-sheet";

const POLL_MS = 4000;

interface Props {
  chainId: number;
  config: TradingConfigConfigured;
  onSaved: (next: TradingConfigConfigured) => void;
  adminToken: string;
  actor: string;
}

export function RuntimeCartridgesTab({ chainId, config, onSaved, adminToken, actor }: Props) {
  const [cartridges, setCartridges] = useState<RuntimeCartridge[]>([]);
  const [registryOk, setRegistryOk] = useState<boolean | null>(null);
  const [registryReason, setRegistryReason] = useState<string | null>(null);
  const [updatedAt, setUpdatedAt] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [groupFilter, setGroupFilter] = useState<string>("all");
  const [toggling, setToggling] = useState<string | null>(null);
  const [hasSession, setHasSession] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  // Strategy settings sheet — opened on click on a cartridge card.
  const [settingsFor, setSettingsFor] = useState<RuntimeCartridge | null>(null);
  const mountedRef = useRef(true);

  useEffect(() => {
    mountedRef.current = true;
    setHasSession(hasAdminSession());
    const id = setInterval(() => setHasSession(hasAdminSession()), 30_000);
    return () => {
      mountedRef.current = false;
      clearInterval(id);
    };
  }, []);

  const load = useCallback(async () => {
    const r = await getRuntimeCartridges(chainId);
    if (!mountedRef.current) return;
    if (r.ok && r.data.ok && r.data.data) {
      setCartridges(r.data.data.cartridges);
      setRegistryOk(true);
      setRegistryReason(null);
      setUpdatedAt(r.data.data.updated_at ?? r.data.updated_at ?? null);
    } else {
      setCartridges([]);
      setRegistryOk(false);
      setRegistryReason(
        r.ok ? r.data.reason ?? "registry_unavailable" : r.error,
      );
      setUpdatedAt(null);
    }
  }, [chainId]);

  useEffect(() => {
    void load();
    const id = setInterval(() => void load(), POLL_MS);
    return () => clearInterval(id);
  }, [load]);

  // Derive family groups from cartridge id prefix (mev_<group>_...) or category.
  const groups = useMemo(() => {
    const g = new Map<string, number>();
    for (const c of cartridges) {
      const fam = familyOf(c);
      g.set(fam, (g.get(fam) ?? 0) + 1);
    }
    return Array.from(g.entries()).sort((a, b) => a[0].localeCompare(b[0]));
  }, [cartridges]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    return cartridges.filter((c) => {
      if (groupFilter !== "all" && familyOf(c) !== groupFilter) return false;
      if (!q) return true;
      return (
        c.id.toLowerCase().includes(q) ||
        (c.name ?? "").toLowerCase().includes(q) ||
        (c.category ?? "").toLowerCase().includes(q) ||
        (c.description ?? "").toLowerCase().includes(q)
      );
    });
  }, [cartridges, query, groupFilter]);

  const activeCount = useMemo(
    () => cartridges.filter((c) => isActive(c.state)).length,
    [cartridges],
  );

  const onToggle = useCallback(
    async (c: RuntimeCartridge, next: boolean) => {
      if (!hasAdminSession()) {
        setHasSession(false);
        setNotice("Login required: unlock an admin session at /killswitch first.");
        return;
      }
      setToggling(c.id);
      setNotice(null);
      const res = await toggleRuntimeCartridge(c.id, next ? "resume" : "pause", adminToken, actor);
      setToggling(null);
      if (res.ok) {
        // Optimistic update; reconciled on next poll.
        setCartridges((prev) =>
          prev.map((x) =>
            x.id === c.id ? { ...x, state: next ? "Active" : "Paused" } : x,
          ),
        );
        setNotice(`${next ? "Resumed" : "Paused"} ${c.id} — hot-reload published`);
      } else {
        setNotice(`Toggle failed for ${c.id}: ${res.error}`);
      }
    },
    [adminToken, actor],
  );

  return (
    <div className="grid gap-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <p className="text-sm text-muted-foreground">
          {registryOk
            ? `${activeCount} active · ${cartridges.length} loaded on searcher (chain ${chainId})`
            : "Searcher cartridge registry unavailable"}
          {updatedAt && (
            <span className="ml-2 font-mono text-xs">· updated {updatedAt}</span>
          )}
        </p>
        <div className="flex items-center gap-2">
          <Input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search id / name / category…"
            className="h-8 w-64 text-xs"
          />
          <select
            value={groupFilter}
            onChange={(e) => setGroupFilter(e.target.value)}
            className="h-8 rounded-md border border-border bg-background px-2 text-xs"
          >
            <option value="all">All families ({cartridges.length})</option>
            {groups.map(([g, n]) => (
              <option key={g} value={g}>
                {g} ({n})
              </option>
            ))}
          </select>
          <Button variant="outline" size="sm" onClick={() => void load()}>
            Refresh
          </Button>
        </div>
      </div>

      {notice && (
        <p className="text-xs font-mono text-muted-foreground">{notice}</p>
      )}
      {!hasSession && (
        <p className="text-xs font-mono text-warning">
          No admin session — toggles disabled. <a href="/killswitch" className="underline">Unlock at /killswitch</a>
        </p>
      )}

      {registryOk === false && (
        <Card>
          <CardContent className="py-8 text-center">
            <p className="font-mono text-xs uppercase tracking-widest text-muted-foreground">
              Registry unavailable — {registryReason}
            </p>
            <p className="mt-2 text-sm text-muted-foreground">
              The searcher has not published its cartridge registry (down, boot
              pending, or TTL expired). R8 fail-honest: no cartridges fabricated.
            </p>
          </CardContent>
        </Card>
      )}

      {registryOk && filtered.length === 0 && (
        <p className="py-8 text-center text-sm text-muted-foreground italic">
          No cartridges match the current filter.
        </p>
      )}

      <div className="grid gap-3 md:grid-cols-2 lg:grid-cols-3">
        {filtered.map((c) => {
          const active = isActive(c.state);
          const hasOverride = config.strategy_configs?.[c.id] != null;
          return (
            <Card
              key={c.id}
              className={`cursor-pointer transition-colors hover:border-primary/50 ${
                hasOverride ? "border-info/40" : undefined
              }`}
              onClick={() => setSettingsFor(c)}
              title="Click to configure this strategy's parameters"
            >
              <CardContent className="space-y-2 py-4">
                <div className="flex items-start justify-between gap-2">
                  <div className="min-w-0">
                    <div className="truncate font-medium" title={c.name ?? c.id}>
                      {c.name ?? c.id}
                    </div>
                    <div className="truncate font-mono text-xs text-muted-foreground" title={c.id}>
                      {c.id}
                    </div>
                  </div>
                  <span onClick={(e) => e.stopPropagation()}>
                    <Switch
                      checked={active}
                      disabled={!hasSession || toggling === c.id}
                      onCheckedChange={(next) => void onToggle(c, next)}
                      title={
                        !hasSession
                          ? "Admin session required"
                          : active
                            ? "Pause this cartridge on the searcher (hot-reload)"
                            : "Resume this cartridge on the searcher (hot-reload)"
                      }
                    />
                  </span>
                </div>
                {c.description && (
                  <p className="line-clamp-2 text-xs text-muted-foreground">{c.description}</p>
                )}
                <div className="flex flex-wrap gap-1 pt-1">
                  <Badge
                    variant="outline"
                    className={`text-[10px] font-bold ${
                      active
                        ? "bg-success/15 text-success border-success/40"
                        : "bg-muted text-muted-foreground border-border"
                    }`}
                  >
                    {c.state ?? "unknown"}
                  </Badge>
                  <Badge variant="outline" className="text-[10px]">
                    {familyOf(c)}
                  </Badge>
                  {c.category && (
                    <Badge variant="outline" className="text-[10px]">
                      {c.category}
                    </Badge>
                  )}
                  {c.version && (
                    <Badge variant="outline" className="text-[10px]">
                      v{c.version}
                    </Badge>
                  )}
                  {/* 264×31 operator LED strip — one dot per applicable math
                      operator. LIT (strategy color) when the cartridge is Active
                      (operators are live on the searcher); DIM when paused.
                      Derived from the cartridge id → MEV-XX-YYY canonical id. */}
                  {(() => {
                    const ops = strategyOperators(cartridgeToMevId(c.id), STRATEGY_MAPPING);
                    if (ops.length === 0) return null;
                    return (
                      <span
                        className="flex items-center gap-[3px] flex-wrap"
                        title={`${ops.length} math operators applied by this strategy (264×31 matrix): ${ops.map((o) => `#${o}`).join(", ")}. ${active ? "LIVE on searcher" : "paused"}`}
                      >
                        {ops.map((op) => (
                          <span
                            key={op}
                            title={`Operator #${op}${active ? " — live" : " — paused"}`}
                            className={`inline-block h-2 w-2 rounded-full transition-colors ${
                              active
                                ? "bg-emerald-400 shadow-[0_0_6px_rgba(52,211,153,0.8)]"
                                : "bg-muted-foreground/25"
                            }`}
                          />
                        ))}
                        <span className="ml-1 text-[10px] text-muted-foreground">
                          {ops.length} ops
                        </span>
                      </span>
                    );
                  })()}
                  {hasOverride && (
                    <Badge
                      variant="outline"
                      className="text-[10px] bg-warning/10 text-warning border-warning/40"
                      title="This strategy has a per-strategy settings override (click to edit)"
                    >
                      ⚙ settings
                    </Badge>
                  )}
                </div>
              </CardContent>
            </Card>
          );
        })}
      </div>

      {/* Per-strategy settings sheet — click on a cartridge card opens this.
          Edits trading_config.strategy_configs[c.id] with the Excel-spec
          parameters (yield gates, legs model, cost ceilings, pool constraints)
          and round-trips via putTradingConfig (searcher hot-reloads ≤1s). */}
      {settingsFor && (
        <StrategySettingsSheet
          open={settingsFor != null}
          onOpenChange={(open) => {
            if (!open) setSettingsFor(null);
          }}
          strategyKind={settingsFor.id}
          displayName={settingsFor.name ?? settingsFor.id}
          config={config}
          onSaved={onSaved}
          adminToken={adminToken}
          actor={actor}
        />
      )}
    </div>
  );
}

function isActive(state: string | null | undefined): boolean {
  return (state ?? "").toLowerCase() === "active";
}

// Family group derived from the cartridge id (mev_<group>_... → "MEV-<group>")
// or falls back to the declared category / "core".
function familyOf(c: RuntimeCartridge): string {
  const m = /^mev_(\d+)_/i.exec(c.id);
  if (m) return `MEV-${m[1]}`;
  if (c.category) return c.category;
  return "core";
}

// Convert a cartridge id (mev_01_001_dex_dex) to the canonical MEV id used by
// the 264×31 mapping (MEV-01-001). Non-mev cartridges (core pack: dex_arb,
// backrun, …) return "" → no operator badge (mapping only covers the library).
function cartridgeToMevId(id: string): string {
  const m = /^mev_(\d+)_(\d+)_/i.exec(id);
  if (!m) return "";
  return `MEV-${m[1]}-${m[2]}`;
}
