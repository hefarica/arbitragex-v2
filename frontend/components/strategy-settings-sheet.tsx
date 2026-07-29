/**
 * Strategy Settings Sheet — per-cartridge configuration panel (Excel spec).
 *
 * Opens on click on a cartridge. Edits the strategy's entry in
 * `trading_config.strategy_configs[kind]` with the operator's parameters:
 * yield gates (min profit USD + min convergence ratio %), route legs model
 * (min/max), slippage, price impact, gas ceiling, and atomicity/cross-chain
 * constraints. Saves via the existing putTradingConfig round-trip (the same
 * write path StrategyCatalogTab uses), which hot-reloads to the searcher ≤1s.
 *
 * Fields left empty inherit the chain-level defaults (R8 fail-honest — a null
 * override reads as "no per-strategy override", never a fabricated 0).
 */
"use client";

import { useEffect, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
import { Switch } from "@/components/ui/switch";
import { putTradingConfig } from "@/lib/api-client";
import {
  DEFAULT_STRATEGY_RUNTIME_CONFIG,
  type StrategyRuntimeConfig,
  type TradingConfigConfigured,
} from "@/lib/schemas";

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  strategyKind: string;
  displayName: string;
  config: TradingConfigConfigured;
  onSaved: (next: TradingConfigConfigured) => void;
  adminToken: string;
  actor: string;
}

