"use client";

/**
 * RouteDiscoverySettingsSheet — floating config panel for /routes/discovery.
 *
 * ROUTES_CROWN_JEWEL §3.3: discreet, elegant, same design language as the
 * frontend. A small gear button on the status strip opens a right-side Sheet
 * (liquid-glass) with the live route-discovery runtime knobs:
 *   Capa 1: routes_per_tick (500/600/1000), max_hops (2..12, floor 7 shadow)
 *   Capa 2: financing toggles (5 modes) + own inventory + min notional +
 *           per-provider fee overrides (advanced)
 *
 * GET current config → edit → PUT /admin/route-discovery-config/:chain
 * (admin session; same discipline as trading-config-form). The Rust worker
 * reads the Redis mirror live (1s cache) — no restart needed.
 */

import { useCallback, useEffect, useState } from "react";
import { Settings2, Loader2 } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle, SheetTrigger,
} from "@/components/ui/sheet";
import { Switch } from "@/components/ui/switch";
import { Label } from "@/components/ui/label";
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from "@/components/ui/select";
import { Input } from "@/components/ui/input";
import { getApiBaseUrl } from "@/lib/api-client";
import { getAdminToken } from "@/lib/admin-token";

interface FinancingVerdict {
  mode: string;
  label: string;
  enabled: boolean;
  viable: boolean;
  reason?: string | null;
  fee_bps: number;
  max_size_usd: number;
}

interface RdConfig {
  routes_per_tick: number;
  max_hops: number;
  financing_enabled: Record<string, boolean>;
  fee_bps_overrides: Record<string, number>;
  own_inventory_usd: number;
  min_notional_usd: number;
}

const MODES: { key: string; label: string; hint: string }[] = [
  { key: "own_capital", label: "Capital propio", hint: "0 bps · capped por inventario" },
  { key: "aave_fl", label: "Aave V3 flash loan", hint: "5 bps default (gobernable)" },
  { key: "balancer_fl", label: "Balancer flash loan", hint: "0 bps · vault depth" },
  { key: "v2_flash_swap", label: "V2 flash swap", hint: "≈0 bps marginal si hay pierna V2" },
  { key: "flash_mint_dai", label: "Maker flash mint DAI", hint: "0 bps · solo rutas DAI" },
];

const ROUTES_OPTIONS = [500, 600, 1000];

