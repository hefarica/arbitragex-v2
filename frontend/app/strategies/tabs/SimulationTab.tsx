/**
 * Simulation tab — paper-trade preview knobs for the operator.
 *
 * All fields are OPTIONAL on the backend (serde(default)) — empty values
 * here mean "no override, fall back to operational capital_usd". Hot-reloads
 * to searcher in ≤1s on save (same Redis pub/sub channel as other tabs).
 *
 * Doctrine: paper-trade mode (ARBX_PAPER_TRADE=true default) ensures these
 * knobs NEVER trigger on-chain execution. Pure visualization layer.
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

type KvRow = { key: string; value: string };

function mapToRows(map: Record<string, number> | undefined): KvRow[] {
  if (!map) return [];
  return Object.entries(map).map(([key, value]) => ({ key, value: String(value) }));
}

function rowsToMap(rows: KvRow[]): Record<string, number> {
  const out: Record<string, number> = {};
  for (const r of rows) {
    const k = r.key.trim().toUpperCase();
    const v = Number(r.value);
    if (k.length === 0) continue;
    if (!Number.isFinite(v) || v < 0) continue;
    out[k] = v;
  }
  return out;
}

function nullableNumber(v: string): number | null {
  const t = v.trim();
  if (t.length === 0) return null;
  const n = Number(t);
  return Number.isFinite(n) ? n : null;
}

export function SimulationTab({ config, onSaved, adminToken, actor }: Props) {
  // Top-level numeric knobs.
  const [capital, setCapital] = useState<string>(
    config.simulation_capital_usd != null ? String(config.simulation_capital_usd) : "",
  );
  const [targetProfit, setTargetProfit] = useState<string>(
    config.simulation_target_profit_usd != null ? String(config.simulation_target_profit_usd) : "",
  );
  const [targetRoi, setTargetRoi] = useState<string>(
    config.simulation_target_roi_pct != null ? String(config.simulation_target_roi_pct) : "",
  );

  // Maps rendered as editable rows.
  const [tokenRows, setTokenRows] = useState<KvRow[]>(mapToRows(config.simulation_per_token_amounts_usd));
  const [strategyRows, setStrategyRows] = useState<KvRow[]>(
    mapToRows(config.simulation_per_strategy_caps_usd),
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

  const onSave = async () => {
    if (!hasSession) {
      setMsg("Login required: open /killswitch and unlock an admin session first.");
      return;
    }
    setSaving(true);
    setMsg(null);

    const next: TradingConfigConfigured = {
      ...config,
      simulation_capital_usd: nullableNumber(capital) ?? undefined,
      simulation_target_profit_usd: nullableNumber(targetProfit) ?? undefined,
      simulation_target_roi_pct: nullableNumber(targetRoi) ?? undefined,
      simulation_per_token_amounts_usd: rowsToMap(tokenRows),
      simulation_per_strategy_caps_usd: rowsToMap(strategyRows),
    };

    const { configured: _c, chain_id: _cid, updated_at: _ua, updated_by: _ub, ...body } = next;
    void _c;
    void _cid;
    void _ua;
    void _ub;

    const res = await putTradingConfig(next.chain_id, body, adminToken, actor);
    setSaving(false);
    if (res.ok) {
      onSaved({ ...next, updated_at: res.data.updated_at, updated_by: res.data.updated_by });
      setMsg("Saved · hot-reload to searcher in ≤1s");
    } else {
      setMsg(res.error);
    }
  };

  return (
    <div className="grid gap-4">
      {/* ── Global simulation knobs ─────────────────────────────────── */}
      <Card>
        <CardHeader>
          <CardTitle>Simulación · paper-trade preview · chain {config.chain_id}</CardTitle>
        </CardHeader>
        <CardContent className="grid gap-4 md:grid-cols-3">
          <Field
            label="Capital simulación (USD) · global cap"
            value={capital}
            placeholder="(vacío = usa capital operacional)"
            onChange={setCapital}
          />
          <Field
            label="Target profit min (USD) · UI filter"
            value={targetProfit}
            placeholder="(vacío = sin filtro)"
            onChange={setTargetProfit}
            step="0.01"
          />
          <Field
            label="Target ROI min (%) · UI filter"
            value={targetRoi}
            placeholder="(vacío = sin filtro)"
            onChange={setTargetRoi}
            step="0.01"
          />
          <p className="text-xs text-muted-foreground md:col-span-3">
            <strong>Capital simulación</strong> sobreescribe el capital operacional para el cálculo del cap_ratio
            del spine — pero solo si NO hay un cap más estricto por-token o por-estrategia abajo. <strong>Target
            profit/ROI</strong> son filtros de UI: el backend persiste todas las opps, la dashboard suprime las
            que estén por debajo del target. Modo paper-trade activo por defecto · cero ejecución on-chain.
          </p>
        </CardContent>
      </Card>

      {/* ── Per-token caps ──────────────────────────────────────────── */}
      <Card>
        <CardHeader>
          <CardTitle>Caps per-token (USD)</CardTitle>
        </CardHeader>
        <CardContent className="grid gap-3">
          <p className="text-xs text-muted-foreground">
            Cap específico cuando el token de entrada coincide. Lookup case-insensitive (WETH/weth/WeTh todos
            matchean). Se aplica el MIN entre este cap, el global, y el per-strategy.
          </p>
          <KeyValueEditor
            keyPlaceholder="símbolo (e.g. WETH)"
            valuePlaceholder="cap USD"
            rows={tokenRows}
            onChange={setTokenRows}
          />
        </CardContent>
      </Card>

      {/* ── Per-strategy caps ───────────────────────────────────────── */}
      <Card>
        <CardHeader>
          <CardTitle>Caps per-strategy (USD)</CardTitle>
        </CardHeader>
        <CardContent className="grid gap-3">
          <p className="text-xs text-muted-foreground">
            Cap específico por tipo de arbitraje (e.g. <code>dex_arb_v2v2</code>, <code>triangular</code>,
            <code> cex_dex</code>). Misma resolución MIN que per-token. Útil para modelar envelopes
            de riesgo distintos por estrategia (ej. triangular más conservador que dex_arb).
          </p>
          <KeyValueEditor
            keyPlaceholder="strategy_kind (e.g. dex_arb_v2v2)"
            valuePlaceholder="cap USD"
            rows={strategyRows}
            onChange={setStrategyRows}
          />
        </CardContent>
      </Card>

      {/* ── Save ────────────────────────────────────────────────────── */}
      <div className="flex items-center gap-3 pt-2">
        <Button onClick={onSave} disabled={saving || !hasSession} title={!hasSession ? "Admin session required — open /killswitch first" : undefined}>
          {saving ? "Saving…" : "Save changes"}
        </Button>
        {msg && <span className="text-xs text-muted-foreground font-mono">{msg}</span>}
        {!hasSession && !msg && (
          <span className="text-xs text-amber-400 font-mono">No admin session — <a href="/killswitch" className="underline">unlock at /killswitch</a></span>
        )}
        <span className="ml-auto text-xs text-muted-foreground">
          updated_by: {config.updated_by ?? "—"} · {config.updated_at}
        </span>
      </div>
    </div>
  );
}

