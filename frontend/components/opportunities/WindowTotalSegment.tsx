// frontend/components/opportunities/WindowTotalSegment.tsx
//
// AUDIT-CARDS-MINOR (§2) · WO-H4 — the /api/opportunities/live envelope's
// `window_total` finally has a consumer: a small text segment next to the
// opportunities page's header counters pairing what the grid SHOWS with what
// the window CONTAINS ("50 mostradas · 12155 en ventana (≤5 min)"). The
// payload returns up to `limit` (50) items while window_total counts every
// distinct route in the ≤5 min window (COUNT(*) OVER () on the LIVE_QUERY) —
// until now that number crossed the wire and died (finding: "window_total sin
// consumidor").
//
// R8 fail-honest: windowTotal null/undefined (pre-WO-H4 edge, bare-array
// payload, first paint before any snapshot) renders NOTHING — absent is never
// invented as 0. A real 0 (computed empty window) DOES render: computed-and-
// exactly-zero is a different, honest claim.
//
// R1: pure function of props — no clock, no locale formatting, no effects.
// Deterministic across SSR/CSR so the mounted-only counter line cannot
// hydrate-mismatch.

export function WindowTotalSegment({
  shown,
  windowTotal,
}: {
  /** Cards the store currently holds from the last snapshot (grid count). */
  shown: number;
  /** Envelope window_total — null when the payload did not carry one (R8). */
  windowTotal: number | null;
}) {
  if (windowTotal == null) return null;
  return (
    <span
      className="text-muted-foreground/80"
      title="Rutas distintas en la ventana (~5 min) del último snapshot (window_total, COUNT over window) — el feed entrega y muestra como máximo las primeras 50."
    >
      {" · "}
      <span className="text-foreground font-semibold">{shown}</span> mostradas ·{" "}
      {windowTotal} en ventana (≤5 min)
    </span>
  );
}
