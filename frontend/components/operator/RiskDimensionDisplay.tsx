import * as React from "react";
import { cn } from "@/lib/utils";

/**
 * One risk-limit dimension: label + value + an optional 0–100 meter bar.
 *
 * `pct === null` means "not a 0–100 magnitude" (e.g. a consecutive-loss COUNT)
 * OR "no deployed capital to take a fraction of" — in both cases we render the
 * value with no bar (R8 fail-honest: we don't draw a fake-full bar).
 *
 * The meter is a plain element styled with the design-system tokens (same
 * bg-secondary track / bg-primary fill as components/ui/progress) so it composes
 * cleanly under the node (no-jsdom) SSR test env and respects Mirror Law (no new
 * style tokens, no modified shadcn primitive).
 */
export interface RiskDimensionDisplayProps {
  label: string;
  /** Pre-formatted value text, e.g. "$1,250.00" or "5 trades". */
  valueText: string;
  /** Bar fill 0..100, or null when the dimension has no 0–100 magnitude. */
  pct: number | null;
  testId?: string;
  className?: string;
}

export function RiskDimensionDisplay({
  label,
  valueText,
  pct,
  testId,
  className,
}: RiskDimensionDisplayProps) {
  const hasBar = pct !== null && Number.isFinite(pct);
  const width = hasBar ? Math.max(0, Math.min(100, pct as number)) : 0;
  return (
    <div className={cn("flex flex-col gap-1", className)} data-testid={testId}>
      <div className="flex items-center justify-between text-sm">
        <span className="text-muted-foreground">{label}</span>
        <span
          className="font-medium tabular-nums"
          data-testid={testId ? `${testId}-value` : undefined}
        >
          {valueText}
        </span>
      </div>
      {hasBar ? (
        <div
          className="bg-secondary relative h-2 w-full overflow-hidden rounded-full"
          role="meter"
          aria-valuenow={Math.round(width)}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-label={`${label} ${valueText}`}
        >
          <div
            className="bg-primary h-full rounded-full transition-all"
            style={{ width: `${width}%` }}
          />
        </div>
      ) : null}
    </div>
  );
}
