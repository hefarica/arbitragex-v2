"use client";

/**
 * LocalTime — hydration-safe timestamp (C-01).
 *
 * `fmtTime`/`fmtDateTime` use `Date.toLocaleString()` with no locale/tz args,
 * so SSR (Node, UTC) and the browser (local tz) render different strings for
 * the same ISO → React #425/#422 hydration mismatch on /risk, /paper/history,
 * /agent-insights. This component renders a stable placeholder on the server
 * AND the first client paint (identical → no mismatch, R1), then swaps to the
 * locale-aware string only after mount (useEffect).
 *
 * Usage: <LocalTime iso={row.created_at} /> or <LocalTime iso={ts} mode="datetime" />
 * For non-React / pure-string contexts where hydration is not a concern, keep
 * using fmtTime/fmtDateTime directly.
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
