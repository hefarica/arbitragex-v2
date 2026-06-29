"use client";

import * as React from "react";
import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Alert, AlertTitle, AlertDescription } from "@/components/ui/alert";
import { PresetCard } from "@/components/operator/PresetCard";
import { useRiskPresets } from "@/hooks/useRiskPresets";
import { RISK_POSTURES, RISK_POSTURE_ORDER, RISK_DISCLAIMER } from "@/lib/presets/riskPresets";

function usd(n: number): string {
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    maximumFractionDigits: 2,
  }).format(n);
}

/**
 * Operator risk-preset console (client). Mounted by the server page.tsx under
 * the preserved shell <main> — this renders a <div>, not a second <main>
 * (Mirror Law: panels mount inside the shell, they do not replace it).
 */
export function PresetsClient() {
  const {
    capitalUsd,
    setCapitalUsd,
    capitalError,
    selectedPostureId,
    selectPosture,
    resolvedByPosture,
    apply,
    appliedDraft,
  } = useRiskPresets();

  const applyDisabled = !Number.isFinite(capitalUsd) || capitalUsd <= 0;

  return (
    <div
      className="mx-auto flex w-full max-w-6xl flex-col gap-6 p-6"
      data-testid="operator-presets-panel"
    >
      <header className="flex flex-col gap-1">
        <h1 className="text-2xl font-semibold">Operator Risk Presets</h1>
        <p className="text-muted-foreground text-sm">
          Choose a risk posture. Each preset resolves the 7 mandatory risk limits as a
          fraction of your configured capital. Presets configure how much you can lose —
          never what you can earn.
        </p>
      </header>

      <Alert variant="warning" data-testid="presets-disclaimer">
        <AlertTitle>Risk disclaimer</AlertTitle>
        <AlertDescription>
          <p>{RISK_DISCLAIMER}</p>
          <p>Past performance ≠ future results.</p>
        </AlertDescription>
      </Alert>

      <Card>
        <CardHeader>
          <CardTitle>Capital</CardTitle>
          <CardDescription>
            Operator capital these caps are computed from. Paper-mode exposure is 0.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex max-w-sm flex-col gap-2">
          <Label htmlFor="capital-usd">Capital (USD)</Label>
          <Input
            id="capital-usd"
            data-testid="capital-input"
            type="number"
            min={0}
            step="100"
            inputMode="decimal"
            value={Number.isFinite(capitalUsd) ? String(capitalUsd) : ""}
            aria-invalid={capitalError ? true : undefined}
            onChange={(e) => setCapitalUsd(Number(e.target.value))}
          />
          {capitalError ? (
            <p className="text-destructive text-sm" data-testid="capital-warning">
              {capitalError}
            </p>
          ) : null}
        </CardContent>
      </Card>

      <section
        className="grid grid-cols-1 gap-4 md:grid-cols-3"
        data-testid="presets-grid"
      >
        {RISK_POSTURE_ORDER.map((id) => (
          <PresetCard
            key={id}
            posture={RISK_POSTURES[id]}
            resolved={resolvedByPosture[id]}
            selected={selectedPostureId === id}
            onSelect={selectPosture}
          />
        ))}
      </section>

      <div className="flex flex-wrap items-center gap-3">
        <Button data-testid="apply-preset" disabled={applyDisabled} onClick={() => apply()}>
          Apply (stage local draft)
        </Button>
        <span className="text-muted-foreground text-xs">
          Staged locally for review. Persisting to trading_config is a separate
          admin-authenticated step.
        </span>
      </div>

      {appliedDraft ? (
        <Alert variant="success" data-testid="applied-draft">
          <AlertTitle>Draft staged: {RISK_POSTURES[appliedDraft.postureId].label}</AlertTitle>
          <AlertDescription>
            <p>
              Capital {usd(appliedDraft.capitalUsd)} · daily-loss cap{" "}
              {usd(appliedDraft.resolved.dailyLossCapUsd)} · per-token cap{" "}
              {usd(appliedDraft.resolved.perTokenCapUsd)}.
            </p>
            <p>Review, then push via the admin trading-config flow.</p>
          </AlertDescription>
        </Alert>
      ) : null}
    </div>
  );
}
