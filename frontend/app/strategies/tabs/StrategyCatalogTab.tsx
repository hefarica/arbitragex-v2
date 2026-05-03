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

import { useMemo, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Switch } from "@/components/ui/switch";
import { putTradingConfig } from "@/lib/api-client";
import type { StrategyCatalogEntry, TradingConfigConfigured } from "@/lib/schemas";

interface Props {
  config: TradingConfigConfigured;
  catalog: StrategyCatalogEntry[];
  onSaved: (next: TradingConfigConfigured) => void;
  adminToken: string;
  actor: string;
}

export function StrategyCatalogTab({ config, catalog, onSaved, adminToken, actor }: Props) {
  const [enabled, setEnabled] = useState<string[]>(config.enabled_strategies);
  const [saving, setSaving] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);

  const dirty = useMemo(() => {
    const a = [...enabled].sort().join(",");
    const b = [...config.enabled_strategies].sort().join(",");
    return a !== b;
  }, [enabled, config.enabled_strategies]);

  const toggle = (kind: string, checked: boolean) => {
    setEnabled((prev) => (checked ? Array.from(new Set([...prev, kind])) : prev.filter((k) => k !== kind)));
  };

  const onSave = async () => {
    setSaving(true);
    setMsg(null);
    const { configured: _c, chain_id: _cid, updated_at: _ua, updated_by: _ub, ...rest } = config;
    void _c; void _cid; void _ua; void _ub;
    const body = { ...rest, enabled_strategies: enabled };
    const res = await putTradingConfig(config.chain_id, body, adminToken, actor);
    setSaving(false);
    if (res.ok) {
      onSaved({ ...config, enabled_strategies: enabled, updated_at: res.data.updated_at, updated_by: res.data.updated_by });
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
          <Button onClick={onSave} disabled={!dirty || saving}>
            {saving ? "Saving…" : "Save changes"}
          </Button>
        </div>
      </div>
      <div className="grid gap-3 md:grid-cols-2 lg:grid-cols-3">
        {catalog.map((s) => {
          const isOn = enabled.includes(s.kind);
          const isDefensive = s.ethical_constraint === "defensive_only";
          return (
            <Card key={s.kind} className={isDefensive ? "border-red-500/40" : undefined}>
              <CardContent className="space-y-2 py-4">
                <div className="flex items-start justify-between gap-2">
                  <div>
                    <div className="font-medium">{s.display_name}</div>
                    <div className="text-xs text-muted-foreground font-mono">{s.kind}</div>
                  </div>
                  <Switch
                    checked={isDefensive ? true : isOn}
                    disabled={isDefensive}
                    onCheckedChange={(c) => toggle(s.kind, c)}
                  />
                </div>
                <p className="text-xs text-muted-foreground">{s.description}</p>
                <div className="flex flex-wrap gap-1 pt-1">
                  <Badge variant={s.is_implemented ? "success" : "outline"} className="text-[10px]">
                    {s.is_implemented ? "Implemented" : "Schema-only"}
                  </Badge>
                  <Badge variant="outline" className="text-[10px]">{s.category}</Badge>
                  <Badge variant="outline" className="text-[10px]">risk: {s.risk_level}</Badge>
                  {s.competitive_advantage && (
                    <Badge variant="outline" className="text-[10px]">edge: {s.competitive_advantage}</Badge>
                  )}
                  {s.requires_flashloan && (
                    <Badge variant="outline" className="text-[10px]">flashloan</Badge>
                  )}
                  {isDefensive && (
                    <Badge variant="destructive" className="text-[10px]">DEFENSIVE ONLY</Badge>
                  )}
                </div>
              </CardContent>
            </Card>
          );
        })}
      </div>
    </div>
  );
}