export function StrategySettingsSheet({
  open,
  onOpenChange,
  strategyKind,
  displayName,
  config,
  onSaved,
  adminToken,
  actor,
}: Props) {
  const [cfg, setCfg] = useState<StrategyRuntimeConfig>({ ...DEFAULT_STRATEGY_RUNTIME_CONFIG });
  const [saving, setSaving] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);

  // Load the existing per-strategy override (or canonical default) when the
  // sheet opens for a different strategy.
  useEffect(() => {
    const existing = config.strategy_configs?.[strategyKind];
    setCfg(existing ? { ...existing } : { ...DEFAULT_STRATEGY_RUNTIME_CONFIG });
    setMsg(null);
  }, [strategyKind, open, config.strategy_configs]);

  const update = (patch: Partial<StrategyRuntimeConfig>) =>
    setCfg((prev) => ({ ...prev, ...patch }));

  const updateRoute = (patch: Partial<StrategyRuntimeConfig["route_constraints"]>) =>
    setCfg((prev) => ({
      ...prev,
      route_constraints: { ...prev.route_constraints, ...patch },
    }));

  const numOrNull = (v: string): number | null => {
    const t = v.trim();
    if (t === "") return null;
    const n = Number(t);
    return Number.isFinite(n) ? n : null;
  };

  const onSave = async () => {
    setSaving(true);
    setMsg(null);
    const nextConfigs = {
      ...(config.strategy_configs ?? {}),
      [strategyKind]: cfg,
    };
    const { configured: _c, chain_id: _cid, updated_at: _ua, updated_by: _ub, ...rest } = config;
    void _c; void _cid; void _ua; void _ub;
    const body = { ...rest, strategy_configs: nextConfigs };
    const res = await putTradingConfig(config.chain_id, body, adminToken, actor);
    setSaving(false);
    if (res.ok) {
      onSaved({
        ...config,
        strategy_configs: nextConfigs,
        updated_at: res.data.updated_at,
        updated_by: res.data.updated_by,
      });
      setMsg("Saved · searcher reloads in ≤1s");
      onOpenChange(false);
    } else {
      setMsg(res.error);
    }
  };

  const onClear = async () => {
    // Remove the per-strategy override entirely (inherit chain defaults).
    setSaving(true);
    setMsg(null);
    const nextConfigs = { ...(config.strategy_configs ?? {}) };
    delete nextConfigs[strategyKind];
    const { configured: _c, chain_id: _cid, updated_at: _ua, updated_by: _ub, ...rest } = config;
    void _c; void _cid; void _ua; void _ub;
    const body = { ...rest, strategy_configs: nextConfigs };
    const res = await putTradingConfig(config.chain_id, body, adminToken, actor);
    setSaving(false);
    if (res.ok) {
      onSaved({
        ...config,
        strategy_configs: nextConfigs,
        updated_at: res.data.updated_at,
        updated_by: res.data.updated_by,
      });
      setMsg("Override cleared — inherits chain defaults");
      onOpenChange(false);
    } else {
      setMsg(res.error);
    }
  };

  const hasOverride = config.strategy_configs?.[strategyKind] != null;

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent side="right" className="w-full sm:max-w-lg overflow-y-auto">
        <SheetHeader>
          <SheetTitle className="flex items-center gap-2">
            {displayName}
            {hasOverride && (
              <Badge variant="outline" className="bg-info/10 text-info border-info/30 text-[10px]">
                override
              </Badge>
            )}
          </SheetTitle>
          <SheetDescription className="font-mono text-xs">{strategyKind}</SheetDescription>
        </SheetHeader>

        <div className="mt-6 space-y-6 px-1">
          {/* Yield gates */}
          <section className="space-y-3">
            <h3 className="text-sm font-semibold uppercase tracking-wide text-muted-foreground">
              Yield gates
            </h3>
            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-1">
                <Label htmlFor="ss-min-profit" className="text-xs">
                  Min profit (USD)
                </Label>
                <Input
                  id="ss-min-profit"
                  type="number"
                  inputMode="decimal"
                  step="0.01"
                  min="0"
                  placeholder="(chain default)"
                  value={cfg.min_profit_usd ?? ""}
                  onChange={(e) => update({ min_profit_usd: numOrNull(e.target.value) })}
                />
              </div>
              <div className="space-y-1">
                <Label htmlFor="ss-min-roi" className="text-xs">
                  Min convergence ratio %
                </Label>
                <Input
                  id="ss-min-roi"
                  type="number"
                  inputMode="decimal"
                  step="0.01"
                  min="0"
                  placeholder="(chain default)"
                  value={cfg.min_roi_pct ?? ""}
                  onChange={(e) => update({ min_roi_pct: numOrNull(e.target.value) })}
                />
              </div>
            </div>
            <div className="flex items-center justify-between rounded-md border border-border px-3 py-2">
              <Label htmlFor="ss-enabled" className="text-xs">
                Strategy enabled
              </Label>
              <Switch
                id="ss-enabled"
                checked={cfg.enabled}
                onCheckedChange={(v) => update({ enabled: v })}
              />
            </div>
          </section>

          {/* Route legs model */}
          <section className="space-y-3">
            <h3 className="text-sm font-semibold uppercase tracking-wide text-muted-foreground">
              Route legs model
            </h3>
            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-1">
                <Label htmlFor="ss-min-legs" className="text-xs">
                  Min legs
                </Label>
                <Input
                  id="ss-min-legs"
                  type="number"
                  inputMode="numeric"
                  step="1"
                  min="1"
                  max="8"
                  value={cfg.route_constraints.min_legs}
                  onChange={(e) =>
                    updateRoute({ min_legs: Math.max(1, Math.min(8, Number(e.target.value) || 1)) })
                  }
                />
              </div>
              <div className="space-y-1">
                <Label htmlFor="ss-max-legs" className="text-xs">
                  Max legs
                </Label>
                <Input
                  id="ss-max-legs"
                  type="number"
                  inputMode="numeric"
                  step="1"
                  min="1"
                  max="8"
                  value={cfg.route_constraints.max_legs}
                  onChange={(e) =>
                    updateRoute({ max_legs: Math.max(1, Math.min(8, Number(e.target.value) || 8)) })
                  }
                />
              </div>
            </div>
            <div className="flex items-center justify-between rounded-md border border-border px-3 py-2">
              <Label htmlFor="ss-atomic" className="text-xs">
                Require atomic (single-tx / flash convergence)
              </Label>
              <Switch
                id="ss-atomic"
                checked={cfg.route_constraints.require_atomic}
                onCheckedChange={(v) => updateRoute({ require_atomic: v })}
              />
            </div>
            <div className="flex items-center justify-between rounded-md border border-border px-3 py-2">
              <Label htmlFor="ss-crosschain" className="text-xs">
                Allow cross-chain routes
              </Label>
              <Switch
                id="ss-crosschain"
                checked={cfg.route_constraints.allow_cross_chain}
                onCheckedChange={(v) => updateRoute({ allow_cross_chain: v })}
              />
            </div>
          </section>

          {/* Cost ceilings */}
          <section className="space-y-3">
            <h3 className="text-sm font-semibold uppercase tracking-wide text-muted-foreground">
              Cost ceilings
            </h3>
            <div className="grid grid-cols-3 gap-3">
              <div className="space-y-1">
                <Label htmlFor="ss-slippage" className="text-xs">
                  Max slippage %
                </Label>
                <Input
                  id="ss-slippage"
                  type="number"
                  inputMode="decimal"
                  step="0.01"
                  min="0"
                  max="50"
                  placeholder="(default)"
                  value={cfg.max_slippage_pct ?? ""}
                  onChange={(e) => update({ max_slippage_pct: numOrNull(e.target.value) })}
                />
              </div>
              <div className="space-y-1">
                <Label htmlFor="ss-impact" className="text-xs">
                  Max price impact %
                </Label>
                <Input
                  id="ss-impact"
                  type="number"
                  inputMode="decimal"
                  step="0.01"
                  min="0"
                  max="50"
                  placeholder="(default)"
                  value={cfg.max_price_impact_pct ?? ""}
                  onChange={(e) => update({ max_price_impact_pct: numOrNull(e.target.value) })}
                />
              </div>
              <div className="space-y-1">
                <Label htmlFor="ss-gas" className="text-xs">
                  Max gas (USD)
                </Label>
                <Input
                  id="ss-gas"
                  type="number"
                  inputMode="decimal"
                  step="0.01"
                  min="0"
                  placeholder="(default)"
                  value={cfg.max_gas_usd ?? ""}
                  onChange={(e) => update({ max_gas_usd: numOrNull(e.target.value) })}
                />
              </div>
            </div>
          </section>

          {/* Pool constraints */}
          <section className="space-y-3">
            <h3 className="text-sm font-semibold uppercase tracking-wide text-muted-foreground">
              Pool constraints
            </h3>
            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-1">
                <Label htmlFor="ss-tvl" className="text-xs">
                  Min pool TVL (USD)
                </Label>
                <Input
                  id="ss-tvl"
                  type="number"
                  inputMode="decimal"
                  step="1"
                  min="0"
                  placeholder="(default)"
                  value={cfg.route_constraints.min_pool_tvl_usd ?? ""}
                  onChange={(e) => updateRoute({ min_pool_tvl_usd: numOrNull(e.target.value) })}
                />
              </div>
              <div className="space-y-1">
                <Label htmlFor="ss-vol" className="text-xs">
                  Min pool 24h volume (USD)
                </Label>
                <Input
                  id="ss-vol"
                  type="number"
                  inputMode="decimal"
                  step="1"
                  min="0"
                  placeholder="(default)"
                  value={cfg.route_constraints.min_pool_volume_24h_usd ?? ""}
                  onChange={(e) =>
                    updateRoute({ min_pool_volume_24h_usd: numOrNull(e.target.value) })
                  }
                />
              </div>
            </div>
          </section>

          {msg && <p className="text-xs font-mono text-muted-foreground">{msg}</p>}

          <div className="flex items-center gap-3 pb-6">
            <Button onClick={onSave} disabled={saving}>
              {saving ? "Saving…" : "Save settings"}
            </Button>
            {hasOverride && (
              <Button variant="outline" onClick={onClear} disabled={saving}>
                Clear override
              </Button>
            )}
          </div>
        </div>
      </SheetContent>
    </Sheet>
  );
}
