"use client";

/**
 * LiveTicker — sticky-bottom marquee of live Asimetría Topológica detections.
 *
 * REAL data only (RULE 00): polls /api/opportunities/live on its own lightweight
 * 10s loop. Deliberately independent of the omni-store connection hook
 * (useOmniOpportunities) so it does NOT double-mount / interfere with the
 * /opportunities page subscription. Empty feed → honest empty state (no fabricated
 * items, no fake placeholders). Failures leave the last known items (stale-but-real),
 * never invented.
 *
 * Mounted globally as a sibling of <Toaster/> in app/layout.tsx (outside
 * Web3Provider, so the wagmi tree is untouched).
 */
import { useEffect, useState } from "react";
import { getApiBaseUrl } from "@/lib/api-client";

type TickerRow = {
  pair_symbol: string | null;
  dex_a: string;
  dex_b: string | null;
  roi_pct: number | null;
  detected_at: string;
};

function fmtAgo(iso: string): string {
  const s = Math.max(0, Math.floor((Date.now() - new Date(iso).getTime()) / 1000));
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  return m < 60 ? `${m}m` : `${Math.floor(m / 60)}h`;
}

function pairLabel(r: TickerRow): string {
  return r.pair_symbol ?? "—";
}

export function LiveTicker() {
  const [items, setItems] = useState<TickerRow[]>([]);
  const [mounted, setMounted] = useState(false);

  useEffect(() => {
    setMounted(true);
    let cancelled = false;
    const base = getApiBaseUrl();
    if (!base) return; // no edge configured → ticker stays empty (R8 fail-honest)

    const poll = async () => {
      try {
        const res = await fetch(
          `${base}/api/opportunities/live?limit=20`,
          { headers: { accept: "application/json" }, cache: "no-store" },
        );
        if (!res.ok || cancelled) return;
        const data: unknown = await res.json();
        const rows: TickerRow[] = Array.isArray((data as { items?: unknown[] })?.items)
          ? (data as { items: TickerRow[] }).items
          : [];
        if (!cancelled) setItems(rows);
      } catch {
        // fail-honest: keep last known items (stale, real) — never invent.
      }
    };

    poll();
    const id = setInterval(poll, 10000);
    return () => {
      cancelled = true;
      clearInterval(id);
    };
  }, []);

  // Avoid SSR/CSR mismatch (Date.now + fetch are client-only) — render nothing on server.
  if (!mounted) return null;

  const hasItems = items.length > 0;
  // Duplicate the list so the marquee translateX(-50%) loops seamlessly.
  const loop = hasItems ? [...items, ...items] : [];

  return (
    <div
      aria-label="Live Asimetría Topológica ticker"
      className="arbx-ticker fixed bottom-0 left-0 right-0 z-[45] border-t"
      style={{
        backgroundColor: 'var(--ticker-bg)',
        borderColor: 'var(--border)',
        backdropFilter: 'blur(12px) saturate(1.4)',
        WebkitBackdropFilter: 'blur(12px) saturate(1.4)',
        fontFamily: 'var(--font-data)',
        fontSize: '12.5px',
        letterSpacing: '0.08em',
      }}
    >
      <div
        className={hasItems ? "arbx-ticker-track flex items-center whitespace-nowrap" : "flex items-center"}
        style={hasItems ? {
          display: 'inline-flex',
          alignItems: 'center',
          gap: '2.5rem',
          padding: '16px 0',
          animation: 'arbx-tick 60s linear infinite',
        } : {}}
      >
        {!hasItems ? (
          <span className="data-label px-4 text-muted-foreground/70">
            sin Asimetría Topológica activa · observando manifolds
          </span>
        ) : (
          loop.map((r, i) => {
            const roi = r.roi_pct;
            const pos = typeof roi === "number" && roi >= 0;
            return (
              <span key={i} className="data-label inline-flex items-center gap-2 px-1">
                <span className="text-foreground font-bold">{pairLabel(r)}</span>
                <span className="text-muted-foreground">
                  {r.dex_a}
                  {r.dex_b ? ` → ${r.dex_b}` : ""}
                </span>
                {typeof roi === "number" && (
                  <span className={pos ? "text-success" : "text-destructive"}>
                    {pos ? "+" : ""}
                    {roi.toFixed(2)}% {pos ? "▲" : "▼"}
                  </span>
                )}
                <span className="text-muted-foreground/60">{fmtAgo(r.detected_at)}</span>
              </span>
            );
          })
        )}
      </div>
    </div>
  );
}
