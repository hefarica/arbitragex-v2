import type { ReactNode } from "react";

import { RefreshButton } from "@/components/refresh-button";

export function PageHeader({
  title,
  lede,
  meta,
  actions,
  showRefresh = false,
}: {
  title: string;
  lede?: string;
  meta?: string[];
  actions?: ReactNode;
  showRefresh?: boolean;
}) {
  const hasTrailing = showRefresh || actions;
  return (
    <div className="mb-8 flex flex-col gap-3 border-b pb-6">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
        <div>
          {/* data-testid: stable hook for Playwright smoke. The h1 stays a
              real h1 (level-1, accessible) — the testid is additive, not a
              substitute for semantic markup. */}
          <h1 data-testid="page-title">{title}</h1>
          {lede && (
            <p className="mt-2 max-w-3xl text-base text-muted-foreground">{lede}</p>
          )}
        </div>
        {hasTrailing && (
          <div className="flex shrink-0 gap-2">
            {showRefresh && <RefreshButton />}
            {actions}
          </div>
        )}
      </div>
      {meta && (
        <div className="flex flex-wrap gap-4 text-xs uppercase tracking-widest text-muted-foreground/70">
          {meta.map((m) => (
            <span key={m}>{m}</span>
          ))}
        </div>
      )}
    </div>
  );
}
