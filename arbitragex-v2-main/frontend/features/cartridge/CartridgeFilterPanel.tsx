"use client";

/**
 * CartridgeFilterPanel — Idea 1 (Phase-1) operator route pre-filter preferences.
 *
 * Lets the operator persist a per-chain cartridge filter: a strategy allowlist
 * (Excel-style multi-select), a min net-profit floor, a max-slippage ceiling, and an
 * optional custom .rhai/.wasm cartridge (base64 via Dropzone). Saved through the
 * admin-gated edge route (httpOnly session cookie -> upstream token).
 *
 * HONESTY (RULE 00 / R8): this is a STORED PREFERENCE. A prominent banner states the
 * filter is NOT yet enforced by searcher-rs — the route pre-filter consumer is a Phase-2
 * (hot-path) follow-up. Strategy options come from the chain's REAL trading_config
 * (enabled_strategies); nothing is fabricated. Initial render is deterministic (no
 * Date/random) so it hydrates cleanly.
 */

import * as React from "react";
import { SlidersHorizontalIcon, SaveIcon, AlertCircleIcon, InfoIcon } from "lucide-react";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Separator } from "@/components/ui/separator";
import { MultiSelectWithAll, type MultiSelectOption } from "@/components/ui/multi-select-all";
import { Dropzone, type UploadedScript } from "@/components/ui/dropzone";
import { getApiBaseUrl } from "@/lib/api-client";
import { useCartridgeFilters } from "./useCartridgeFilters";
import { DEFAULT_CARTRIDGE_FILTER } from "./cartridge-filter-types";