export function RouteDiscoverySettingsSheet({ chainId = 1 }: { chainId?: number }) {
  const [open, setOpen] = useState(false);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [cfg, setCfg] = useState<RdConfig | null>(null);
  const [source, setSource] = useState<string>("defaults");

  const fetchConfig = useCallback(async () => {
    setLoading(true);
    try {
      const res = await fetch(
        `${getApiBaseUrl()}/api/v1/route-discovery/config?chain_id=${chainId}`,
        { headers: { accept: "application/json" }, cache: "no-store" },
      );
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const body = (await res.json()) as { source?: string; config?: RdConfig };
      if (body.config) {
        setCfg({
          ...body.config,
          financing_enabled: body.config.financing_enabled ?? {},
          fee_bps_overrides: body.config.fee_bps_overrides ?? {},
        });
        setSource(body.source ?? "defaults");
      }
    } catch {
      toast.error("No se pudo leer la config del discovery");
    } finally {
      setLoading(false);
    }
  }, [chainId]);

  useEffect(() => {
    if (open && !cfg) void fetchConfig();
  }, [open, cfg, fetchConfig]);

  const save = async () => {
    if (!cfg) return;
    setSaving(true);
    try {
      const adminToken = getAdminToken();
      if (!adminToken) {
        toast.error("Sesión admin requerida — inicia sesión en /admin/signin primero");
        return;
      }
      const res = await fetch(
        `${getApiBaseUrl()}/admin/route-discovery-config/${chainId}`,
        {
          method: "PUT",
          credentials: "include",
          headers: {
            "content-type": "application/json",
            accept: "application/json",
            "x-arbx-admin-token": adminToken,
            "x-arbx-actor": "operator",
          },
          body: JSON.stringify(cfg),
        },
      );
      if (!res.ok) {
        const detail = await res.json().catch(() => ({}));
        throw new Error((detail as { detail?: string }).detail ?? `HTTP ${res.status}`);
      }
      toast.success("Config aplicada — el worker la lee en vivo (≤1s)");
      setOpen(false);
    } catch (e) {
      toast.error(`Fallo al guardar: ${(e as Error).message}`);
    } finally {
      setSaving(false);
    }
  };

  const setMode = (key: string, val: boolean) => {
    if (!cfg) return;
    setCfg({ ...cfg, financing_enabled: { ...cfg.financing_enabled, [key]: val } });
  };

  const setFeeOverride = (key: string, val: string) => {
    if (!cfg) return;
    const num = parseFloat(val);
    const next = { ...cfg.fee_bps_overrides };
    if (Number.isFinite(num) && num >= 0) next[key] = num;
    else delete next[key];
    setCfg({ ...cfg, fee_bps_overrides: next });
  };

  return (
    <Sheet open={open} onOpenChange={setOpen}>
      <SheetTrigger asChild>
        <Button
          variant="outline"
          size="sm"
          className="h-7 gap-1.5 text-xs liquid-glass border-primary/20"
          title="Configuración del discovery (viva — sin restart)"
          data-testid="rd-settings-trigger"
        >
          <Settings2 className="h-3.5 w-3.5" />
          Settings
        </Button>
      </SheetTrigger>
      <SheetContent side="right" className="w-full sm:max-w-md overflow-y-auto liquid-glass">
        <SheetHeader>
          <SheetTitle className="text-base">Route Discovery · Config viva</SheetTitle>
          <SheetDescription className="text-xs">
            El worker lee estos valores en vivo (cache 1s) — sin restart.
            Fuente actual: <span className="font-mono">{source}</span>
          </SheetDescription>
        </SheetHeader>

        {loading && !cfg ? (
          <div className="flex items-center justify-center py-12 gap-2 text-sm text-muted-foreground">
            <Loader2 className="h-4 w-4 animate-spin" /> Cargando config…
          </div>
        ) : cfg ? (
          <div className="space-y-6 px-1 pb-8">
            {/* ── CAPA 1 · Discovery ──────────────────────────────────── */}
            <section className="space-y-3">
              <h3 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                Capa 1 · Discovery
              </h3>
              <div className="grid grid-cols-2 gap-3">
                <div className="space-y-1.5">
                  <Label className="text-xs">Rutas / tick</Label>
                  <Select
                    value={String(cfg.routes_per_tick)}
                    onValueChange={(v: string) => setCfg({ ...cfg, routes_per_tick: Number(v) })}
                  >
                    <SelectTrigger className="h-8 text-xs">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {ROUTES_OPTIONS.map((n) => (
                        <SelectItem key={n} value={String(n)}>{n}</SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                  <p className="text-[10px] text-muted-foreground">
                    Ritmo de emisión (DeferNeverDrop: nunca cobertura)
                  </p>
                </div>
                <div className="space-y-1.5">
                  <Label className="text-xs">Max hops</Label>
                  <Select
                    value={String(cfg.max_hops)}
                    onValueChange={(v: string) => setCfg({ ...cfg, max_hops: Number(v) })}
                  >
                    <SelectTrigger className="h-8 text-xs">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {[2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12].map((n) => (
                        <SelectItem key={n} value={String(n)}>{n}</SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                  <p className="text-[10px] text-muted-foreground">
                    Profundidad de ciclo (floor shadow: 7)
                  </p>
                </div>
              </div>
            </section>

            {/* ── CAPA 2 · Financiamiento ─────────────────────────────── */}
            <section className="space-y-3">
              <h3 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                Capa 2 · Financiamiento (paralelo)
              </h3>
              <p className="text-[10px] text-muted-foreground leading-relaxed">
                La MISMA ruta se evalúa bajo TODOS los modos — desactivar un modo
                no oculta su veredicto hipotético, solo marca que no está en uso.
              </p>
              {MODES.map((m) => (
                <div
                  key={m.key}
                  className="flex items-center justify-between gap-3 rounded-lg border border-border/40 bg-card/30 p-3"
                >
                  <div className="min-w-0">
                    <p className="text-xs font-medium truncate">{m.label}</p>
                    <p className="text-[10px] text-muted-foreground truncate">{m.hint}</p>
                    <div className="flex items-center gap-1 mt-1">
                      <Input
                        className="h-6 w-16 text-[10px] font-mono px-1.5"
                        placeholder={`${m.key === "aave_fl" ? 5 : 0}`}
                        value={cfg.fee_bps_overrides[m.key] ?? ""}
                        onChange={(e) => setFeeOverride(m.key, e.target.value)}
                      />
                      <span className="text-[9px] text-muted-foreground">bps override</span>
                    </div>
                  </div>
                  <Switch
                    checked={cfg.financing_enabled[m.key] ?? true}
                    onCheckedChange={(v) => setMode(m.key, v)}
                  />
                </div>
              ))}
            </section>

            {/* ── Umbrales ────────────────────────────────────────────── */}
            <section className="space-y-3">
              <h3 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                Umbrales
              </h3>
              <div className="grid grid-cols-2 gap-3">
                <div className="space-y-1.5">
                  <Label className="text-xs">Capital propio (USD)</Label>
                  <Input
                    className="h-8 text-xs font-mono"
                    type="number"
                    value={cfg.own_inventory_usd}
                    onChange={(e) =>
                      setCfg({ ...cfg, own_inventory_usd: Number(e.target.value) || 0 })
                    }
                  />
                </div>
                <div className="space-y-1.5">
                  <Label className="text-xs">Notional mínimo (USD)</Label>
                  <Input
                    className="h-8 text-xs font-mono"
                    type="number"
                    value={cfg.min_notional_usd}
                    onChange={(e) =>
                      setCfg({ ...cfg, min_notional_usd: Number(e.target.value) || 0 })
                    }
                  />
                </div>
              </div>
            </section>

            <Button onClick={save} disabled={saving} className="w-full" size="sm">
              {saving ? <Loader2 className="h-4 w-4 animate-spin mr-2" /> : null}
              {saving ? "Aplicando…" : "Aplicar config (vivo)"}
            </Button>
          </div>
        ) : null}
      </SheetContent>
    </Sheet>
  );
}

/** Financing badges for a RouteCard — the born/died/resized comparison. */
export function FinancingBadges({ financing }: { financing: FinancingVerdict[] }) {
  if (!financing || financing.length === 0) {
    return <span className="text-[10px] text-muted-foreground">financing: —</span>;
  }
  return (
    <div className="flex flex-wrap gap-1 mt-1" data-testid="financing-badges">
      {financing.map((f) => {
        const alive = f.viable;
        const dim = !f.enabled;
        const tone = alive
          ? "border-success/40 bg-success/10 text-success"
          : "border-destructive/40 bg-destructive/10 text-destructive";
        const title = alive
          ? `${f.label}: VIABLE — hasta $${Math.round(f.max_size_usd).toLocaleString()} · ${f.fee_bps} bps`
          : `${f.label}: MUERE — ${f.reason}`;
        return (
          <span
            key={f.mode}
            title={title}
            className={`inline-flex items-center gap-0.5 rounded border px-1 py-px text-[9px] font-medium ${tone} ${
              dim ? "opacity-50" : ""
            }`}
          >
            {alive ? "✓" : "✗"}
            {f.mode === "own_capital" ? "OWN" : f.mode === "aave_fl" ? "AAVE" : f.mode === "balancer_fl" ? "BAL" : f.mode === "v2_flash_swap" ? "V2SW" : "DAI"}
            {alive && f.max_size_usd > 0 ? ` $${compactUsd(f.max_size_usd)}` : ""}
          </span>
        );
      })}
    </div>
  );
}

function compactUsd(v: number): string {
  if (v >= 1_000_000) return `$${(v / 1_000_000).toFixed(1)}M`.replace("$$", "");
  if (v >= 1_000) return `${(v / 1_000).toFixed(0)}k`;
  return v.toFixed(0);
}
