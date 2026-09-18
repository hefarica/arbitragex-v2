// frontend/components/cex/Sparkline.tsx
//
// WO-PRICE-EXCHANGE-V1 (FE) — inline SVG mini-trend (no chart library).
//
// Pure presentational component in the repo's hand-rolled-SVG style: plain
// <polyline> + soft <polygon> fill, colored by the series' own direction
// (last vs first). Deterministic for a given `points` array — no Date, no
// window, no randomness — so SSR markup is byte-stable (R1).
//
// Data contract: `points?: number[]` is the single intake. Today the card
// fills it from the local in-memory route stream (useRouteSeries); when the
// backend price_history / price_delta mirror lands, the SAME prop carries
// that source — no visual change, only the feed.
import React from "react";

export interface SparklineProps {
  /** Chronological series (oldest → newest). <2 finite points → placeholder. */
  points?: number[];
  width?: number;
  height?: number;
  className?: string;
}

export function Sparkline({
  points,
  width = 96,
  height = 24,
  className,
}: SparklineProps) {
  const pts = (points ?? []).filter((v) => Number.isFinite(v));
  const n = pts.length;

  if (n < 2) {
    return (
      <span
        className="text-[9px] font-mono text-muted-foreground/50"
        aria-hidden="true"
        data-testid="sparkline-empty"
      >
        ·
      </span>
    );
  }

  const min = Math.min(...pts);
  const max = Math.max(...pts);
  const span = max - min;
  const x0 = 1;
  const x1 = width - 1;
  const y0 = 2;
  const y1 = height - 2;
  const coords = pts.map((v, i) => {
    const x = x0 + (i / (n - 1)) * (x1 - x0);
    const y = span === 0 ? (y0 + y1) / 2 : y1 - ((v - min) / span) * (y1 - y0);
    return `${x.toFixed(1)},${y.toFixed(1)}`;
  });

  const first = pts[0] ?? 0;
  const last = pts[n - 1] ?? 0;
  const toneCls = last >= first ? "text-success" : "text-destructive";

  return (
    <svg
      width={width}
      height={height}
      viewBox={`0 0 ${width} ${height}`}
      className={`${toneCls}${className ? ` ${className}` : ""}`}
      role="img"
      aria-label={`Route trend — last ${n} values`}
      data-testid="sparkline"
      data-points={n}
    >
      <polygon
        points={`${x0},${y1} ${coords.join(" ")} ${x1},${y1}`}
        fill="currentColor"
        opacity={0.1}
      />
      <polyline
        points={coords.join(" ")}
        fill="none"
        stroke="currentColor"
        strokeWidth={1.25}
        strokeLinejoin="round"
        strokeLinecap="round"
      />
    </svg>
  );
}
