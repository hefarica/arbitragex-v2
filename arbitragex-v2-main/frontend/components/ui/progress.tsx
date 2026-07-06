"use client";

import * as React from "react";
import { Progress as ProgressPrimitive } from "radix-ui";

import { cn } from "@/lib/utils";

function Progress({
  className,
  value,
  ...props
}: React.ComponentProps<typeof ProgressPrimitive.Root> & { value?: number | null }) {
  // R8 fail-honest: `null` means "no data" → render empty track without a fill
  // bar. `0` means "real measured zero" → render the track + a 0%-width fill so
  // the operator can distinguish "I have data and it's 0" from "I have no data".
  const isUnavailable = value === null || value === undefined;
  const displayValue = isUnavailable ? 0 : Math.max(0, Math.min(100, value));
  return (
    <ProgressPrimitive.Root
      data-slot="progress"
      data-unavailable={isUnavailable || undefined}
      className={cn(
        "relative h-2 w-full overflow-hidden rounded-full bg-secondary",
        className,
      )}
      {...props}
    >
      <ProgressPrimitive.Indicator
        data-slot="progress-indicator"
        className={cn(
          "h-full w-full flex-1 transition-all",
          isUnavailable ? "bg-muted" : "bg-primary",
        )}
        style={{ transform: `translateX(-${100 - displayValue}%)` }}
      />
    </ProgressPrimitive.Root>
  );
}

export { Progress };
