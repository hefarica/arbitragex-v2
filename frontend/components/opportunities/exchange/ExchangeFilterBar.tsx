/**
 * ExchangeFilterBar — the configurable top bar for `/opportunities/exchange`.
 *
 * Lets the operator choose WHICH opportunities surface (the operator's
 * "configurable de que tipo de oportunidades deben salir"):
 *   - Strategy family chips — the 5 base families + every MEV-XX cartridge
 *     family present in the live feed (covers all 264 cartridges via their
 *     family prefix). Multi-select; an opportunity passes when its family is
 *     enabled.
 *   - Chain selector — filters by chain_id (from the /api/chains catalog).
 *   - Cartridge search — free-text substring over strategy_kind (the granular
 *     264-cartridge IDs, e.g. "mev_01_007").
 *   - Viable-only toggle — mirrors the existing /opportunities page behaviour.
 *   - Min net yield — optional floor on the displayed net (USD).
 *
 * The right-hand mode badge surfaces the effective execution terminus
 * (paper / live) read from `usePaperModeState` — read-only display only; the
 * page never flips the mode (handled by /config + /killswitch per §34).
 *
 * R1 Mounted Snapshot: all interactive state is client-only; the bar renders a
 * deterministic shell on SSR.
 */
"use client";

import React, { useMemo } from "react";
import { Eye, EyeOff, Search, Filter } from "lucide-react";
import { useChains } from "@/lib/chains";
import { familyOf, BASE_STRATEGIES } from "@/lib/strategy-kinds";
import type { OmniOpportunity } from "@/lib/store/types";

/** Base family → display label (mirrors StrategyBadge canonical labels). */
const BASE_FAMILY_LABEL: Record<string, string> = {
  dex_arb: "DEX",
  triangular: "TRI",
  backrun: "BACKRUN",
  liquidation: "LIQ",
  flashloan_arb: "FLASH",
};

export interface ExchangeFilters {
  enabledFamilies: Set<string>;
  chainId: number | "all";
  search: string;
  viableOnly: boolean;
  minYieldUsd: number | null;
}

export const DEFAULT_FILTERS: ExchangeFilters = {
  enabledFamilies: new Set<string>(),
  chainId: "all",
  search: "",
  viableOnly: false,
  minYieldUsd: null,
};

export interface ExchangeFilterBarProps {
  opportunities: OmniOpportunity[];
  filters: ExchangeFilters;
  onChange: (next: ExchangeFilters) => void;
  /** Effective execution terminus label (paper / live) for the read-only badge. */
  modeLabel: "paper" | "live";
  /** Confidence string from usePaperModeState (DEFAULT_SAFE / OK / …). */
  modeConfidence?: string;
}

