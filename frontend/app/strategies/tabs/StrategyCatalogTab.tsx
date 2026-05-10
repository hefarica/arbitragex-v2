/**
 * Sprint 2 Task 2.3 — Strategy Catalog tab.
 *
 * Renders all entries from /api/strategy-catalog. Each card shows whether
 * the strategy is currently in `trading_config.enabled_strategies`. Toggle
 * mutates the array and PUTs the whole config back.
 *
 * Cards with `ethical_constraint='defensive_only'` show a red badge and
 * the switch is disabled-ON: defensive sandwich protection cannot be
 * turned off (operator must keep anti-sandwich active), and it never
 * enables an offensive sandwich behaviour either.
 */
"use client";

import { useEffect, useMemo, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { hasAdminSession } from "@/lib/admin-token";
import { putTradingConfig } from "@/lib/api-client";
import {
  DEFAULT_STRATEGY_RUNTIME_CONFIG,
  type LifecycleStatus,
  type StrategyCatalogEntry,
  type StrategyConfigs,
  type StrategyRuntimeConfig,
  type TradingConfigConfigured,
} from "@/lib/schemas";

// Truth labels for the lifecycle badge. Wording is chosen so the operator sees
// at a glance whether toggling does anything.
const LIFECYCLE_LABEL: Record<LifecycleStatus, string> = {
  live: "LIVE",
  designed: "DESIGNED",
  scaffold: "SCAFFOLD",
  not_started: "NOT STARTED",
  defensive_only: "DEFENSIVE",
};

const LIFECYCLE_TOOLTIP: Record<LifecycleStatus, string> = {
  live: "Searcher emits this kind in production. Toggle controls the gate.",
  designed: "Design doc complete; no Rust emitter yet. Toggle is informational.",
  scaffold: "Enum + persistence ready; no scanner emits yet. Toggle is informational.",
  not_started: "Identifier reserved; no code, no design. Toggle is informational.",
  defensive_only: "Detection-only protection for our own swaps. Forced-on by policy.",
};

// Tailwind classes per status — token-based, theme-aware.
const LIFECYCLE_CLASSES: Record<LifecycleStatus, string> = {
  live: "bg-success/15 text-success border-success/40",
  designed: "bg-info/15 text-info border-info/40",
  scaffold: "bg-warning/15 text-warning border-warning/40",
  not_started: "bg-muted text-muted-foreground border-border",
  defensive_only: "bg-destructive/15 text-destructive border-destructive/40",
};

interface Props {
  config: TradingConfigConfigured;
  catalog: StrategyCatalogEntry[];
  onSaved: (next: TradingConfigConfigured) => void;
  adminToken: string;
  actor: string;
}

export function StrategyCatalogTab({ config, catalog, onSaved, adminToken, actor }: Props) {
  const [enabled, setEnabled] = useState<string[]>(config.enabled_strategies);
  // Migration 056 — per-strategy fine-grained config editor.
  // Default to existing strategy_configs from server snapshot; mutations
  // happen in the card thresholds below and round-trip with the same PUT.
  const [strategyConfigs, setStrategyConfigs] = useState<StrategyConfigs>(
    config.strategy_configs ?? {},
  );
  const [saving, setSaving] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  // R1: session check deferred to useEffect — never read document.cookie during SSR.
  const [hasSession, setHasSession] = useState(false);
  useEffect(() => {
    setHasSession(hasAdminSession());
    const id = setInterval(() => setHasSession(hasAdminSession()), 30_000);
    return () => clearInterval(id);
  }, []);

  const dirty = useMemo(() => {
    const enabledA = [...enabled].sort().join(",");
    const enabledB = [...config.enabled_strategies].sort().join(",");
    if (enabledA !== enabledB) return true;
    // Per-strategy config diff: cheap stable JSON compare.
    const cfgA = JSON.stringify(strategyConfigs);
    const cfgB = JSON.stringify(config.strategy_configs ?? {});
    return cfgA !== cfgB;
  }, [enabled, strategyConfigs, config.enabled_strategies, config.strategy_configs]);

  const toggle = (kind: string, checked: boolean) => {
    setEnabled((prev) => (checked ? Array.from(new Set([...prev, kind])) : prev.filter((k) => k !== kind)));
  };

  /**
   * Update a per-strategy config field. Lazily creates the entry from the
   * canonical default when the operator first edits the card. Empty
   * strings clear the field back to null (R8 fail-honest — operator's
   * "no override" intent reads as null, not 0).
   */
  const updateStrategy = (
    kind: string,
    patch: Partial<StrategyRuntimeConfig>,
  ) => {
    setStrategyConfigs((prev) => {
      const existing = prev[kind] ?? { ...DEFAULT_STRATEGY_RUNTIME_CONFIG };
      return { ...prev, [kind]: { ...existing, ...patch } };
    });
  };

  const onSave = async () => {
    if (!hasAdminSession()) {
      setHasSession(false);  // sync React state with reality
      setMsg("Login required: open /killswitch and unlock an admin session first.");
      return;
    }
    setSaving(true);
    setMsg(null);
    const { configured: _c, chain_id: _cid, updated_at: _ua, updated_by: _ub, ...rest } = config;
    void _c; void _cid; void _ua; void _ub;
    const body = {
      ...rest,
      enabled_strategies: enabled,
      strategy_configs: strategyConfigs,
    };
    const res = await putTradingConfig(config.chain_id, body, adminToken, actor);
    setSaving(false);
    if (res.ok) {
      onSaved({
        ...config,
        enabled_strategies: enabled,
        strategy_configs: strategyConfigs,
        updated_at: res.data.updated_at,
        updated_by: res.data.updated_by,
      });
      setMsg("Saved · searcher reloads in ≤1s");
    } else {
      setMsg(res.error);
    }
  };

  return (
    <div className="grid gap-4">
      <div className="flex items-center justify-between">
        <p className="text-sm text-muted-foreground">
          {enabled.length} active · {catalog.length} total in catalog
        </p>
        <div className="flex items-center gap-3">
          {msg && <span className="text-xs text-muted-foreground font-mono">{msg}</span>}
          {!hasSession && !msg && (
            <span className="text-xs text-warning font-mono">No admin session — <a href="/killswitch" className="underline">unlock at /killswitch</a></span>
          )}
          <Button onClick={onSave} disabled={!dirty || saving || !hasSession} title={!hasSession ? "Admin session required — open /killswitch first" : undefined}>
            {saving ? "Saving…" : "Save changes"}
          </Button>
        </div>
      </div>
      <div className="grid gap-3 md:grid-cols-2 lg:grid-cols-3">
        {catalog.map((s) => {
          const isOn = enabled.includes(s.kind);
          const isDefensive = s.ethical_constraint === "defensive_only";
          const status: LifecycleStatus | null = s.lifecycle_status ?? null;
          // Toggle is operative ONLY for live strategies. Defensive is forced-on
          // by policy. Everything else (designed/scaffold/not_started/null) is
          // informational — the toggle is rendered disabled with a tooltip so
          // the operator never thinks "I marked it, it must be running".
          const toggleEnabled = status === "live" && !isDefensive;
          const switchChecked = isDefensive ? true : status === "live" ? isOn : false;
          // Migration 056 — per-strategy config edit panel only for LIVE strategies
          // (where toggling actually affects detection). Defensive / informational
          // cards keep the old read-only layout to avoid implying control.
          const stratCfg = strategyConfigs[s.kind];
          const showThresholds = toggleEnabled;
          return (
            <Card key={s.kind} className={isDefensive ? "border-destructive/40" : undefined}>
              <CardContent className="space-y-2 py-4">
                <div className="flex items-start justify-between gap-2">
                  <div>
                    <div className="font-medium">{s.display_name}</div>
                    <div className="text-xs text-muted-foreground font-mono">{s.kind}</div>
                  </div>
                  <Switch
                    checked={switchChecked}
                    disabled={!toggleEnabled}
                    onCheckedChange={(c) => toggle(s.kind, c)}
                    title={status ? LIFECYCLE_TOOLTIP[status] : "Lifecycle status unknown"}
                  />
                </div>
                <p className="text-xs text-muted-foreground">{s.description}</p>
                <div className="flex flex-wrap gap-1 pt-1">
                  {status && (
                    <Badge
                      variant="outline"
                      className={`text-[10px] font-bold ${LIFECYCLE_CLASSES[status]}`}
                      title={LIFECYCLE_TOOLTIP[status]}
                    >
                      {LIFECYCLE_LABEL[status]}
                    </Badge>
                  )}
                  <Badge variant="outline" className="text-[10px]">{s.category}</Badge>
                  <Badge variant="outline" className="text-[10px]">risk: {s.risk_level}</Badge>
                  {s.competitive_advantage && (
                    <Badge variant="outline" className="text-[10px]">edge: {s.competitive_advantage}</Badge>
                  )}
                  {s.requires_flashloan && (
                    <Badge variant="outline" className="text-[10px]">flashloan</Badge>
                  )}
                  {stratCfg && (
                    <Badge variant="outline" className="text-[10px] bg-info/10 text-info">
                      override
                    </Badge>
                  )}
                </div>
                {showThresholds && (
                  <div className="grid grid-cols-2 gap-2 border-t border-border pt-3 mt-2">
                    <div className="space-y-1">
                      <Label htmlFor={`${s.kind}-min-profit`} className="text-[10px] uppercase tracking-wide text-muted-foreground">
                        min profit USD
                      </Label>
                      <Input
                        id={`${s.kind}-min-profit`}
                        type="number"
                        inputMode="decimal"
                        step="0.01"
                        min="0"
                        placeholder="(chain default)"
                        className="h-8 text-xs font-mono"
                        value={stratCfg?.min_profit_usd ?? ""}
                        onChange={(e) => {
                          const v = e.target.value.trim();
                          updateStrategy(s.kind, {
                            min_profit_usd: v === "" ? null : Number(v),
                          });
                        }}
                        title="Per-strategy minimum net profit in USD. Empty inherits the chain-level threshold."
                      />
                    </div>
                    <div className="space-y-1">
                      <Label htmlFor={`${s.kind}-min-roi`} className="text-[10px] uppercase tracking-wide text-muted-foreground">
                        min ROI %
                      </Label>
                      <Input
                        id={`${s.kind}-min-roi`}
                        type="number"
                        inputMode="decimal"
                        step="0.01"
                        min="0"
                        placeholder="(chain default)"
                        className="h-8 text-xs font-mono"
                        value={stratCfg?.min_roi_pct ?? ""}
                        onChange={(e) => {
                          const v = e.target.value.trim();
                          updateStrategy(s.kind, {
                            min_roi_pct: v === "" ? null : Number(v),
                          });
                        }}
                        title="Per-strategy minimum net ROI percent. Empty inherits the chain-level threshold."
                      />
                    </div>
                    <div className="space-y-1 col-span-2">
                      <Label className="text-[10px] uppercase tracking-wide text-muted-foreground">
                        route legs (min / max)
                      </Label>
                      <div className="flex gap-2">
                        <Input
                          type="number"
                          inputMode="numeric"
                          step="1"
                          min="1"
                          max="8"
                          className="h-8 text-xs font-mono"
                          placeholder="min"
                          value={stratCfg?.route_constraints.min_legs ?? 1}
                          onChange={(e) => {
                            const n = Math.max(1, Math.min(8, Number(e.target.value) || 1));
                            updateStrategy(s.kind, {
                              route_constraints: {
                                ...(stratCfg?.route_constraints ??
                                  DEFAULT_STRATEGY_RUNTIME_CONFIG.route_constraints),
                                min_legs: n,
                              },
                            });
                          }}
                        />
                        <Input
                          type="number"
                          inputMode="numeric"
                          step="1"
                          min="1"
                          max="8"
                          className="h-8 text-xs font-mono"
                          placeholder="max"
                          value={stratCfg?.route_constraints.max_legs ?? 8}
                          onChange={(e) => {
                            const n = Math.max(1, Math.min(8, Number(e.target.value) || 8));
                            updateStrategy(s.kind, {
                              route_constraints: {
                                ...(stratCfg?.route_constraints ??
                                  DEFAULT_STRATEGY_RUNTIME_CONFIG.route_constraints),
                                max_legs: n,
                              },
                            });
                          }}
                        />
                      </div>
                    </div>
                  </div>
                )}
              </CardContent>
            </Card>
          );
        })}
      </div>
    </div>
  );
}
