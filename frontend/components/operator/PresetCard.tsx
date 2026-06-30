import * as React from "react";
import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
  CardFooter,
} from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import { RiskDimensionDisplay } from "./RiskDimensionDisplay";
import type { RiskPosture, ResolvedRiskLimits } from "@/lib/presets/riskPresets";

function usd(n: number): string {
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    maximumFractionDigits: 2,
  }).format(n);
}

/** Cap as a percentage of deployed capital, or null when nothing is deployed. */
function pctOfDeployed(capUsd: number, deployed: number): number | null {
  if (deployed <= 0) return null;
  return Math.max(0, Math.min(100, (capUsd / deployed) * 100));
}

export interface PresetCardProps {
  posture: RiskPosture;
  resolved: ResolvedRiskLimits;
  selected: boolean;
  onSelect: (id: RiskPosture["id"]) => void;
}

/**
 * Selectable risk-posture card. Pure presentational: it renders the resolved 7
 * dimensions (already computed from operator capital by resolvePosture) and
 * raises onSelect. It never computes or invents a dollar figure itself.
 */
export function PresetCard({ posture, resolved, selected, onSelect }: PresetCardProps) {
  const d = resolved.deployedCapitalUsd;
  const id = posture.id;
  return (
    <Card
      role="button"
      tabIndex={0}
      aria-pressed={selected}
      data-testid={`preset-card-${id}`}
      data-selected={selected || undefined}
      onClick={() => onSelect(id)}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onSelect(id);
        }
      }}
      className={cn(
        "cursor-pointer transition-colors",
        selected ? "border-primary ring-primary/40 ring-2" : "hover:border-accent-foreground/30",
      )}
    >
      <CardHeader>
        <CardTitle className="flex items-center justify-between">
          <span>{posture.label}</span>
          {selected ? (
            <Badge variant="default" data-testid={`preset-selected-${id}`}>
              Selected
            </Badge>
          ) : null}
        </CardTitle>
        <CardDescription>{posture.description}</CardDescription>
      </CardHeader>

      <CardContent className="flex flex-col gap-3">
        <RiskDimensionDisplay
          testId={`dim-${id}-deployed`}
          label="Deployed capital"
          valueText={usd(d)}
          pct={d > 0 ? 100 : null}
        />
        <RiskDimensionDisplay
          testId={`dim-${id}-daily-loss`}
          label="Daily loss cap"
          valueText={usd(resolved.dailyLossCapUsd)}
          pct={pctOfDeployed(resolved.dailyLossCapUsd, d)}
        />
        <RiskDimensionDisplay
          testId={`dim-${id}-per-token`}
          label="Per-token cap"
          valueText={usd(resolved.perTokenCapUsd)}
          pct={pctOfDeployed(resolved.perTokenCapUsd, d)}
        />
        <RiskDimensionDisplay
          testId={`dim-${id}-per-pool`}
          label="Per-pool cap"
          valueText={usd(resolved.perPoolCapUsd)}
          pct={pctOfDeployed(resolved.perPoolCapUsd, d)}
        />
        <RiskDimensionDisplay
          testId={`dim-${id}-per-dex`}
          label="Per-DEX cap"
          valueText={usd(resolved.perDexCapUsd)}
          pct={pctOfDeployed(resolved.perDexCapUsd, d)}
        />
        <RiskDimensionDisplay
          testId={`dim-${id}-per-chain`}
          label="Per-chain cap"
          valueText={usd(resolved.perChainCapUsd)}
          pct={pctOfDeployed(resolved.perChainCapUsd, d)}
        />
        <RiskDimensionDisplay
          testId={`dim-${id}-per-strategy`}
          label="Per-strategy cap"
          valueText={usd(resolved.perStrategyCapUsd)}
          pct={pctOfDeployed(resolved.perStrategyCapUsd, d)}
        />
        <RiskDimensionDisplay
          testId={`dim-${id}-consecutive`}
          label="Max consecutive losses"
          valueText={`${resolved.maxConsecutiveLosses} trades`}
          pct={null}
        />

        <div className="flex flex-wrap gap-1 pt-1" data-testid={`preset-strategies-${id}`}>
          {resolved.enabledStrategyKinds.map((k) => (
            <Badge key={k} variant="secondary" className="text-xs">
              {k}
            </Badge>
          ))}
        </div>

        {resolved.warnings.length > 0 ? (
          <ul
            className="text-warning list-disc pl-4 text-xs"
            data-testid={`preset-warnings-${id}`}
          >
            {resolved.warnings.map((w, i) => (
              <li key={i}>{w}</li>
            ))}
          </ul>
        ) : null}
      </CardContent>

      <CardFooter className="text-muted-foreground text-xs">
        Slippage ≤ {posture.maxSlippagePct}% · Min ROI {posture.minRoiPct}%
      </CardFooter>
    </Card>
  );
}