export function ExchangeFilterBar({
  opportunities,
  filters,
  onChange,
  modeLabel,
  modeConfidence,
}: ExchangeFilterBarProps) {
  const { chains } = useChains();

  // Families present in the live feed (base kinds + MEV-XX prefixes), unioned
  // with the canonical base 5 so the chips are stable even before data lands.
  const families = useMemo(() => {
    const set = new Set<string>(BASE_STRATEGIES as readonly string[]);
    for (const o of opportunities) set.add(familyOf(o.strategy_kind));
    return Array.from(set).sort((a, b) => a.localeCompare(b));
  }, [opportunities]);

  // If the operator hasn't touched the family filter yet (empty set = "all"),
  // every family is considered enabled.
  const allEnabled = filters.enabledFamilies.size === 0;
  const isFamilyEnabled = (fam: string) => allEnabled || filters.enabledFamilies.has(fam);
  const toggleFamily = (fam: string) => {
    const next = new Set<string>(allEnabled ? families : filters.enabledFamilies);
    if (next.has(fam)) next.delete(fam);
    else next.add(fam);
    onChange({ ...filters, enabledFamilies: next });
  };

  return (
    <div className="mb-6 rounded-2xl border border-border bg-card/60 backdrop-blur p-3 shadow-sm">
      {/* Row 1: family chips + mode badge */}
      <div className="flex items-start justify-between gap-3 flex-wrap">
        <div className="flex items-center gap-1.5 flex-wrap min-w-0">
          <span className="inline-flex items-center gap-1 text-[10px] uppercase tracking-wide text-muted-foreground font-semibold mr-1">
            <Filter size={11} /> Strategy
          </span>
          {families.map((fam) => {
            const enabled = isFamilyEnabled(fam);
            const label = BASE_FAMILY_LABEL[fam] ?? fam;
            return (
              <button
                key={fam}
                type="button"
                onClick={() => toggleFamily(fam)}
                aria-pressed={enabled}
                title={fam}
                className={`px-2 py-0.5 rounded-full border text-[10px] font-bold uppercase tracking-wide transition-colors ${
                  enabled
                    ? "bg-primary/15 text-primary border-primary/40 hover:bg-primary/25"
                    : "bg-muted/30 text-muted-foreground/60 border-border hover:bg-muted/50"
                }`}
              >
                {label}
              </button>
            );
          })}
        </div>

        {/* Mode badge — read-only terminus display */}
        <div
          className={`shrink-0 inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full border text-[10px] font-bold uppercase tracking-wide ${
            modeLabel === "live"
              ? "bg-destructive/10 border-destructive/40 text-destructive"
              : "bg-info/10 border-info/40 text-info"
          }`}
          title={`Effective execution terminus: ${modeLabel}${modeConfidence ? ` · confidence ${modeConfidence}` : ""}. Mode switching lives in /config + /killswitch (§34).`}
        >
          <span className={`size-1.5 rounded-full ${modeLabel === "live" ? "bg-destructive" : "bg-info"} animate-pulse`} />
          {modeLabel}
          {modeConfidence && modeConfidence !== "explicit" && (
            <span className="opacity-70 normal-case font-mono">· {modeConfidence}</span>
          )}
        </div>
      </div>

      {/* Row 2: chain · search · viable · min-yield */}
      <div className="flex items-center gap-2 flex-wrap mt-3">
        <select
          value={String(filters.chainId)}
          onChange={(e) =>
            onChange({
              ...filters,
              chainId: e.target.value === "all" ? "all" : Number(e.target.value),
            })
          }
          className="bg-muted/40 border border-border rounded-lg px-2 py-1.5 text-xs text-foreground focus:outline-none focus:border-primary/50"
          title="Filter by chain"
        >
          <option value="all">All chains</option>
          {chains.map((c) => (
            <option key={c.chain_id} value={String(c.chain_id)}>
              {c.short} · {c.name}
            </option>
          ))}
        </select>

        <div className="relative flex-1 min-w-[160px]">
          <Search size={13} className="absolute left-2 top-1/2 -translate-y-1/2 text-muted-foreground/60" />
          <input
            type="text"
            value={filters.search}
            onChange={(e) => onChange({ ...filters, search: e.target.value })}
            placeholder="cartridge / strategy id (e.g. mev_01_007)"
            className="w-full bg-muted/40 border border-border rounded-lg pl-7 pr-2 py-1.5 text-xs text-foreground placeholder:text-muted-foreground/50 focus:outline-none focus:border-primary/50"
          />
        </div>

        <label className="inline-flex items-center gap-1.5 text-xs text-muted-foreground">
          <span>min net $</span>
          <input
            type="number"
            min={0}
            step={0.5}
            value={filters.minYieldUsd ?? ""}
            onChange={(e) =>
              onChange({
                ...filters,
                minYieldUsd: e.target.value === "" ? null : Number(e.target.value),
              })
            }
            placeholder="—"
            className="w-16 bg-muted/40 border border-border rounded-lg px-2 py-1.5 text-xs text-foreground placeholder:text-muted-foreground/50 focus:outline-none focus:border-primary/50"
          />
        </label>

        <button
          type="button"
          onClick={() => onChange({ ...filters, viableOnly: !filters.viableOnly })}
          aria-pressed={filters.viableOnly}
          className={`inline-flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg border text-xs font-semibold transition-colors ${
            filters.viableOnly
              ? "bg-success/10 border-success/40 text-success hover:bg-success/20"
              : "bg-destructive/10 border-destructive/40 text-destructive hover:bg-destructive/20"
          }`}
          title={filters.viableOnly ? "Showing viable only — click to show all" : "Showing all — click for viable only"}
        >
          {filters.viableOnly ? <Eye size={13} /> : <EyeOff size={13} />}
          {filters.viableOnly ? "Viable" : "All"}
        </button>
      </div>
    </div>
  );
}

/**
 * Applies the filter set to an opportunity list. Pure — no side effects.
 * Used by the page client to derive the visible grid.
 */
export function applyExchangeFilters(
  opportunities: OmniOpportunity[],
  filters: ExchangeFilters,
): OmniOpportunity[] {
  const allEnabled = filters.enabledFamilies.size === 0;
  const search = filters.search.trim().toLowerCase();
  return opportunities.filter((o) => {
    if (!allEnabled && !filters.enabledFamilies.has(familyOf(o.strategy_kind))) return false;
    if (filters.chainId !== "all" && o.chain_id !== filters.chainId) return false;
    if (search && !o.strategy_kind.toLowerCase().includes(search)) return false;
    if (filters.viableOnly && (o.status === "rejected" || o.status === "failed")) return false;
    if (filters.minYieldUsd != null && Number.isFinite(filters.minYieldUsd)) {
      const net = o.net_expected_profit_usd ?? o.simulated_net_profit_usd ?? o.expected_profit_usd;
      if (net == null || net < filters.minYieldUsd) return false;
    }
    return true;
  });
}

