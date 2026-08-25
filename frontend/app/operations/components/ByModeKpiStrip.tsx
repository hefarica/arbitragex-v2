/**
 * ARBX-0011 (REQ-DASH-BY-MODE) — by-mode KPI scope strip.
 *
 * One math pipeline, mode as a LABEL, never a fork (§34.1 hot-path
 * mode-invariance): the KPI cards below this strip belong to the ONE
 * operations pipeline; the strip states which trading-mode terminus they
 * reflect and the REAL availability of the other two canonical termini.
 * Every status line is doctrine text (EXECUTION_MODES / live_exec_policy),
 * never a fabricated per-mode number (RULE 00 — no per-mode dataset exists
 * today; the paper ledger is the only terminus producing data).
 *
 * Declarative display only: mode authority is relays-client
 * `live_exec_policy` (§34.3), NOT this surface.
 */
import * as React from "react";
import { AlertCircleIcon } from "lucide-react";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  EXECUTION_MODES,
  type CanonicalModeView,
  type ExecutionMode,
} from "@/lib/apex/schemas/knobs";

interface Props {
  view: CanonicalModeView | null;
  /** Honest fetch/absence reason rendered when view is null (R8). */
  error: string | null;
}

/** Doctrine status per terminus — static facts, not measurements. */
function terminusStatus(mode: ExecutionMode, active: boolean): string {
  switch (mode) {
    case "PAPER_SHADOW":
      return active
        ? "active · KPIs below reflect this terminus (paper ledger)"
        : "paper ledger terminus (no broadcast)";
    case "TESTNET":
      return active
        ? "active · broadcast only to ARBX_LIVE_EXEC_CHAINS (Sepolia)"
        : "broadcast gated: testnet allowlist (ARBX_LIVE_EXEC_CHAINS)";
    case "LIVE_MAINNET":
      return active
        ? "active · mainnet terminus (§34.3 gates all satisfied)"
        : "default-deny: MainnetRefused (§34.3)";
  }
}

export function ByModeKpiStrip({ view, error }: Props) {
  return (
    <Card className="mb-6">
      <CardHeader className="pb-2">
        <CardTitle className="text-sm font-medium text-muted-foreground">
          Execution modes · KPI scope
        </CardTitle>
        <p className="text-xs text-muted-foreground">
          one math pipeline (§34.1) · mode authority: relays-client live_exec_policy
        </p>
      </CardHeader>
      <CardContent>
        {view === null ? (
          <div className="flex items-start gap-2 text-xs text-muted-foreground" role="status">
            <AlertCircleIcon className="mt-0.5 h-4 w-4 shrink-0" aria-hidden />
            <span className="font-mono break-all">
              {error ?? "canonical mode fields absent from knobs snapshot"}
            </span>
          </div>
        ) : (
          <>
            <ul className="flex flex-wrap gap-2" aria-label="Trading modes">
              {EXECUTION_MODES.map((mode) => {
                const active = mode === view.execution_mode;
                return (
                  <li
                    key={mode}
                    aria-current={active ? "true" : undefined}
                    className={`min-w-0 flex-1 basis-52 rounded-md border px-3 py-2 ${
                      active
                        ? "border-primary bg-primary/10"
                        : "border-border bg-card text-muted-foreground"
                    }`}
                  >
                    <div className="flex flex-wrap items-baseline gap-x-2 gap-y-0.5">
                      <span className="font-mono text-xs font-semibold">{mode}</span>
                      {active && (
                        <span className="text-[10px] uppercase tracking-wide text-primary">
                          active
                        </span>
                      )}
                    </div>
                    <p className="mt-1 text-xs leading-snug">{terminusStatus(mode, active)}</p>
                  </li>
                );
              })}
            </ul>
            <p className="mt-3 text-xs text-muted-foreground">
              selected_execution_mode{" "}
              <span className="font-mono">{view.selected_execution_mode}</span>
              {view.coherent ? (
                <span className="text-success"> · coherent with boot mode</span>
              ) : (
                <span className="text-warning">
                  {" "}
                  · MISMATCH with boot mode ({view.execution_mode}) — surfaced, not reconciled here
                </span>
              )}
            </p>
          </>
        )}
      </CardContent>
    </Card>
  );
}
