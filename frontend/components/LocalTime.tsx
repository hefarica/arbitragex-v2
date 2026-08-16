"use client";

/**
 * LocalTime — locale-aware timestamp, hydration-safe (C-01).
 *
 * `fmtTime`/`fmtDateTime` render DETERMINISTIC UTC (since HYDRATE-09,
 * 2026-08-16) — safe for SSR everywhere, but not locale-aware. When a surface
 * genuinely wants the VIEWER's local time (dashboard conveniences), use this
 * component: it renders a stable placeholder on the server AND the first
 * client paint (identical → no React #425/#422 mismatch, R1), then swaps to
 * the locale-aware string only after mount (useEffect).
 *
 * Usage: <LocalTime iso={row.created_at} /> or <LocalTime iso={ts} mode="datetime" />
 * Ledgers / anything correlated with block timestamps or by-hour analytics
 * should prefer plain fmtTime/fmtDateTime (UTC) — deterministic AND honest.
 */
import { useEffect, useState } from "react";

const DASH = "—";

type Mode = "time" | "datetime";

function format(iso: string, mode: Mode): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return DASH;
  return mode === "time" ? d.toLocaleTimeString() : d.toLocaleString();
}

export function LocalTime({
  iso,
  mode = "time",
  className,
}: {
  iso: string | null | undefined;
  mode?: Mode;
  className?: string;
}) {
  // null/empty → honest dash (stable on server + client).
  const [text, setText] = useState<string>(() => (iso ? DASH : DASH));

  useEffect(() => {
    setText(iso ? format(iso, mode) : DASH);
  }, [iso, mode]);

  return <span className={className}>{text}</span>;
}
