/** Display-only lifecycle precedence, shared by the diagnostic, ticker and home.
 * A missing economic result cannot erase a terminal pipeline outcome.
 * This module does not infer executability or change any runtime control.
 */
export function terminalOpportunityState(opp: {
  status?: string | null;
  rejection_reason?: string | null;
  paper_status?: string | null;
}): "failed" | "rejected" | null {
  if (opp.status === "failed") return "failed";
  if (opp.rejection_reason != null || opp.status === "rejected" || opp.paper_status === "paper_rejected") {
    return "rejected";
  }
  return null;
}
