"use client";

import { useEffect, useMemo, useState } from "react";
import { useRouter } from "next/navigation";
import { CheckCircle2Icon, AlertCircleIcon, SaveIcon, ShieldAlertIcon } from "lucide-react";
import { toast } from "sonner";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { adminTokenExpiresInMs, fmtRemaining, getAdminToken, setAdminToken } from "@/lib/admin-token";
import { getStrategyCatalog, putTradingConfig } from "@/lib/api-client";
import type {
  GasPriceStrategy,
  StrategyCatalogEntry,
  TradingConfigConfigured,
  TradingConfigResponse,
} from "@/lib/schemas";

const GAS_STRATEGIES: { value: GasPriceStrategy; label: string; help: string }[] = [
  { value: "dynamic_basefee_plus_tip", label: "Dynamic (basefee + p75 tip)", help: "Reacts to live gas; preferred when block utilisation is volatile." },
  { value: "percentile_75", label: "P75 of recent blocks", help: "Conservative: targets the 75th percentile of priority fees observed in the last 5 blocks." },
  { value: "fixed", label: "Fixed gwei", help: "Operator pins a hard ceiling. Use when arbitrage edge depends on predictable bidding." },
];

// Audit 2026-05-10 fix: replaced hardcoded STRATEGY_OPTIONS with a dynamic
// fetch from /api/strategy-catalog (the canonical source seeded by migration
// 035+036+039+040). Drift between the form, the Rust StrategyKind enum, and
// the catalog table caused operator confusion (toggling a strategy that did
// not exist downstream). Now both the catalog tab and this form share a
// single dynamic source. R8 fail-honest: catalog fetch error → empty array
// (no fabricated entries) and the form shows an inline warning.

const DEFAULT_TOKENS = ["WETH", "USDC", "USDT", "DAI", "WBTC"];

interface FormState {
  capital_usd: number;
  base_token_symbol: string;
  base_token_price_usd: number;
  allowed_tokens: string;
  min_profit_usd: number;
  min_roi_pct: number;
  min_landing_probability: number;
  min_liquidity_confidence: number;
  max_token_risk_score: number;
  gas_price_strategy: GasPriceStrategy;
  fixed_gas_price_gwei: number;
  gas_estimate_units: number;
  max_slippage_pct: number;
  failure_risk_buffer_pct: number;
  flashloan_fee_pct: number;
  enabled_strategies: Set<string>;
  enabled: boolean;
}

function emptyState(): FormState {
  // No invented productive defaults — operator must seed values explicitly.
  // Numbers default to 0 / empty so the form forces conscious entry.
  return {
    capital_usd: 0,
    base_token_symbol: "WETH",
    base_token_price_usd: 0,
    allowed_tokens: DEFAULT_TOKENS.join(", "),
    min_profit_usd: 0,
    min_roi_pct: 0,
    min_landing_probability: 0.5,
    min_liquidity_confidence: 0.7,
    max_token_risk_score: 1.0,
    gas_price_strategy: "dynamic_basefee_plus_tip",
    fixed_gas_price_gwei: 0,
    gas_estimate_units: 250000,
    max_slippage_pct: 0.5,
    failure_risk_buffer_pct: 0.001,
    flashloan_fee_pct: 0.0009,
    enabled_strategies: new Set(["dex_arb_v2v2"]),
    enabled: true,
  };
}

function fromInitial(initial: TradingConfigResponse): FormState {
  if (!initial.configured) return emptyState();
  return {
    capital_usd: initial.capital_usd,
    base_token_symbol: initial.base_token_symbol,
    base_token_price_usd: initial.base_token_price_usd,
    allowed_tokens: initial.allowed_token_symbols.join(", "),
    min_profit_usd: initial.min_profit_usd,
    min_roi_pct: initial.min_roi_pct,
    min_landing_probability: initial.min_landing_probability,
    min_liquidity_confidence: initial.min_liquidity_confidence,
    max_token_risk_score: initial.max_token_risk_score,
    gas_price_strategy: initial.gas_price_strategy,
    fixed_gas_price_gwei: initial.fixed_gas_price_gwei ?? 0,
    gas_estimate_units: initial.gas_estimate_units,
    max_slippage_pct: initial.max_slippage_pct,
    failure_risk_buffer_pct: initial.failure_risk_buffer_pct,
    flashloan_fee_pct: initial.flashloan_fee_pct,
    enabled_strategies: new Set(initial.enabled_strategies),
    enabled: initial.enabled,
  };
}

