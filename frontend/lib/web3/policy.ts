// Policy Engine — a PURE, deny-by-default evaluator run BEFORE any sign/broadcast/execute.
//
// It reuses the { name, status, value, reason } shape of the existing backend SafetyGate
// (see frontend/hooks/useWalletSafety.ts) so a rendered gate list is uniform. If ANY gate fails,
// the result is allow=false and the caller MUST NOT sign, broadcast, execute, or persist a live
// plan. Missing/unknown inputs fail closed (deny). This module holds no key and sends nothing.

import type { TransactionIntent } from "./intent";
import { isUnlimitedWei } from "./intent";

export type GateStatus = "pass" | "fail";

export interface PolicyGate {
  name: string;
  status: GateStatus;
  value?: string;
  reason?: string;
}

export interface PolicyResult {
  allow: boolean;
  gates: PolicyGate[];
  denied: string[];
}

// Everything the 19 gates need. Allowlists are lowercased addresses/selectors. Amounts are decimal
// wei strings. Anything absent is treated as "not permitted".
export interface PolicyContext {
  chainAllowlist: number[];
  tokenAllowlist: string[];
  routerAllowlist: string[];
  dexAllowlist: string[];
  spenderAllowlist: string[];
  selectorAllowlist: string[];
  router: string;
  dex: string;
  spender: string;
  selector: string;
  amountCapWei: string;
  approvalAmountWei: string;
  approvalCapWei: string;
  approvalExpiry: number; // unix seconds; must be > now
  simulationPassed: boolean;
  simulationCalldataHash: string; // must equal intent.calldataHash
  usedNonces: string[];
  netProfitUsd: number;
  riskScoreMin: number;
  gasCapWei: string;
  nowSec: number;
  readinessGreen: boolean;
  killSwitchOff: boolean;
  liveGateOpen: boolean;
}

// The canonical 19 gate names, in evaluation order.
export const POLICY_GATE_NAMES = [
  "chain_allowed",
  "token_allowed",
  "router_allowed",
  "dex_allowed",
  "spender_allowed",
  "selector_allowed",
  "amount_within_cap",
  "approval_within_cap",
  "approval_has_expiry",
  "simulation_passed",
  "net_profit_usd_positive",
  "risk_score_minimum",
  "gas_within_cap",
  "deadline_valid",
  "nonce_unused",
  "calldata_hash_matches_sim",
  "readiness_green",
  "kill_switch_off",
  "live_gate_open",
] as const;

function lc(s: string): string {
  return (s || "").toLowerCase();
}

// safe bigint compare of decimal strings; unparseable => fail-closed (returns false for <=)
function leWei(a: string, b: string): boolean {
  try {
    return BigInt(a) <= BigInt(b);
  } catch {
    return false;
  }
}

export function evaluatePolicy(intent: TransactionIntent, ctx: PolicyContext): PolicyResult {
  const gates: PolicyGate[] = [];
  const gate = (name: string, ok: boolean, reason: string, value?: string) =>
    gates.push({ name, status: ok ? "pass" : "fail", reason: ok ? undefined : reason, value });

  const tokensAllowed =
    ctx.tokenAllowlist.includes(lc(intent.tokenIn)) && ctx.tokenAllowlist.includes(lc(intent.tokenOut));

  gate("chain_allowed", ctx.chainAllowlist.includes(intent.chainId), "chain not in allowlist", String(intent.chainId));
  gate("token_allowed", tokensAllowed, "tokenIn/tokenOut not in allowlist");
  gate("router_allowed", ctx.routerAllowlist.includes(lc(ctx.router)), "router not in allowlist", ctx.router);
  gate("dex_allowed", ctx.dexAllowlist.includes(lc(ctx.dex)), "dex not in allowlist", ctx.dex);
  gate("spender_allowed", ctx.spenderAllowlist.includes(lc(ctx.spender)), "spender not in allowlist", ctx.spender);
  gate("selector_allowed", ctx.selectorAllowlist.includes(lc(ctx.selector)), "selector not in allowlist", ctx.selector);
  gate("amount_within_cap", leWei(intent.amountIn, ctx.amountCapWei), "amountIn exceeds cap");
  gate(
    "approval_within_cap",
    !isUnlimitedWei(ctx.approvalAmountWei) && leWei(ctx.approvalAmountWei, ctx.approvalCapWei),
    "approval is unlimited or exceeds cap",
  );
  gate("approval_has_expiry", ctx.approvalExpiry > ctx.nowSec, "approval has no future expiry");
  gate("simulation_passed", ctx.simulationPassed === true, "simulation not passed");
  gate("net_profit_usd_positive", ctx.netProfitUsd > 0, "net profit not positive", String(ctx.netProfitUsd));
  gate("risk_score_minimum", intent.riskScore >= ctx.riskScoreMin, "risk score below minimum", String(intent.riskScore));
  gate("gas_within_cap", leWei(intent.maxGasWei, ctx.gasCapWei), "maxGas exceeds cap");
  gate("deadline_valid", intent.deadline > ctx.nowSec, "deadline expired", String(intent.deadline));
  gate("nonce_unused", !ctx.usedNonces.includes(intent.nonce), "nonce already used", intent.nonce);
  gate(
    "calldata_hash_matches_sim",
    !!intent.calldataHash && !!ctx.simulationCalldataHash && lc(intent.calldataHash) === lc(ctx.simulationCalldataHash),
    "calldata hash does not match the simulated calldata",
  );
  gate("readiness_green", ctx.readinessGreen === true, "readiness not green");
  gate("kill_switch_off", ctx.killSwitchOff === true, "kill switch active");
  gate("live_gate_open", ctx.liveGateOpen === true, "live gate closed");

  const denied = gates.filter((g) => g.status === "fail").map((g) => g.name);
  return { allow: denied.length === 0, gates, denied };
}

// Convenience: the terminal action a caller may take given a policy result. Deny-by-default.
export type PolicyVerdict = "SIGN_ALLOWED" | "DENIED";
export function policyVerdict(result: PolicyResult): PolicyVerdict {
  return result.allow ? "SIGN_ALLOWED" : "DENIED";
}
