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
import Link from "next/link";

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
  | "max_slippage_pct"
  // Net Yield Gate — Sprint A+B+C (migrations 047+048)
  | "capital_cost_rate_annual_pct"
  | "ops_overhead_usd_per_attempt"
  | "spread_sanity_mult"
  | "p_copied_volume_threshold_usd"
  | "p_copied_max";

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
    // Empty string → clear optional field (undefined); never coerce to 0 (R8 fail-honest).
    if (v === "" || v === null) {
      setDraft({ ...draft, [k]: undefined });
      return;
    }
    const n = Number(v);
    if (!Number.isFinite(n)) return;
    setDraft({ ...draft, [k]: n });
  };

  const onSave = async () => {
    if (!hasAdminSession()) {
      setHasSession(false);  // sync React state with reality
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
        <Field label="Min yield (USD)" value={draft.min_profit_usd} step="0.01" onChange={(v) => setField("min_profit_usd", v)} />
        <Field label="Min convergence ratio (%)" value={draft.min_roi_pct} step="0.01" onChange={(v) => setField("min_roi_pct", v)} />
        <Field label="Max slippage (%)" value={draft.max_slippage_pct} step="0.01" onChange={(v) => setField("max_slippage_pct", v)} />
        <Field label="Min landing probability" value={draft.min_landing_probability} step="0.01" min={0} max={1} onChange={(v) => setField("min_landing_probability", v)} />
        <Field label="Min liquidity confidence" value={draft.min_liquidity_confidence} step="0.01" min={0} max={1} onChange={(v) => setField("min_liquidity_confidence", v)} />
        <Field label="Max token risk score" value={draft.max_token_risk_score} step="0.01" min={0} max={1} onChange={(v) => setField("max_token_risk_score", v)} />

        {/* ── Capital costs (Sprint A) ─────────────────────────────────── */}
        <div className="col-span-2 pt-3 border-t mt-1">
          <p className="text-xs font-semibold text-muted-foreground uppercase tracking-wide mb-3">Capital costs</p>
          <div className="grid gap-4 md:grid-cols-2">
            <Field
              label="Capital Cost Rate (% APR)"
              value={draft.capital_cost_rate_annual_pct}
              step="0.1"
              min={0}
              max={50}
              tooltip="Hurdle rate for locked capital. 0 = ignore (correct for flash convergence resolution where capital is not locked). Set > 0 to require yield > capital × rate/8760 per attempt."
              onChange={(v) => setField("capital_cost_rate_annual_pct", v)}
            />
            <Field
              label="Ops Overhead ($/attempt)"
              value={draft.ops_overhead_usd_per_attempt}
              step="0.001"
              min={0}
              max={100}
              tooltip="Amortised infrastructure + RPC cost subtracted from gross yield before the net-yield gate. Example: 0.01 = 1 cent per scored opportunity."
              onChange={(v) => setField("ops_overhead_usd_per_attempt", v)}
            />
          </div>
        </div>

        {/* ── Risk modelling (Sprint B + C) ────────────────────────────── */}
        <div className="col-span-2 pt-3 border-t mt-1">
          <p className="text-xs font-semibold text-muted-foreground uppercase tracking-wide mb-3">Risk modelling</p>
          <div className="grid gap-4 md:grid-cols-3">
            <Field
              label="Spread Sanity Multiplier"
              value={draft.spread_sanity_mult}
              step="0.1"
              min={1}
              max={100}
              tooltip="Gate multiplier for AMM-vs-oracle deviation. Default 3 = reject if the observed spread is more than 3× what the oracle reference predicts. Catches stale-pool or data-feed glitches."
              onChange={(v) => setField("spread_sanity_mult", v)}
            />
            <Field
              label="p_copied Volume Threshold (USD)"
              value={draft.p_copied_volume_threshold_usd}
              step="1000"
              min={0}
              tooltip="24 h pool volume (USD) at which the copy-trade probability model assigns P = 0.5. Lower value = model becomes more suspicious at low-volume pools."
              onChange={(v) => setField("p_copied_volume_threshold_usd", v)}
            />
            <Field
              label="p_copied Max"
              value={draft.p_copied_max}
              step="0.01"
              min={0}
              max={1}
              tooltip="Hard ceiling on P(opportunity is a copy-trade front-run). 0.5 = reject any candidate where the model assigns ≥ 50% probability it is a copied/front-run trade."
              onChange={(v) => setField("p_copied_max", v)}
            />
          </div>
        </div>

        <div className="col-span-2 flex items-center gap-3 pt-2 border-t mt-2">
          <Button onClick={onSave} disabled={saving || !hasSession} title={!hasSession ? "Admin session required — open /killswitch first" : undefined}>
            {saving ? "Saving…" : "Save changes"}
          </Button>
          {msg && <span className="text-xs text-muted-foreground font-mono">{msg}</span>}
          {!hasSession && !msg && (
            <span className="text-xs text-warning font-mono">No admin session — <Link href="/killswitch" className="underline">unlock at /killswitch</Link></span>
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
  tooltip,
  onChange,
}: {
  label: string;
  // R8 fail-honest: undefined/null from API renders as empty input, never as 0.
  value: number | undefined | null;
  step?: string;
  min?: number;
  max?: number;
  tooltip?: string;
  onChange: (v: string) => void;
}) {
  // Controlled value: use empty string when no data so the browser renders
  // the placeholder instead of a synthetic zero (R8).
  const controlled = value == null ? "" : value;
  return (
    <div className="grid gap-1.5">
      <Label className="text-xs text-muted-foreground" title={tooltip}>
        {label}
        {tooltip && (
          <span className="ml-1 text-muted-foreground cursor-help" title={tooltip}>
            &#9432;
          </span>
        )}
      </Label>
      <Input
        type="number"
        value={controlled}
        step={step}
        min={min}
        max={max}
        placeholder={value == null ? "—" : undefined}
        onChange={(e) => onChange(e.target.value)}
      />
    </div>
  );
}
