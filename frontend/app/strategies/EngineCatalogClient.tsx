/**
 * FE-MASTER · Engine Catalog container (P6 — PRESERVE→EXTEND).
 *
 * Hosts the strategies TabsContent's sub-views behind a segmented control
 * (same WAI-ARIA pattern as TokensTabClient, shapes §6/§7):
 *   - Runtime kinds  → the existing StrategyCatalogTab (untouched — the PG
 *     `strategy_catalog` domain of internal kinds with the enabled_strategies
 *     toggles and per-kind thresholds);
 *   - Workbook 264   → WorkbookStrategiesPanel (FE-0021..0024, EMIT-07
 *     consumer — the workbook canon with honest dispatch states, the ×hop
 *     matrix and the detail drawer).
 *
 * a11y: a plain WAI-ARIA tablist — role=tablist/tab, aria-selected, roving
 * tabIndex, ArrowLeft/ArrowRight + Home/End. The outer Radix Tabs shell in
 * StrategiesClient stays untouched.
 */
"use client";

import { useCallback, useRef, useState } from "react";

import type {
  StrategyCatalogEntry,
  TradingConfigConfigured,
} from "@/lib/schemas";

import { StrategyCatalogTab } from "./tabs/StrategyCatalogTab";
import { WorkbookStrategiesPanel } from "./tabs/WorkbookStrategiesPanel";

type SubView = "runtime-kinds" | "workbook";

const VIEWS: { id: SubView; label: string }[] = [
  { id: "runtime-kinds", label: "Runtime kinds" },
  { id: "workbook", label: "Workbook 264" },
];

interface Props {
  config: TradingConfigConfigured;
  catalog: StrategyCatalogEntry[];
  onSaved: (next: TradingConfigConfigured) => void;
  adminToken: string;
  actor: string;
}

export function EngineCatalogClient({ config, catalog, onSaved, adminToken, actor }: Props) {
  const [view, setView] = useState<SubView>("runtime-kinds");
  const tabRefs = useRef<(HTMLButtonElement | null)[]>([]);

  const onTabKeyDown = useCallback(
    (e: React.KeyboardEvent, idx: number) => {
      const count = VIEWS.length;
      let next: number | null = null;
      if (e.key === "ArrowRight") next = (idx + 1) % count;
      else if (e.key === "ArrowLeft") next = (idx - 1 + count) % count;
      else if (e.key === "Home") next = 0;
      else if (e.key === "End") next = count - 1;
      if (next === null) return;
      e.preventDefault();
      setView(VIEWS[next]!.id);
      tabRefs.current[next]?.focus();
    },
    [],
  );

  return (
    <div className="space-y-4">
      <div
        role="tablist"
        aria-label="Engine Catalog sub-vistas"
        className="inline-flex h-9 items-center justify-center rounded-lg bg-muted p-1 text-muted-foreground"
      >
        {VIEWS.map((v, i) => (
          <button
            key={v.id}
            ref={(el) => {
              tabRefs.current[i] = el;
            }}
            role="tab"
            type="button"
            id={`engine-catalog-tab-${v.id}`}
            aria-selected={view === v.id}
            aria-controls={`engine-catalog-panel-${v.id}`}
            tabIndex={view === v.id ? 0 : -1}
            onClick={() => setView(v.id)}
            onKeyDown={(e) => onTabKeyDown(e, i)}
            className={`inline-flex items-center justify-center whitespace-nowrap rounded-md px-3 py-1 text-sm font-medium transition-colors focus-visible:outline focus-visible:outline-2 focus-visible:outline-ring ${
              view === v.id
                ? "bg-background text-foreground shadow-sm"
                : "hover:text-foreground"
            }`}
          >
            {v.label}
          </button>
        ))}
      </div>

      {view === "runtime-kinds" ? (
        <div role="tabpanel" id="engine-catalog-panel-runtime-kinds" aria-labelledby="engine-catalog-tab-runtime-kinds">
          <StrategyCatalogTab
            config={config}
            catalog={catalog}
            onSaved={onSaved}
            adminToken={adminToken}
            actor={actor}
          />
        </div>
      ) : (
        <div role="tabpanel" id="engine-catalog-panel-workbook" aria-labelledby="engine-catalog-tab-workbook">
          <WorkbookStrategiesPanel />
        </div>
      )}
    </div>
  );
}
