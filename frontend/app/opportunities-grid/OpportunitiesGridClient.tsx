/**
 * =============================================================================
 * OMEGA Opportunity Grid — Client Integrator
 * =============================================================================
 *
 * Orchestrates the new card-grid surface:
 *   AiAgentBanner → TerminalStatsBar → PresetSelector → OpportunityGrid
 *
 * Hydrates from the same Omni-Store as /opportunities (single source of
 * truth). Re-uses OpportunityDetailDialog for inspection.
 *
 * Doctrine:
 *   - R1 Mounted Snapshot: server passes initialSnapshot; client only
 *     subscribes to store after mount via useOmniOpportunities.
 *   - R8 Fail-Honest: filteredCount reflects the real store count under the
 *     current preset; stats default to null (TerminalStatsBar renders "—").
 *     No fabricated PnL/win-rate ever surfaces in this view.
 *
 * Attribution: layout & operational anatomy inspired by DeFiBot.trade's
 * terminal. **No code, wallets, opcodes, addresses or colours copied.**
 */

"use client";

import React, { useMemo, useState } from "react";
import { useOmniOpportunities } from "@/lib/store/useOmniOpportunities";
import { useOmniStore } from "@/lib/store/omni-store";
import type { OmniOpportunity } from "@/lib/store/types";
import {
  OpportunityDetailDialog,
  type OpportunityDetail,
} from "@/components/OpportunityDetailDialog";

import { AiAgentBanner, type AgentStatus } from "@/features/opportunities-grid/AiAgentBanner";
import { TerminalStatsBar } from "@/features/opportunities-grid/TerminalStatsBar";
import {
  PresetSelector,
  PRESETS,
  type PresetKey,
  type PresetDescriptor,
} from "@/features/opportunities-grid/PresetSelector";
import { OpportunityGrid } from "@/features/opportunities-grid/OpportunityGrid";

export type OpportunitiesGridSnapshot = {
  opportunities: OmniOpportunity[];
  serverTime: string | null;
  source: string;
};

interface Props {
  initialSnapshot: OpportunitiesGridSnapshot;
}

/**
 * Resolve the AI agent banner status from public environment flags.
 *
 * Rationale: we cannot snoop on the backend's process tree from the browser,
 * so we expose declarative `NEXT_PUBLIC_AGENT_*` flags that ops sets when
 * spawning the corresponding workers. If a flag is unset, we render `false`
 * (idle) rather than guessing — that is the R8 fail-honest behaviour.
 */
function resolveAgentStatus(): AgentStatus {
  const flag = (key: string): boolean =>
    (process.env[key] ?? "").toString().toLowerCase() === "true";
  return {
    agent_simulator: flag("NEXT_PUBLIC_AGENT_SIMULATOR"),
    agent_executor: flag("NEXT_PUBLIC_AGENT_EXECUTOR"),
    agent_notifier: flag("NEXT_PUBLIC_AGENT_NOTIFIER"),
    agent_breaker: flag("NEXT_PUBLIC_AGENT_BREAKER"),
  };
}

