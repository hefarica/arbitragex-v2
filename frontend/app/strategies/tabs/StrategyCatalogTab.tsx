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
import { Switch } from "@/components/ui/switch";
import { hasAdminSession } from "@/lib/admin-token";
import { putTradingConfig } from "@/lib/api-client";
import type {
  LifecycleStatus,
  StrategyCatalogEntry,
  TradingConfigConfigured,
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

// Tailwind classes per status — tuned so LIVE pops, others read as muted.
const LIFECYCLE_CLASSES: Record<LifecycleStatus, string> = {
  live: "bg-emerald-500/15 text-emerald-300 border-emerald-500/40",
  designed: "bg-sky-500/15 text-sky-300 border-sky-500/40",
  scaffold: "bg-amber-500/15 text-amber-300 border-amber-500/40",
  not_started: "bg-slate-500/15 text-slate-300 border-slate-500/40",
  defensive_only: "bg-red-500/15 text-red-300 border-red-500/40",
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
    const a = [...enabled].sort().join(",");
    const b = [...config.enabled_strategies].sort().join(",");
    return a !== b;
  }, [enabled, config.enabled_strategies]);

  const toggle = (kind: string, checked: boolean) => {
    setEnabled((prev) => (checked ? Array.from(new Set([...prev, kind])) : prev.filter((k) => k !== kind)));
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
          {!hasSession && !msg && (
            <span className="text-xs text-amber-400 font-mono">No admin session — <a href="/killswitch" className="underline">unlock at /killswitch</a></span>
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
          return (
            <Card key={s.kind} className={isDefensive ? "border-red-500/40" : undefined}>
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
                </div>
              </CardContent>
            </Card>
          );
        })}
      </div>
    </div>
  );
}
