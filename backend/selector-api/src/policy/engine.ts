/**
 * Policy engine — prefilter + decide.
 *
 * prefilter runs BEFORE expensive work (token safety API, scoring).
 * decide runs AFTER score + safety are known.
 */

import type { Redis } from "ioredis";
import type { Opportunity, SimulationResult, AppConfig, CircuitBreaker } from "@arbx/shared";
import type { ScoredOpportunity } from "../scoring/engine.js";
import type { TokenSafetyRecord } from "../token_safety/cache.js";
import { pairAllowed } from "./blacklist.js";

export type RejectSeverity = "info" | "warning" | "critical";

export type Decision =
  | { kind: "accept"; score: number; reason: null }
  | { kind: "reject"; score: number | null; reason: string; severity: RejectSeverity };

export type PrefilterInput = {
  opportunity: Opportunity;
  killSwitchOn: boolean;
  tokenSafetyCb: CircuitBreaker;
};

/**
 * SEL-GATE-01b (2026-09-27): reason families that describe the QUOTER's
 * transport state, NOT the row's executability. A row stamped with one of
 * these can still carry complete, non-zero economics on the same pass, and
 * treating the stamp as a terminal lifecycle verdict starves the whole
 * validated -> sim -> card chain (measured 2026-09-27T02:2xZ: family
 * `v3_quote_unavailable` = 66 767 of 93 049 rows/30 min, of which 7 691
 * carried `expected_profit_usd` and 2 964 carried a POSITIVE one; the
 * `arbx:opps:validated` stream had been frozen since 2026-09-17T09:26:12Z
 * because `arbx_selector_decisions_total{reason="producer_rejected"}` was
 * 2 542/2 542 = 100% of decisions).
 */
const TRANSIENT_TRANSPORT_FAMILIES: ReadonlySet<string> = new Set([
  "v3_quote_unavailable",
]);

/**
 * Reason family = the substring before the first `:`, trimmed + lowercased.
 * Mirrors `split_part(rejection_reason, ':', 1)` used by the
 * rejection-breakdown SQL so the two stay in lockstep.
 */
export function rejectionFamily(reason: string | null | undefined): string | null {
  if (reason == null || reason === "") return null;
  const i = reason.indexOf(":");
  return (i === -1 ? reason : reason.slice(0, i)).trim().toLowerCase();
}

/**
 * R8 fail-honest: a row has usable economics only when it carries a non-zero
 * input amount AND at least one computed profit figure. This is exactly the
 * shape the 2026-09-17 incident rows LACKED (`amount_in_wei="0"`), so the
 * original RPC-burn protection is preserved verbatim.
 */
export function hasUsableEconomics(opp: Opportunity): boolean {
  const amt = opp.amount_in_wei;
  if (amt == null || amt === "" || /^0+$/.test(amt)) return false;
  return opp.expected_profit_usd != null || opp.net_expected_profit_usd != null;
}

/**
 * SEL-GATE-01 (2026-09-17): true when the PRODUCER already rejected this
 * opportunity upstream (searcher lifecycle verdict), so it must never reach
 * the validated stream — publishing it only burns sim-ctl fork RPC on rows
 * that can never execute (87% of sims were `status="rejected"` /
 * `amount_in_wei="0"` rows re-decided as accepts).
 *
 * SEL-GATE-01b (2026-09-27): the verdict is now ECONOMICS-AWARE. A declared
 * rejection whose ONLY cause is a transient transport family, on a row that
 * nonetheless carries usable economics, is admitted for evaluation — the
 * label describes the quoter's moment, not the row's executability. Every
 * other declared verdict stays terminal.
 *
 * Classification is lifecycle-only and conservative: absent status AND absent
 * rejection_reason (older producers) is NOT dropped.
 */
export function producerRejected(opp: Opportunity): boolean {
  const declared =
    opp.status === "rejected" || (opp.rejection_reason != null && opp.rejection_reason !== "");
  if (!declared) return false;
  const fam = rejectionFamily(opp.rejection_reason);
  const transientTransport = fam != null && TRANSIENT_TRANSPORT_FAMILIES.has(fam);
  return !(transientTransport && hasUsableEconomics(opp));
}

export async function prefilter(
  redis: Redis,
  input: PrefilterInput,
): Promise<Decision | null> {
  const { opportunity: opp, killSwitchOn, tokenSafetyCb } = input;

  if (killSwitchOn) {
    return { kind: "reject", score: null, reason: "kill_switch_on", severity: "info" };
  }

  // SEL-GATE-01: producer-rejected rows are terminal — before any expensive
  // work (safety fetch, scoring) and before the persist/publish tail. The
  // original producer reason stays queryable in opportunities.rejection_reason;
  // the selector decision carries the bounded `producer_rejected` label so
  // decisionsTotal{reason} keeps low cardinality.
  if (producerRejected(opp)) {
    return { kind: "reject", score: null, reason: "producer_rejected", severity: "info" };
  }

  const bl = await pairAllowed(redis, opp.chain_id, opp.token_in, opp.token_out);
  if (!bl.allowed) {
    return {
      kind: "reject",
      score: null,
      reason: bl.reason ?? "blacklist_hit",
      severity: "warning",
    };
  }

  if (tokenSafetyCb.state() === "open") {
    return {
      kind: "reject",
      score: null,
      reason: "token_safety_circuit_open",
      severity: "warning",
    };
  }

  return null; // no rejection at prefilter stage
}

export type DecideInput = {
  scored: ScoredOpportunity;
  safety: TokenSafetyRecord;
  sim: SimulationResult | null;
  cfg: AppConfig;
};

export function decide(input: DecideInput): Decision {
  const { scored, safety, sim, cfg } = input;

  // 1. Safety hard floor.
  if (safety.safety_score < cfg.token_safety.min_acceptable_score) {
    return {
      kind: "reject", score: scored.score,
      reason: "safety_below_threshold", severity: "warning",
    };
  }

  // 2. Simulation explicit failure (only if run).
  if (sim && sim.passed === false) {
    return {
      kind: "reject", score: scored.score,
      reason: "simulation_failed", severity: "info",
    };
  }

  // 3. Revert-risk hard cap (2× configured target revert rate).
  if (sim && sim.revert_risk_pct != null && sim.revert_risk_pct > cfg.risk.max_revert_rate_pct * 2) {
    return {
      kind: "reject", score: scored.score,
      reason: "revert_risk_too_high", severity: "warning",
    };
  }

  // 4. Score threshold.
  if (scored.score < cfg.scoring.min_accept_score) {
    return {
      kind: "reject", score: scored.score,
      reason: "score_below_min", severity: "info",
    };
  }

  return { kind: "accept", score: scored.score, reason: null };
}
