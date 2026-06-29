/**
 * DegradedBanner — slim inline banner shown ABOVE still-rendered data when the
 * upstream partially failed or went stale (a poll error, a 503 with a reason, a
 * preserved last-good snapshot). Consolidates the ad-hoc `pollError && <Alert>`
 * banners scattered across the operator console into one honest, consistent
 * surface.
 *
 * Doctrine (RULE 00 / fail-honest): it never hides the failure — it states it,
 * shows the VERBATIM upstream reason, and makes clear the data below may be
 * stale/last-known. Presentation only; it fabricates nothing.
 */
import * as React from "react";
import { AlertTriangleIcon } from "lucide-react";

export function DegradedBanner({
  title = "Degraded — showing last known data",
  reason,
  endpoint,
  lastOk,
  className = "",
}: {
  title?: string;
  /** verbatim upstream error/reason — never paraphrased. */
  reason?: React.ReactNode;
  endpoint?: string;
  /** provenance of the last good snapshot, e.g. a timestamp. */
  lastOk?: string;
  className?: string;
}) {
  return (
    <div
      role="status"
      data-testid="degraded-banner"
      className={`mb-4 flex items-start gap-2.5 rounded-lg border border-warning/30 bg-warning/10 px-3.5 py-2.5 ${className}`}
    >
      <AlertTriangleIcon className="mt-0.5 size-4 shrink-0 text-warning" aria-hidden />
      <div className="min-w-0 text-sm">
        <p className="font-medium text-foreground">{title}</p>
        {reason != null && (
          <p className="mt-0.5 break-words text-xs text-muted-foreground">
            upstream:{" "}
            <code className="font-mono text-[11px] text-foreground/80">{reason}</code>
          </p>
        )}
        {(endpoint || lastOk) && (
          <p className="mt-1 font-mono text-[10px] uppercase tracking-wider text-muted-foreground/70">
            {endpoint}
            {endpoint && lastOk ? " · " : ""}
            {lastOk ? `last good · ${lastOk}` : ""}
          </p>
        )}
      </div>
    </div>
  );
}

export default DegradedBanner;
