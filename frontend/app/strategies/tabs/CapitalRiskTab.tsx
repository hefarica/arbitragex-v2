/**
 * Sprint 2 Task 2.3 — Capital & Risk tab.
 *
 * Edits the operator-tunable trading_config row for chain_id=1. Server is
 * source of truth (PUT /admin/trading-config/:chain_id); UI mirrors the
 * persisted state. Mounted Snapshot pattern — initialConfig comes from
 * the parent client component which got it from the Server Component.
 */
"use client";

import { useEffect, useState } from "react";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { hasAdminSession } from "@/lib/admin-token";
import { putTradingConfig } from "@/lib/api-client";
import type { TradingConfigConfigured } from "@/lib/schemas";

interface Props {
  config: TradingConfigConfigured;
  onSaved: (next: TradingConfigConfigured) => void;
  adminToken: string;
  actor: string;
}

type DraftField =
  | "capital_usd"
  | "base_token_price_usd"
  | "min_profit_usd"
  | "min_roi_pct"
  | "min_landing_probability"
  | "min_liquidity_confidence"
  | "max_token_risk_score"
  | "max_slippage_pct";

export function CapitalRiskTab({ config, onSaved, adminToken, actor }: Props) {
  const [draft, setDraft] = useState<TradingConfigConfigured>(config);
  const [saving, setSaving] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  // R1: session check deferred to useEffect — never read document.cookie during SSR.
  const [hasSession, setHasSession] = useState(false);
  useEffect(() => {
    setHasSession(hasAdminSession());
    const id = setInterval(() => setHasSession(hasAdminSession()), 30_000);
    return () => clearInterval(id);
  }, []);

  const setField = (k: DraftField, v: string) => {
    const n = Number(v);
    if (!Number.isFinite(n)) return;
    setDraft({ ...draft, [k]: n });
  };

  const onSave = async () => {
    if (!hasSession) {
      setMsg("Login required: open /killswitch and unlock an admin session first.");
      return;
    }
    setSaving(true);
    setMsg(null);
    const { configured: _c, chain_id: _cid, updated_at: _ua, updated_by: _ub, ...body } = draft;
    void _c; void _cid; void _ua; void _ub;
    const res = await putTradingConfig(draft.chain_id, body, adminToken, actor);
    setSaving(false);
    if (res.ok) {
      onSaved({ ...draft, updated_at: res.data.updated_at, updated_by: res.data.updated_by });
      setMsg("Saved · hot-reload to searcher in ≤1s");
    } else {
      setMsg(res.error);
    }
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle>Capital &amp; Risk gates · chain {draft.chain_id}</CardTitle>
      </CardHeader>
      <CardContent className="grid gap-4 md:grid-cols-2">
        <Field label="Capital (USD)" value={draft.capital_usd} onChange={(v) => setField("capital_usd", v)} />
        <Field label="Base token price (USD)" value={draft.base_token_price_usd} step="0.01" onChange={(v) => setField("base_token_price_usd", v)} />
        <Field label="Min profit (USD)" value={draft.min_profit_usd} step="0.01" onChange={(v) => setField("min_profit_usd", v)} />
        <Field label="Min ROI (%)" value={draft.min_roi_pct} step="0.01" onChange={(v) => setField("min_roi_pct", v)} />
        <Field label="Max slippage (%)" value={draft.max_slippage_pct} step="0.01" onChange={(v) => setField("max_slippage_pct", v)} />
        <Field label="Min landing probability" value={draft.min_landing_probability} step="0.01" min={0} max={1} onChange={(v) => setField("min_landing_probability", v)} />
        <Field label="Min liquidity confidence" value={draft.min_liquidity_confidence} step="0.01" min={0} max={1} onChange={(v) => setField("min_liquidity_confidence", v)} />
        <Field label="Max token risk score" value={draft.max_token_risk_score} step="0.01" min={0} max={1} onChange={(v) => setField("max_token_risk_score", v)} />

        <div className="col-span-2 flex items-center gap-3 pt-2 border-t mt-2">
          <Button onClick={onSave} disabled={saving || !hasSession} title={!hasSession ? "Admin session required — open /killswitch first" : undefined}>
            {saving ? "Saving…" : "Save changes"}
          </Button>
          {msg && <span className="text-xs text-muted-foreground font-mono">{msg}</span>}
          {!hasSession && !msg && (
            <span className="text-xs text-amber-400 font-mono">No admin session — <a href="/killswitch" className="underline">unlock at /killswitch</a></span>
          )}
          <span className="ml-auto text-xs text-muted-foreground">
            updated_by: {draft.updated_by ?? "—"} · {draft.updated_at}
          </span>
        </div>
      </CardContent>
    </Card>
  );
}

function Field({
  label,
  value,
  step,
  min,
  max,
  onChange,
}: {
  label: string;
  value: number;
  step?: string;
  min?: number;
  max?: number;
  onChange: (v: string) => void;
}) {
  return (
    <div className="grid gap-1.5">
      <Label className="text-xs text-muted-foreground">{label}</Label>
      <Input
        type="number"
        value={value}
        step={step}
        min={min}
        max={max}
        onChange={(e) => onChange(e.target.value)}
      />
    </div>
  );
}
