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
import { Eye, EyeOff, Filter } from "lucide-react";
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
  // FE-0029 (§28): a null strategy_kind (malformed payload) joins NO family —
  // it must not mint taxonomy chips; it stays visible only under "all".
  const families = useMemo(() => {
    const set = new Set<string>(BASE_STRATEGIES as readonly string[]);
    for (const o of opportunities) {
      if (o.strategy_kind != null) set.add(familyOf(o.strategy_kind));
    }
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
    <div className="controls atlas-controls">
      {/* Row 1: family chips + mode badge (led-group style) */}
      <div className="atlas-chiprow">
        <span className="atlas-chiplabel">
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
              className={`chip ${enabled ? "chip-on" : "chip-off"}`}
            >
              {label}
            </button>
          );
        })}

        {/* Mode badge — read-only terminus display (led-group style) */}
        <span
          className="led-group"
          title={`Effective execution terminus: ${modeLabel}${modeConfidence ? ` · confidence ${modeConfidence}` : ""}. Mode switching lives in /config + /killswitch (§34).`}
        >
          <span className={`led wait ${modeLabel === "live" ? "led-hot" : ""}`} />
          <span className={modeLabel === "live" ? "led-text-hot" : "led-text-live"}>{modeLabel}</span>
          {modeConfidence && modeConfidence !== "explicit" && (
            <span style={{ opacity: 0.7 }}>· {modeConfidence}</span>
          )}
        </span>
      </div>

      {/* Row 2: chain · search · viable · min-yield */}
      <select
        value={String(filters.chainId)}
        onChange={(e) =>
          onChange({
            ...filters,
            chainId: e.target.value === "all" ? "all" : Number(e.target.value),
          })
        }
        title="Filter by chain"
      >
        <option value="all">All chains</option>
        {chains.map((c) => (
          <option key={c.chain_id} value={String(c.chain_id)}>
            {c.short} · {c.name}
          </option>
        ))}
      </select>

      <input
        type="search"
        value={filters.search}
        onChange={(e) => onChange({ ...filters, search: e.target.value })}
        placeholder="cartridge / strategy id (e.g. mev_01_007)"
        aria-label="Filter by cartridge / strategy id"
      />

      <label className="atlas-minlabel">
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
        />
      </label>

      <button
        type="button"
        onClick={() => onChange({ ...filters, viableOnly: !filters.viableOnly })}
        aria-pressed={filters.viableOnly}
        className={`chip ${filters.viableOnly ? "chip-on" : "chip-off"}`}
        title={filters.viableOnly ? "Showing viable only — click to show all" : "Showing all — click for viable only"}
      >
        {filters.viableOnly ? <Eye size={12} /> : <EyeOff size={12} />}
        {filters.viableOnly ? "Viable" : "All"}
      </button>
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
    // FE-0029 (§28): null strategy_kind belongs to no family, matches no text
    // search — visible only when everything is enabled and unsearched.
    if (!allEnabled && (o.strategy_kind == null || !filters.enabledFamilies.has(familyOf(o.strategy_kind))))
      return false;
    if (filters.chainId !== "all" && o.chain_id !== filters.chainId) return false;
    if (search && (o.strategy_kind == null || !o.strategy_kind.toLowerCase().includes(search)))
      return false;
    // Fail-safe: viableOnly cannot assert "not rejected" for an unstatused row.
    if (filters.viableOnly && (o.status == null || o.status === "rejected" || o.status === "failed"))
      return false;
    if (filters.minYieldUsd != null && Number.isFinite(filters.minYieldUsd)) {
      const net = o.net_expected_profit_usd ?? o.simulated_net_profit_usd ?? o.expected_profit_usd;
      if (net == null || net < filters.minYieldUsd) return false;
    }
    return true;
  });
}

