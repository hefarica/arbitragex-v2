// frontend/app/operations/components/latency-budget.ts
//
// ARBX-QB-07-008 (REQ-QB-015, workbook 10_LATENCY) — pure display helpers
// for the lat.* stage panel. The wire serves µs (LatencyStageRowSchema);
// the workbook's 10_LATENCY columns are ms — these are the ONLY two
// conversions in the surface, so a benchmark p95 and a panel p95 can never
// drift apart in formatting (same number-shape doctrine as
// latency_budget.rs:114).
//
// R8: null = no samples yet — rendered as the honest dash, NEVER 0.

export const LAT_DASH = "—";

/** Honest-absence copy when the snapshot carries no lat.* rows. */
export const LAT_ABSENCE_NOTE =
  "lat.* not served in this snapshot — discovery latency knob off or no samples yet (R8: not computed, never zero).";

/** µs → fixed-2 ms text; null passes through as the honest dash. */
export function usToMsText(us: number | null | undefined): string {
  if (us === null || us === undefined) return LAT_DASH;
  return `${(us / 1000).toFixed(2)}`;
}

/**
 * Signed headroom text (target − p95, µs → ms): the sign is ALWAYS visible
 * ("+1.20" / "-0.50") so over-budget reads at a glance, not by comparison.
 */
export function headroomMsText(headroomUs: number | null | undefined): string {
  if (headroomUs === null || headroomUs === undefined) return LAT_DASH;
  const ms = headroomUs / 1000;
  return `${ms >= 0 ? "+" : ""}${ms.toFixed(2)}`;
}

/** True only for a COMPUTED over-budget headroom (negative). */
export function headroomOverBudget(headroomUs: number | null | undefined): boolean {
  return headroomUs !== null && headroomUs !== undefined && headroomUs < 0;
}

/** The aggregate row the PASS_p95 gate is decided on. */
export const LAT_TOTAL_KEY = "lat.total";