export default function OpportunitiesGridClient({ initialSnapshot }: Props) {
  // ─── Hydrate Omni-Store with WebSocket + polling fallback ─────────────────
  const EDGE_URL = process.env.NEXT_PUBLIC_EDGE_URL ?? "";
  useOmniOpportunities({
    edgeUrl: EDGE_URL,
    initialOpportunities: initialSnapshot.opportunities,
  });

  const opportunities = useOmniStore((state) => state.opportunities);
  const wsStatus = useOmniStore((state) => state.wsStatus);

  // ─── Local UI state ───────────────────────────────────────────────────────
  const [presetKey, setPresetKey] = useState<PresetKey>("balanced");
  const [selectedOpp, setSelectedOpp] = useState<OpportunityDetail | null>(null);

  // ─── Preset-driven filter (read-only client filter; backend still owns
  //     the policy for execution gates). Honest semantics:
  //     - We never fabricate ROI.  We just hide cards that are clearly
  //       outside the preset's spread band.
  //     - If ROI is unknown, we keep the card visible to avoid hiding
  //       useful signal from the user.
  // ────────────────────────────────────────────────────────────────────────
  const preset = useMemo<PresetDescriptor>(
    () => PRESETS.find((p) => p.key === presetKey) ?? PRESETS[1],
    [presetKey]
  );

  const filtered = useMemo(() => {
    const lo = preset.spread_min_pct;
    const hi = preset.spread_max_pct;
    return opportunities.filter((o) => {
      const roi = typeof o.roi_pct === "number" ? o.roi_pct : null;
      if (roi === null) return true; // honest: don't hide unknown ROI
      return roi >= lo && roi <= hi;
    });
  }, [opportunities, preset]);

  // ─── Agent status (R8: no inference — we only read declared flags) ────────
  const agentStatus = useMemo(resolveAgentStatus, []);

  // ─── Stats: until we wire a real /api/stats endpoint, surface nulls so
  //     TerminalStatsBar renders "—" with the explicit caption. This is the
  //     deliberate R8 fail-honest behaviour — F-71 Public verifiable stats
  //     requires reconciled execution data, which the simulator-only stack
  //     does not yet emit.
  // ────────────────────────────────────────────────────────────────────────
  const stats = useMemo(
    () => [
      { label: "Total P&L", value: null, format: "money" as const },
      { label: "Win Rate", value: null, format: "percent" as const },
      { label: "Volume (24h)", value: null, format: "money" as const },
      { label: "Best Trade", value: null, format: "money" as const },
    ],
    []
  );

  // ─── Inspect handler — open the existing detail sheet ─────────────────────
  const handleInspect = (opp: OmniOpportunity) => {
    // OmniOpportunity is structurally a superset of OpportunityDetail in
    // the legacy sheet; cast is safe because the sheet only reads fields
    // that the store guarantees to be present (id, chain_id, pair_symbol…).
    setSelectedOpp(opp as unknown as OpportunityDetail);
  };

  // ─── Execute handler — R8: we DO NOT execute from the UI yet. The CTA
  //     is gated by status inside OpportunityCard. Until /execute is wired,
  //     we open the detail sheet so the operator can review.
  // ────────────────────────────────────────────────────────────────────────
  const handleExecute = (opp: OmniOpportunity) => {
    handleInspect(opp);
  };

  return (
    <div className="container mx-auto px-4 py-6 space-y-6">
      {/* ─── Header (no fake live-ticker, just honest status) ────────── */}
      <header className="flex flex-wrap items-baseline justify-between gap-2">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight">
            Opportunity Grid
          </h1>
          <p className="text-sm text-muted-foreground">
            Card-grid surface · Feed:{" "}
            <span
              className={
                wsStatus === "LIVE"
                  ? "text-success"
                  : wsStatus === "POLLING"
                  ? "text-warning"
                  : "text-muted-foreground"
              }
            >
              {wsStatus}
            </span>{" "}
            · Source: {initialSnapshot.source}
          </p>
        </div>
        <a
          href="/opportunities"
          className="text-sm underline text-muted-foreground hover:text-foreground"
        >
          Switch to dense table →
        </a>
      </header>

      {/* ─── AI Agent Banner ─────────────────────────────────────────── */}
      <AiAgentBanner status={agentStatus} />

      {/* ─── Stats Strip ─────────────────────────────────────────────── */}
      <TerminalStatsBar stats={stats} />

      {/* ─── Preset Selector ─────────────────────────────────────────── */}
      <PresetSelector
        selected={presetKey}
        onChange={(p) => setPresetKey(p.key)}
      />

      {/* ─── Opportunity Grid ────────────────────────────────────────── */}
      <OpportunityGrid
        items={filtered}
        onExecute={handleExecute}
        onInspect={handleInspect}
      />

      {/* ─── Detail Sheet (reuses legacy component) ──────────────────── */}
      <OpportunityDetailDialog
        opportunity={selectedOpp}
        onClose={() => setSelectedOpp(null)}
      />
    </div>
  );
}