function Field({
  label,
  value,
  placeholder,
  step,
  onChange,
}: {
  label: string;
  value: string;
  placeholder?: string;
  step?: string;
  onChange: (v: string) => void;
}) {
  return (
    <div className="grid gap-1.5">
      <Label className="text-xs text-muted-foreground">{label}</Label>
      <Input
        type="number"
        value={value}
        placeholder={placeholder}
        step={step}
        onChange={(e) => onChange(e.target.value)}
      />
    </div>
  );
}

function KeyValueEditor({
  keyPlaceholder,
  valuePlaceholder,
  rows,
  onChange,
}: {
  keyPlaceholder: string;
  valuePlaceholder: string;
  rows: KvRow[];
  onChange: (next: KvRow[]) => void;
}) {
  const setRow = (i: number, next: KvRow) => {
    const updated = rows.slice();
    updated[i] = next;
    onChange(updated);
  };
  const removeRow = (i: number) => {
    const updated = rows.slice();
    updated.splice(i, 1);
    onChange(updated);
  };
  const addRow = () => onChange([...rows, { key: "", value: "" }]);

  return (
    <div className="grid gap-2">
      {rows.length === 0 && (
        <p className="text-xs text-muted-foreground italic">
          (vacío — clic "Add row" para añadir un cap)
        </p>
      )}
      {rows.map((r, i) => (
        <div key={i} className="grid grid-cols-[2fr_1fr_auto] gap-2 items-center">
          <Input
            placeholder={keyPlaceholder}
            value={r.key}
            onChange={(e) => setRow(i, { ...r, key: e.target.value })}
          />
          <Input
            type="number"
            step="0.01"
            placeholder={valuePlaceholder}
            value={r.value}
            onChange={(e) => setRow(i, { ...r, value: e.target.value })}
          />
          <Button variant="ghost" size="sm" onClick={() => removeRow(i)} aria-label="Remove row">
            ✕
          </Button>
        </div>
      ))}
      <Button variant="outline" size="sm" onClick={addRow} className="w-fit">
        + Add row
      </Button>
    </div>
  );
}
