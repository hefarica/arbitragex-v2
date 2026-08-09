"use client";

import { Sheet, SheetContent, SheetHeader, SheetTitle, SheetDescription } from "@/components/ui/sheet";
import { SimulateButton } from "@/components/SimulateButton";

// ─── Inline types ─────────────────────────────────────────────────────────────
// Mirrors the subset of OpportunityListItem used in OpportunitiesClient.tsx.
// Kept local to avoid cross-package imports (see note at top of OpportunitiesClient.tsx).

interface TokenInfo {
  symbol: string | null;
  decimals: number | null;
  logo_url: string | null;
  resolved_via: "onchain_full" | "onchain_partial" | "trustwallet_only" | "failed";
}

/** Accepts any strategy_kind (5 base + 264 cartridge IDs). */
type StrategyKind = string;

type OpportunityStatus =
  | "detected"
  | "validated"
  | "simulated"
  | "scored"
  | "executing"
  | "executed"
  | "reconciled"
  | "rejected"
  | "failed";

export interface OpportunityDetail {
  id: string;
  chain_id: number;
  strategy_kind: StrategyKind;
  dex_a: string;
  dex_b: string | null;
  pair_symbol: string | null;
  token_in: string;
  token_in_info: TokenInfo | null;
  token_out: string;
  token_out_info: TokenInfo | null;
  amount_in_wei: string;
  expected_profit_usd: number | null;
  roi_pct: number | null;
  risk_score: number | null;
  block_number: number | null;
  rejection_reason: string | null;
  status: OpportunityStatus;
  detected_at: string;
  trace_id: string;
  chain_id_out: number | null;
  bridge: string | null;
  bridge_fee_usd: number | null;
}

interface Props {
  opportunity: OpportunityDetail | null;
  onClose: () => void;
}

function Row({ label, value }: { label: string; value: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-0.5 py-2 border-b border-border last:border-0">
      <span className="text-xs text-muted-foreground uppercase tracking-wide">{label}</span>
      <span className="text-sm font-mono break-all">{value ?? <span className="text-muted-foreground/50 italic">—</span>}</span>
    </div>
  );
}

export function OpportunityDetailDialog({ opportunity, onClose }: Props) {
  const opp = opportunity;

  return (
    <Sheet open={opp !== null} onOpenChange={(open) => !open && onClose()}>
      <SheetContent className="w-[400px] sm:w-[560px] overflow-y-auto">
        <SheetHeader className="mb-6">
          <SheetTitle>Opportunity Detail</SheetTitle>
          <SheetDescription>
            {opp ? `${opp.strategy_kind} · ${opp.status}` : ""}
          </SheetDescription>
        </SheetHeader>

        {opp && (
          <div className="space-y-1">
            <Row label="ID" value={opp.id} />
            <Row label="Status" value={opp.status} />
            <Row label="Strategy" value={opp.strategy_kind} />
            <Row label="Chain" value={`${opp.chain_id}${opp.chain_id_out != null && opp.chain_id_out !== opp.chain_id ? ` → ${opp.chain_id_out}` : ""}`} />
            <Row
              label="Token In"
              value={`${opp.token_in_info?.symbol ?? "?"} (${opp.token_in.slice(0, 10)}…)`}
            />
            <Row
              label="Token Out"
              value={`${opp.token_out_info?.symbol ?? "?"} (${opp.token_out.slice(0, 10)}…)`}
            />
            <Row label="Amount In (wei)" value={opp.amount_in_wei} />
            <Row label="DEX A" value={opp.dex_a} />
            <Row label="DEX B" value={opp.dex_b} />
            <Row label="Pair" value={opp.pair_symbol} />
            <Row
              label="Net Yield (USD)"
              value={
                opp.expected_profit_usd != null
                  ? `$${opp.expected_profit_usd.toFixed(4)}`
                  : null
              }
            />
            <Row
              label="Convergence Ratio"
              value={opp.roi_pct != null ? `${opp.roi_pct.toFixed(4)}%` : null}
            />
            <Row
              label="Risk Score"
              value={opp.risk_score != null ? opp.risk_score.toFixed(4) : null}
            />
            <Row label="Block" value={opp.block_number} />
            <Row label="Bridge" value={opp.bridge} />
            <Row
              label="Bridge Fee (USD)"
              value={opp.bridge_fee_usd != null ? `$${opp.bridge_fee_usd.toFixed(4)}` : null}
            />
            <Row
              label="Gas Breakdown"
              value={<span className="italic text-muted-foreground/70">Pending simulation</span>}
            />
            <Row
              label="Bribe (MEV)"
              value={<span className="italic text-muted-foreground/70">Pending simulation</span>}
            />
            {opp.rejection_reason && (
              <Row label="Rejection Reason" value={opp.rejection_reason} />
            )}
            <Row label="Detected At" value={new Date(opp.detected_at).toLocaleString()} />
            <Row label="Trace ID" value={opp.trace_id} />

            {/* G-SIM-1 PR-B2b Fase 5 — on-demand simulation with route_source selector.
                The three enrichment paths (A1/A2/A3) are selectable from the dropdown.
                Result displays inline (loading/success/error). Until B2c wires the
                sim-core encoder, this surfaces the honest "not_implemented" state. */}
            <div className="mt-4 border-t border-border pt-4">
              <div className="mb-2 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Simulation
              </div>
              <SimulateButton opportunityId={opp.id} />
            </div>
          </div>
        )}
      </SheetContent>
    </Sheet>
  );
}