export function CartridgeFilterPanel() {
  const [chainId, setChainId] = React.useState(1);
  const { filter, configured, status, error, note, save } = useCartridgeFilters(chainId);

  const [strategies, setStrategies] = React.useState<string[]>(DEFAULT_CARTRIDGE_FILTER.enabled_strategies);
  const [minProfit, setMinProfit] = React.useState<number>(DEFAULT_CARTRIDGE_FILTER.min_profit_usd);
  const [maxSlippage, setMaxSlippage] = React.useState<number>(DEFAULT_CARTRIDGE_FILTER.max_slippage_bps);
  const [uploaded, setUploaded] = React.useState<UploadedScript | null>(null);
  const [strategyOptions, setStrategyOptions] = React.useState<MultiSelectOption[]>([]);

  // Sync the form from the loaded filter (when it arrives / chain changes).
  React.useEffect(() => {
    if (filter) {
      setStrategies(filter.enabled_strategies);
      setMinProfit(filter.min_profit_usd);
      setMaxSlippage(filter.max_slippage_bps);
    } else {
      setStrategies(DEFAULT_CARTRIDGE_FILTER.enabled_strategies);
      setMinProfit(DEFAULT_CARTRIDGE_FILTER.min_profit_usd);
      setMaxSlippage(DEFAULT_CARTRIDGE_FILTER.max_slippage_bps);
    }
  }, [filter]);

  // Strategy options = the chain's REAL enabled strategies from trading_config (no hardcode).
  React.useEffect(() => {
    if (typeof window === "undefined") return;
    const base = getApiBaseUrl();
    let alive = true;
    void fetch(`${base}/api/trading-config?chain_id=${chainId}`, { credentials: "include" })
      .then(async (r) => {
        const b = (await r.json()) as { enabled_strategies?: unknown };
        if (!alive) return;
        const list = Array.isArray(b.enabled_strategies) ? (b.enabled_strategies as string[]) : [];
        setStrategyOptions(list.map((s) => ({ value: s, label: s })));
      })
      .catch(() => {
        if (alive) setStrategyOptions([]);
      });
    return () => {
      alive = false;
    };
  }, [chainId]);

  const onSave = () => {
    void save(
      {
        enabled_strategies: strategies,
        min_profit_usd: Number.isFinite(minProfit) ? minProfit : 0,
        max_slippage_bps: Number.isFinite(maxSlippage) ? Math.round(maxSlippage) : 10_000,
        custom_cartridge_b64: uploaded ? uploaded.content : (filter?.custom_cartridge_b64 ?? null),
        enabled: true,
      },
      "operator",
    );
  };

  const saving = status === "saving";
  const loading = status === "loading";

  return (
    <Card data-slot="cartridge-filter-panel">
      <CardHeader className="gap-3">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <CardTitle className="flex items-center gap-2 text-base">
            <SlidersHorizontalIcon className="h-5 w-5 text-primary" />
            Cartridge Filters
          </CardTitle>
          <div className="flex items-center gap-2">
            {configured ? (
              <Badge variant="secondary" className="text-[10px] uppercase tracking-wide">configured</Badge>
            ) : (
              <Badge variant="outline" className="text-[10px] uppercase tracking-wide text-muted-foreground">unset</Badge>
            )}
            <Badge variant="outline" className="font-mono text-[10px]">chain {chainId}</Badge>
          </div>
        </div>
        <Alert>
          <InfoIcon className="h-4 w-4" />
          <AlertTitle>Stored preference — not yet enforced</AlertTitle>
          <AlertDescription className="text-xs">
            This persists the operator&apos;s route pre-filter selection. As of Phase&nbsp;1 the
            engine does <span className="font-semibold">not</span> consume it yet — the searcher-rs
            consumer + durable persistence are a Phase&nbsp;2 (hot-path) follow-up.
          </AlertDescription>
        </Alert>
      </CardHeader>

      <CardContent className="space-y-5">
        <div className="grid gap-4 sm:grid-cols-2">
          <div className="space-y-1.5">
            <Label htmlFor="cf-chain">Chain ID</Label>
            <Input
              id="cf-chain"
              type="number"
              min={1}
              value={chainId}
              onChange={(e) => setChainId(Math.max(1, Number(e.target.value) || 1))}
              className="font-mono"
            />
          </div>
          <div className="space-y-1.5">
            <Label>Strategy allowlist</Label>
            <MultiSelectWithAll
              options={strategyOptions}
              selected={strategies}
              onChange={setStrategies}
              placeholder={strategyOptions.length === 0 ? "no strategies on this chain" : "All strategies"}
              disabled={loading || strategyOptions.length === 0}
            />
            <p className="text-[11px] text-muted-foreground">empty = all (no filter)</p>
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="cf-profit">Min net profit (USD)</Label>
            <Input
              id="cf-profit"
              type="number"
              min={0}
              step={0.1}
              value={minProfit}
              onChange={(e) => setMinProfit(Number(e.target.value))}
              className="font-mono"
            />
            <p className="text-[11px] text-muted-foreground">0 = no filter</p>
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="cf-slip">Max slippage (bps)</Label>
            <Input
              id="cf-slip"
              type="number"
              min={0}
              max={10000}
              step={1}
              value={maxSlippage}
              onChange={(e) => setMaxSlippage(Number(e.target.value))}
              className="font-mono"
            />
            <p className="text-[11px] text-muted-foreground">10000 = no filter</p>
          </div>
        </div>

        <div className="space-y-1.5">
          <Label>Custom cartridge (optional)</Label>
          <Dropzone value={uploaded} onChange={setUploaded} />
          {filter?.custom_cartridge_b64 && !uploaded ? (
            <p className="text-[11px] text-muted-foreground">
              a custom cartridge is already stored ({filter.custom_cartridge_b64.length} b64 chars) — upload to replace
            </p>
          ) : null}
        </div>

        {error ? (
          <Alert variant="destructive">
            <AlertCircleIcon className="h-4 w-4" />
            <AlertTitle>Could not {saving ? "save" : "load"} filter</AlertTitle>
            <AlertDescription className="text-xs">
              <span className="font-mono">{error}</span>
              {error.startsWith("http_401") || error === "missing_admin_token"
                ? " — admin session required to save (you are not signed in as operator)."
                : null}
            </AlertDescription>
          </Alert>
        ) : null}

        {status === "saved" ? (
          <Alert>
            <InfoIcon className="h-4 w-4" />
            <AlertTitle>Saved</AlertTitle>
            <AlertDescription className="text-xs">{note ?? "Filter preference stored."}</AlertDescription>
          </Alert>
        ) : null}

        <Separator />
        <div className="flex items-center justify-between gap-2">
          <span className="text-[11px] text-muted-foreground">
            {filter?.updated_at ? `last saved ${filter.updated_at} by ${filter.updated_by ?? "?"}` : "not saved yet"}
          </span>
          <Button onClick={onSave} disabled={saving || loading} className="gap-1.5">
            <SaveIcon className="h-4 w-4" />
            {saving ? "Saving…" : "Save filter"}
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}
