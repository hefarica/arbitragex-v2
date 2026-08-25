/**
 * FE-MASTER · Tokens tab container (shapes doc §6 — PRESERVE→EXTEND).
 *
 * Hosts the tokens TabsContent's sub-views behind a segmented control:
 *   - Universe       → the existing TokenAllowlistTab (untouched);
 *   - Quote/Base     → QuoteBasePanel (FE-0013..0015, EMIT-02/03 consumer);
 *   - Pair Intelligence → PairIntelligencePanel (FE-0017, EMIT-06 consumer;
 *     deep-linkable via `#pair=<aAddr>-<bAddr>` from Route Discovery §12).
 *
 * a11y (shapes §6: no nested Radix Tabs "a ciegas"): a plain WAI-ARIA
 * tablist — role=tablist/tab, aria-selected, roving tabIndex, ArrowLeft/
 * ArrowRight + Home/End. The outer Radix Tabs shell stays untouched.
 */
"use client";

import { useCallback, useRef, useState } from "react";

import type { TradingConfigConfigured } from "@/lib/schemas";

import { PairIntelligencePanel } from "./PairIntelligencePanel";
import { QuoteBasePanel } from "./QuoteBasePanel";
import { TokenAllowlistTab } from "./TokenAllowlistTab";

type SubView = "universe" | "quote-base" | "pair-intelligence";

const VIEWS: { id: SubView; label: string }[] = [
  { id: "universe", label: "Universe" },
  { id: "quote-base", label: "Quote/Base" },
  { id: "pair-intelligence", label: "Pair Intelligence" },
];

interface Props {
  config: TradingConfigConfigured;
  onSaved: (next: TradingConfigConfigured) => void;
  adminToken: string;
  actor: string;
}

export function TokensTabClient({ config, onSaved, adminToken, actor }: Props) {
  const [view, setView] = useState<SubView>("universe");
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
        aria-label="Tokens sub-vistas"
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
            id={`tokens-tab-${v.id}`}
            aria-selected={view === v.id}
            aria-controls={`tokens-panel-${v.id}`}
            tabIndex={view === v.id ? 0 : -1}
            onClick={() => setView(v.id)}
            onKeyDown={(e) => onTabKeyDown(e, i)}
            className={`inline-flex items-center justify-center whitespace-nowrap rounded-md px-3 py-1 text-sm font-medium transition-colors focus-visible:outline focus-visible:outline-2 focus-visible:outline-ring ${
              view === v.id ? "bg-background text-foreground shadow-sm" : "hover:text-foreground"
            }`}
          >
            {v.label}
          </button>
        ))}
      </div>
      {view === "universe" && (
        <div role="tabpanel" id="tokens-panel-universe" aria-labelledby="tokens-tab-universe">
          <TokenAllowlistTab config={config} onSaved={onSaved} adminToken={adminToken} actor={actor} />
        </div>
      )}
      {view === "quote-base" && (
        <div role="tabpanel" id="tokens-panel-quote-base" aria-labelledby="tokens-tab-quote-base">
          <QuoteBasePanel chainId={config.chain_id} adminToken={adminToken} actor={actor} />
        </div>
      )}
      {view === "pair-intelligence" && (
        <div role="tabpanel" id="tokens-panel-pair-intelligence" aria-labelledby="tokens-tab-pair-intelligence">
          <PairIntelligencePanel chainId={config.chain_id} />
        </div>
      )}
    </div>
  );
}