export function TradingConfigForm({
  chainId,
  initial,
}: {
  chainId: number;
  initial: TradingConfigResponse;
}) {
  const [mounted, setMounted] = useState(false);
  const [form, setForm] = useState<FormState>(() => fromInitial(initial));
  const [busy, setBusy] = useState(false);
  const [actor, setActor] = useState("");
  const [adminTokenInput, setAdminTokenInput] = useState("");
  const [sessionTtlMs, setSessionTtlMs] = useState(0);
  const [validationErrors, setValidationErrors] = useState<string[]>([]);
  // Audit 2026-05-10 fix: dynamic strategy catalog (replaces hardcoded
  // STRATEGY_OPTIONS). Empty initial state + R8: catalog load failure
  // surfaces an inline warning rather than fabricating strategy entries.
  const [strategyCatalog, setStrategyCatalog] = useState<StrategyCatalogEntry[]>([]);
  const [catalogError, setCatalogError] = useState<string | null>(null);
  const router = useRouter();

  useEffect(() => {
    setMounted(true);
    setSessionTtlMs(adminTokenExpiresInMs());
    // Refresh TTL display every 30s so the operator sees the session winding down.
    const id = setInterval(() => setSessionTtlMs(adminTokenExpiresInMs()), 30_000);
    return () => clearInterval(id);
  }, []);

  // Audit 2026-05-10 fix: fetch strategy catalog on mount.
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      const r = await getStrategyCatalog();
      if (cancelled) return;
      if (r.ok) {
        setStrategyCatalog(r.data.entries);
        setCatalogError(null);
      } else {
        setStrategyCatalog([]);
        setCatalogError(r.error);
      }
    })();
    return () => { cancelled = true; };
  }, []);

  const validate = useMemo(() => {
    return (): string[] => {
      const errs: string[] = [];
      if (form.capital_usd <= 0) errs.push("capital_usd must be > 0 (no risk-free defaults)");
      if (form.base_token_price_usd <= 0) errs.push("base_token_price_usd must be > 0");
      if (!form.base_token_symbol.trim()) errs.push("base_token_symbol required");
      if (form.allowed_tokens.split(",").map((s) => s.trim()).filter(Boolean).length === 0)
        errs.push("at least one allowed token is required");
      if (form.min_profit_usd < 0) errs.push("min_profit_usd >= 0");
      if (form.min_roi_pct < 0) errs.push("min_roi_pct >= 0");
      if (form.gas_price_strategy === "fixed" && form.fixed_gas_price_gwei <= 0)
        errs.push("fixed gas strategy requires fixed_gas_price_gwei > 0");
      if (form.max_slippage_pct < 0 || form.max_slippage_pct > 50)
        errs.push("max_slippage_pct in [0, 50]");
      if (actor.trim().length < 1) errs.push("actor required (your initials/email for audit)");
      return errs;
    };
  }, [form, actor]);

  async function handleSubmit() {
    const errs = validate();
    setValidationErrors(errs);
    if (errs.length > 0) return;
    setBusy(true);
    try {
      // If the operator typed a token in the inline input, establish the session
      // (POST /admin/session → httpOnly cookie). Otherwise reuse the existing one.
      if (adminTokenInput.trim().length > 0) {
        const ok = await setAdminToken(adminTokenInput.trim());
        if (!ok) {
          toast.error("Admin sign-in failed — token rejected by edge");
          return;
        }
        setAdminTokenInput("");
        setSessionTtlMs(adminTokenExpiresInMs());
      }
      const token = getAdminToken();
      if (!token) {
        toast.error("Admin token missing — paste it in the field above and Save again");
        return;
      }
      const body: Omit<TradingConfigConfigured, "chain_id" | "configured" | "updated_at" | "updated_by"> = {
        capital_usd: form.capital_usd,
        base_token_symbol: form.base_token_symbol.trim(),
        base_token_price_usd: form.base_token_price_usd,
        allowed_token_symbols: form.allowed_tokens
          .split(",")
          .map((s) => s.trim().toUpperCase())
          .filter(Boolean),
        min_profit_usd: form.min_profit_usd,
        min_roi_pct: form.min_roi_pct,
        min_landing_probability: form.min_landing_probability,
        min_liquidity_confidence: form.min_liquidity_confidence,
        max_token_risk_score: form.max_token_risk_score,
        gas_price_strategy: form.gas_price_strategy,
        fixed_gas_price_gwei:
          form.gas_price_strategy === "fixed" ? form.fixed_gas_price_gwei : null,
        gas_estimate_units: form.gas_estimate_units,
        max_slippage_pct: form.max_slippage_pct,
        failure_risk_buffer_pct: form.failure_risk_buffer_pct,
        flashloan_fee_pct: form.flashloan_fee_pct,
        enabled_strategies: Array.from(form.enabled_strategies),
        // Migration 056 — legacy form does not edit per-strategy configs.
        // Empty map preserves chain-level governance; the dedicated /strategies
        // editor is the way to mutate fine-grained per-strategy fields.
        strategy_configs: {},
        enabled: form.enabled,
      };
      const r = await putTradingConfig(chainId, body, token, actor.trim());
      if (!r.ok) {
        toast.error(r.error);
        return;
      }
      toast.success(
        `Saved. ${r.data.subscribers_notified} searcher(s) notified — config takes effect within ~1s.`,
      );
      router.refresh();
    } finally {
      setBusy(false);
    }
  }

  if (!mounted) return null;

  return (
    <div className="grid gap-6">
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            Capital & token universe
            {initial.configured && initial.enabled && (
              <Badge className="bg-success/20 text-success border-success/40">live</Badge>
            )}
            {initial.configured && !initial.enabled && (
              <Badge variant="destructive">disabled</Badge>
            )}
            {!initial.configured && (
              <Badge variant="outline">not configured</Badge>
            )}
          </CardTitle>
          <CardDescription>
            Soft sizing target. Hard caps are enforced by execution_policy (migration 015) — effective amount = min(hard_cap, soft_target).
          </CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4 md:grid-cols-2">
          <div>
            <Label htmlFor="capital_usd">Deployable capital (USD)</Label>
            <Input
              id="capital_usd"
              type="number"
              step="0.01"
              min="0"
              value={form.capital_usd}
              onChange={(e) => setForm({ ...form, capital_usd: Number(e.target.value) })}
            />
            <p className="text-xs text-muted-foreground mt-1">
              Caps amount_in per opportunity. Larger candidates get sized down to this value.
            </p>
          </div>
          <div>
            <Label htmlFor="base_token_symbol">Base token symbol</Label>
            <Input
              id="base_token_symbol"
              value={form.base_token_symbol}
              onChange={(e) => setForm({ ...form, base_token_symbol: e.target.value })}
              placeholder="WETH"
            />
            <p className="text-xs text-muted-foreground mt-1">
              Used to denominate ROI and convert profit-token → USD.
            </p>
          </div>
          <div>
            <Label htmlFor="base_price">{form.base_token_symbol} price (USD)</Label>
            <Input
              id="base_price"
              type="number"
              step="0.01"
              min="0"
              value={form.base_token_price_usd}
              onChange={(e) => setForm({ ...form, base_token_price_usd: Number(e.target.value) })}
            />
            <p className="text-xs text-muted-foreground mt-1">
              Operator-supplied for now; oracle integration replaces this in the next sprint.
            </p>
          </div>
          <div>
            <Label htmlFor="allowed">Allowed token symbols (comma-separated)</Label>
            <Input
              id="allowed"
              value={form.allowed_tokens}
              onChange={(e) => setForm({ ...form, allowed_tokens: e.target.value })}
              placeholder="WETH, USDC, USDT, DAI, WBTC"
            />
            <p className="text-xs text-muted-foreground mt-1">
              Anything outside this list is silently skipped by the scanner.
            </p>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Profit gate thresholds</CardTitle>
          <CardDescription>
            Decide when a candidate is &quot;worth it&quot;. The math-engine computes net_profit and ROI deterministically; these are the floors.
          </CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4 md:grid-cols-3">
          <div>
            <Label htmlFor="min_profit">Min net profit (USD)</Label>
            <Input
              id="min_profit"
              type="number"
              step="0.01"
              min="0"
              value={form.min_profit_usd}
              onChange={(e) => setForm({ ...form, min_profit_usd: Number(e.target.value) })}
            />
          </div>
          <div>
            <Label htmlFor="min_roi">Min ROI (%)</Label>
            <Input
              id="min_roi"
              type="number"
              step="0.01"
              min="0"
              value={form.min_roi_pct}
              onChange={(e) => setForm({ ...form, min_roi_pct: Number(e.target.value) })}
            />
          </div>
          <div>
            <Label htmlFor="failure_buffer">Failure risk buffer (fraction)</Label>
            <Input
              id="failure_buffer"
              type="number"
              step="0.0001"
              min="0"
              value={form.failure_risk_buffer_pct}
              onChange={(e) => setForm({ ...form, failure_risk_buffer_pct: Number(e.target.value) })}
            />
            <p className="text-xs text-muted-foreground mt-1">
              0.001 = 0.1% reserve against revert / inclusion failure.
            </p>
          </div>
          <div>
            <Label htmlFor="land">Min landing probability (0–1)</Label>
            <Input
              id="land"
              type="number"
              step="0.01"
              min="0"
              max="1"
              value={form.min_landing_probability}
              onChange={(e) => setForm({ ...form, min_landing_probability: Number(e.target.value) })}
            />
          </div>
          <div>
            <Label htmlFor="liq">Min liquidity confidence (0–1)</Label>
            <Input
              id="liq"
              type="number"
              step="0.01"
              min="0"
              max="1"
              value={form.min_liquidity_confidence}
              onChange={(e) => setForm({ ...form, min_liquidity_confidence: Number(e.target.value) })}
            />
          </div>
          <div>
            <Label htmlFor="risk">Max token risk score (0–1)</Label>
            <Input
              id="risk"
              type="number"
              step="0.01"
              min="0"
              max="1"
              value={form.max_token_risk_score}
              onChange={(e) => setForm({ ...form, max_token_risk_score: Number(e.target.value) })}
            />
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Gas & slippage strategy</CardTitle>
          <CardDescription>How searcher-rs prices gas at scoring time.</CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4">
          <div className="grid gap-2">
            {GAS_STRATEGIES.map((s) => (
              <label key={s.value} className="flex items-start gap-3 cursor-pointer">
                <input
                  type="radio"
                  name="gas_strategy"
                  value={s.value}
                  checked={form.gas_price_strategy === s.value}
                  onChange={() => setForm({ ...form, gas_price_strategy: s.value })}
                  className="mt-1"
                />
                <div>
                  <div className="text-sm font-medium">{s.label}</div>
                  <div className="text-xs text-muted-foreground">{s.help}</div>
                </div>
              </label>
            ))}
          </div>
          {form.gas_price_strategy === "fixed" && (
            <div>
              <Label htmlFor="fixed_gas">Fixed gas price (gwei)</Label>
              <Input
                id="fixed_gas"
                type="number"
                step="0.01"
                min="0"
                value={form.fixed_gas_price_gwei}
                onChange={(e) => setForm({ ...form, fixed_gas_price_gwei: Number(e.target.value) })}
              />
            </div>
          )}
          <div className="grid gap-4 md:grid-cols-3">
            <div>
              <Label htmlFor="gas_units">Gas estimate (units)</Label>
              <Input
                id="gas_units"
                type="number"
                step="1000"
                min="21000"
                value={form.gas_estimate_units}
                onChange={(e) => setForm({ ...form, gas_estimate_units: Number(e.target.value) })}
              />
            </div>
            <div>
              <Label htmlFor="slippage">Max slippage (%)</Label>
              <Input
                id="slippage"
                type="number"
                step="0.01"
                min="0"
                max="50"
                value={form.max_slippage_pct}
                onChange={(e) => setForm({ ...form, max_slippage_pct: Number(e.target.value) })}
              />
            </div>
            <div>
              <Label htmlFor="fl_fee">Flash-loan fee (fraction)</Label>
              <Input
                id="fl_fee"
                type="number"
                step="0.0001"
                min="0"
                value={form.flashloan_fee_pct}
                onChange={(e) => setForm({ ...form, flashloan_fee_pct: Number(e.target.value) })}
              />
            </div>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Strategy mix</CardTitle>
          <CardDescription>
            Polymorphic dispatcher — searcher routes candidates to the strategy class flagged here.
            Empty = permissive (everything observed is evaluated). The list is sourced
            dynamically from <code className="font-mono text-xs">/api/strategy-catalog</code> so
            new strategies appear automatically once they ship.
          </CardDescription>
        </CardHeader>
        <CardContent>
          {catalogError && (
            <p className="text-xs font-mono text-destructive bg-destructive/10 px-3 py-2 rounded mb-3">
              Failed to load strategy catalog: {catalogError}
            </p>
          )}
          {!catalogError && strategyCatalog.length === 0 && (
            <p className="text-xs text-muted-foreground py-2">
              Loading strategy catalog…
            </p>
          )}
          <div className="grid grid-cols-2 md:grid-cols-3 gap-2">
            {strategyCatalog.map((s) => {
              const checked = form.enabled_strategies.has(s.kind);
              const isDefensive = s.ethical_constraint === "defensive_only";
              const isLive = s.lifecycle_status === "live";
              const disabled = isDefensive || !isLive;
              return (
                <label
                  key={s.kind}
                  className={`flex items-center gap-2 ${disabled ? "cursor-not-allowed opacity-60" : "cursor-pointer"}`}
                  title={
                    isDefensive
                      ? "Defensive-only — forced ON by policy"
                      : !isLive
                      ? `lifecycle=${s.lifecycle_status ?? "unknown"} — toggle is informational, no Rust emitter yet`
                      : s.description
                  }
                >
                  <input
                    type="checkbox"
                    checked={isDefensive ? true : checked}
                    disabled={disabled}
                    onChange={(e) => {
                      const next = new Set(form.enabled_strategies);
                      if (e.target.checked) next.add(s.kind);
                      else next.delete(s.kind);
                      setForm({ ...form, enabled_strategies: next });
                    }}
                  />
                  <span className="text-sm font-mono">{s.kind}</span>
                </label>
              );
            })}
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Lifecycle &amp; admin sign-in</CardTitle>
          <CardDescription>
            Saving this form requires an admin session. Paste your admin token below if no
            session is active; the edge stores it in an httpOnly cookie that lasts 8h.
          </CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4 md:grid-cols-2">
          <div className="flex items-center gap-3">
            <Switch
              id="enabled"
              checked={form.enabled}
              onCheckedChange={(v) => setForm({ ...form, enabled: v })}
            />
            <Label htmlFor="enabled" className="cursor-pointer">
              {form.enabled ? "Active — searcher uses this config" : "Disabled — searcher idles for this chain"}
            </Label>
          </div>
          <div>
            <Label htmlFor="actor">Actor (your initials/email — audit)</Label>
            <Input
              id="actor"
              value={actor}
              onChange={(e) => setActor(e.target.value)}
              placeholder="ops@example.com"
            />
          </div>
          <div className="md:col-span-2">
            <div className="flex items-center justify-between">
              <Label htmlFor="admin-token">
                Admin token{" "}
                <span className="text-muted-foreground font-normal">
                  (only required if no session is active)
                </span>
              </Label>
              <span className="text-xs font-mono text-muted-foreground">
                {sessionTtlMs > 0
                  ? `session active · auto-clears in ${fmtRemaining(sessionTtlMs)}`
                  : "no active session"}
              </span>
            </div>
            <Input
              id="admin-token"
              type="password"
              value={adminTokenInput}
              onChange={(e) => setAdminTokenInput(e.target.value)}
              placeholder={sessionTtlMs > 0 ? "(leave blank to reuse current session)" : "paste admin token to sign in"}
              autoComplete="off"
            />
          </div>
        </CardContent>
      </Card>

      {validationErrors.length > 0 && (
        <Alert variant="destructive">
          <ShieldAlertIcon />
          <AlertTitle>Cannot save</AlertTitle>
          <AlertDescription>
            <ul className="list-disc ml-5 space-y-1">
              {validationErrors.map((err, i) => (
                <li key={i} className="text-sm font-mono">{err}</li>
              ))}
            </ul>
          </AlertDescription>
        </Alert>
      )}

      {initial.configured && (
        <Alert>
          <CheckCircle2Icon />
          <AlertTitle>Last update</AlertTitle>
          <AlertDescription>
            <span className="font-mono text-xs">
              {initial.updated_at} by {initial.updated_by ?? "n/a"}
            </span>
          </AlertDescription>
        </Alert>
      )}

      <div className="flex justify-end">
        <Button disabled={busy} onClick={handleSubmit}>
          <SaveIcon className="mr-2 h-4 w-4" />
          {busy ? "Saving…" : "Save & broadcast to searcher"}
        </Button>
      </div>
    </div>
  );
}
